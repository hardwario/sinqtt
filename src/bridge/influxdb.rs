//! InfluxDB writer using line protocol over HTTP.

use crate::config::InfluxDBConfig;
use crate::error::SinqttError;
use reqwest::Client;
use serde_json::Value;
use std::collections::HashMap;
use std::io::Write;
use tracing::{debug, error};

/// InfluxDB writer for sending data points.
pub struct InfluxDBWriter {
    client: Client,
    write_url: String,
    token: String,
    default_bucket: String,
    org: String,
    enable_gzip: bool,
}

impl InfluxDBWriter {
    /// Create a new InfluxDB writer from configuration.
    pub fn new(config: &InfluxDBConfig) -> Result<Self, SinqttError> {
        let client = Client::new();

        // Build base URL with proper scheme
        let base_url = if config.host.starts_with("http://") || config.host.starts_with("https://")
        {
            format!("{}:{}", config.host, config.port)
        } else {
            format!("http://{}:{}", config.host, config.port)
        };

        Ok(Self {
            client,
            write_url: base_url,
            token: config.token.clone(),
            org: config.org.clone(),
            default_bucket: config.bucket.clone(),
            enable_gzip: config.enable_gzip,
        })
    }

    /// Write a point to InfluxDB.
    pub async fn write_point(
        &self,
        point: &Point,
        bucket: Option<&str>,
    ) -> Result<(), SinqttError> {
        let bucket = bucket.unwrap_or(&self.default_bucket);
        let url = format!(
            "{}/api/v2/write?org={}&bucket={}&precision=ns",
            self.write_url, self.org, bucket
        );

        let line = point.to_line_protocol();
        debug!("Writing to InfluxDB: {}", line);

        self.send_write_request(&url, line).await
    }

    /// Write multiple points to InfluxDB in a single request.
    pub async fn write_points(
        &self,
        points: &[Point],
        bucket: Option<&str>,
    ) -> Result<(), SinqttError> {
        if points.is_empty() {
            return Ok(());
        }

        let bucket = bucket.unwrap_or(&self.default_bucket);
        let url = format!(
            "{}/api/v2/write?org={}&bucket={}&precision=ns",
            self.write_url, self.org, bucket
        );

        let lines: Vec<String> = points.iter().map(|p| p.to_line_protocol()).collect();
        let body = lines.join("\n");
        debug!("Writing {} points to InfluxDB", points.len());

        self.send_write_request(&url, body).await
    }

    /// Send write request with optional gzip compression.
    async fn send_write_request(&self, url: &str, body: String) -> Result<(), SinqttError> {
        let mut request = self
            .client
            .post(url)
            .header("Authorization", format!("Token {}", self.token))
            .header("Content-Type", "text/plain; charset=utf-8");

        let body_bytes = if self.enable_gzip {
            // Compress with gzip
            let mut encoder =
                flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
            encoder.write_all(body.as_bytes())?;
            let compressed = encoder.finish()?;
            request = request.header("Content-Encoding", "gzip");
            compressed
        } else {
            body.into_bytes()
        };

        let response = request.body(body_bytes).send().await?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            error!("InfluxDB write failed: {} - {}", status, body);
            return Err(SinqttError::InfluxDb(format!(
                "Write failed with status {}: {}",
                status, body
            )));
        }

