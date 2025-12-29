//! InfluxDB writer using line protocol over HTTP.

use crate::config::InfluxDBConfig;
use crate::error::SinqttError;
use reqwest::Client;
use std::collections::HashMap;

/// InfluxDB writer for sending data points.
pub struct InfluxDBWriter {
    client: Client,
    url: String,
    token: String,
    org: String,
    default_bucket: String,
    #[allow(dead_code)]
    enable_gzip: bool,
}

impl InfluxDBWriter {
    /// Create a new InfluxDB writer from configuration.
    pub fn new(config: &InfluxDBConfig) -> Result<Self, SinqttError> {
        let client = Client::new();
        let url = format!("{}:{}", config.host, config.port);

        Ok(Self {
            client,
            url,
            token: config.token.clone(),
            org: config.org.clone(),
            default_bucket: config.bucket.clone(),
            enable_gzip: config.enable_gzip,
        })
    }

    /// Write a point to InfluxDB.
    pub async fn write_point(&self, point: &Point, bucket: Option<&str>) -> Result<(), SinqttError> {
        let bucket = bucket.unwrap_or(&self.default_bucket);
        let write_url = format!(
            "{}/api/v2/write?org={}&bucket={}",
            self.url, self.org, bucket
        );

        let line = point.to_line_protocol();

        self.client
            .post(&write_url)
            .header("Authorization", format!("Token {}", self.token))
            .header("Content-Type", "text/plain; charset=utf-8")
            .body(line)
            .send()
            .await?
            .error_for_status()
            .map_err(|e| SinqttError::InfluxDb(e.to_string()))?;

        Ok(())
    }
}

/// Represents an InfluxDB data point.
#[derive(Debug, Clone)]
pub struct Point {
    pub measurement: String,
    pub tags: HashMap<String, String>,
    pub fields: HashMap<String, FieldValue>,
    pub timestamp: Option<i64>,
}

/// Field value types supported by InfluxDB.
#[derive(Debug, Clone)]
pub enum FieldValue {
    Float(f64),
    Int(i64),
    String(String),
    Bool(bool),
}

impl Point {
    /// Create a new point with the given measurement name.
    pub fn new(measurement: impl Into<String>) -> Self {
        Self {
            measurement: measurement.into(),
            tags: HashMap::new(),
            fields: HashMap::new(),
            timestamp: None,
        }
    }

    /// Add a tag to the point.
    pub fn tag(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.tags.insert(key.into(), value.into());
        self
    }

    /// Add a field to the point.
    pub fn field(mut self, key: impl Into<String>, value: FieldValue) -> Self {
        self.fields.insert(key.into(), value);
        self
    }

    /// Set the timestamp.
    pub fn timestamp(mut self, ts: i64) -> Self {
        self.timestamp = Some(ts);
        self
    }

    /// Convert the point to InfluxDB line protocol format.
    pub fn to_line_protocol(&self) -> String {
        let mut line = escape_measurement(&self.measurement);

        // Add tags (sorted for consistency)
        let mut tag_pairs: Vec<_> = self.tags.iter().collect();
        tag_pairs.sort_by_key(|(k, _)| *k);
        for (key, value) in tag_pairs {
            line.push(',');
            line.push_str(&escape_tag_key(key));
            line.push('=');
            line.push_str(&escape_tag_value(value));
        }

        // Add fields (sorted for consistency)
        line.push(' ');
        let mut field_pairs: Vec<_> = self.fields.iter().collect();
        field_pairs.sort_by_key(|(k, _)| *k);
        let field_strs: Vec<String> = field_pairs
            .iter()
            .map(|(key, value)| {
                let escaped_key = escape_field_key(key);
                let value_str = match value {
                    FieldValue::Float(f) => format!("{}", f),
                    FieldValue::Int(i) => format!("{}i", i),
                    FieldValue::String(s) => format!("\"{}\"", escape_string_value(s)),
                    FieldValue::Bool(b) => format!("{}", b),
                };
                format!("{}={}", escaped_key, value_str)
            })
            .collect();
        line.push_str(&field_strs.join(","));

        // Add timestamp if present
        if let Some(ts) = self.timestamp {
            line.push(' ');
            line.push_str(&ts.to_string());
        }

        line
    }
}

fn escape_measurement(s: &str) -> String {
    s.replace(',', "\\,").replace(' ', "\\ ")
}

fn escape_tag_key(s: &str) -> String {
    s.replace(',', "\\,")
        .replace('=', "\\=")
        .replace(' ', "\\ ")
}

fn escape_tag_value(s: &str) -> String {
    s.replace(',', "\\,")
        .replace('=', "\\=")
        .replace(' ', "\\ ")
}

fn escape_field_key(s: &str) -> String {
    s.replace(',', "\\,")
        .replace('=', "\\=")
        .replace(' ', "\\ ")
}

fn escape_string_value(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_line_protocol_simple() {
        let point = Point::new("temperature")
            .field("value", FieldValue::Float(23.5));

        assert_eq!(point.to_line_protocol(), "temperature value=23.5");
    }

    #[test]
    fn test_line_protocol_with_tags() {
        let point = Point::new("temperature")
            .tag("location", "room1")
            .field("value", FieldValue::Float(23.5));

        assert_eq!(
            point.to_line_protocol(),
            "temperature,location=room1 value=23.5"
        );
    }

    #[test]
    fn test_line_protocol_with_timestamp() {
        let point = Point::new("temperature")
            .field("value", FieldValue::Float(23.5))
            .timestamp(1609459200000000000);

        assert_eq!(
            point.to_line_protocol(),
            "temperature value=23.5 1609459200000000000"
        );
    }
}
