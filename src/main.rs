//! sinqtt - MQTT to InfluxDB bridge for IoT applications.

use clap::Parser;
use sinqtt::cli::Args;
use sinqtt::error::SinqttError;
use sinqtt::load_config;
use std::time::Duration;
use tracing::{error, info};
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

    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .init();

    // Load configuration
    info!("Loading configuration from: {:?}", args.config);
    let config = load_config(&args.config)?;

    if args.test {
        println!("Configuration file is valid.");
        return Ok(());
    }

    info!("Configuration loaded successfully");
    info!("MQTT broker: {}:{}", config.mqtt.host, config.mqtt.port);
    info!("InfluxDB: {}:{}", config.influxdb.host, config.influxdb.port);
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

async fn run_bridge(_config: &sinqtt::Config) -> Result<(), SinqttError> {
    // TODO: Implement bridge orchestration
    info!("Bridge started (not yet implemented)");
    Ok(())
}
