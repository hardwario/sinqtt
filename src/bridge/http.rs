//! HTTP forwarding for webhook support.

use crate::config::HttpConfig;
use crate::error::SinqttError;
use reqwest::Client;
use serde_json::Value;
use std::collections::HashMap;
use tracing::{debug, error, warn};

/// HTTP forwarder for sending data to webhooks.
pub struct HttpForwarder {
    client: Client,
    destination: String,
    action: HttpAction,
    username: Option<String>,
    password: Option<String>,
}

/// Supported HTTP actions.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum HttpAction {
    Post,
    Put,
    Patch,
}

impl HttpAction {
    /// Parse an HTTP action from a string.
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "post" => Some(HttpAction::Post),
            "put" => Some(HttpAction::Put),
            "patch" => Some(HttpAction::Patch),
            _ => None,
        }
    }

    /// Get the action name as a string.
    pub fn as_str(&self) -> &'static str {
        match self {
            HttpAction::Post => "POST",
            HttpAction::Put => "PUT",
            HttpAction::Patch => "PATCH",
        }
    }
}

impl Default for HttpAction {
    fn default() -> Self {
        HttpAction::Post
    }
}

impl HttpForwarder {
    /// Create a new HTTP forwarder from configuration.
    pub fn new(config: &HttpConfig) -> Self {
        let action = HttpAction::from_str(&config.action).unwrap_or_else(|| {
            warn!(
                "Unknown HTTP action '{}', defaulting to POST",
                config.action
            );
            HttpAction::Post
        });

        Self {
            client: Client::new(),
            destination: config.destination.clone(),
            action,
            username: config.username.clone(),
            password: config.password.clone(),
        }
    }

    /// Get the destination URL.
    pub fn destination(&self) -> &str {
        &self.destination
    }

    /// Get the HTTP action.
    pub fn action(&self) -> HttpAction {
        self.action
    }

    /// Check if authentication is configured.
    pub fn has_auth(&self) -> bool {
        self.username.is_some() && self.password.is_some()
    }

    /// Forward JSON data to the configured destination.
    pub async fn forward_json(&self, data: &Value) -> Result<(), SinqttError> {
        debug!(
            "Forwarding JSON to {} via {}",
            self.destination,
            self.action.as_str()
        );

        let request = self.build_request();
        let response = request
            .json(data)
            .send()
            .await?;

        self.handle_response(response).await
    }

    /// Forward form data to the configured destination.
    pub async fn forward_form(&self, data: &HashMap<String, String>) -> Result<(), SinqttError> {
        debug!(
            "Forwarding form data to {} via {}",
            self.destination,
            self.action.as_str()
        );

        let request = self.build_request();
        let response = request
            .form(data)
            .send()
            .await?;

        self.handle_response(response).await
    }

    /// Forward data with a custom content type.
    pub async fn forward_raw(&self, data: String, content_type: &str) -> Result<(), SinqttError> {
        debug!(
            "Forwarding raw data to {} via {} ({})",
            self.destination,
            self.action.as_str(),
            content_type
        );

        let request = self.build_request();
        let response = request
            .header("Content-Type", content_type)
            .body(data)
            .send()
            .await?;

        self.handle_response(response).await
    }

    /// Build the base request with method and auth.
    fn build_request(&self) -> reqwest::RequestBuilder {
        let mut request = match self.action {
            HttpAction::Post => self.client.post(&self.destination),
            HttpAction::Put => self.client.put(&self.destination),
            HttpAction::Patch => self.client.patch(&self.destination),
        };

        // Add basic auth if configured
        if let (Some(username), Some(password)) = (&self.username, &self.password) {
            request = request.basic_auth(username, Some(password));
        }

        request
    }

    /// Handle the HTTP response.
    async fn handle_response(&self, response: reqwest::Response) -> Result<(), SinqttError> {
        let status = response.status();

        if status.is_success() {
            debug!("HTTP forward successful: {}", status);
            Ok(())
        } else {
            let body = response.text().await.unwrap_or_default();
            error!(
                "HTTP forward failed: {} - {} - {}",
                self.destination, status, body
            );
            Err(SinqttError::HttpForward(format!(
                "HTTP {} to {} failed: {} - {}",
                self.action.as_str(),
                self.destination,
                status,
                body
            )))
        }
    }
}

/// Builder for extracting and formatting HTTP content from messages.
pub struct HttpContentBuilder {
    content: HashMap<String, String>,
}

impl HttpContentBuilder {
    /// Create a new HTTP content builder.
    pub fn new() -> Self {
        Self {
            content: HashMap::new(),
        }
    }

    /// Add a field to the content.
    pub fn add(&mut self, key: impl Into<String>, value: impl Into<String>) -> &mut Self {
        self.content.insert(key.into(), value.into());
        self
    }

    /// Add a field from a JSON value.
    pub fn add_from_json(&mut self, key: impl Into<String>, value: &Value) -> &mut Self {
        let string_value = match value {
            Value::String(s) => s.clone(),
            Value::Number(n) => n.to_string(),
            Value::Bool(b) => b.to_string(),
            Value::Null => String::new(),
            _ => value.to_string(),
        };
        self.content.insert(key.into(), string_value);
        self
    }

    /// Check if the content is empty.
    pub fn is_empty(&self) -> bool {
        self.content.is_empty()
    }

    /// Build the content as a HashMap.
    pub fn build(self) -> HashMap<String, String> {
        self.content
    }

