//! CLI integration tests.
//!
//! Ported from Python `test_cli.py`.

use assert_cmd::Command;
use predicates::prelude::*;
use std::io::Write;
use tempfile::NamedTempFile;

/// Helper to create a valid config file.
fn create_valid_config() -> NamedTempFile {
    let config = r#"
mqtt:
  host: localhost
  port: 1883

influxdb:
  host: localhost
  port: 8181
  token: test-token
  org: test-org
  bucket: test-bucket

points:
  - measurement: temperature
    topic: "test/+/temperature"
    fields:
      value: "$.payload"
"#;

    let mut file = NamedTempFile::new().unwrap();
    file.write_all(config.as_bytes()).unwrap();
    file
}

/// Helper to create an invalid config file (invalid port).
fn create_invalid_port_config() -> NamedTempFile {
    let config = r#"
mqtt:
  host: localhost
  port: 99999

influxdb:
  host: localhost
  port: 8181
  token: test-token
  org: test-org
  bucket: test-bucket

points:
  - measurement: test
    topic: test
    fields:
      value: "$.payload"
"#;

    let mut file = NamedTempFile::new().unwrap();
    file.write_all(config.as_bytes()).unwrap();
    file
}

// ============================================================================
// Version Tests
// ============================================================================

#[test]
fn test_version_option() {
    let mut cmd = Command::cargo_bin("sinqtt").unwrap();
    cmd.arg("--version")
        .assert()
        .success()
        .stdout(predicate::str::contains("sinqtt"));
}

// ============================================================================
// Help Tests
// ============================================================================

#[test]
fn test_help_option() {
    let mut cmd = Command::cargo_bin("sinqtt").unwrap();
    cmd.arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("Usage:"))
        .stdout(predicate::str::contains("--config"))
        .stdout(predicate::str::contains("--debug"))
        .stdout(predicate::str::contains("--test"));
}

#[test]
fn test_help_short_option() {
    let mut cmd = Command::cargo_bin("sinqtt").unwrap();
    cmd.arg("-h")
        .assert()
        .success()
        .stdout(predicate::str::contains("Usage:"));
}

// ============================================================================
// Config Required Tests
// ============================================================================

#[test]
fn test_missing_config_shows_error() {
    let mut cmd = Command::cargo_bin("sinqtt").unwrap();
    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("config").or(predicate::str::contains("required")));
}

#[test]
fn test_nonexistent_config_shows_error() {
    let mut cmd = Command::cargo_bin("sinqtt").unwrap();
    cmd.args(["-c", "/nonexistent/path/config.yml"])
        .assert()
        .failure();
}

// ============================================================================
// Test Mode Tests
// ============================================================================

#[test]
fn test_test_mode_valid_config() {
    let config_file = create_valid_config();
    let mut cmd = Command::cargo_bin("sinqtt").unwrap();
    cmd.args(["-c", config_file.path().to_str().unwrap(), "--test"])
        .assert()
        .success()
        .stdout(predicate::str::contains("valid").or(predicate::str::contains("Valid")));
}

#[test]
fn test_test_mode_invalid_port() {
    let config_file = create_invalid_port_config();
    let mut cmd = Command::cargo_bin("sinqtt").unwrap();
    cmd.args(["-c", config_file.path().to_str().unwrap(), "--test"])
        .assert()
        .failure();
}

#[test]
fn test_test_mode_short_option() {
    let config_file = create_valid_config();
    let mut cmd = Command::cargo_bin("sinqtt").unwrap();
    cmd.args(["-c", config_file.path().to_str().unwrap(), "-t"])
        .assert()
        .success();
}

// ============================================================================
// Debug Mode Tests
// ============================================================================

#[test]
fn test_debug_short_option_accepted() {
    let config_file = create_valid_config();
    let mut cmd = Command::cargo_bin("sinqtt").unwrap();
    cmd.args(["-c", config_file.path().to_str().unwrap(), "-t", "-D"])
        .assert()
        .success();
}

// ============================================================================
// Daemon Mode Tests
// ============================================================================

#[test]
fn test_daemon_short_option_accepted() {
    let config_file = create_valid_config();
    let mut cmd = Command::cargo_bin("sinqtt").unwrap();
    // Test that -d flag is accepted (use with -t to avoid needing network)
    cmd.args(["-c", config_file.path().to_str().unwrap(), "-t", "-d"])
        .assert()
        .success();
}

