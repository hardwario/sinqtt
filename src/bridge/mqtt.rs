//! MQTT client handler.

use crate::config::MqttConfig;
use crate::error::SinqttError;
use rumqttc::{AsyncClient, Event, EventLoop, MqttOptions, Packet, QoS, Transport};
use std::time::Duration;
use tokio::sync::mpsc;
use tracing::{debug, error, info, warn};

/// Message received from MQTT broker.
#[derive(Debug, Clone)]
pub struct MqttMessage {
    pub topic: String,
    pub payload: Vec<u8>,
    pub qos: u8,
}

/// MQTT handler for connecting to broker and receiving messages.
pub struct MqttHandler {
    client: AsyncClient,
    eventloop: EventLoop,
    topics: Vec<String>,
}

impl MqttHandler {
    /// Create a new MQTT handler from configuration.
    pub fn new(config: &MqttConfig, topics: Vec<String>) -> Result<Self, SinqttError> {
        let client_id = format!("sinqtt-{}", std::process::id());
        let mut options = MqttOptions::new(client_id, &config.host, config.port);
        options.set_keep_alive(Duration::from_secs(60));
        options.set_clean_session(true);

        // Set credentials if provided
        if let (Some(username), Some(password)) = (&config.username, &config.password) {
            options.set_credentials(username, password);
        }

        // TLS configuration
        if let Some(cafile) = &config.cafile {
            let transport = Self::create_tls_transport(config)?;
            options.set_transport(transport);
            debug!("TLS enabled with CA file: {:?}", cafile);
        }

        let (client, eventloop) = AsyncClient::new(options, 100);

        Ok(Self {
            client,
            eventloop,
            topics,
        })
    }

    /// Create TLS transport configuration.
    #[cfg(feature = "tls")]
    fn create_tls_transport(config: &MqttConfig) -> Result<Transport, SinqttError> {
        use std::fs;
        use std::sync::Arc;

        let cafile = config.cafile.as_ref().ok_or_else(|| {
            SinqttError::Config(crate::error::ConfigError::Validation(
                "CA file required for TLS".to_string(),
            ))
        })?;

        let ca_cert = fs::read(cafile)?;

        let mut root_cert_store = rustls::RootCertStore::empty();
        let certs = rustls_pemfile::certs(&mut ca_cert.as_slice())
            .filter_map(|r| r.ok())
            .collect::<Vec<_>>();

        for cert in certs {
            root_cert_store.add(cert).map_err(|e| {
                SinqttError::Config(crate::error::ConfigError::Validation(format!(
                    "Invalid CA certificate: {}",
                    e
                )))
            })?;
        }

        let client_config = if let (Some(certfile), Some(keyfile)) =
            (&config.certfile, &config.keyfile)
        {
            let cert_chain = fs::read(certfile)?;
            let key = fs::read(keyfile)?;

            let certs = rustls_pemfile::certs(&mut cert_chain.as_slice())
                .filter_map(|r| r.ok())
                .collect();
            let key = rustls_pemfile::private_key(&mut key.as_slice())
                .map_err(|e| {
                    SinqttError::Config(crate::error::ConfigError::Validation(format!(
                        "Invalid key file: {}",
                        e
                    )))
                })?
                .ok_or_else(|| {
                    SinqttError::Config(crate::error::ConfigError::Validation(
                        "No private key found in key file".to_string(),
                    ))
                })?;

            rustls::ClientConfig::builder()
                .with_root_certificates(root_cert_store)
                .with_client_auth_cert(certs, key)
                .map_err(|e| {
                    SinqttError::Config(crate::error::ConfigError::Validation(format!(
                        "TLS client auth error: {}",
                        e
                    )))
                })?
        } else {
            rustls::ClientConfig::builder()
                .with_root_certificates(root_cert_store)
                .with_no_client_auth()
        };

        Ok(Transport::tls_with_config(rumqttc::TlsConfiguration::Rustls(
            Arc::new(client_config),
        )))
    }

    /// Create TLS transport (no-op when TLS feature is disabled).
    #[cfg(not(feature = "tls"))]
    fn create_tls_transport(_config: &MqttConfig) -> Result<Transport, SinqttError> {
        Err(SinqttError::Config(crate::error::ConfigError::Validation(
            "TLS support not compiled in. Enable the 'tls' feature.".to_string(),
        )))
    }

    /// Get a reference to the MQTT client.
    pub fn client(&self) -> &AsyncClient {
        &self.client
    }

    /// Subscribe to configured topics.
    async fn subscribe_topics(&self) -> Result<(), SinqttError> {
        for topic in &self.topics {
            info!("Subscribing to topic: {}", topic);
            self.client.subscribe(topic, QoS::AtLeastOnce).await?;
        }
        Ok(())
    }

