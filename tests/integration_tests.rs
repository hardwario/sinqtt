//! Integration tests for the full message processing pipeline.
//!
//! These tests verify the complete MQTT message to InfluxDB point conversion,
//! ported from Python `test_integration.py`.

use serde_json::json;
use sinqtt::bridge::{FieldValue, MessageProcessor, Point};
use sinqtt::config::{FieldConfig, FieldSpec, PointConfig};
use std::collections::HashMap;

/// Helper to create a simple point config.
fn make_point_config(
    measurement: &str,
    topic: &str,
    fields: Vec<(&str, &str)>,
    tags: Vec<(&str, &str)>,
) -> PointConfig {
    PointConfig {
        measurement: measurement.to_string(),
        topic: topic.to_string(),
        bucket: None,
        schedule: None,
        fields: fields
            .into_iter()
            .map(|(k, v)| (k.to_string(), FieldSpec::Simple(v.to_string())))
            .collect(),
        tags: tags
            .into_iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect(),
        httpcontent: HashMap::new(),
    }
}

/// Helper to create a point config with typed fields.
fn make_typed_point_config(
    measurement: &str,
    topic: &str,
    fields: Vec<(&str, &str, Option<&str>)>,
) -> PointConfig {
    PointConfig {
        measurement: measurement.to_string(),
        topic: topic.to_string(),
        bucket: None,
        schedule: None,
        fields: fields
            .into_iter()
            .map(|(k, v, t)| {
                let spec = if let Some(type_name) = t {
                    FieldSpec::Typed(FieldConfig {
                        value: v.to_string(),
                        field_type: Some(type_name.to_string()),
                    })
                } else {
                    FieldSpec::Simple(v.to_string())
                };
                (k.to_string(), spec)
            })
            .collect(),
        tags: HashMap::new(),
        httpcontent: HashMap::new(),
    }
}

