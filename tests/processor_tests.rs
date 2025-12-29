//! Comprehensive tests for message processing module.
//!
//! Ported from Python `test_mqtt2influxdb.py`.

use serde_json::{json, Value};
use sinqtt::bridge::MessageProcessor;
use sinqtt::config::{Base64DecodeConfig, FieldConfig, FieldSpec};

// ============================================================================
// Message Parsing Tests
// ============================================================================

#[test]
fn test_parse_simple_json_number() {
    let processor = MessageProcessor::new(None);
    let msg = processor.parse_message("test/sensor1/temperature", b"25.5", 0).unwrap();

    assert_eq!(msg.topic, vec!["test", "sensor1", "temperature"]);
    assert_eq!(msg.payload, json!(25.5));
    assert_eq!(msg.qos, 0);
}

#[test]
fn test_parse_object_json() {
    let processor = MessageProcessor::new(None);
    let msg = processor
        .parse_message("test/device/data", br#"{"temperature": 23.5, "humidity": 65.2}"#, 1)
        .unwrap();

    assert_eq!(msg.payload["temperature"], json!(23.5));
    assert_eq!(msg.payload["humidity"], json!(65.2));
}

#[test]
fn test_parse_empty_payload() {
    let processor = MessageProcessor::new(None);
    let msg = processor.parse_message("test/topic", b"", 0).unwrap();

    assert_eq!(msg.payload, Value::Null);
}

#[test]
fn test_parse_raw_string_off() {
    let processor = MessageProcessor::new(None);
    let msg = processor.parse_message("test/topic", b"OFF", 0).unwrap();

    assert_eq!(msg.payload, json!("OFF"));
}

#[test]
fn test_parse_raw_string_on() {
    let processor = MessageProcessor::new(None);
    let msg = processor.parse_message("stat/device/POWER", b"ON", 0).unwrap();

    assert_eq!(msg.payload, json!("ON"));
}

#[test]
fn test_parse_raw_text_payload() {
    let processor = MessageProcessor::new(None);
    let msg = processor.parse_message("test/status", b"Device is running", 0).unwrap();

    assert_eq!(msg.payload, json!("Device is running"));
}

#[test]
fn test_parse_malformed_json_as_string() {
    let processor = MessageProcessor::new(None);
    let msg = processor.parse_message("test/topic", b"not valid json {", 0).unwrap();

    assert_eq!(msg.payload, json!("not valid json {"));
}

#[test]
fn test_parse_array_json() {
    let processor = MessageProcessor::new(None);
    let msg = processor.parse_message("test/array", b"[1, 2, 3, 4, 5]", 0).unwrap();

    assert_eq!(msg.payload, json!([1, 2, 3, 4, 5]));
}

#[test]
fn test_parse_string_json() {
    let processor = MessageProcessor::new(None);
    let msg = processor.parse_message("test/string", br#""hello world""#, 0).unwrap();

    assert_eq!(msg.payload, json!("hello world"));
}

#[test]
fn test_parse_null_json() {
    let processor = MessageProcessor::new(None);
    let msg = processor.parse_message("test/null", b"null", 0).unwrap();

    assert_eq!(msg.payload, Value::Null);
}

#[test]
fn test_parse_boolean_json() {
    let processor = MessageProcessor::new(None);

    let msg = processor.parse_message("test/bool", b"true", 0).unwrap();
    assert_eq!(msg.payload, json!(true));

    let msg = processor.parse_message("test/bool", b"false", 0).unwrap();
    assert_eq!(msg.payload, json!(false));
}

#[test]
fn test_parse_nested_json() {
    let processor = MessageProcessor::new(None);
    let payload = br#"{"sensor": {"readings": {"temp": 23.5, "humidity": 65}}}"#;
    let msg = processor.parse_message("test/nested", payload, 0).unwrap();

    assert_eq!(msg.payload["sensor"]["readings"]["temp"], json!(23.5));
    assert_eq!(msg.payload["sensor"]["readings"]["humidity"], json!(65));
}

// ============================================================================
// Value Extraction Tests
// ============================================================================

#[test]
fn test_get_literal_value() {
    let processor = MessageProcessor::new(None);
    let msg = processor.parse_message("test", b"25.5", 0).unwrap();

    let result = processor.get_value("static_value", &msg).unwrap();
    assert_eq!(result, json!("static_value"));
}

#[test]
fn test_get_payload_value() {
    let processor = MessageProcessor::new(None);
    let msg = processor.parse_message("test", b"25.5", 0).unwrap();

    let result = processor.get_value("$.payload", &msg).unwrap();
    assert_eq!(result, json!(25.5));
}

#[test]
fn test_get_nested_value() {
    let processor = MessageProcessor::new(None);
    let msg = processor
        .parse_message("test", br#"{"temperature": 23.5, "humidity": 65}"#, 0)
        .unwrap();

    let result = processor.get_value("$.payload.temperature", &msg).unwrap();
    assert_eq!(result, json!(23.5));
}

#[test]
fn test_get_topic_segment() {
    let processor = MessageProcessor::new(None);
    let msg = processor.parse_message("node/sensor123/temperature", b"25.5", 0).unwrap();

    let result = processor.get_value("$.topic[1]", &msg).unwrap();
    assert_eq!(result, json!("sensor123"));
}

#[test]
fn test_get_topic_first_segment() {
    let processor = MessageProcessor::new(None);
    let msg = processor.parse_message("node/sensor123/temperature", b"25.5", 0).unwrap();

    let result = processor.get_value("$.topic[0]", &msg).unwrap();
    assert_eq!(result, json!("node"));
}

#[test]
fn test_get_topic_last_segment() {
    let processor = MessageProcessor::new(None);
    let msg = processor.parse_message("node/sensor123/temperature", b"25.5", 0).unwrap();

    let result = processor.get_value("$.topic[2]", &msg).unwrap();
    assert_eq!(result, json!("temperature"));
}

#[test]
fn test_get_array_element() {
    let processor = MessageProcessor::new(None);
    let msg = processor
        .parse_message("test", br#"{"values": [10, 20, 30]}"#, 0)
        .unwrap();

    let result = processor.get_value("$.payload.values[0]", &msg).unwrap();
    assert_eq!(result, json!(10));

    let result = processor.get_value("$.payload.values[2]", &msg).unwrap();
    assert_eq!(result, json!(30));
}

#[test]
fn test_get_bracket_notation_special_chars() {
    let processor = MessageProcessor::new(None);
    // Simulates Tasmota VINDRIKTNING sensor with PM2.5 field
    let msg = processor
        .parse_message(
            "tele/tasmota/device1/SENSOR",
            br#"{"VINDRIKTNING": {"PM2.5": 5, "PM10": 12}}"#,
            0,
        )
        .unwrap();

    let result = processor.get_value("$.payload.VINDRIKTNING['PM2.5']", &msg).unwrap();
    assert_eq!(result, json!(5));
}

#[test]
fn test_get_expression_celsius_to_fahrenheit_0() {
    let processor = MessageProcessor::new(None);
    let msg = processor.parse_message("test", b"0", 0).unwrap();

    // 0°C = 32°F
    let result = processor.get_value("= 32 + ($.payload * 9 / 5)", &msg).unwrap();
    assert_eq!(result, json!(32.0));
}

#[test]
fn test_get_expression_celsius_to_fahrenheit_100() {
    let processor = MessageProcessor::new(None);
    let msg = processor.parse_message("test", b"100", 0).unwrap();

    // 100°C = 212°F
    let result = processor.get_value("= 32 + ($.payload * 9 / 5)", &msg).unwrap();
    assert_eq!(result, json!(212.0));
}

#[test]
fn test_get_expression_with_nested_value() {
    let processor = MessageProcessor::new(None);
    let msg = processor
        .parse_message("test", br#"{"temperature": 37}"#, 0)
        .unwrap();

    // 37°C ≈ 98.6°F
    let result = processor.get_value("= 32 + ($.payload.temperature * 9 / 5)", &msg).unwrap();
    let value = result.as_f64().unwrap();
    assert!((value - 98.6).abs() < 0.01);
}

#[test]
fn test_get_missing_value_returns_none() {
    let processor = MessageProcessor::new(None);
    let msg = processor
        .parse_message("test", br#"{"temperature": 25}"#, 0)
        .unwrap();

    let result = processor.get_value("$.payload.nonexistent", &msg);
    assert!(result.is_none());
}

#[test]
fn test_get_empty_spec_returns_none() {
    let processor = MessageProcessor::new(None);
    let msg = processor.parse_message("test", b"25", 0).unwrap();

    let result = processor.get_value("", &msg);
    assert!(result.is_none());
}

#[test]
fn test_get_qos() {
    let processor = MessageProcessor::new(None);
    let msg = processor.parse_message("test", b"25.5", 2).unwrap();

    let result = processor.get_value("$.qos", &msg).unwrap();
    assert_eq!(result, json!(2));
}

// ============================================================================
// Type Conversion Tests
// ============================================================================

#[test]
fn test_convert_string_to_float() {
    let processor = MessageProcessor::new(None);

    let result = processor.convert_type(&json!("123.456"), "float").unwrap();
    assert_eq!(result, json!(123.456));
}

#[test]
fn test_convert_int_to_float() {
    let processor = MessageProcessor::new(None);

    let result = processor.convert_type(&json!(123), "float").unwrap();
    assert_eq!(result, json!(123.0));
}

#[test]
fn test_convert_string_int_to_float() {
    let processor = MessageProcessor::new(None);

    let result = processor.convert_type(&json!("42"), "float").unwrap();
    assert_eq!(result, json!(42.0));
}

#[test]
fn test_convert_string_to_int() {
    let processor = MessageProcessor::new(None);

    let result = processor.convert_type(&json!("789"), "int").unwrap();
    assert_eq!(result, json!(789));
}

#[test]
fn test_convert_float_to_int() {
    let processor = MessageProcessor::new(None);

    let result = processor.convert_type(&json!(42.7), "int").unwrap();
    assert_eq!(result, json!(42));
}

#[test]
fn test_convert_to_string() {
    let processor = MessageProcessor::new(None);

    let result = processor.convert_type(&json!(42), "str").unwrap();
    assert_eq!(result, json!("42"));

    let result = processor.convert_type(&json!(3.14), "str").unwrap();
    assert_eq!(result, json!("3.14"));
}

#[test]
fn test_convert_string_true_to_bool() {
    let processor = MessageProcessor::new(None);

    let result = processor.convert_type(&json!("true"), "bool").unwrap();
    assert_eq!(result, json!(true));

    let result = processor.convert_type(&json!("TRUE"), "bool").unwrap();
    assert_eq!(result, json!(true));

    let result = processor.convert_type(&json!("True"), "bool").unwrap();
    assert_eq!(result, json!(true));
}

#[test]
fn test_convert_string_false_to_bool() {
    let processor = MessageProcessor::new(None);

    let result = processor.convert_type(&json!("false"), "bool").unwrap();
    assert_eq!(result, json!(false));

    let result = processor.convert_type(&json!("FALSE"), "bool").unwrap();
    assert_eq!(result, json!(false));
}

#[test]
fn test_convert_string_yes_no_to_bool() {
    let processor = MessageProcessor::new(None);

    let result = processor.convert_type(&json!("yes"), "bool").unwrap();
    assert_eq!(result, json!(true));

    let result = processor.convert_type(&json!("no"), "bool").unwrap();
    assert_eq!(result, json!(false));
}

#[test]
fn test_convert_string_on_off_to_bool() {
    let processor = MessageProcessor::new(None);

    let result = processor.convert_type(&json!("on"), "bool").unwrap();
    assert_eq!(result, json!(true));

    let result = processor.convert_type(&json!("off"), "bool").unwrap();
    assert_eq!(result, json!(false));
}

#[test]
fn test_convert_string_1_0_to_bool() {
    let processor = MessageProcessor::new(None);

    let result = processor.convert_type(&json!("1"), "bool").unwrap();
    assert_eq!(result, json!(true));

    let result = processor.convert_type(&json!("0"), "bool").unwrap();
    assert_eq!(result, json!(false));
}

#[test]
fn test_convert_int_to_bool() {
    let processor = MessageProcessor::new(None);

    let result = processor.convert_type(&json!(1), "bool").unwrap();
    assert_eq!(result, json!(true));

    let result = processor.convert_type(&json!(0), "bool").unwrap();
    assert_eq!(result, json!(false));

    let result = processor.convert_type(&json!(42), "bool").unwrap();
    assert_eq!(result, json!(true));
}

#[test]
fn test_convert_bool_to_booltoint() {
    let processor = MessageProcessor::new(None);

    let result = processor.convert_type(&json!(true), "booltoint").unwrap();
    assert_eq!(result, json!(1));

    let result = processor.convert_type(&json!(false), "booltoint").unwrap();
    assert_eq!(result, json!(0));
}

#[test]
fn test_convert_string_to_booltoint() {
    let processor = MessageProcessor::new(None);

    let result = processor.convert_type(&json!("true"), "booltoint").unwrap();
    assert_eq!(result, json!(1));

    let result = processor.convert_type(&json!("false"), "booltoint").unwrap();
    assert_eq!(result, json!(0));

    let result = processor.convert_type(&json!("ON"), "booltoint").unwrap();
    assert_eq!(result, json!(1));

    let result = processor.convert_type(&json!("OFF"), "booltoint").unwrap();
    assert_eq!(result, json!(0));
}

#[test]
fn test_convert_invalid_string_to_int_returns_none() {
    let processor = MessageProcessor::new(None);

    let result = processor.convert_type(&json!("not a number"), "int");
    assert!(result.is_none());
}

#[test]
fn test_convert_invalid_string_to_float_returns_none() {
    let processor = MessageProcessor::new(None);

    let result = processor.convert_type(&json!("not a number"), "float");
    assert!(result.is_none());
}

#[test]
fn test_convert_unknown_type_returns_none() {
    let processor = MessageProcessor::new(None);

    let result = processor.convert_type(&json!(42), "unknown_type");
    assert!(result.is_none());
}

// ============================================================================
// Topic Matching Tests
// ============================================================================

#[test]
fn test_topic_exact_match() {
    let processor = MessageProcessor::new(None);

    assert!(processor.topic_matches("test/sensor/temp", "test/sensor/temp"));
    assert!(!processor.topic_matches("test/sensor/temp", "test/sensor/humidity"));
}

#[test]
fn test_topic_single_level_wildcard() {
    let processor = MessageProcessor::new(None);

    assert!(processor.topic_matches("test/+/temp", "test/sensor1/temp"));
    assert!(processor.topic_matches("test/+/temp", "test/sensor2/temp"));
    assert!(processor.topic_matches("test/+/temp", "test/any/temp"));
    assert!(!processor.topic_matches("test/+/temp", "test/sensor1/humidity"));
    assert!(!processor.topic_matches("test/+/temp", "other/sensor1/temp"));
}

#[test]
fn test_topic_multi_level_wildcard() {
    let processor = MessageProcessor::new(None);

    assert!(processor.topic_matches("test/#", "test/sensor1/temp"));
    assert!(processor.topic_matches("test/#", "test/sensor1/humidity"));
    assert!(processor.topic_matches("test/#", "test/a/b/c/d"));
    assert!(processor.topic_matches("test/#", "test"));
    assert!(!processor.topic_matches("test/#", "other/sensor1/temp"));
}

#[test]
fn test_topic_multiple_single_wildcards() {
    let processor = MessageProcessor::new(None);

    assert!(processor.topic_matches("node/+/sensor/+/value", "node/abc/sensor/xyz/value"));
    assert!(!processor.topic_matches("node/+/sensor/+/value", "node/abc/sensor/xyz/other"));
}

#[test]
fn test_topic_wildcard_at_start() {
    let processor = MessageProcessor::new(None);

    assert!(processor.topic_matches("+/sensor/temp", "room1/sensor/temp"));
    assert!(processor.topic_matches("+/sensor/temp", "room2/sensor/temp"));
}

#[test]
fn test_topic_no_match_different_length() {
    let processor = MessageProcessor::new(None);

    assert!(!processor.topic_matches("test/sensor", "test/sensor/temp"));
    assert!(!processor.topic_matches("test/sensor/temp", "test/sensor"));
}

// ============================================================================
// Base64 Decoding Tests
// ============================================================================

#[test]
fn test_base64_decode() {
    use base64::Engine;

    let encoded = base64::engine::general_purpose::STANDARD.encode(b"Hello, World!");
    let payload = format!(r#"{{"data": "{}"}}"#, encoded);

    let config = Base64DecodeConfig {
        source: "$.payload.data".to_string(),
        target: "decoded".to_string(),
    };
    let processor = MessageProcessor::new(Some(config));

    let msg = processor.parse_message("test", payload.as_bytes(), 0).unwrap();

    assert!(msg.base64decoded.is_some());
    let decoded = msg.base64decoded.as_ref().unwrap();
    let target = decoded.get("decoded").unwrap();

    assert_eq!(target.raw, b"Hello, World!");
    assert_eq!(target.hex, "48656c6c6f2c20576f726c6421");
}

#[test]
fn test_base64_decode_binary_data() {
    use base64::Engine;

    let binary_data: Vec<u8> = vec![0x00, 0x01, 0x02, 0xFF, 0xFE, 0xFD];
    let encoded = base64::engine::general_purpose::STANDARD.encode(&binary_data);
    let payload = format!(r#"{{"binary": "{}"}}"#, encoded);

    let config = Base64DecodeConfig {
        source: "$.payload.binary".to_string(),
        target: "decoded".to_string(),
    };
    let processor = MessageProcessor::new(Some(config));

    let msg = processor.parse_message("test", payload.as_bytes(), 0).unwrap();

    let decoded = msg.base64decoded.as_ref().unwrap();
    let target = decoded.get("decoded").unwrap();

    assert_eq!(target.raw, binary_data);
    assert_eq!(target.hex, "000102fffefd");
}

#[test]
fn test_base64_decode_missing_source_no_panic() {
    let config = Base64DecodeConfig {
        source: "$.payload.nonexistent".to_string(),
        target: "decoded".to_string(),
    };
    let processor = MessageProcessor::new(Some(config));

    let msg = processor.parse_message("test", br#"{"other": "data"}"#, 0).unwrap();

    // Should not have base64decoded when source is missing
    assert!(msg.base64decoded.is_none());
}

#[test]
fn test_base64_decode_invalid_base64_no_panic() {
    let config = Base64DecodeConfig {
        source: "$.payload.data".to_string(),
        target: "decoded".to_string(),
    };
    let processor = MessageProcessor::new(Some(config));

    // Invalid base64 string
    let msg = processor.parse_message("test", br#"{"data": "not-valid-base64!!!"}"#, 0).unwrap();

    // Should not have base64decoded when decoding fails
    assert!(msg.base64decoded.is_none());
}

// ============================================================================
// Field Spec Extraction Tests
// ============================================================================

#[test]
fn test_extract_field_simple() {
    let processor = MessageProcessor::new(None);
    let msg = processor.parse_message("test", br#"{"value": 42.5}"#, 0).unwrap();

    let spec = FieldSpec::Simple("$.payload.value".to_string());
    let result = processor.extract_field(&spec, &msg).unwrap();

    assert_eq!(result, json!(42.5));
}

#[test]
fn test_extract_field_typed_float() {
    let processor = MessageProcessor::new(None);
    let msg = processor.parse_message("test", br#"{"value": "123.45"}"#, 0).unwrap();

    let spec = FieldSpec::Typed(FieldConfig {
        value: "$.payload.value".to_string(),
        field_type: Some("float".to_string()),
    });
    let result = processor.extract_field(&spec, &msg).unwrap();

    assert_eq!(result, json!(123.45));
}

#[test]
fn test_extract_field_typed_int() {
    let processor = MessageProcessor::new(None);
    let msg = processor.parse_message("test", br#"{"value": 42.9}"#, 0).unwrap();

    let spec = FieldSpec::Typed(FieldConfig {
        value: "$.payload.value".to_string(),
        field_type: Some("int".to_string()),
    });
    let result = processor.extract_field(&spec, &msg).unwrap();

    assert_eq!(result, json!(42));
}

#[test]
fn test_extract_field_typed_booltoint() {
    let processor = MessageProcessor::new(None);
    let msg = processor.parse_message("test", br#"{"status": "ON"}"#, 0).unwrap();

    let spec = FieldSpec::Typed(FieldConfig {
        value: "$.payload.status".to_string(),
        field_type: Some("booltoint".to_string()),
    });
    let result = processor.extract_field(&spec, &msg).unwrap();

    assert_eq!(result, json!(1));
}

#[test]
fn test_extract_field_typed_no_type() {
    let processor = MessageProcessor::new(None);
    let msg = processor.parse_message("test", br#"{"value": 42}"#, 0).unwrap();

    let spec = FieldSpec::Typed(FieldConfig {
        value: "$.payload.value".to_string(),
        field_type: None,
    });
    let result = processor.extract_field(&spec, &msg).unwrap();

    assert_eq!(result, json!(42));
}

// ============================================================================
// Expression Tests
// ============================================================================

#[test]
fn test_expression_simple_addition() {
    let processor = MessageProcessor::new(None);
    let msg = processor.parse_message("test", b"10", 0).unwrap();

    let result = processor.get_value("= $.payload + 5", &msg).unwrap();
    assert_eq!(result, json!(15.0));
}

#[test]
fn test_expression_multiplication() {
    let processor = MessageProcessor::new(None);
    let msg = processor.parse_message("test", b"7", 0).unwrap();

    let result = processor.get_value("= $.payload * 3", &msg).unwrap();
    assert_eq!(result, json!(21.0));
}

#[test]
fn test_expression_division() {
    let processor = MessageProcessor::new(None);
    let msg = processor.parse_message("test", b"100", 0).unwrap();

    let result = processor.get_value("= $.payload / 4", &msg).unwrap();
    assert_eq!(result, json!(25.0));
}

#[test]
fn test_expression_power() {
    let processor = MessageProcessor::new(None);
    let msg = processor.parse_message("test", b"2", 0).unwrap();

    let result = processor.get_value("= $.payload ^ 3", &msg).unwrap();
    assert_eq!(result, json!(8.0));
}

#[test]
fn test_expression_modulo() {
    let processor = MessageProcessor::new(None);
    let msg = processor.parse_message("test", b"17", 0).unwrap();

    let result = processor.get_value("= $.payload % 5", &msg).unwrap();
    assert_eq!(result, json!(2.0));
}

#[test]
fn test_expression_complex_parentheses() {
    let processor = MessageProcessor::new(None);
    let msg = processor.parse_message("test", b"10", 0).unwrap();

    let result = processor.get_value("= ($.payload + 5) * 2", &msg).unwrap();
    assert_eq!(result, json!(30.0));
}

#[test]
fn test_expression_multiple_variables() {
    let processor = MessageProcessor::new(None);
    let msg = processor
        .parse_message("test", br#"{"a": 10, "b": 3}"#, 0)
        .unwrap();

    let result = processor.get_value("= $.payload.a + $.payload.b", &msg).unwrap();
    assert_eq!(result, json!(13.0));
}

// ============================================================================
// Build Message Object Tests
// ============================================================================

#[test]
fn test_build_message_object_contains_all_fields() {
    let processor = MessageProcessor::new(None);
    let msg = processor.parse_message("test/topic", b"42", 1).unwrap();

    let obj = processor.build_message_object(&msg);

    assert_eq!(obj["topic"], json!(["test", "topic"]));
    assert_eq!(obj["payload"], json!(42));
    assert_eq!(obj["qos"], json!(1));
    assert_eq!(obj["timestamp"], Value::Null);
}

#[test]
fn test_build_message_object_with_base64decoded() {
    use base64::Engine;

    let encoded = base64::engine::general_purpose::STANDARD.encode(b"test");
    let payload = format!(r#"{{"data": "{}"}}"#, encoded);

    let config = Base64DecodeConfig {
        source: "$.payload.data".to_string(),
        target: "decoded".to_string(),
    };
    let processor = MessageProcessor::new(Some(config));
    let msg = processor.parse_message("test", payload.as_bytes(), 0).unwrap();

    let obj = processor.build_message_object(&msg);

    assert!(obj["base64decoded"]["decoded"]["raw"].is_array());
    assert_eq!(obj["base64decoded"]["decoded"]["hex"], json!("74657374"));
}
