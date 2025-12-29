//! MQTT client handler.

use crate::config::MqttConfig;
use crate::error::SinqttError;
use rumqttc::{AsyncClient, EventLoop, MqttOptions, QoS};
use std::time::Duration;

/// MQTT handler for connecting to broker and receiving messages.
pub struct MqttHandler {
    client: AsyncClient,
    eventloop: EventLoop,
}

impl MqttHandler {
    /// Create a new MQTT handler from configuration.
    pub fn new(config: &MqttConfig) -> Result<Self, SinqttError> {
        let mut options = MqttOptions::new("sinqtt", &config.host, config.port);
        options.set_keep_alive(Duration::from_secs(60));

        // Set credentials if provided
        if let (Some(username), Some(password)) = (&config.username, &config.password) {
            options.set_credentials(username, password);
        }

        // TLS configuration would go here
        // TODO: Implement TLS support with cafile, certfile, keyfile

        let (client, eventloop) = AsyncClient::new(options, 10);

        Ok(Self { client, eventloop })
    }

    /// Get a reference to the MQTT client.
    pub fn client(&self) -> &AsyncClient {
        &self.client
    }

    /// Get a mutable reference to the event loop.
    pub fn eventloop(&mut self) -> &mut EventLoop {
        &mut self.eventloop
    }

    /// Subscribe to a topic.
    pub async fn subscribe(&self, topic: &str) -> Result<(), SinqttError> {
        self.client.subscribe(topic, QoS::AtLeastOnce).await?;
        Ok(())
    }
}
