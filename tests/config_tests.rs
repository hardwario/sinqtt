//! Comprehensive tests for configuration module.

use sinqtt::config::{Config, FieldSpec};
use sinqtt::error::ConfigError;
use sinqtt::load_config;
use std::io::Write;
use tempfile::NamedTempFile;

// Helper to create a config file and load it
fn load_yaml_config(yaml: &str) -> Result<Config, ConfigError> {
    let mut file = NamedTempFile::new().unwrap();
    file.write_all(yaml.as_bytes()).unwrap();
    load_config(file.path())
}

// ============================================================================
// MqttConfig Tests
// ============================================================================

#[test]
fn test_mqtt_config_minimal() {
    let yaml = r#"
mqtt:
  host: localhost
  port: 1883
influxdb:
  host: localhost
  token: test-token
  org: test-org
  bucket: test-bucket
points:
  - measurement: test
    topic: test/#
    fields:
      value: "$.payload"
"#;
    let config = load_yaml_config(yaml).unwrap();
    assert_eq!(config.mqtt.host, "localhost");
    assert_eq!(config.mqtt.port, 1883);
    assert!(config.mqtt.username.is_none());
    assert!(config.mqtt.password.is_none());
}

#[test]
fn test_mqtt_config_full() {
    let yaml = r#"
mqtt:
  host: mqtt.example.com
  port: 8883
  username: user
  password: secret
influxdb:
  host: localhost
  token: test-token
  org: test-org
  bucket: test-bucket
points:
  - measurement: test
    topic: test/#
    fields:
      value: "$.payload"
"#;
    let config = load_yaml_config(yaml).unwrap();
    assert_eq!(config.mqtt.host, "mqtt.example.com");
    assert_eq!(config.mqtt.port, 8883);
    assert_eq!(config.mqtt.username, Some("user".to_string()));
    assert_eq!(config.mqtt.password, Some("secret".to_string()));
}

#[test]
fn test_mqtt_port_validation_min() {
    let yaml = r#"
mqtt:
  host: localhost
  port: 0
influxdb:
  host: localhost
  token: test-token
  org: test-org
  bucket: test-bucket
points:
  - measurement: test
    topic: test/#
    fields:
      value: "$.payload"
"#;
    let config = load_yaml_config(yaml).unwrap();
    assert_eq!(config.mqtt.port, 0);
}

#[test]
fn test_mqtt_port_validation_max() {
    let yaml = r#"
mqtt:
  host: localhost
  port: 65535
influxdb:
  host: localhost
  token: test-token
  org: test-org
  bucket: test-bucket
points:
  - measurement: test
    topic: test/#
    fields:
      value: "$.payload"
"#;
    let config = load_yaml_config(yaml).unwrap();
    assert_eq!(config.mqtt.port, 65535);
}

#[test]
fn test_mqtt_empty_host_rejected() {
    let yaml = r#"
mqtt:
  host: ""
  port: 1883
influxdb:
  host: localhost
  token: test-token
  org: test-org
  bucket: test-bucket
points:
  - measurement: test
    topic: test/#
    fields:
      value: "$.payload"
"#;
    let result = load_yaml_config(yaml);
    assert!(result.is_err());
}

// ============================================================================
// InfluxDBConfig Tests
// ============================================================================

#[test]
fn test_influxdb_config_minimal() {
    let yaml = r#"
mqtt:
  host: localhost
  port: 1883
influxdb:
  host: localhost
  token: my-token
  org: my-org
  bucket: my-bucket
points:
  - measurement: test
    topic: test/#
    fields:
      value: "$.payload"
"#;
    let config = load_yaml_config(yaml).unwrap();
    assert_eq!(config.influxdb.host, "localhost");
    assert_eq!(config.influxdb.port, 8181); // Default
    assert_eq!(config.influxdb.token, "my-token");
    assert_eq!(config.influxdb.org, "my-org");
    assert_eq!(config.influxdb.bucket, "my-bucket");
    assert!(!config.influxdb.enable_gzip);
}