// ============================================================================
// Config Validation Tests
// ============================================================================

#[test]
fn test_empty_points_rejected() {
    let config = r#"
mqtt:
  host: localhost
  port: 1883

influxdb:
  host: localhost
  port: 8181
  token: test-token
  org: test-org
  bucket: test-bucket

points: []
"#;

    let mut file = NamedTempFile::new().unwrap();
    file.write_all(config.as_bytes()).unwrap();

    let mut cmd = Command::cargo_bin("sinqtt").unwrap();
    cmd.args(["-c", file.path().to_str().unwrap(), "-t"])
        .assert()
        .failure();
}

#[test]
fn test_missing_token_rejected() {
    let config = r#"
mqtt:
  host: localhost
  port: 1883

influxdb:
  host: localhost
  port: 8181
  org: test-org
  bucket: test-bucket

points:
  - measurement: test
    topic: test
    fields:
      value: "$.payload"
"#;

    let mut file = NamedTempFile::new().unwrap();
    file.write_all(config.as_bytes()).unwrap();

    let mut cmd = Command::cargo_bin("sinqtt").unwrap();
    cmd.args(["-c", file.path().to_str().unwrap(), "-t"])
        .assert()
        .failure();
}

#[test]
fn test_empty_token_rejected() {
    let config = r#"
mqtt:
  host: localhost
  port: 1883

influxdb:
  host: localhost
  port: 8181
  token: ""
  org: test-org
  bucket: test-bucket

points:
  - measurement: test
    topic: test
    fields:
      value: "$.payload"
"#;

    let mut file = NamedTempFile::new().unwrap();
    file.write_all(config.as_bytes()).unwrap();

    let mut cmd = Command::cargo_bin("sinqtt").unwrap();
    cmd.args(["-c", file.path().to_str().unwrap(), "-t"])
        .assert()
        .failure();
}

#[test]
fn test_invalid_schedule_rejected() {
    let config = r#"
mqtt:
  host: localhost
  port: 1883

influxdb:
  host: localhost
  port: 8181
  token: test-token
  org: test-org
  bucket: test-bucket

points:
  - measurement: test
    topic: test
    schedule: "invalid cron"
    fields:
      value: "$.payload"
"#;

    let mut file = NamedTempFile::new().unwrap();
    file.write_all(config.as_bytes()).unwrap();

    let mut cmd = Command::cargo_bin("sinqtt").unwrap();
    cmd.args(["-c", file.path().to_str().unwrap(), "-t"])
        .assert()
        .failure();
}

#[test]
fn test_missing_mqtt_host_rejected() {
    let config = r#"
mqtt:
  port: 1883

influxdb:
  host: localhost
  port: 8181
  token: test-token
  org: test-org
  bucket: test-bucket

points:
  - measurement: test
    topic: test
    fields:
      value: "$.payload"
"#;

    let mut file = NamedTempFile::new().unwrap();
    file.write_all(config.as_bytes()).unwrap();

    let mut cmd = Command::cargo_bin("sinqtt").unwrap();
    cmd.args(["-c", file.path().to_str().unwrap(), "-t"])
        .assert()
        .failure();
}

#[test]
fn test_combined_short_options() {
    let config_file = create_valid_config();
    let mut cmd = Command::cargo_bin("sinqtt").unwrap();
    // Test combining multiple short options
    cmd.args(["-c", config_file.path().to_str().unwrap(), "-t", "-D", "-d"])
        .assert()
        .success();
}

#[test]
fn test_env_var_substitution_in_config() {
    let config = r#"
mqtt:
  host: ${SINQTT_TEST_HOST:localhost}
  port: 1883

influxdb:
  host: localhost
  port: 8181
  token: test-token
  org: test-org
  bucket: test-bucket

points:
  - measurement: test
    topic: test
    fields:
      value: "$.payload"
"#;

    let mut file = NamedTempFile::new().unwrap();
    file.write_all(config.as_bytes()).unwrap();

    let mut cmd = Command::cargo_bin("sinqtt").unwrap();
    cmd.args(["-c", file.path().to_str().unwrap(), "-t"])
        .assert()
        .success();
}
