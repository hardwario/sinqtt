//! Error types for sinqtt.

use thiserror::Error;

/// Top-level error type for sinqtt.
#[derive(Error, Debug)]
pub enum SinqttError {
    #[error("Configuration error: {0}")]
    Config(#[from] ConfigError),

    #[error("MQTT error: {0}")]
    Mqtt(#[from] rumqttc::ClientError),

    #[error("Connection error: {0}")]
    Connection(Box<rumqttc::ConnectionError>),

    #[error("InfluxDB error: {0}")]
    InfluxDb(String),

    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),

    #[error("HTTP forward error: {0}")]
    HttpForward(String),

    #[error("Expression error: {0}")]
    Expression(#[from] ExpressionError),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

impl From<rumqttc::ConnectionError> for SinqttError {
    fn from(err: rumqttc::ConnectionError) -> Self {
        SinqttError::Connection(Box::new(err))
    }
}

/// Configuration parsing and validation errors.
#[derive(Error, Debug)]
pub enum ConfigError {
    #[error("YAML parse error: {0}")]
    YamlParse(#[from] serde_yaml::Error),

    #[error("Environment variable '{0}' is not set")]
    MissingEnvVar(String),

    #[error("Invalid cron format: {0}")]
    InvalidCron(String),

    #[error("Invalid JSONPath: {0}")]
    InvalidJsonPath(String),

    #[error("Validation error: {0}")]
    Validation(String),

    #[error("File not found: {0}")]
    FileNotFound(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

/// Expression parsing and evaluation errors.
#[derive(Error, Debug)]
pub enum ExpressionError {
    #[error("Invalid expression: {0}")]
    Parse(String),

    #[error("Evaluation error: {0}")]
    Evaluation(String),

    #[error("Missing variable: {0}")]
    MissingVariable(String),
}
