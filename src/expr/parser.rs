//! Expression parsing utilities.
//!
//! Converts JSONPath expressions to variable names and back.

use regex::Regex;
use std::sync::LazyLock;

/// Regex to match JSONPath expressions in text.
/// Matches patterns like $.payload, $.payload.temp, $.topic[1], etc.
static JSONPATH_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\$\.[\w.\[\]']+").expect("invalid JSONPATH_REGEX pattern"));

/// Regex to match power operator patterns like "2 ^ 3" or "var ^ 5".
static POWER_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(\w+|\d+\.?\d*|\([^)]+\))\s*\^\s*(\w+|\d+\.?\d*|\([^)]+\))")
        .expect("invalid POWER_REGEX pattern")
});

/// Convert a JSONPath expression to a variable name.
///
/// This matches the Python implementation exactly:
/// - `$` → `JSON_`
/// - `.` → `_`
///
/// Example: `$.payload.temperature` -> `JSON__payload_temperature`
/// Example: `$.topic[1]` -> `JSON__topic[1]`
pub fn jsonpath_to_variable(path: &str) -> String {
    path.replace('$', "JSON_").replace('.', "_")
}

/// Convert a variable name back to a JSONPath expression.
///
/// This is the reverse of `jsonpath_to_variable`:
/// - `JSON_` → `$`
/// - `_` → `.`
///
/// Note: This is a simple string replacement and may not perfectly
/// reconstruct complex paths with array indices.
///
/// Example: `JSON__payload_temperature` -> `$.payload.temperature`
pub fn variable_to_jsonpath(var: &str) -> String {
    var.replace("JSON_", "$").replace('_', ".")
}

/// Parse an expression and extract JSONPath variables.
///
/// Returns the expression with JSONPath converted to variables,
/// and a list of the original JSONPath expressions found.
///
/// Also converts `^` (power) to `math::pow` for evalexpr compatibility.
pub fn parse_expression(text: &str) -> (String, Vec<String>) {
    let mut result = text.to_string();
    let mut jsonpaths: Vec<String> = Vec::new();

    // Extract all JSONPath expressions
    for cap in JSONPATH_REGEX.find_iter(text) {
        let jsonpath = cap.as_str();
        jsonpaths.push(jsonpath.to_string());
    }

    // Sort by length descending to replace longer paths first
    // This prevents $.payload from being replaced before $.payload.offset
    jsonpaths.sort_by_key(|s| std::cmp::Reverse(s.len()));

    // Convert JSONPath to variables
    for jsonpath in &jsonpaths {
        let variable = jsonpath_to_variable(jsonpath);
        result = result.replace(jsonpath, &variable);
    }

    // Convert ^ (power) to math::pow for evalexpr
    // e.g., "2 ^ 3" becomes "math::pow(2, 3)"
    result = convert_power_operator(&result);

    (result, jsonpaths)
}

/// Convert ^ operator to math::pow() function calls.
/// e.g., "2 ^ 3" becomes "math::pow(2, 3)"
fn convert_power_operator(expr: &str) -> String {
    let mut result = expr.to_string();
    while let Some(cap) = POWER_REGEX.captures(&result) {
        // These capture groups are guaranteed to exist after a successful match
        let Some(full_match) = cap.get(0) else { break };
        let Some(base) = cap.get(1) else { break };
        let Some(exp) = cap.get(2) else { break };

        let replacement = format!("math::pow({}, {})", base.as_str(), exp.as_str());
        result = result.replacen(full_match.as_str(), &replacement, 1);
    }

    result
}

/// Extract variable names from an expression.
///
/// Returns a list of JSON__ prefixed variable names found in the expression.
pub fn extract_variables(text: &str) -> Vec<String> {
    let (_, jsonpaths) = parse_expression(text);
    jsonpaths.iter().map(|p| jsonpath_to_variable(p)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_jsonpath_to_variable_simple() {
        assert_eq!(jsonpath_to_variable("$.payload"), "JSON__payload");
    }

    #[test]
    fn test_jsonpath_to_variable_nested() {
        assert_eq!(
            jsonpath_to_variable("$.payload.temperature"),
            "JSON__payload_temperature"
        );
    }

    #[test]
    fn test_jsonpath_to_variable_topic_index() {
        assert_eq!(jsonpath_to_variable("$.topic[1]"), "JSON__topic[1]");
    }

    #[test]
    fn test_jsonpath_to_variable_deeply_nested() {
        assert_eq!(
            jsonpath_to_variable("$.payload.data.sensor.value"),
            "JSON__payload_data_sensor_value"
        );
    }

    #[test]
    fn test_variable_to_jsonpath_simple() {
        assert_eq!(variable_to_jsonpath("JSON__payload"), "$.payload");
    }

    #[test]
    fn test_variable_to_jsonpath_nested() {
        assert_eq!(
            variable_to_jsonpath("JSON__payload_temperature"),
            "$.payload.temperature"
        );
    }

    #[test]
    fn test_parse_expression_simple() {
        let (expr, paths) = parse_expression("32 + ($.payload * 9 / 5)");
        assert_eq!(expr, "32 + (JSON__payload * 9 / 5)");
        assert_eq!(paths, vec!["$.payload"]);
    }

    #[test]
    fn test_parse_expression_multiple_vars() {
        let (expr, paths) = parse_expression("$.payload + $.payload.offset");
        assert_eq!(expr, "JSON__payload + JSON__payload_offset");
        assert_eq!(paths.len(), 2);
        assert!(paths.contains(&"$.payload".to_string()));
        assert!(paths.contains(&"$.payload.offset".to_string()));
    }

    #[test]
    fn test_parse_expression_power() {
        let (expr, _) = parse_expression("2 ^ 3");
        assert_eq!(expr, "math::pow(2, 3)");
    }

    #[test]
    fn test_extract_variables() {
        let vars = extract_variables("$.payload + $.payload.offset");
        assert!(vars.contains(&"JSON__payload".to_string()));
        assert!(vars.contains(&"JSON__payload_offset".to_string()));
    }
}