    /// Build the content as a JSON Value.
    pub fn build_json(self) -> Value {
        let map: serde_json::Map<String, Value> = self
            .content
            .into_iter()
            .map(|(k, v)| (k, Value::String(v)))
            .collect();
        Value::Object(map)
    }
}

impl Default for HttpContentBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_config(action: &str) -> HttpConfig {
        HttpConfig {
            destination: "http://example.com/webhook".to_string(),
            action: action.to_string(),
            username: None,
            password: None,
        }
    }

    fn make_config_with_auth(action: &str, username: &str, password: &str) -> HttpConfig {
        HttpConfig {
            destination: "http://example.com/webhook".to_string(),
            action: action.to_string(),
            username: Some(username.to_string()),
            password: Some(password.to_string()),
        }
    }

    #[test]
    fn test_http_forwarder_creation_post() {
        let config = make_config("post");
        let forwarder = HttpForwarder::new(&config);

        assert_eq!(forwarder.destination(), "http://example.com/webhook");
        assert_eq!(forwarder.action(), HttpAction::Post);
        assert!(!forwarder.has_auth());
    }

    #[test]
    fn test_http_forwarder_creation_put() {
        let config = make_config("put");
        let forwarder = HttpForwarder::new(&config);

        assert_eq!(forwarder.action(), HttpAction::Put);
    }

    #[test]
    fn test_http_forwarder_creation_patch() {
        let config = make_config("patch");
        let forwarder = HttpForwarder::new(&config);

        assert_eq!(forwarder.action(), HttpAction::Patch);
    }

    #[test]
    fn test_http_forwarder_case_insensitive() {
        let config = make_config("POST");
        let forwarder = HttpForwarder::new(&config);
        assert_eq!(forwarder.action(), HttpAction::Post);

        let config = make_config("Put");
        let forwarder = HttpForwarder::new(&config);
        assert_eq!(forwarder.action(), HttpAction::Put);

        let config = make_config("PATCH");
        let forwarder = HttpForwarder::new(&config);
        assert_eq!(forwarder.action(), HttpAction::Patch);
    }

    #[test]
    fn test_http_forwarder_unknown_action_defaults_to_post() {
        let config = make_config("unknown");
        let forwarder = HttpForwarder::new(&config);

        assert_eq!(forwarder.action(), HttpAction::Post);
    }

    #[test]
    fn test_http_forwarder_with_auth() {
        let config = make_config_with_auth("post", "user", "secret");
        let forwarder = HttpForwarder::new(&config);

        assert!(forwarder.has_auth());
    }

    #[test]
    fn test_http_action_as_str() {
        assert_eq!(HttpAction::Post.as_str(), "POST");
        assert_eq!(HttpAction::Put.as_str(), "PUT");
        assert_eq!(HttpAction::Patch.as_str(), "PATCH");
    }

    #[test]
    fn test_http_action_from_str() {
        assert_eq!(HttpAction::from_str("post"), Some(HttpAction::Post));
        assert_eq!(HttpAction::from_str("put"), Some(HttpAction::Put));
        assert_eq!(HttpAction::from_str("patch"), Some(HttpAction::Patch));
        assert_eq!(HttpAction::from_str("get"), None);
        assert_eq!(HttpAction::from_str("delete"), None);
    }

    #[test]
    fn test_http_content_builder_basic() {
        let mut builder = HttpContentBuilder::new();
        builder.add("key1", "value1");
        builder.add("key2", "value2");

        let content = builder.build();

        assert_eq!(content.get("key1"), Some(&"value1".to_string()));
        assert_eq!(content.get("key2"), Some(&"value2".to_string()));
    }

    #[test]
    fn test_http_content_builder_from_json() {
        let mut builder = HttpContentBuilder::new();
        builder.add_from_json("string", &serde_json::json!("hello"));
        builder.add_from_json("number", &serde_json::json!(42));
        builder.add_from_json("float", &serde_json::json!(3.14));
        builder.add_from_json("bool", &serde_json::json!(true));
        builder.add_from_json("null", &serde_json::json!(null));

        let content = builder.build();

        assert_eq!(content.get("string"), Some(&"hello".to_string()));
        assert_eq!(content.get("number"), Some(&"42".to_string()));
        assert_eq!(content.get("float"), Some(&"3.14".to_string()));
        assert_eq!(content.get("bool"), Some(&"true".to_string()));
        assert_eq!(content.get("null"), Some(&"".to_string()));
    }

    #[test]
    fn test_http_content_builder_is_empty() {
        let builder = HttpContentBuilder::new();
        assert!(builder.is_empty());

        let mut builder = HttpContentBuilder::new();
        builder.add("key", "value");
        assert!(!builder.is_empty());
    }

    #[test]
    fn test_http_content_builder_build_json() {
        let mut builder = HttpContentBuilder::new();
        builder.add("key1", "value1");
        builder.add("key2", "value2");

        let json = builder.build_json();

        assert!(json.is_object());
        assert_eq!(json["key1"], serde_json::json!("value1"));
        assert_eq!(json["key2"], serde_json::json!("value2"));
    }

    #[test]
    fn test_http_content_builder_chaining() {
        let mut builder = HttpContentBuilder::new();
        builder
            .add("a", "1")
            .add("b", "2")
            .add("c", "3");

        let content = builder.build();
        assert_eq!(content.len(), 3);
    }
}