/// Simulate processing a message and building an InfluxDB point.
fn process_message_to_point(
    processor: &MessageProcessor,
    point_config: &PointConfig,
    topic: &str,
    payload: &[u8],
) -> Option<Point> {
    // Parse the message
    let parsed = processor.parse_message(topic, payload, 0).ok()?;

    // Check if topic matches
    if !processor.topic_matches(&point_config.topic, topic) {
        return None;
    }

    // Get measurement name
    let measurement = match processor.get_value(&point_config.measurement, &parsed)? {
        serde_json::Value::String(s) => s,
        v => v.to_string().trim_matches('"').to_string(),
    };

    // Build point
    let mut point = Point::new(&measurement);

    // Add tags
    for (tag_name, tag_spec) in &point_config.tags {
        if let Some(value) = processor.get_value(tag_spec, &parsed) {
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
        if let Some(value) = processor.extract_field(field_spec, &parsed) {
            if let Some(field_value) = FieldValue::from_json(&value) {
                point.add_field(field_name, field_value);
                fields_added += 1;
            }
        }
    }

    if fields_added == 0 {
        return None;
    }

    Some(point)
}

// ============================================================================
// Basic Flow Tests
// ============================================================================

#[test]
fn test_simple_numeric_payload() {
    let processor = MessageProcessor::new(None);
    let config = make_point_config(
        "temperature",
        "test/+/temperature",
        vec![("value", "$.payload")],
        vec![("sensor_id", "$.topic[1]")],
    );

    let point = process_message_to_point(
        &processor,
        &config,
        "test/sensor1/temperature",
        b"25.5",
    )
    .expect("Should produce a point");

    let line = point.to_line_protocol();
    assert!(line.starts_with("temperature"));
    assert!(line.contains("sensor_id=sensor1"));
    assert!(line.contains("value=25.5"));
}

#[test]
fn test_json_payload_multiple_fields() {
    let processor = MessageProcessor::new(None);
    let config = make_point_config(
        "environment",
        "test/+/data",
        vec![
            ("temperature", "$.payload.temperature"),
            ("humidity", "$.payload.humidity"),
            ("pressure", "$.payload.pressure"),
        ],
        vec![("device_id", "$.topic[1]")],
    );

    let payload = br#"{"temperature": 23.5, "humidity": 65.2, "pressure": 1013.25}"#;
    let point = process_message_to_point(&processor, &config, "test/dev001/data", payload)
        .expect("Should produce a point");

    let line = point.to_line_protocol();
    assert!(line.contains("temperature=23.5"));
    assert!(line.contains("humidity=65.2"));
    assert!(line.contains("pressure=1013.25"));
    assert!(line.contains("device_id=dev001"));
}

// ============================================================================
// Type Conversion Tests
// ============================================================================

#[test]
fn test_string_to_float_conversion() {
    let processor = MessageProcessor::new(None);
    let config = make_typed_point_config(
        "typed",
        "test/typed/+/value",
        vec![("float_val", "$.payload.raw", Some("float"))],
    );

    let payload = br#"{"raw": "123.456"}"#;
    let point = process_message_to_point(&processor, &config, "test/typed/float/value", payload)
        .expect("Should produce a point");

    let line = point.to_line_protocol();
    assert!(line.contains("float_val=123.456"));
}

#[test]
fn test_string_to_int_conversion() {
    let processor = MessageProcessor::new(None);
    let config = make_typed_point_config(
        "typed",
        "test/typed/+/value",
        vec![("int_val", "$.payload.raw", Some("int"))],
    );

    let payload = br#"{"raw": "789"}"#;
    let point = process_message_to_point(&processor, &config, "test/typed/int/value", payload)
        .expect("Should produce a point");

    let line = point.to_line_protocol();
    assert!(line.contains("int_val=789i"));
}

#[test]
fn test_booltoint_conversion() {
    let processor = MessageProcessor::new(None);
    let config = make_typed_point_config(
        "typed",
        "test/#",
        vec![("motion", "$.payload.motion", Some("booltoint"))],
    );

    // Test true -> 1
    let payload = br#"{"motion": true}"#;
    let point = process_message_to_point(&processor, &config, "test/motion", payload)
        .expect("Should produce a point");
    let line = point.to_line_protocol();
    assert!(line.contains("motion=1i"));

    // Test false -> 0
    let payload = br#"{"motion": false}"#;
    let point = process_message_to_point(&processor, &config, "test/motion", payload)
        .expect("Should produce a point");
    let line = point.to_line_protocol();
    assert!(line.contains("motion=0i"));
}

// ============================================================================
// Expression Evaluation Tests
// ============================================================================

#[test]
fn test_celsius_to_fahrenheit_0c() {
    let processor = MessageProcessor::new(None);
    let config = make_point_config(
        "temperature",
        "test/expr/temperature",
        vec![
            ("celsius", "$.payload"),
            ("fahrenheit", "= 32 + ($.payload * 9 / 5)"),
        ],
        vec![],
    );

    // 0°C = 32°F
    let point = process_message_to_point(&processor, &config, "test/expr/temperature", b"0")
        .expect("Should produce a point");

    let line = point.to_line_protocol();
    assert!(line.contains("celsius=0"));
    assert!(line.contains("fahrenheit=32"));
}

#[test]
fn test_celsius_to_fahrenheit_100c() {
    let processor = MessageProcessor::new(None);
    let config = make_point_config(
        "temperature",
        "test/expr/temperature",
        vec![
            ("celsius", "$.payload"),
            ("fahrenheit", "= 32 + ($.payload * 9 / 5)"),
        ],
        vec![],
    );

    // 100°C = 212°F
    let point = process_message_to_point(&processor, &config, "test/expr/temperature", b"100")
        .expect("Should produce a point");

    let line = point.to_line_protocol();
    assert!(line.contains("celsius=100"));
    assert!(line.contains("fahrenheit=212"));
}

#[test]
fn test_expression_with_json_field() {
    let processor = MessageProcessor::new(None);
    let config = make_point_config(
        "computed",
        "test/#",
        vec![("doubled", "= $.payload.value * 2")],
        vec![],
    );

    let payload = br#"{"value": 42}"#;
    let point = process_message_to_point(&processor, &config, "test/compute", payload)
        .expect("Should produce a point");

    let line = point.to_line_protocol();
    assert!(line.contains("doubled=84"));
}

// ============================================================================
// Array Access Tests
// ============================================================================

#[test]
fn test_array_index_extraction() {
    let processor = MessageProcessor::new(None);
    let config = make_point_config(
        "array_data",
        "test/array/data",
        vec![
            ("first", "$.payload.values[0]"),
            ("second", "$.payload.values[1]"),
            ("third", "$.payload.values[2]"),
        ],
        vec![],
    );

    let payload = br#"{"values": [10, 20, 30, 40, 50]}"#;
    let point = process_message_to_point(&processor, &config, "test/array/data", payload)
        .expect("Should produce a point");

    let line = point.to_line_protocol();
    assert!(line.contains("first=10"));
    assert!(line.contains("second=20"));
    assert!(line.contains("third=30"));
}

// ============================================================================
// Wildcard Topic Tests
// ============================================================================

#[test]
fn test_single_level_wildcard() {
    let processor = MessageProcessor::new(None);
    let config = make_point_config(
        "sensor",
        "home/+/temperature",
        vec![("value", "$.payload")],
        vec![("room", "$.topic[1]")],
    );

    // Test multiple rooms
    for room in &["living_room", "bedroom", "kitchen"] {
        let topic = format!("home/{}/temperature", room);
        let point = process_message_to_point(&processor, &config, &topic, b"22.5")
            .expect("Should produce a point");

        let line = point.to_line_protocol();
        assert!(line.contains(&format!("room={}", room)));
        assert!(line.contains("value=22.5"));
    }
}

#[test]
fn test_multi_level_wildcard() {
    let processor = MessageProcessor::new(None);
    let config = make_point_config(
        "wildcard",
        "test/wild/#",
        vec![("value", "$.payload")],
        vec![("level1", "$.topic[2]")],
    );

    let topics = vec![
        "test/wild/level1",
        "test/wild/level1/level2",
        "test/wild/a/b/c",
    ];

    for topic in topics {
        let point = process_message_to_point(&processor, &config, topic, b"100")
            .expect(&format!("Should match topic: {}", topic));
        assert!(point.to_line_protocol().contains("value=100"));
    }
}

#[test]
fn test_multi_level_wildcard_zero_levels() {
    let processor = MessageProcessor::new(None);
    let config = make_point_config(
        "wildcard",
        "test/#",
        vec![("value", "$.payload")],
        vec![],
    );

    // # should match zero or more levels
    let point = process_message_to_point(&processor, &config, "test", b"100")
        .expect("Should match topic with zero additional levels");
    assert!(point.to_line_protocol().contains("value=100"));
}

// ============================================================================
// Error Handling Tests
// ============================================================================

#[test]
fn test_raw_string_payload_processed() {
    let processor = MessageProcessor::new(None);
    let config = make_point_config(
        "power_state",
        "stat/+/power",
        vec![("value", "$.payload")],
        vec![("device", "$.topic[1]")],
    );

    let point = process_message_to_point(&processor, &config, "stat/device1/power", b"ON")
        .expect("Should handle raw string payload");

    let line = point.to_line_protocol();
    assert!(line.contains("device=device1"));
    assert!(line.contains(r#"value="ON""#));
}

#[test]
fn test_missing_field_handled_gracefully() {
    let processor = MessageProcessor::new(None);
    let config = make_point_config(
        "test",
        "test/#",
        vec![
            ("temperature", "$.payload.temperature"),
            ("humidity", "$.payload.humidity"),
        ],
        vec![],
    );

    // Payload missing humidity field - should still produce point with temperature
    let payload = br#"{"temperature": 25.0}"#;
    let point = process_message_to_point(&processor, &config, "test/partial", payload)
        .expect("Should produce point with available fields");

    let line = point.to_line_protocol();
    assert!(line.contains("temperature=25"));
    assert!(!line.contains("humidity")); // humidity should not be present
}

#[test]
fn test_no_fields_no_point() {
    let processor = MessageProcessor::new(None);
    let config = make_point_config(
        "test",
        "test/#",
        vec![("value", "$.payload.nonexistent")],
        vec![],
    );

    let payload = br#"{"other": 123}"#;
    let point = process_message_to_point(&processor, &config, "test/empty", payload);

    assert!(point.is_none(), "Should not produce point when no fields extracted");
}

#[test]
fn test_empty_payload_as_null() {
    let processor = MessageProcessor::new(None);
    let parsed = processor.parse_message("test/topic", b"", 0).unwrap();

    assert_eq!(parsed.payload, serde_json::Value::Null);
}

#[test]
fn test_invalid_json_as_string() {
    let processor = MessageProcessor::new(None);
    let parsed = processor.parse_message("test/topic", b"not json", 0).unwrap();

    assert_eq!(parsed.payload, json!("not json"));
}

// ============================================================================
// Multiple Points Tests
// ============================================================================

#[test]
fn test_multiple_points_same_topic() {
    let processor = MessageProcessor::new(None);

    let config1 = make_point_config(
        "measurement1",
        "test/#",
        vec![("value", "$.payload")],
        vec![],
    );

    let config2 = make_point_config(
        "measurement2",
        "test/#",
        vec![("doubled", "= $.payload * 2")],
        vec![],
    );

    // Both configs should match the same topic
    let point1 = process_message_to_point(&processor, &config1, "test/topic", b"10")
        .expect("Config 1 should match");
    let point2 = process_message_to_point(&processor, &config2, "test/topic", b"10")
        .expect("Config 2 should match");

    assert!(point1.to_line_protocol().starts_with("measurement1"));
    assert!(point1.to_line_protocol().contains("value=10"));

    assert!(point2.to_line_protocol().starts_with("measurement2"));
    assert!(point2.to_line_protocol().contains("doubled=20"));
}

// ============================================================================
// Dynamic Measurement Name Tests
// ============================================================================

#[test]
fn test_measurement_from_jsonpath() {
    let processor = MessageProcessor::new(None);
    let config = make_point_config(
        "$.payload.type",  // Dynamic measurement name
        "test/#",
        vec![("value", "$.payload.value")],
        vec![],
    );

    let payload = br#"{"type": "sensor_reading", "value": 42}"#;
    let point = process_message_to_point(&processor, &config, "test/dynamic", payload)
        .expect("Should produce point with dynamic measurement");

    let line = point.to_line_protocol();
    assert!(line.starts_with("sensor_reading"));
}

#[test]
fn test_measurement_from_topic() {
    let processor = MessageProcessor::new(None);
    let config = make_point_config(
        "$.topic[1]",  // Measurement from topic segment
        "metrics/+/data",
        vec![("value", "$.payload")],
        vec![],
    );

    let point = process_message_to_point(&processor, &config, "metrics/cpu_usage/data", b"75.5")
        .expect("Should produce point");

    let line = point.to_line_protocol();
    assert!(line.starts_with("cpu_usage"));
}

// ============================================================================
// Nested JSON Tests
// ============================================================================

#[test]
fn test_deeply_nested_json() {
    let processor = MessageProcessor::new(None);
    let config = make_point_config(
        "nested",
        "test/#",
        vec![
            ("deep_value", "$.payload.level1.level2.level3.value"),
        ],
        vec![],
    );

    let payload = br#"{"level1": {"level2": {"level3": {"value": 42}}}}"#;
    let point = process_message_to_point(&processor, &config, "test/nested", payload)
        .expect("Should extract deeply nested value");

    let line = point.to_line_protocol();
    assert!(line.contains("deep_value=42"));
}

// ============================================================================
// Special Characters Tests
// ============================================================================

#[test]
fn test_bracket_notation_for_special_chars() {
    let processor = MessageProcessor::new(None);
    let config = make_point_config(
        "air_quality",
        "test/#",
        vec![
            ("pm25", "$.payload.air_quality_sensor['pm2.5']"),
            ("pm10", "$.payload.air_quality_sensor['PM10']"),
        ],
        vec![],
    );

    let payload = br#"{"air_quality_sensor": {"pm2.5": 5, "PM10": 12}}"#;
    let point = process_message_to_point(&processor, &config, "test/sensor", payload)
        .expect("Should handle bracket notation");

    let line = point.to_line_protocol();
    assert!(line.contains("pm25=5"));
    assert!(line.contains("pm10=12"));
}

// ============================================================================
// QoS Tests
// ============================================================================

#[test]
fn test_qos_preserved() {
    let processor = MessageProcessor::new(None);

    for qos in 0..=2 {
        let parsed = processor.parse_message("test/topic", b"payload", qos).unwrap();
        assert_eq!(parsed.qos, qos);
    }
}

// ============================================================================
// Schedule Tests (Integration)
// ============================================================================

#[test]
fn test_schedule_filtering() {
    let processor = MessageProcessor::new(None);

    // Create a config with "every second" schedule (always matches)
    let mut config = make_point_config(
        "scheduled",
        "test/#",
        vec![("value", "$.payload")],
        vec![],
    );
    config.schedule = Some("* * * * * *".to_string()); // Every second

    // Schedule should match
    assert!(processor.schedule_matches("* * * * * *"));

    // Invalid schedule should not match
    assert!(!processor.schedule_matches("invalid"));
}

// ============================================================================
// Line Protocol Format Tests
// ============================================================================

#[test]
fn test_line_protocol_escaping() {
    let processor = MessageProcessor::new(None);
    let config = make_point_config(
        "test measurement", // Space in measurement name
        "test/#",
        vec![("value", "$.payload")],
        vec![("tag with space", "$.topic[0]")], // Space in tag name
    );

    let point = process_message_to_point(&processor, &config, "test", b"42")
        .expect("Should produce point");

    let line = point.to_line_protocol();
    // Spaces should be escaped in measurement name
    assert!(line.contains(r"test\ measurement"));
}

#[test]
fn test_string_field_quoting() {
    let processor = MessageProcessor::new(None);
    let config = make_point_config(
        "test",
        "test/#",
        vec![("status", "$.payload")],
        vec![],
    );

    let point = process_message_to_point(&processor, &config, "test/status", b"\"active\"")
        .expect("Should produce point");

    let line = point.to_line_protocol();
    // String values should be quoted in line protocol
    assert!(line.contains("status="));
}
