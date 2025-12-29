//! Configuration types for sinqtt.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

/// Root configuration structure.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub mqtt: MqttConfig,
    pub influxdb: InfluxDBConfig,
    #[serde(default)]
    pub http: Option<HttpConfig>,
    #[serde(default)]
    pub base64decode: Option<Base64DecodeConfig>,
    pub points: Vec<PointConfig>,
}

/// MQTT broker configuration.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct MqttConfig {
    pub host: String,
    pub port: u16,
    #[serde(default)]
    pub username: Option<String>,
    #[serde(default)]
    pub password: Option<String>,
    #[serde(default)]
    pub cafile: Option<PathBuf>,
    #[serde(default)]
    pub certfile: Option<PathBuf>,
    #[serde(default)]
    pub keyfile: Option<PathBuf>,
}

/// InfluxDB configuration.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct InfluxDBConfig {
    pub host: String,
    #[serde(default = "default_influxdb_port")]
    pub port: u16,
    pub token: String,
    pub org: String,
    pub bucket: String,
    #[serde(default)]
    pub enable_gzip: bool,
}

fn default_influxdb_port() -> u16 {
    8181
}

/// HTTP forwarding configuration.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct HttpConfig {
    pub destination: String,
    pub action: String,
    #[serde(default)]
    pub username: Option<String>,
    #[serde(default)]
    pub password: Option<String>,
}

/// Base64 decoding configuration.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Base64DecodeConfig {
    pub source: String,
    pub target: String,
}

/// Point configuration for mapping MQTT topics to InfluxDB points.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PointConfig {
    pub measurement: String,
    pub topic: String,
    #[serde(default)]
    pub bucket: Option<String>,
    #[serde(default)]
    pub schedule: Option<String>,
    pub fields: HashMap<String, FieldSpec>,
    #[serde(default)]
    pub tags: HashMap<String, String>,
    #[serde(default)]
    pub httpcontent: HashMap<String, String>,
}

/// Field specification - either a simple string or typed config.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(untagged)]
pub enum FieldSpec {
    Simple(String),
    Typed(FieldConfig),
}

/// Typed field configuration with optional type conversion.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FieldConfig {
    pub value: String,
    #[serde(rename = "type")]
    pub field_type: Option<String>,
}
