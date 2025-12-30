//! Command-line interface for sinqtt.

use clap::Parser;
use std::path::PathBuf;

/// MQTT to InfluxDB v3 bridge with flexible JSONPath transformation.
#[derive(Parser, Debug)]
#[command(name = "sinqtt")]
#[command(about = "MQTT to InfluxDB v3 bridge with flexible JSONPath transformation")]
#[command(version)]
pub struct Args {
    /// Path to configuration file (YAML format)
    #[arg(short = 'c', long = "config", required = true)]
    pub config: PathBuf,

    /// Enable debug logging
    #[arg(short = 'D', long = "debug")]
    pub debug: bool,

    /// Validate configuration without running
    #[arg(short = 't', long = "test")]
    pub test: bool,

    /// Daemon mode: retry on error instead of exiting
    #[arg(short = 'd', long = "daemon")]
    pub daemon: bool,
}