#[test]
fn test_influxdb_config_with_gzip() {
    let yaml = r#"
mqtt:
  host: localhost
  port: 1883
influxdb:
  host: localhost
  token: token
  org: org
  bucket: bucket
  enable_gzip: true
points:
  - measurement: test
    topic: test/#
    fields:
      value: "$.payload"
"#;
    let config = load_yaml_config(yaml).unwrap();
    assert!(config.influxdb.enable_gzip);
}

#[test]
fn test_influxdb_empty_token_rejected() {
    let yaml = r#"
mqtt:
  host: localhost
  port: 1883
influxdb:
  host: localhost
  token: ""
  org: org
  bucket: bucket
points:
  - measurement: test
    topic: test/#
    fields:
      value: "$.payload"
"#;
    let result = load_yaml_config(yaml);
    assert!(result.is_err());
}

// ============================================================================
// HttpConfig Tests
// ============================================================================

#[test]
fn test_http_config_minimal() {
    let yaml = r#"
mqtt:
  host: localhost
  port: 1883
influxdb:
  host: localhost
  token: token
  org: org
  bucket: bucket
http:
  destination: http://example.com/api
  action: post
points:
  - measurement: test
    topic: test/#
    fields:
      value: "$.payload"
"#;
    let config = load_yaml_config(yaml).unwrap();
    let http = config.http.unwrap();
    assert_eq!(http.destination, "http://example.com/api");
    assert_eq!(http.action, "post");
    assert!(http.username.is_none());
    assert!(http.password.is_none());
}

#[test]
fn test_http_config_with_auth() {
    let yaml = r#"
mqtt:
  host: localhost
  port: 1883
influxdb:
  host: localhost
  token: token
  org: org
  bucket: bucket
http:
  destination: http://example.com/api
  action: post
  username: user
  password: pass
points:
  - measurement: test
    topic: test/#
    fields:
      value: "$.payload"
"#;
    let config = load_yaml_config(yaml).unwrap();
    let http = config.http.unwrap();
    assert_eq!(http.username, Some("user".to_string()));
    assert_eq!(http.password, Some("pass".to_string()));
}

// ============================================================================
// Base64DecodeConfig Tests
// ============================================================================

#[test]
fn test_base64decode_config() {
    let yaml = r#"
mqtt:
  host: localhost
  port: 1883
influxdb:
  host: localhost
  token: token
  org: org
  bucket: bucket
base64decode:
  source: "$.payload.data"
  target: decoded
points:
  - measurement: test
    topic: test/#
    fields:
      value: "$.payload"
"#;
    let config = load_yaml_config(yaml).unwrap();
    let b64 = config.base64decode.unwrap();
    assert_eq!(b64.source, "$.payload.data");
    assert_eq!(b64.target, "decoded");
}

// ============================================================================
// PointConfig Tests
// ============================================================================

#[test]
fn test_point_config_minimal() {
    let yaml = r#"
mqtt:
  host: localhost
  port: 1883
influxdb:
  host: localhost
  token: token
  org: org
  bucket: bucket
points:
  - measurement: temperature
    topic: test/+/temperature
    fields:
      value: "$.payload"
"#;
    let config = load_yaml_config(yaml).unwrap();
    let point = &config.points[0];
    assert_eq!(point.measurement, "temperature");
    assert_eq!(point.topic, "test/+/temperature");
    assert!(point.bucket.is_none());
    assert!(point.schedule.is_none());
}

#[test]
fn test_point_config_full() {
    let yaml = r#"
mqtt:
  host: localhost
  port: 1883
influxdb:
  host: localhost
  token: token
  org: org
  bucket: bucket
points:
  - measurement: temperature
    topic: node/+/thermometer/+/temperature
    bucket: custom-bucket
    schedule: "0 * * * *"
    fields:
      value: "$.payload"
    tags:
      sensor_id: "$.topic[1]"
"#;
    let config = load_yaml_config(yaml).unwrap();
    let point = &config.points[0];
    assert_eq!(point.bucket, Some("custom-bucket".to_string()));
    assert_eq!(point.schedule, Some("0 * * * *".to_string()));
    assert!(point.tags.contains_key("sensor_id"));
}

