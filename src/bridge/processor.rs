//! Message processing pipeline.

use crate::config::{Base64DecodeConfig, FieldSpec};
use crate::error::SinqttError;
use crate::expr::{evaluate_expression, jsonpath_to_variable, parse_expression};
use base64::Engine;
use jsonpath_rust::JsonPath;
use serde_json::{Value, json};
use std::collections::HashMap;

/// Parsed MQTT message ready for processing.
#[derive(Debug, Clone)]
pub struct ParsedMessage {
    pub topic: Vec<String>,
    pub payload: Value,
    pub timestamp: Option<i64>,
    pub qos: u8,
    pub base64decoded: Option<HashMap<String, Base64Decoded>>,
}

/// Base64 decoded data.
#[derive(Debug, Clone)]
pub struct Base64Decoded {
    pub raw: Vec<u8>,
    pub hex: String,
}

/// Message processor for transforming MQTT messages.
pub struct MessageProcessor {
    base64_config: Option<Base64DecodeConfig>,
}

impl MessageProcessor {
    /// Create a new message processor.
    pub fn new(base64_config: Option<Base64DecodeConfig>) -> Self {
        Self { base64_config }
    }

    /// Parse an MQTT message into a structured format.
    pub fn parse_message(
        &self,
        topic: &str,
        payload: &[u8],
        qos: u8,
    ) -> Result<ParsedMessage, SinqttError> {
        let topic_parts: Vec<String> = topic.split('/').map(String::from).collect();

        // Try UTF-8 decode
        let payload_str = String::from_utf8_lossy(payload);

        // Try JSON parse, fallback to raw string
        let payload_value = if payload_str.is_empty() {
            Value::Null
        } else {
            serde_json::from_str(&payload_str)
                .unwrap_or_else(|_| Value::String(payload_str.to_string()))
        };

        let mut msg = ParsedMessage {
            topic: topic_parts,
            payload: payload_value,
            timestamp: None,
            qos,
            base64decoded: None,
        };

        // Handle base64 decoding if configured
        if let Some(config) = &self.base64_config
            && let Some(decoded) = self.decode_base64(&msg, config)
        {
            let mut map = HashMap::new();
            map.insert(config.target.clone(), decoded);
            msg.base64decoded = Some(map);
        }

        Ok(msg)
    }

    /// Decode base64 content from message.
    fn decode_base64(
        &self,
        msg: &ParsedMessage,
        config: &Base64DecodeConfig,
    ) -> Option<Base64Decoded> {
        let msg_value = self.build_message_object(msg);
        let value = self.extract_jsonpath(&config.source, &msg_value)?;

        let encoded = value.as_str()?;
        let raw = base64::engine::general_purpose::STANDARD
            .decode(encoded)
            .ok()?;
        let hex = hex::encode(&raw);

        Some(Base64Decoded { raw, hex })
    }

    /// Build a JSON object representing the message for JSONPath queries.
    pub fn build_message_object(&self, msg: &ParsedMessage) -> Value {
        let mut obj = json!({
            "topic": msg.topic,
            "payload": msg.payload,
            "timestamp": msg.timestamp,
            "qos": msg.qos,
        });

        if let Some(decoded) = &msg.base64decoded {
            let mut decoded_obj = json!({});
            for (key, value) in decoded {
                decoded_obj[key] = json!({
                    "raw": value.raw,
                    "hex": value.hex,
                });
            }
            obj["base64decoded"] = decoded_obj;
        }

        obj
    }

    /// Extract a value using the given specification.
    ///
    /// Supports three modes:
    /// - Expression mode (starts with `=`): Evaluate mathematical expression
    /// - JSONPath mode (contains `$.`): Extract using JSONPath
    /// - Literal mode: Return the spec string as-is
    pub fn get_value(&self, spec: &str, msg: &ParsedMessage) -> Option<Value> {
        if spec.is_empty() {
            return None;
        }

        let msg_value = self.build_message_object(msg);

        // Expression mode
        if spec.starts_with('=') {
            return self.evaluate_expression_spec(spec, &msg_value);
        }

        // JSONPath mode
        if spec.contains("$.") {
            return self.extract_jsonpath(spec, &msg_value);
        }

        // Literal mode
        Some(Value::String(spec.to_string()))
    }

