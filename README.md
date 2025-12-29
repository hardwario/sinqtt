# sinqtt

[![GitHub Actions](https://img.shields.io/github/actions/workflow/status/hardwario/sinqtt/test.yml)](https://github.com/hardwario/sinqtt/actions)
[![GitHub Release](https://img.shields.io/github/v/release/hardwario/sinqtt?sort=semver)](https://github.com/hardwario/sinqtt/releases)
[![GitHub License](https://img.shields.io/github/license/hardwario/sinqtt)](https://github.com/hardwario/sinqtt/blob/main/LICENSE)

A high-performance MQTT to InfluxDB v3 bridge for IoT applications, written in Rust. Subscribe to MQTT topics, process incoming messages, and write data points to InfluxDB.

> The name `sinqtt` is derived from a combination of "Sink" and "MQTT".

---

## Features

- Subscribe to multiple MQTT topics with wildcard support (`+`, `#`)
- Write data to InfluxDB v3 with tags and fields
- Support for both JSON and raw string payloads
- JSONPath extraction from message payloads
- Mathematical expressions for computed fields
- Cron-based scheduling for conditional writes
- HTTP forwarding of processed data
- Base64 decoding support
- Environment variable substitution in configuration
- Optional TLS/SSL for MQTT connections
- Gzip compression for InfluxDB writes
- Daemon mode with automatic reconnection

---

## Requirements

- Rust 1.70+ (for building from source)
- InfluxDB v3 instance (Cloud or self-hosted)
- MQTT broker (Mosquitto, etc.)

---

## Installation

### From source

```bash
git clone https://github.com/hardwario/sinqtt.git
cd sinqtt
cargo build --release
```

The binary will be available at `./target/release/sinqtt`.

### With TLS support

```bash
cargo build --release --features tls
```

---

## Quick Start

1. Create a configuration file `config.yml`:

```yaml
mqtt:
  host: localhost
  port: 1883

influxdb:
  host: localhost
  port: 8181
  token: your-api-token
  org: your-organization
  bucket: your-bucket

points:
  - measurement: temperature
    topic: sensors/+/temperature
    fields:
      value: $.payload
    tags:
      sensor_id: $.topic[1]
```

2. Run the bridge:

```bash
sinqtt -c config.yml
```

---

## CLI Usage

```
sinqtt [OPTIONS]

Options:
  -c, --config <FILE>  Path to configuration file (YAML)  [required]
  -D, --debug          Enable debug logging
  -t, --test           Validate configuration without running
  -d, --daemon         Daemon mode: retry on error
  -h, --help           Print help
  -V, --version        Print version
```

---

## Configuration Reference

### MQTT Section

```yaml
mqtt:
  host: localhost          # Broker hostname
  port: 1883               # Broker port
  username: user           # Optional authentication
  password: pass
  cafile: /path/to/ca.crt  # Optional TLS (requires --features tls)
  certfile: /path/to/cert
  keyfile: /path/to/key
```

### InfluxDB Section

```yaml
influxdb:
  host: localhost          # InfluxDB hostname
  port: 8181               # InfluxDB port
  token: your-api-token    # API token
  org: your-organization   # Organization name
  bucket: your-bucket      # Default bucket
  enable_gzip: false       # Optional gzip compression
```

### Points Section

```yaml
points:
  - measurement: temperature
    topic: node/+/thermometer/+/temperature
    bucket: custom_bucket   # Optional: override default bucket
    schedule: '0 * * * *'   # Optional: cron filter
    fields:
      value: $.payload
      converted:
        value: $.payload.raw
        type: float
      calculated: = 32 + ($.payload.celsius * 9 / 5)
    tags:
      id: $.topic[1]
      channel: $.topic[3]
    httpcontent:            # Optional: fields to forward via HTTP
      temp: $.payload.temperature
```

### Type Conversion

Fields support optional type conversion:

```yaml
fields:
  temperature:
    value: $.payload.temp
    type: float
```

| Type | Description | Example |
|------|-------------|---------|
| `float` | Floating-point number | `"123"` -> `123.0` |
| `int` | Integer number | `"42"` -> `42` |
| `str` | String | `123` -> `"123"` |
| `bool` | Boolean | `1` -> `true` |
| `booltoint` | Boolean converted to 0/1 | `true` -> `1` |

### Payload Formats

Both JSON and raw string payloads are supported:

| Payload | Parsed As | `$.payload` Value |
|---------|-----------|-------------------|
| `25.5` | JSON number | `25.5` (float) |
| `{"temp": 25}` | JSON object | `{"temp": 25}` |
| `[1, 2, 3]` | JSON array | `[1, 2, 3]` |
| `"hello"` | JSON string | `"hello"` |
| `ON` | Raw string | `"ON"` |
| `Device ready` | Raw string | `"Device ready"` |

Raw strings are useful for simple MQTT messages like Tasmota power states (`ON`/`OFF`) or status messages.

### JSONPath Syntax

- `$.payload` - Entire payload (JSON or raw string)
- `$.payload.temperature` - Nested field (JSON only)
- `$.payload.data[0]` - Array index (JSON only)
- `$.topic[n]` - Topic segment (0-indexed)
- `$.payload['pm2.5']` - Field with special characters (dot, space, etc.)

**Special Characters:** Use bracket notation with quotes for field names containing dots, spaces, or other reserved characters:

```yaml
# For payload: {"air_quality_sensor": {"pm2.5": 5}}
fields:
  pm25: $.payload.air_quality_sensor['pm2.5']
```

### Mathematical Expressions

Fields starting with `=` are evaluated as mathematical expressions:

```yaml
fields:
  fahrenheit: = 32 + ($.payload * 9 / 5)
  doubled: = $.payload.value * 2
  power: = $.payload.base ^ $.payload.exponent
```

Supported operators:
- Arithmetic: `+`, `-`, `*`, `/`, `%`
- Power: `^`
- Parentheses for grouping

### Environment Variables

Use `${VAR}` or `${VAR:default}` syntax to substitute environment variables in any string value.

```yaml
mqtt:
  host: ${SINQTT_MQTT_HOST:localhost}
  port: ${SINQTT_MQTT_PORT:1883}
  username: ${SINQTT_MQTT_USERNAME:}
  password: ${SINQTT_MQTT_PASSWORD:}

influxdb:
  host: ${SINQTT_INFLUXDB_HOST:localhost}
  port: ${SINQTT_INFLUXDB_PORT:8181}
  token: ${SINQTT_INFLUXDB_TOKEN}
  org: ${SINQTT_INFLUXDB_ORG:default}
  bucket: ${SINQTT_INFLUXDB_BUCKET:metrics}
```

- `${VAR}` - Required variable (error if not set)
- `${VAR:default}` - Optional variable with default value
- `${VAR:}` - Optional variable with empty default

### Optional HTTP Forwarding

```yaml
http:
  destination: https://example.com/api
  action: post    # Values post, put, or patch
  username: user  # Optional basic auth
  password: pass
```

### Optional Base64 Decoding

```yaml
base64decode:
  source: $.payload.data
  target: data
```

---

## Complete Example

```yaml
mqtt:
  host: ${MQTT_HOST:localhost}
  port: 1883
  username: ${MQTT_USER:}
  password: ${MQTT_PASS:}

influxdb:
  host: ${INFLUXDB_HOST:localhost}
  port: 8181
  token: ${INFLUXDB_TOKEN}
  org: my-org
  bucket: iot-data
  enable_gzip: true

http:
  destination: https://webhook.example.com/data
  action: post

points:
  # Simple numeric sensor
  - measurement: temperature
    topic: sensors/+/temperature
    fields:
      value: $.payload
    tags:
      sensor_id: $.topic[1]

  # JSON telemetry with multiple fields
  - measurement: environment
    topic: devices/+/telemetry
    fields:
      temperature: $.payload.temperature
      humidity: $.payload.humidity
      pressure:
        value: $.payload.pressure
        type: float
    tags:
      device_id: $.topic[1]

  # Computed field (Celsius to Fahrenheit)
  - measurement: temperature_fahrenheit
    topic: sensors/+/temperature
    fields:
      celsius: $.payload
      fahrenheit: = 32 + ($.payload * 9 / 5)
    tags:
      location: $.topic[1]

  # Tasmota power state (raw string)
  - measurement: power_state
    topic: stat/+/power
    fields:
      state: $.payload
    tags:
      device: $.topic[1]

  # Scheduled write (every 5 minutes)
  - measurement: periodic_reading
    topic: sensors/#
    schedule: '0 */5 * * * *'
    fields:
      value: $.payload
```

---

## Development

```bash
git clone https://github.com/hardwario/sinqtt.git
cd sinqtt
cargo build
cargo test
cargo run -- -c tests/fixtures/sample-config.yml --debug
```

### Running Tests

```bash
# Unit tests
cargo test

# With output
cargo test -- --nocapture
```

---

## Python Version

This project is a Rust reimplementation of [mqtt2influxdb](python/README.md). The Python version is available on PyPI and provides the same functionality with a Python runtime.

---

## License

This project is licensed under the **MIT License** - see the [LICENSE](LICENSE) file for details.

---

Made with ❤ by [**HARDWARIO a.s.**](https://www.hardwario.com/) in the heart of Europe.