#[test]
fn test_point_valid_schedules() {
    let schedules = vec![
        "* * * * *",
        "0 * * * *",
        "*/5 * * * *",
        "0 9 * * 1-5",
        "30 14 * * *",
        "0 0 1 * *",
        "0,30 * * * *",
    ];

    for schedule in schedules {
        let yaml = format!(
            r#"
mqtt:
  host: localhost
  port: 1883
influxdb:
  host: localhost
  token: token
  org: org
  bucket: bucket
points:
  - measurement: test
    topic: test
    schedule: "{}"
    fields:
      value: "$.payload"
"#,
            schedule
        );
        let result = load_yaml_config(&yaml);
        assert!(result.is_ok(), "Schedule '{}' should be valid", schedule);
    }
}

#[test]
fn test_point_invalid_schedule() {
    let yaml = r#"
mqtt:
  host: localhost
  port: 1883
influxdb:
  host: localhost
  token: token
  org: org
  bucket: bucket
points:
  - measurement: test
    topic: test
    schedule: "invalid"
    fields:
      value: "$.payload"
"#;
    let result = load_yaml_config(yaml);
    assert!(result.is_err());
}

#[test]
fn test_point_measurement_jsonpath() {
    let yaml = r#"
mqtt:
  host: localhost
  port: 1883
influxdb:
  host: localhost
  token: token
  org: org
  bucket: bucket
points:
  - measurement: "$.payload.type"
    topic: test
    fields:
      value: "$.payload"
"#;
    let config = load_yaml_config(yaml).unwrap();
    assert_eq!(config.points[0].measurement, "$.payload.type");
}

#[test]
fn test_point_invalid_measurement_jsonpath() {
    let yaml = r#"
mqtt:
  host: localhost
  port: 1883
influxdb:
  host: localhost
  token: token
  org: org
  bucket: bucket
points:
  - measurement: "$.invalid["
    topic: test
    fields:
      value: "$.payload"
"#;
    let result = load_yaml_config(yaml);
    assert!(result.is_err());
}

// ============================================================================
// Config Tests
// ============================================================================

#[test]
fn test_empty_points_rejected() {
    let yaml = r#"
mqtt:
  host: localhost
  port: 1883
influxdb:
  host: localhost
  token: token
  org: org
  bucket: bucket
points: []
"#;
    let result = load_yaml_config(yaml);
    assert!(result.is_err());
}

#[test]
fn test_missing_mqtt_rejected() {
    let yaml = r#"
influxdb:
  host: localhost
  token: token
  org: org
  bucket: bucket
points:
  - measurement: test
    topic: test
    fields:
      value: "$.payload"
"#;
    let result = load_yaml_config(yaml);
    assert!(result.is_err());
}

#[test]
fn test_missing_influxdb_rejected() {
    let yaml = r#"
mqtt:
  host: localhost
  port: 1883
points:
  - measurement: test
    topic: test
    fields:
      value: "$.payload"
"#;
    let result = load_yaml_config(yaml);
    assert!(result.is_err());
}

// ============================================================================
// LoadConfig Tests
// ============================================================================

#[test]
fn test_load_empty_file() {
    let result = load_yaml_config("");
    assert!(result.is_err());
}

#[test]
fn test_load_invalid_yaml() {
    let result = load_yaml_config("invalid: yaml: content:");
    assert!(result.is_err());
}

// ============================================================================
// Environment Variable Tests
// ============================================================================

#[test]
fn test_env_var_substitution() {
    std::env::set_var("SINQTT_TEST_TOKEN", "secret-token-123");

    let yaml = r#"
mqtt:
  host: localhost
  port: 1883
influxdb:
  host: localhost
  token: ${SINQTT_TEST_TOKEN}
  org: test-org
  bucket: test-bucket
points:
  - measurement: test
    topic: test/#
    fields:
      value: "$.payload"
"#;
    let config = load_yaml_config(yaml).unwrap();
    assert_eq!(config.influxdb.token, "secret-token-123");

    std::env::remove_var("SINQTT_TEST_TOKEN");
}