    /// Evaluate an expression specification.
    fn evaluate_expression_spec(&self, spec: &str, msg_value: &Value) -> Option<Value> {
        let expr = spec.trim_start_matches('=').trim();
        let (_, jsonpaths) = parse_expression(expr);

        // Extract variables from message
        let mut variables = HashMap::new();
        for path in jsonpaths {
            if let Some(value) = self.extract_jsonpath(&path, msg_value) {
                let var_name = jsonpath_to_variable(&path);
                if let Some(num) = value.as_f64() {
                    variables.insert(var_name, num);
                } else if let Some(num) = value.as_i64() {
                    variables.insert(var_name, num as f64);
                }
            }
        }

        // Evaluate expression
        evaluate_expression(spec, &variables).ok().map(Value::from)
    }

    /// Extract a value using JSONPath.
    fn extract_jsonpath(&self, path: &str, value: &Value) -> Option<Value> {
        let results = value.query(path).ok()?;

        // jsonpath-rust returns a vector of references
        results.into_iter().next().cloned()
    }

    /// Convert a value to the specified type.
    pub fn convert_type(&self, value: &Value, type_name: &str) -> Option<Value> {
        match type_name {
            "float" => {
                if let Some(f) = value.as_f64() {
                    Some(Value::from(f))
                } else if let Some(s) = value.as_str() {
                    s.parse::<f64>().ok().map(Value::from)
                } else {
                    value.as_i64().map(|i| Value::from(i as f64))
                }
            }
            "int" => {
                if let Some(i) = value.as_i64() {
                    Some(Value::from(i))
                } else if let Some(f) = value.as_f64() {
                    Some(Value::from(f as i64))
                } else if let Some(s) = value.as_str() {
                    s.parse::<i64>().ok().map(Value::from)
                } else {
                    None
                }
            }
            "str" | "string" => Some(Value::String(match value {
                Value::String(s) => s.clone(),
                other => other.to_string(),
            })),
            "bool" => {
                if let Some(b) = value.as_bool() {
                    Some(Value::Bool(b))
                } else if let Some(s) = value.as_str() {
                    match s.to_lowercase().as_str() {
                        "true" | "1" | "yes" | "on" => Some(Value::Bool(true)),
                        "false" | "0" | "no" | "off" => Some(Value::Bool(false)),
                        _ => None,
                    }
                } else {
                    value.as_i64().map(|i| Value::Bool(i != 0))
                }
            }
            "booltoint" => {
                let bool_val = if let Some(b) = value.as_bool() {
                    Some(b)
                } else if let Some(s) = value.as_str() {
                    match s.to_lowercase().as_str() {
                        "true" | "1" | "yes" | "on" => Some(true),
                        "false" | "0" | "no" | "off" => Some(false),
                        _ => None,
                    }
                } else {
                    None
                };
                bool_val.map(|b| Value::from(if b { 1i64 } else { 0i64 }))
            }
            _ => None,
        }
    }

    /// Extract field value according to field specification.
    pub fn extract_field(&self, spec: &FieldSpec, msg: &ParsedMessage) -> Option<Value> {
        match spec {
            FieldSpec::Simple(s) => self.get_value(s, msg),
            FieldSpec::Typed(config) => {
                let value = self.get_value(&config.value, msg)?;
                if let Some(type_name) = &config.field_type {
                    self.convert_type(&value, type_name)
                } else {
                    Some(value)
                }
            }
        }
    }

    /// Check if a topic matches a subscription pattern.
    pub fn topic_matches(&self, pattern: &str, topic: &str) -> bool {
        let pattern_parts: Vec<&str> = pattern.split('/').collect();
        let topic_parts: Vec<&str> = topic.split('/').collect();

        let mut p_idx = 0;
        let mut t_idx = 0;

        while p_idx < pattern_parts.len() && t_idx < topic_parts.len() {
            match pattern_parts[p_idx] {
                "#" => return true, // Multi-level wildcard matches everything remaining
                "+" => {
                    // Single-level wildcard matches one level
                    p_idx += 1;
                    t_idx += 1;
                }
                part => {
                    if part != topic_parts[t_idx] {
                        return false;
                    }
                    p_idx += 1;
                    t_idx += 1;
                }
            }
        }

        // Check if we've matched everything
        // Special case: if pattern ends with # and we've consumed all topic parts, it's a match
        if p_idx < pattern_parts.len() && pattern_parts[p_idx] == "#" {
            return true;
        }

        p_idx == pattern_parts.len() && t_idx == topic_parts.len()
    }