    /// Run the MQTT event loop and send messages to the provided channel.
    ///
    /// This method handles:
    /// - Connection acknowledgment and topic subscription
    /// - Disconnection events with logging
    /// - Incoming messages routed to the channel
    /// - Automatic reconnection (handled by rumqttc)
    pub async fn run(
        mut self,
        tx: mpsc::Sender<MqttMessage>,
    ) -> Result<(), SinqttError> {
        info!(
            "Starting MQTT event loop, {} topics configured",
            self.topics.len()
        );

        loop {
            match self.eventloop.poll().await {
                Ok(Event::Incoming(Packet::ConnAck(connack))) => {
                    if connack.code == rumqttc::ConnectReturnCode::Success {
                        info!("Connected to MQTT broker");
                        if let Err(e) = self.subscribe_topics().await {
                            error!("Failed to subscribe to topics: {}", e);
                        }
                    } else {
                        error!("MQTT connection failed: {:?}", connack.code);
                    }
                }
                Ok(Event::Incoming(Packet::SubAck(suback))) => {
                    debug!("Subscription acknowledged: {:?}", suback);
                }
                Ok(Event::Incoming(Packet::Publish(publish))) => {
                    debug!(
                        "Received message on topic: {} ({} bytes)",
                        publish.topic,
                        publish.payload.len()
                    );

                    let msg = MqttMessage {
                        topic: publish.topic,
                        payload: publish.payload.to_vec(),
                        qos: publish.qos as u8,
                    };

                    if tx.send(msg).await.is_err() {
                        warn!("Message receiver dropped, stopping MQTT handler");
                        break;
                    }
                }
                Ok(Event::Incoming(Packet::PingResp)) => {
                    debug!("Ping response received");
                }
                Ok(Event::Incoming(Packet::Disconnect)) => {
                    warn!("Disconnected from MQTT broker");
                }
                Ok(Event::Outgoing(_)) => {
                    // Outgoing events are internal, ignore
                }
                Ok(event) => {
                    debug!("MQTT event: {:?}", event);
                }
                Err(e) => {
                    error!("MQTT connection error: {}", e);
                    // rumqttc will automatically try to reconnect
                    tokio::time::sleep(Duration::from_secs(1)).await;
                }
            }
        }

        Ok(())
    }

    /// Disconnect from the MQTT broker.
    pub async fn disconnect(&self) -> Result<(), SinqttError> {
        info!("Disconnecting from MQTT broker");
        self.client.disconnect().await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn make_config(host: &str, port: u16) -> MqttConfig {
        MqttConfig {
            host: host.to_string(),
            port,
            username: None,
            password: None,
            cafile: None,
            certfile: None,
            keyfile: None,
        }
    }

    #[test]
    fn test_mqtt_handler_creation() {
        let config = make_config("localhost", 1883);
        let topics = vec!["test/#".to_string()];
        let handler = MqttHandler::new(&config, topics);
        assert!(handler.is_ok());
    }

    #[test]
    fn test_mqtt_handler_with_auth() {
        let config = MqttConfig {
            host: "localhost".to_string(),
            port: 1883,
            username: Some("user".to_string()),
            password: Some("pass".to_string()),
            cafile: None,
            certfile: None,
            keyfile: None,
        };
        let topics = vec!["test/+/temp".to_string()];
        let handler = MqttHandler::new(&config, topics);
        assert!(handler.is_ok());
    }

    #[test]
    fn test_mqtt_message_struct() {
        let msg = MqttMessage {
            topic: "test/sensor/temp".to_string(),
            payload: b"25.5".to_vec(),
            qos: 1,
        };

        assert_eq!(msg.topic, "test/sensor/temp");
        assert_eq!(msg.payload, b"25.5");
        assert_eq!(msg.qos, 1);
    }

    #[test]
    fn test_mqtt_handler_multiple_topics() {
        let config = make_config("localhost", 1883);
        let topics = vec![
            "node/+/temperature".to_string(),
            "node/+/humidity".to_string(),
            "stat/#".to_string(),
        ];
        let handler = MqttHandler::new(&config, topics.clone()).unwrap();
        assert_eq!(handler.topics.len(), 3);
    }

    #[cfg(not(feature = "tls"))]
    #[test]
    fn test_tls_without_feature_returns_error() {
        let config = MqttConfig {
            host: "localhost".to_string(),
            port: 8883,
            username: None,
            password: None,
            cafile: Some(PathBuf::from("/path/to/ca.crt")),
            certfile: None,
            keyfile: None,
        };
        let topics = vec!["test/#".to_string()];
        let result = MqttHandler::new(&config, topics);
        assert!(result.is_err());
    }
}
