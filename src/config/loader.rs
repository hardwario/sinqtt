//! Configuration loader with environment variable expansion.

use super::types::Config;
use super::validation::validate_config;
use crate::error::ConfigError;
use regex::Regex;
use serde_yaml::Value;
use std::path::Path;
use std::sync::LazyLock;

static ENV_VAR_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\$\{([^}:]+)(?::([^}]*))?\}").expect("invalid ENV_VAR_REGEX pattern")
});

/// Load and validate configuration from a YAML file.
pub fn load_config(path: &Path) -> Result<Config, ConfigError> {
    let content = std::fs::read_to_string(path)?;

    if content.trim().is_empty() {
        return Err(ConfigError::Validation("Empty configuration file".into()));
    }

    let value: Value = serde_yaml::from_str(&content)?;
    let expanded = expand_env_vars_recursive(value)?;
    let config: Config = serde_yaml::from_value(expanded)?;

    validate_config(&config)?;

    Ok(config)
}

/// Recursively expand environment variables in a YAML value.
fn expand_env_vars_recursive(value: Value) -> Result<Value, ConfigError> {
    match value {
        Value::String(s) => Ok(Value::String(expand_env_vars(&s)?)),
        Value::Mapping(map) => {
            let mut new_map = serde_yaml::Mapping::new();
            for (k, v) in map {
                new_map.insert(k, expand_env_vars_recursive(v)?);
            }
            Ok(Value::Mapping(new_map))
        }
        Value::Sequence(seq) => {
            let new_seq: Result<Vec<Value>, ConfigError> =
                seq.into_iter().map(expand_env_vars_recursive).collect();
            Ok(Value::Sequence(new_seq?))
        }
        other => Ok(other),
    }
}

/// Expand environment variables in a string.
/// Supports ${VAR} and ${VAR:default} syntax.
fn expand_env_vars(input: &str) -> Result<String, ConfigError> {
    let mut result = input.to_string();

    for cap in ENV_VAR_REGEX.captures_iter(input) {
        // Groups 0 and 1 are guaranteed to exist after a successful match
        let Some(full_match) = cap.get(0) else { continue };
        let Some(var_match) = cap.get(1) else { continue };

        let var_name = var_match.as_str();
        let default_value = cap.get(2).map(|m| m.as_str());

        let value = match std::env::var(var_name) {
            Ok(v) => v,
            Err(_) => match default_value {
                Some(default) => default.to_string(),
                None => return Err(ConfigError::MissingEnvVar(var_name.to_string())),
            },
        };

        result = result.replace(full_match.as_str(), &value);
    }

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_expand_env_vars_simple() {
        std::env::set_var("TEST_VAR", "test_value");
        let result = expand_env_vars("${TEST_VAR}").unwrap();
        assert_eq!(result, "test_value");
        std::env::remove_var("TEST_VAR");
    }

    #[test]
    fn test_expand_env_vars_with_default() {
        let result = expand_env_vars("${NONEXISTENT_VAR:default_value}").unwrap();
        assert_eq!(result, "default_value");
    }

    #[test]
    fn test_expand_env_vars_missing() {
        let result = expand_env_vars("${MISSING_VAR}");
        assert!(result.is_err());
    }
}
