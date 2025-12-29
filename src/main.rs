//! sinqtt - MQTT to InfluxDB bridge for IoT applications.

use clap::Parser;
use sinqtt::bridge::{
    FieldValue, HttpContentBuilder, HttpForwarder, InfluxDBWriter, MessageProcessor, MqttHandler,
    MqttMessage, Point,
};
use sinqtt::cli::Args;
use sinqtt::config::PointConfig;
use sinqtt::error::SinqttError;
use sinqtt::{Config, load_config};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;
use tracing::{debug, error, info, warn};
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<(), SinqttError> {
    let args = Args::parse();

    // Configure logging
    let filter = if args.debug {
        EnvFilter::new("debug")
    } else {
        EnvFilter::new("info")
    };

    tracing_subscriber::fmt().with_env_filter(filter).init();

    // Load configuration
    info!("Loading configuration from: {:?}", args.config);
    let config = load_config(&args.config)?;

    if args.test {
        println!("Configuration file is valid.");
        return Ok(());
    }

    info!("Configuration loaded successfully");
    info!("MQTT broker: {}:{}", config.mqtt.host, config.mqtt.port);
    info!(
        "InfluxDB: {}:{}",
        config.influxdb.host, config.influxdb.port
    );
    info!("Points configured: {}", config.points.len());

    // Run bridge with retry logic if daemon mode
    loop {
        match run_bridge(&config).await {
            Ok(()) => break,
            Err(e) => {
                if !args.daemon {
                    return Err(e);
                }
                error!("Error: {}", e);
                info!("Retrying in 30 seconds...");
                tokio::time::sleep(Duration::from_secs(30)).await;
            }
        }
    }

    Ok(())
}

async fn run_bridge(config: &Config) -> Result<(), SinqttError> {
    // Collect unique topics from all point configurations
    let topics: Vec<String> = config
        .points
        .iter()
        .map(|p| p.topic.clone())
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .collect();

    info!("Subscribing to {} unique topics", topics.len());

    // Create MQTT handler
    let mqtt_handler = MqttHandler::new(&config.mqtt, topics)?;

    // Create InfluxDB writer
    let influxdb_writer = Arc::new(InfluxDBWriter::new(&config.influxdb)?);

    // Create HTTP forwarder if configured
    let http_forwarder = config
        .http
        .as_ref()
        .map(|h| Arc::new(HttpForwarder::new(h)));
    if http_forwarder.is_some() {
        info!("HTTP forwarding enabled");
    }

    // Create message processor
    let processor = Arc::new(MessageProcessor::new(config.base64decode.clone()));

    // Clone config points for the processing task
    let points = Arc::new(config.points.clone());

    // Create channel for MQTT messages
    let (tx, mut rx) = mpsc::channel::<MqttMessage>(100);

    // Spawn message processing task
    let processor_clone = processor.clone();
    let influxdb_clone = influxdb_writer.clone();
    let http_clone = http_forwarder.clone();
    let points_clone = points.clone();

    let process_task = tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            process_message(
                &msg,
                &points_clone,
                &processor_clone,
                &influxdb_clone,
                http_clone.as_ref().map(|h| h.as_ref()),
            )
            .await;
        }
    });

    info!("Bridge started, waiting for messages...");

    // Run MQTT handler (blocks until disconnected or error)
    let mqtt_result = mqtt_handler.run(tx).await;

    // Wait for processor to finish
    let _ = process_task.await;

    mqtt_result
}

async fn process_message(
    msg: &MqttMessage,
    points: &[PointConfig],
    processor: &MessageProcessor,
    influxdb_writer: &InfluxDBWriter,
    http_forwarder: Option<&HttpForwarder>,
) {
    debug!("Processing message on topic: {}", msg.topic);

    // Parse message once
    let parsed = match processor.parse_message(&msg.topic, &msg.payload, msg.qos) {
        Ok(parsed) => parsed,
        Err(e) => {
            warn!("Failed to parse message: {}", e);
            return;
        }
    };

    // Check each point configuration
    for point_config in points {
        // Check if topic matches
        if !processor.topic_matches(&point_config.topic, &msg.topic) {
            continue;
        }

        // Check schedule if configured
        if let Some(schedule) = &point_config.schedule
            && !processor.schedule_matches(schedule)
        {
            debug!("Skipping {} due to schedule {}", msg.topic, schedule);
            continue;
        }

        // Process this point
        if let Err(e) = process_point(
            point_config,
            &parsed,
            processor,
            influxdb_writer,
            http_forwarder,
        )
        .await
        {
            error!(
                "Failed to process point {}: {}",
                point_config.measurement, e
            );
        }
    }
}

async fn process_point(
    point_config: &PointConfig,
    parsed: &sinqtt::bridge::ParsedMessage,
    processor: &MessageProcessor,
    influxdb_writer: &InfluxDBWriter,
    http_forwarder: Option<&HttpForwarder>,
) -> Result<(), SinqttError> {
    // Get measurement name
    let measurement = match processor.get_value(&point_config.measurement, parsed) {
        Some(serde_json::Value::String(s)) => s,
        Some(v) => v.to_string().trim_matches('"').to_string(),
        None => {
            warn!(
                "Could not determine measurement name for {}",
                point_config.measurement
            );
            return Ok(());
        }
    };

    // Build InfluxDB point
    let mut point = Point::new(&measurement);

    // Add tags
    for (tag_name, tag_spec) in &point_config.tags {
        if let Some(value) = processor.get_value(tag_spec, parsed) {
            let tag_value = match value {
                serde_json::Value::String(s) => s,
                v => v.to_string().trim_matches('"').to_string(),
            };
            if !tag_value.is_empty() {
                point.add_tag(tag_name, &tag_value);
            }
        }
    }

    // Add fields
    let mut fields_added = 0;
    for (field_name, field_spec) in &point_config.fields {
        if let Some(value) = processor.extract_field(field_spec, parsed)
            && let Some(field_value) = FieldValue::from_json(&value)
        {
            point.add_field(field_name, field_value);
            fields_added += 1;
        }
    }

    if fields_added == 0 {
        warn!("No fields to write for measurement: {}", measurement);
        return Ok(());
    }

    // Add timestamp (current time in nanoseconds)
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos() as i64)
        .unwrap_or_else(|_| {
            // Fallback to 0 if system time is before UNIX epoch (should never happen)
            warn!("System time is before UNIX epoch, using 0 as timestamp");
            0
        });
    let point = point.timestamp(timestamp);

    // Write to InfluxDB
    let bucket = point_config.bucket.as_deref();
    influxdb_writer.write_point(&point, bucket).await?;
    debug!("Wrote point to InfluxDB: {}", measurement);

    // HTTP forwarding if configured
    if let Some(forwarder) = http_forwarder
        && !point_config.httpcontent.is_empty()
    {
        let mut content = HttpContentBuilder::new();
        for (key, spec) in &point_config.httpcontent {
            if let Some(value) = processor.get_value(spec, parsed) {
                content.add_from_json(key, &value);
            }
        }
        if !content.is_empty() {
            let json_content = content.build_json();
            if let Err(e) = forwarder.forward_json(&json_content).await {
                warn!("HTTP forward failed: {}", e);
            }
        }
    }

    Ok(())
}
