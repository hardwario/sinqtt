#!/bin/bash
# Stress test script for sinqtt
# Tests various edge cases, malformed inputs, and high throughput scenarios

set -e

MQTT_HOST="${MQTT_HOST:-localhost}"
MQTT_PORT="${MQTT_PORT:-1883}"
TOTAL_MESSAGES=0
FAILED_MESSAGES=0

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

log_info() {
    echo -e "${GREEN}[INFO]${NC} $1"
}

log_warn() {
    echo -e "${YELLOW}[WARN]${NC} $1"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

send_msg() {
    local topic="$1"
    local payload="$2"
    local description="$3"

    TOTAL_MESSAGES=$((TOTAL_MESSAGES + 1))

    if mosquitto_pub -h "$MQTT_HOST" -p "$MQTT_PORT" -t "$topic" -m "$payload" 2>/dev/null; then
        echo "  [OK] $description"
    else
        FAILED_MESSAGES=$((FAILED_MESSAGES + 1))
        echo "  [FAIL] $description"
    fi
}

send_msg_file() {
    local topic="$1"
    local file="$2"
    local description="$3"

    TOTAL_MESSAGES=$((TOTAL_MESSAGES + 1))

    if mosquitto_pub -h "$MQTT_HOST" -p "$MQTT_PORT" -t "$topic" -f "$file" 2>/dev/null; then
        echo "  [OK] $description"
    else
        FAILED_MESSAGES=$((FAILED_MESSAGES + 1))
        echo "  [FAIL] $description"
    fi
}

# ============================================================================
# Test 1: Valid JSON payloads
# ============================================================================
test_valid_json() {
    log_info "Test 1: Valid JSON payloads"

    send_msg "stress/sensor1/json" '{"temperature": 25.5, "humidity": 60, "pressure": 1013.25}' "Simple JSON object"
    send_msg "stress/sensor2/json" '{"temperature": -10.0, "humidity": 0, "pressure": 500}' "JSON with negative/zero values"
    send_msg "stress/sensor3/json" '{"temperature": 999999.999, "humidity": 100, "pressure": 2000}' "JSON with large values"
    send_msg "stress/sensor4/json" '{"temperature": 0.00001, "humidity": 0.5, "pressure": 0.001}' "JSON with small decimals"
    send_msg "stress/sensor5/nested" '{"level1": {"level2": {"level3": {"value": 42}}}}' "Deeply nested JSON"
    send_msg "stress/sensor6/array" '[1, 2, 3, 4, 5]' "JSON array of numbers"
    send_msg "stress/sensor7/array" '["one", "two", "three"]' "JSON array of strings"
    send_msg "stress/sensor8/json" '{"temperature": null, "humidity": null}' "JSON with null values"
    send_msg "stress/sensor9/json" '{"temperature": true, "humidity": false}' "JSON with boolean values"

    echo ""
}

# ============================================================================
# Test 2: Numeric payloads
# ============================================================================
test_numeric() {
    log_info "Test 2: Numeric payloads"

    send_msg "stress/temp1/numeric" "25.5" "Positive float"
    send_msg "stress/temp2/numeric" "-10.5" "Negative float"
    send_msg "stress/temp3/numeric" "0" "Zero"
    send_msg "stress/temp4/numeric" "999999999" "Large integer"
    send_msg "stress/temp5/numeric" "-999999999" "Large negative integer"
    send_msg "stress/temp6/numeric" "0.0000001" "Very small decimal"
    send_msg "stress/temp7/numeric" "1e10" "Scientific notation"
    send_msg "stress/temp8/numeric" "-1.5e-5" "Negative scientific notation"

    echo ""
}

# ============================================================================
# Test 3: Expression evaluation
# ============================================================================
test_expressions() {
    log_info "Test 3: Expression evaluation (Celsius to Fahrenheit)"

    send_msg "stress/loc1/expr" "0" "0C = 32F"
    send_msg "stress/loc2/expr" "100" "100C = 212F"
    send_msg "stress/loc3/expr" "-40" "-40C = -40F"
    send_msg "stress/loc4/expr" "37" "37C = 98.6F (body temp)"
    send_msg "stress/loc5/expr" "20.5" "20.5C = 68.9F"

    echo ""
}

# ============================================================================
# Test 4: Raw string payloads
# ============================================================================
test_raw_strings() {
    log_info "Test 4: Raw string payloads"

    send_msg "stress/switch1/raw" "ON" "Simple ON"
    send_msg "stress/switch2/raw" "OFF" "Simple OFF"
    send_msg "stress/switch3/raw" "TOGGLE" "TOGGLE state"
    send_msg "stress/switch4/raw" "Hello World" "Text with space"
    send_msg "stress/switch5/raw" "" "Empty payload"
    send_msg "stress/switch6/raw" "   " "Whitespace only"
    send_msg "stress/switch7/raw" "Line1\nLine2" "Newline in string"

    echo ""
}

# ============================================================================
# Test 5: Malformed JSON (fuzzy testing)
# ============================================================================
test_malformed_json() {
    log_info "Test 5: Malformed JSON payloads (fuzzy testing)"

    send_msg "stress/fuzz1/json" '{temperature: 25}' "Missing quotes on key"
    send_msg "stress/fuzz2/json" '{"temperature": 25,}' "Trailing comma"
    send_msg "stress/fuzz3/json" '{"temperature": }' "Missing value"
    send_msg "stress/fuzz4/json" '{' "Incomplete object"
    send_msg "stress/fuzz5/json" '}' "Just closing brace"
    send_msg "stress/fuzz6/json" '[' "Incomplete array"
    send_msg "stress/fuzz7/json" '{"a": {"b": {"c": }}}' "Incomplete nested"
    send_msg "stress/fuzz8/json" '{"temperature": NaN}' "NaN value"
    send_msg "stress/fuzz9/json" '{"temperature": Infinity}' "Infinity value"
    send_msg "stress/fuzz10/json" '{"temperature": undefined}' "undefined value"
    send_msg "stress/fuzz11/json" "not json at all" "Plain text"
    send_msg "stress/fuzz12/json" '{"key": "value"' "Missing closing brace"
    send_msg "stress/fuzz13/json" '"just a string"' "Just a quoted string"
    send_msg "stress/fuzz14/json" '123' "Just a number"
    send_msg "stress/fuzz15/json" 'null' "Just null"
    send_msg "stress/fuzz16/json" 'true' "Just true"
    send_msg "stress/fuzz17/json" 'false' "Just false"

    echo ""
}

# ============================================================================
# Test 6: Special characters
# ============================================================================
test_special_chars() {
    log_info "Test 6: Special characters"

    send_msg "stress/special1/raw" 'Hello "World"' "Double quotes"
    send_msg "stress/special2/raw" "Hello 'World'" "Single quotes"
    send_msg "stress/special3/raw" 'Path: C:\Windows\System32' "Backslashes"
    send_msg "stress/special4/raw" 'Unicode: ĚŠČŘŽÝÁÍÉ' "Czech characters"
    send_msg "stress/special5/raw" 'Unicode: 日本語' "Japanese characters"
    send_msg "stress/special6/raw" 'Emoji: 🔥🚀💯' "Emoji"
    send_msg "stress/special7/json" '{"name": "O'\''Brien"}' "Escaped single quote"
    send_msg "stress/special8/json" '{"text": "Line1\nLine2\tTabbed"}' "Escape sequences"
    send_msg "stress/special9/raw" $'Binary\x00\x01\x02' "Binary data (null bytes)"

    echo ""
}

# ============================================================================
# Test 7: Edge case topics
# ============================================================================
test_edge_topics() {
    log_info "Test 7: Edge case topic patterns"

    send_msg "stress/multi/a/b/c" '{"data": 1}' "Multi-level wildcard match"
    send_msg "stress/multi/deep/path/here" '{"data": 2}' "Deeper multi-level"
    send_msg "stress/multi" '{"data": 3}' "Exact multi topic"
    send_msg "stress/a/numeric" "42" "Short device ID"
    send_msg "stress/device-with-dashes/numeric" "42" "Device ID with dashes"
    send_msg "stress/device_with_underscores/numeric" "42" "Device ID with underscores"
    send_msg "stress/UPPERCASE/numeric" "42" "Uppercase device ID"
    send_msg "stress/MixedCase123/numeric" "42" "Mixed case with numbers"

    echo ""
}

# ============================================================================
# Test 8: High throughput burst
# ============================================================================
test_high_throughput() {
    log_info "Test 8: High throughput burst (1000 messages)"

    local start_time=$(date +%s.%N)
    local count=0

    for i in $(seq 1 1000); do
        mosquitto_pub -h "$MQTT_HOST" -p "$MQTT_PORT" -t "stress/burst$i/numeric" -m "$i" 2>/dev/null &
        count=$((count + 1))

        # Batch publish to avoid too many background processes
        if [ $((count % 50)) -eq 0 ]; then
            wait
            echo "  Sent $count/1000 messages..."
        fi
    done
    wait

    local end_time=$(date +%s.%N)
    local duration=$(echo "$end_time - $start_time" | bc)
    local rate=$(echo "1000 / $duration" | bc)

    TOTAL_MESSAGES=$((TOTAL_MESSAGES + 1000))
    echo "  [OK] Sent 1000 messages in ${duration}s (~${rate} msg/s)"
    echo ""
}

# ============================================================================
# Test 9: Rapid fire same topic
# ============================================================================
test_rapid_same_topic() {
    log_info "Test 9: Rapid fire to same topic (500 messages)"

    local start_time=$(date +%s.%N)

    for i in $(seq 1 500); do
        mosquitto_pub -h "$MQTT_HOST" -p "$MQTT_PORT" -t "stress/rapid/numeric" -m "$i" 2>/dev/null &
        if [ $((i % 50)) -eq 0 ]; then
            wait
        fi
    done
    wait

    local end_time=$(date +%s.%N)
    local duration=$(echo "$end_time - $start_time" | bc)

    TOTAL_MESSAGES=$((TOTAL_MESSAGES + 500))
    echo "  [OK] Sent 500 messages to same topic in ${duration}s"
    echo ""
}

# ============================================================================
# Test 10: Large payloads
# ============================================================================
test_large_payloads() {
    log_info "Test 10: Large payloads"

    # 1KB payload
    local payload_1k=$(python3 -c "import json; print(json.dumps({'data': 'x' * 1000}))")
    send_msg "stress/large1/json" "$payload_1k" "1KB JSON payload"

    # 10KB payload
    local payload_10k=$(python3 -c "import json; print(json.dumps({'data': 'x' * 10000}))")
    send_msg "stress/large2/json" "$payload_10k" "10KB JSON payload"

    # 100KB payload
    local payload_100k=$(python3 -c "import json; print(json.dumps({'data': 'x' * 100000}))")
    send_msg "stress/large3/json" "$payload_100k" "100KB JSON payload"

    # Array with many elements
    local payload_array=$(python3 -c "import json; print(json.dumps(list(range(1000))))")
    send_msg "stress/large4/array" "$payload_array" "Array with 1000 elements"

    # Deeply nested structure
    local payload_deep=$(python3 -c "
import json
d = {'value': 42}
for i in range(20):
    d = {'level' + str(i): d}
print(json.dumps(d))
")
    send_msg "stress/large5/nested" "$payload_deep" "20 levels deep nesting"

    echo ""
}

# ============================================================================
# Test 11: JSON edge cases
# ============================================================================
test_json_edge_cases() {
    log_info "Test 11: JSON edge cases"

    send_msg "stress/edge1/json" '{"": "empty key"}' "Empty string key"
    send_msg "stress/edge2/json" '{"a": "", "b": ""}' "Empty string values"
    send_msg "stress/edge3/json" '{"key": []}' "Empty array"
    send_msg "stress/edge4/json" '{"key": {}}' "Empty object"
    send_msg "stress/edge5/json" '[]' "Top-level empty array"
    send_msg "stress/edge6/json" '{}' "Top-level empty object"
    send_msg "stress/edge7/json" '{"temperature": 1.7976931348623157e+308}' "Max float64"
    send_msg "stress/edge8/json" '{"temperature": 5e-324}' "Min positive float64"
    send_msg "stress/edge9/json" '{"count": 9007199254740991}' "Max safe integer"
    send_msg "stress/edge10/json" '{"unicode": "\u0000\u001f"}' "Control characters"

    echo ""
}

# ============================================================================
# Test 12: Concurrent publishers
# ============================================================================
test_concurrent_publishers() {
    log_info "Test 12: Concurrent publishers (10 parallel streams)"

    local start_time=$(date +%s.%N)

    # Start 10 parallel publisher streams
    for stream in $(seq 1 10); do
        (
            for i in $(seq 1 100); do
                mosquitto_pub -h "$MQTT_HOST" -p "$MQTT_PORT" \
                    -t "stress/concurrent$stream/json" \
                    -m "{\"stream\": $stream, \"seq\": $i}" 2>/dev/null
            done
        ) &
    done
    wait

    local end_time=$(date +%s.%N)
    local duration=$(echo "$end_time - $start_time" | bc)

    TOTAL_MESSAGES=$((TOTAL_MESSAGES + 1000))
    echo "  [OK] 10 concurrent streams x 100 messages = 1000 messages in ${duration}s"
    echo ""
}

# ============================================================================
# Main
# ============================================================================
main() {
    echo "=============================================="
    echo "sinqtt Stress Test Suite"
    echo "=============================================="
    echo ""
    echo "MQTT Broker: $MQTT_HOST:$MQTT_PORT"
    echo ""

    # Verify MQTT connectivity
    if ! mosquitto_pub -h "$MQTT_HOST" -p "$MQTT_PORT" -t "stress/test/ping" -m "ping" 2>/dev/null; then
        log_error "Cannot connect to MQTT broker at $MQTT_HOST:$MQTT_PORT"
        exit 1
    fi
    log_info "MQTT broker connection verified"
    echo ""

    # Run all tests
    test_valid_json
    test_numeric
    test_expressions
    test_raw_strings
    test_malformed_json
    test_special_chars
    test_edge_topics
    test_high_throughput
    test_rapid_same_topic
    test_large_payloads
    test_json_edge_cases
    test_concurrent_publishers

    # Summary
    echo "=============================================="
    echo "STRESS TEST SUMMARY"
    echo "=============================================="
    echo "Total messages sent: $TOTAL_MESSAGES"
    echo "Failed sends: $FAILED_MESSAGES"

    if [ $FAILED_MESSAGES -eq 0 ]; then
        log_info "All messages sent successfully!"
    else
        log_warn "$FAILED_MESSAGES messages failed to send"
    fi

    echo ""
    echo "NOTE: Check sinqtt logs to verify message processing"
    echo "=============================================="
}

main "$@"
