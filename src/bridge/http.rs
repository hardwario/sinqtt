//! HTTP forwarding for webhook support.

use crate::config::HttpConfig;
use crate::error::SinqttError;
use reqwest::Client;
use serde_json::Value;

/// HTTP forwarder for sending data to webhooks.
pub struct HttpForwarder {
    client: Client,
    destination: String,
    action: String,
    username: Option<String>,
    password: Option<String>,
}

impl HttpForwarder {
    /// Create a new HTTP forwarder from configuration.
    pub fn new(config: &HttpConfig) -> Self {
        Self {
            client: Client::new(),
            destination: config.destination.clone(),
            action: config.action.clone(),
            username: config.username.clone(),
            password: config.password.clone(),
        }
    }

    /// Forward data to the configured destination.
    pub async fn forward(&self, data: Value) -> Result<(), SinqttError> {
        let mut request = match self.action.to_lowercase().as_str() {
            "post" => self.client.post(&self.destination),
            "put" => self.client.put(&self.destination),
            "patch" => self.client.patch(&self.destination),
            _ => self.client.post(&self.destination),
        };

        // Add basic auth if configured
        if let (Some(username), Some(password)) = (&self.username, &self.password) {
            request = request.basic_auth(username, Some(password));
        }

        request
            .json(&data)
            .send()
            .await?
            .error_for_status()
            .map_err(|e| SinqttError::Http(e))?;

        Ok(())
    }
}
