//! Configuration validation.

use super::types::Config;
use crate::error::ConfigError;
use jsonpath_rust::parser::parse_json_path;

/// Validate the configuration.
pub fn validate_config(config: &Config) -> Result<(), ConfigError> {
    // Validate MQTT config
    if config.mqtt.host.is_empty() {
        return Err(ConfigError::Validation("MQTT host cannot be empty".into()));
    }

    // Validate InfluxDB config
    if config.influxdb.host.is_empty() {
        return Err(ConfigError::Validation(
            "InfluxDB host cannot be empty".into(),
        ));
    }
    if config.influxdb.token.is_empty() {
        return Err(ConfigError::Validation(
            "InfluxDB token cannot be empty".into(),
        ));
    }
    if config.influxdb.org.is_empty() {
        return Err(ConfigError::Validation(
            "InfluxDB org cannot be empty".into(),
        ));
    }
    if config.influxdb.bucket.is_empty() {
        return Err(ConfigError::Validation(
            "InfluxDB bucket cannot be empty".into(),
        ));
    }

    // Validate points
    if config.points.is_empty() {
        return Err(ConfigError::Validation(
            "At least one point must be configured".into(),
        ));
    }

    for (i, point) in config.points.iter().enumerate() {
        if point.measurement.is_empty() {
            return Err(ConfigError::Validation(format!(
                "Point {} measurement cannot be empty",
                i
            )));
        }

        // Validate JSONPath in measurement if present
        if point.measurement.contains("$.") {
            validate_jsonpath(&point.measurement)?;
        }

        if point.topic.is_empty() {
            return Err(ConfigError::Validation(format!(
                "Point {} topic cannot be empty",
                i
            )));
        }
        if point.fields.is_empty() {
            return Err(ConfigError::Validation(format!(
                "Point {} must have at least one field",
                i
            )));
        }

        // Validate schedule if present
        if let Some(schedule) = &point.schedule {
            validate_cron_schedule(schedule)?;
        }
    }

    // Validate TLS file paths if present
    if let Some(cafile) = &config.mqtt.cafile
        && !cafile.exists()
    {
        return Err(ConfigError::FileNotFound(
            cafile.to_string_lossy().to_string(),
        ));
    }
    if let Some(certfile) = &config.mqtt.certfile
        && !certfile.exists()
    {
        return Err(ConfigError::FileNotFound(
            certfile.to_string_lossy().to_string(),
        ));
    }
    if let Some(keyfile) = &config.mqtt.keyfile
        && !keyfile.exists()
    {
        return Err(ConfigError::FileNotFound(
            keyfile.to_string_lossy().to_string(),
        ));
    }

    Ok(())
}

/// Validate a JSONPath expression.
pub fn validate_jsonpath(path: &str) -> Result<(), ConfigError> {
    parse_json_path(path).map_err(|e| ConfigError::InvalidJsonPath(format!("{}: {}", path, e)))?;
    Ok(())
}

/// Validate a cron schedule expression.
///
/// Supports both 5-field (standard) and 6/7-field (with seconds/year) formats.
/// 5-field format is converted to 6-field by prepending "0" for seconds.
fn validate_cron_schedule(schedule: &str) -> Result<(), ConfigError> {
    let parts: Vec<&str> = schedule.split_whitespace().collect();

    // The cron crate requires 6-7 fields (seconds, minute, hour, day, month, weekday, [year])
    // If we have 5 fields (standard cron), prepend "0" for seconds
    let cron_expr = if parts.len() == 5 {
        format!("0 {}", schedule)
    } else if parts.len() >= 6 {
        schedule.to_string()
    } else {
        return Err(ConfigError::InvalidCron(format!(
            "Cron expression must have at least 5 fields: {}",
            schedule
        )));
    };

    // Try to parse with the cron crate
    use std::str::FromStr;
    cron::Schedule::from_str(&cron_expr)
        .map_err(|e| ConfigError::InvalidCron(format!("{}: {}", schedule, e)))?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_cron_valid() {
        // Standard 5-field cron expressions
        assert!(validate_cron_schedule("0 * * * *").is_ok());
        assert!(validate_cron_schedule("*/5 * * * *").is_ok());
        assert!(validate_cron_schedule("0 0 * * *").is_ok());
        // 6-field cron expression (with seconds)
        assert!(validate_cron_schedule("0 0 * * * *").is_ok());
    }

    #[test]
    fn test_validate_cron_invalid() {
        assert!(validate_cron_schedule("invalid").is_err());
        assert!(validate_cron_schedule("* *").is_err());
    }
}