        Ok(())
    }

    /// Get the default bucket name.
    pub fn default_bucket(&self) -> &str {
        &self.default_bucket
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
#[derive(Debug, Clone, PartialEq)]
pub enum FieldValue {
    Float(f64),
    Int(i64),
    UInt(u64),
    String(String),
    Bool(bool),
}

impl FieldValue {
    /// Create a FieldValue from a serde_json::Value.
    pub fn from_json(value: &Value) -> Option<Self> {
        match value {
            Value::Number(n) => {
                if let Some(i) = n.as_i64() {
                    Some(FieldValue::Int(i))
                } else if let Some(u) = n.as_u64() {
                    Some(FieldValue::UInt(u))
                } else {
                    n.as_f64().map(FieldValue::Float)
                }
            }
            Value::String(s) => Some(FieldValue::String(s.clone())),
            Value::Bool(b) => Some(FieldValue::Bool(*b)),
            _ => None,
        }
    }

    /// Create a Float field value.
    pub fn float(value: f64) -> Self {
        FieldValue::Float(value)
    }

    /// Create an Int field value.
    pub fn int(value: i64) -> Self {
        FieldValue::Int(value)
    }

    /// Create a String field value.
    pub fn string(value: impl Into<String>) -> Self {
        FieldValue::String(value.into())
    }

    /// Create a Bool field value.
    pub fn bool(value: bool) -> Self {
        FieldValue::Bool(value)
    }
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

    /// Add a tag to the point (mutable reference version).
    pub fn add_tag(&mut self, key: impl Into<String>, value: impl Into<String>) -> &mut Self {
        self.tags.insert(key.into(), value.into());
        self
    }

    /// Add a field to the point.
    pub fn field(mut self, key: impl Into<String>, value: FieldValue) -> Self {
        self.fields.insert(key.into(), value);
        self
    }

    /// Add a field to the point (mutable reference version).
    pub fn add_field(&mut self, key: impl Into<String>, value: FieldValue) -> &mut Self {
        self.fields.insert(key.into(), value);
        self
    }

    /// Add a field from a JSON value.
    pub fn field_from_json(mut self, key: impl Into<String>, value: &Value) -> Self {
        if let Some(field_value) = FieldValue::from_json(value) {
            self.fields.insert(key.into(), field_value);
        }
        self
    }

    /// Set the timestamp in nanoseconds.
    pub fn timestamp(mut self, ts: i64) -> Self {
        self.timestamp = Some(ts);
        self
    }

    /// Set the timestamp from a chrono DateTime.
    pub fn timestamp_from_datetime(mut self, dt: chrono::DateTime<chrono::Utc>) -> Self {
        self.timestamp = Some(dt.timestamp_nanos_opt().unwrap_or(0));
        self
    }

    /// Check if the point has any fields.
    pub fn has_fields(&self) -> bool {
        !self.fields.is_empty()
    }

    /// Convert the point to InfluxDB line protocol format.
    pub fn to_line_protocol(&self) -> String {
        let mut line = escape_measurement(&self.measurement);

        // Add tags (sorted for consistency)
        let mut tag_pairs: Vec<_> = self.tags.iter().collect();
        tag_pairs.sort_by_key(|(k, _)| *k);
        for (key, value) in tag_pairs {
            if !value.is_empty() {
                line.push(',');
                line.push_str(&escape_tag_key(key));
                line.push('=');
                line.push_str(&escape_tag_value(value));
            }
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
                    FieldValue::Float(f) => {
                        // Ensure we always have decimal point for floats
                        if f.fract() == 0.0 {
                            format!("{}.0", f)
                        } else {
                            format!("{}", f)
                        }
                    }
                    FieldValue::Int(i) => format!("{}i", i),
                    FieldValue::UInt(u) => format!("{}u", u),
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
        let point = Point::new("temperature").field("value", FieldValue::Float(23.5));

        assert_eq!(point.to_line_protocol(), "temperature value=23.5");
    }

    #[test]
    fn test_line_protocol_integer_float() {
        let point = Point::new("count").field("value", FieldValue::Float(100.0));

        // Should have .0 suffix for whole numbers
        assert_eq!(point.to_line_protocol(), "count value=100.0");
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
    fn test_line_protocol_multiple_tags() {
        let point = Point::new("temperature")
            .tag("location", "room1")
            .tag("sensor", "dht22")
            .field("value", FieldValue::Float(23.5));

        // Tags should be sorted alphabetically
        assert_eq!(
            point.to_line_protocol(),
            "temperature,location=room1,sensor=dht22 value=23.5"
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

    #[test]
    fn test_line_protocol_integer_field() {
        let point = Point::new("count").field("value", FieldValue::Int(42));

        assert_eq!(point.to_line_protocol(), "count value=42i");
    }

    #[test]
    fn test_line_protocol_unsigned_field() {
        let point = Point::new("count").field("value", FieldValue::UInt(42));

        assert_eq!(point.to_line_protocol(), "count value=42u");
    }

    #[test]
    fn test_line_protocol_string_field() {
        let point = Point::new("event").field("message", FieldValue::String("hello world".into()));

        assert_eq!(point.to_line_protocol(), r#"event message="hello world""#);
    }

    #[test]
    fn test_line_protocol_bool_field() {
        let point = Point::new("status")
            .field("active", FieldValue::Bool(true))
            .field("error", FieldValue::Bool(false));

        assert_eq!(point.to_line_protocol(), "status active=true,error=false");
    }

    #[test]
    fn test_line_protocol_multiple_fields() {
        let point = Point::new("weather")
            .field("humidity", FieldValue::Float(65.2))
            .field("temperature", FieldValue::Float(23.5));

        // Fields should be sorted alphabetically
        assert_eq!(
            point.to_line_protocol(),
            "weather humidity=65.2,temperature=23.5"
        );
    }

    #[test]
    fn test_line_protocol_escape_measurement() {
        let point = Point::new("my measurement").field("value", FieldValue::Int(1));

        assert_eq!(point.to_line_protocol(), r"my\ measurement value=1i");
    }

    #[test]
    fn test_line_protocol_escape_tag_key() {
        let point = Point::new("test")
            .tag("my tag", "value")
            .field("value", FieldValue::Int(1));

        assert_eq!(point.to_line_protocol(), r"test,my\ tag=value value=1i");
    }

    #[test]
    fn test_line_protocol_escape_tag_value() {
        let point = Point::new("test")
            .tag("location", "room 1")
            .field("value", FieldValue::Int(1));

        assert_eq!(point.to_line_protocol(), r"test,location=room\ 1 value=1i");
    }

    #[test]
    fn test_line_protocol_escape_string_value() {
        let point =
            Point::new("event").field("message", FieldValue::String(r#"say "hello""#.into()));

        assert_eq!(point.to_line_protocol(), r#"event message="say \"hello\"""#);
    }

    #[test]
    fn test_line_protocol_escape_backslash() {
        let point = Point::new("event").field("path", FieldValue::String(r"C:\Users\test".into()));

        assert_eq!(point.to_line_protocol(), r#"event path="C:\\Users\\test""#);
    }

    #[test]
    fn test_line_protocol_empty_tag_skipped() {
        let point = Point::new("test")
            .tag("filled", "value")
            .tag("empty", "")
            .field("value", FieldValue::Int(1));

        assert_eq!(point.to_line_protocol(), "test,filled=value value=1i");
    }

    #[test]
    fn test_line_protocol_full_example() {
        let point = Point::new("sensor_data")
            .tag("device", "sensor01")
            .tag("location", "room1")
            .field("humidity", FieldValue::Float(65.0))
            .field("temperature", FieldValue::Float(23.5))
            .timestamp(1609459200000000000);

        assert_eq!(
            point.to_line_protocol(),
            "sensor_data,device=sensor01,location=room1 humidity=65.0,temperature=23.5 1609459200000000000"
        );
    }

    #[test]
    fn test_field_value_from_json_int() {
        let json = serde_json::json!(42);
        let field = FieldValue::from_json(&json).unwrap();
        assert_eq!(field, FieldValue::Int(42));
    }

    #[test]
    fn test_field_value_from_json_float() {
        let json = serde_json::json!(23.5);
        let field = FieldValue::from_json(&json).unwrap();
        assert_eq!(field, FieldValue::Float(23.5));
    }

    #[test]
    fn test_field_value_from_json_string() {
        let json = serde_json::json!("hello");
        let field = FieldValue::from_json(&json).unwrap();
        assert_eq!(field, FieldValue::String("hello".into()));
    }

    #[test]
    fn test_field_value_from_json_bool() {
        let json = serde_json::json!(true);
        let field = FieldValue::from_json(&json).unwrap();
        assert_eq!(field, FieldValue::Bool(true));
    }

    #[test]
    fn test_field_value_from_json_null_returns_none() {
        let json = serde_json::json!(null);
        assert!(FieldValue::from_json(&json).is_none());
    }

    #[test]
    fn test_field_value_from_json_object_returns_none() {
        let json = serde_json::json!({"key": "value"});
        assert!(FieldValue::from_json(&json).is_none());
    }

    #[test]
    fn test_point_has_fields() {
        let point = Point::new("test");
        assert!(!point.has_fields());

        let point = point.field("value", FieldValue::Int(1));
        assert!(point.has_fields());
    }

    #[test]
    fn test_point_mutable_methods() {
        let mut point = Point::new("test");
        point.add_tag("location", "room1");
        point.add_field("value", FieldValue::Int(42));

        assert_eq!(point.to_line_protocol(), "test,location=room1 value=42i");
    }

    #[test]
    fn test_field_from_json() {
        let json = serde_json::json!({"temp": 23.5, "count": 42});
        let point = Point::new("test")
            .field_from_json("temp", &json["temp"])
            .field_from_json("count", &json["count"]);

        assert_eq!(point.to_line_protocol(), "test count=42i,temp=23.5");
    }
}
