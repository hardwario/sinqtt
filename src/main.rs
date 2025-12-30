//! sinqtt - MQTT to InfluxDB v3 bridge with flexible JSONPath transformation.

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
use tokio_util::sync::CancellationToken;
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

    // Set up graceful shutdown
    let cancel_token = CancellationToken::new();
    let cancel_token_clone = cancel_token.clone();

    // Spawn signal handler
    tokio::spawn(async move {
        shutdown_signal().await;
        info!("Shutdown signal received, stopping...");
        cancel_token_clone.cancel();
    });

    // Run bridge with retry logic if daemon mode
    loop {
        match run_bridge(&config, cancel_token.clone()).await {
            Ok(()) => break,
            Err(e) => {
                if cancel_token.is_cancelled() {
                    info!("Shutdown requested, exiting");
                    break;
                }
                if !args.daemon {
                    return Err(e);
                }
                error!("Error: {e}");
                info!("Retrying in 30 seconds...");
                tokio::select! {
                    () = tokio::time::sleep(Duration::from_secs(30)) => {}
                    () = cancel_token.cancelled() => {
                        info!("Shutdown requested during retry wait");
                        break;
                    }
                }
            }
        }
    }

    Ok(())
}

/// Wait for shutdown signal (Ctrl+C or SIGTERM).
async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("failed to install SIGTERM handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        () = ctrl_c => {}
        () = terminate => {}
    }
}

async fn run_bridge(config: &Config, cancel_token: CancellationToken) -> Result<(), SinqttError> {
    // Collect unique topics from all point configurations (sorted for determinism)
    let mut topics: Vec<String> = config
        .points
        .iter()
        .map(|p| p.topic.clone())
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .collect();
    topics.sort();

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
    let cancel_token_process = cancel_token.clone();

    let process_task = tokio::spawn(async move {
        loop {
            tokio::select! {
                msg = rx.recv() => {
                    match msg {
                        Some(msg) => {
                            process_message(
                                &msg,
                                &points_clone,
                                &processor_clone,
                                &influxdb_clone,
                                http_clone.as_ref().map(std::convert::AsRef::as_ref),
                            )
                            .await;
                        }
                        None => break,
                    }
                }
                () = cancel_token_process.cancelled() => {
                    debug!("Message processor shutting down");
                    break;
                }
            }
        }
    });

    info!("Bridge started, waiting for messages...");

    // Run MQTT handler with cancellation support
    let mqtt_result = tokio::select! {
        result = mqtt_handler.run(tx) => result,
        () = cancel_token.cancelled() => {
            info!("MQTT handler shutting down");
            Ok(())
        }
    };

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
        .map(|d| {
            // Safe conversion: i64 can hold nanoseconds until year ~2262
            i64::try_from(d.as_nanos()).unwrap_or(i64::MAX)
        })
        .unwrap_or(0);
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