#[test]
fn test_env_var_with_default() {
    std::env::remove_var("SINQTT_NONEXISTENT_VAR");

    let yaml = r#"
mqtt:
  host: ${SINQTT_NONEXISTENT_VAR:localhost}
  port: 1883
influxdb:
  host: localhost
  token: test-token
  org: test-org
  bucket: test-bucket
points:
  - measurement: test
    topic: test/#
    fields:
      value: "$.payload"
"#;
    let config = load_yaml_config(yaml).unwrap();
    assert_eq!(config.mqtt.host, "localhost");
}

#[test]
fn test_env_var_with_default_override() {
    std::env::set_var("SINQTT_MQTT_HOST", "mqtt.example.com");

    let yaml = r#"
mqtt:
  host: ${SINQTT_MQTT_HOST:localhost}
  port: 1883
influxdb:
  host: localhost
  token: test-token
  org: test-org
  bucket: test-bucket
points:
  - measurement: test
    topic: test/#
    fields:
      value: "$.payload"
"#;
    let config = load_yaml_config(yaml).unwrap();
    assert_eq!(config.mqtt.host, "mqtt.example.com");

    std::env::remove_var("SINQTT_MQTT_HOST");
}

#[test]
fn test_env_var_missing_raises_error() {
    std::env::remove_var("SINQTT_MISSING_TOKEN");

    let yaml = r#"
mqtt:
  host: localhost
  port: 1883
influxdb:
  host: localhost
  token: ${SINQTT_MISSING_TOKEN}
  org: test-org
  bucket: test-bucket
points:
  - measurement: test
    topic: test/#
    fields:
      value: "$.payload"
"#;
    let result = load_yaml_config(yaml);
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(err.to_string().contains("SINQTT_MISSING_TOKEN"));
}

#[test]
fn test_multiple_env_vars_in_string() {
    std::env::set_var("SINQTT_HTTP_HOST", "api.example.com");
    std::env::set_var("SINQTT_HTTP_PORT", "443");

    let yaml = r#"
mqtt:
  host: localhost
  port: 1883
influxdb:
  host: localhost
  token: test-token
  org: test-org
  bucket: test-bucket
http:
  destination: https://${SINQTT_HTTP_HOST}:${SINQTT_HTTP_PORT}/api
  action: post
points:
  - measurement: test
    topic: test/#
    fields:
      value: "$.payload"
"#;
    let config = load_yaml_config(yaml).unwrap();
    assert_eq!(
        config.http.unwrap().destination,
        "https://api.example.com:443/api"
    );

    std::env::remove_var("SINQTT_HTTP_HOST");
    std::env::remove_var("SINQTT_HTTP_PORT");
}

// ============================================================================
// Field Config Tests
// ============================================================================

#[test]
fn test_field_simple_spec() {
    let yaml = r#"
mqtt:
  host: localhost
  port: 1883
influxdb:
  host: localhost
  token: token
  org: org
  bucket: bucket
points:
  - measurement: test
    topic: test/#
    fields:
      value: "$.payload"
"#;
    let config = load_yaml_config(yaml).unwrap();
    match &config.points[0].fields.get("value").unwrap() {
        FieldSpec::Simple(s) => assert_eq!(s, "$.payload"),
        _ => panic!("Expected Simple field spec"),
    }
}

#[test]
fn test_field_typed_spec() {
    let yaml = r#"
mqtt:
  host: localhost
  port: 1883
influxdb:
  host: localhost
  token: token
  org: org
  bucket: bucket
points:
  - measurement: test
    topic: test/#
    fields:
      value:
        value: "$.payload.raw"
        type: float
"#;
    let config = load_yaml_config(yaml).unwrap();
    match &config.points[0].fields.get("value").unwrap() {
        FieldSpec::Typed(fc) => {
            assert_eq!(fc.value, "$.payload.raw");
            assert_eq!(fc.field_type, Some("float".to_string()));
        }
        _ => panic!("Expected Typed field spec"),
    }
}