    /// Check if a cron schedule matches the current time.
    ///
    /// This implements the same behavior as Python's pycron.is_now():
    /// Returns true if the current minute matches the cron expression.
    pub fn schedule_matches(&self, schedule: &str) -> bool {
        self.schedule_matches_at(schedule, chrono::Utc::now())
    }

    /// Check if a cron schedule matches the given time.
    ///
    /// This is useful for testing with specific times.
    pub fn schedule_matches_at<Tz: chrono::TimeZone>(
        &self,
        schedule: &str,
        time: chrono::DateTime<Tz>,
    ) -> bool {
        use chrono::{Duration, Timelike};
        use cron::Schedule;
        use std::str::FromStr;

        // Normalize 5-field cron (standard) to 6-field (with seconds)
        let parts: Vec<&str> = schedule.split_whitespace().collect();
        let normalized = if parts.len() == 5 {
            format!("0 {}", schedule)
        } else {
            schedule.to_string()
        };

        let Ok(cron_schedule) = Schedule::from_str(&normalized) else {
            return false;
        };

        // Get start of current minute (truncate seconds and nanoseconds)
        let current_minute_start = time
            .with_second(0)
            .and_then(|t| t.with_nanosecond(0))
            .unwrap_or_else(|| time.clone());

        // Look for scheduled times starting just before the current minute
        let check_from = current_minute_start.clone() - Duration::seconds(1);

        // The next scheduled time after check_from should be within current minute if schedule matches
        if let Some(next_scheduled) = cron_schedule.after(&check_from).next() {
            // Check if the next scheduled time is within the current minute
            let next_minute_start = next_scheduled
                .with_second(0)
                .and_then(|t| t.with_nanosecond(0));
            next_minute_start == Some(current_minute_start)
        } else {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_message_json() {
        let processor = MessageProcessor::new(None);
        let msg = processor
            .parse_message("test/topic", b"{\"value\": 42}", 0)
            .unwrap();

        assert_eq!(msg.topic, vec!["test", "topic"]);
        assert_eq!(msg.payload["value"], 42);
    }

    #[test]
    fn test_parse_message_raw_string() {
        let processor = MessageProcessor::new(None);
        let msg = processor.parse_message("test/topic", b"hello", 0).unwrap();

        assert_eq!(msg.payload, Value::String("hello".to_string()));
    }

    #[test]
    fn test_topic_matches() {
        let processor = MessageProcessor::new(None);

        assert!(processor.topic_matches("test/+/temp", "test/sensor1/temp"));
        assert!(processor.topic_matches("test/#", "test/sensor1/temp"));
        assert!(processor.topic_matches("test/sensor1/temp", "test/sensor1/temp"));
        assert!(!processor.topic_matches("test/sensor1/temp", "test/sensor2/temp"));
    }

    #[test]
    fn test_get_value_literal() {
        let processor = MessageProcessor::new(None);
        let msg = processor.parse_message("test/topic", b"42", 0).unwrap();

        let value = processor.get_value("literal_value", &msg).unwrap();
        assert_eq!(value, Value::String("literal_value".to_string()));
    }

    #[test]
    fn test_get_value_jsonpath() {
        let processor = MessageProcessor::new(None);
        let msg = processor
            .parse_message("test/sensor1/temp", b"{\"temperature\": 23.5}", 0)
            .unwrap();

        let value = processor.get_value("$.payload.temperature", &msg).unwrap();
        assert_eq!(value, json!(23.5));
    }

    #[test]
    fn test_convert_type_float() {
        let processor = MessageProcessor::new(None);

        let result = processor.convert_type(&json!("42.5"), "float").unwrap();
        assert_eq!(result, json!(42.5));
    }

    #[test]
    fn test_convert_type_bool() {
        let processor = MessageProcessor::new(None);

        let result = processor.convert_type(&json!("true"), "bool").unwrap();
        assert_eq!(result, json!(true));

        let result = processor.convert_type(&json!("false"), "bool").unwrap();
        assert_eq!(result, json!(false));
    }
}
