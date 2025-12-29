//! Expression parsing utilities.
//!
//! Converts JSONPath expressions to variable names and back.

use regex::Regex;
use std::sync::LazyLock;

static JSONPATH_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\$\.[\w.\[\]']+").unwrap());

/// Convert a JSONPath expression to a variable name.
///
/// Example: `$.payload.temperature` -> `JSON__payload_temperature`
pub fn jsonpath_to_variable(path: &str) -> String {
    path.replace("$", "JSON_").replace(".", "_").replace("[", "_").replace("]", "_").replace("'", "")
}

/// Convert a variable name back to a JSONPath expression.
///
/// Example: `JSON__payload_temperature` -> `$.payload.temperature`
pub fn variable_to_jsonpath(var: &str) -> String {
    var.replace("JSON_", "$").replace("_", ".")
}

/// Parse an expression and extract JSONPath variables.
///
/// Returns the expression with JSONPath converted to variables,
/// and a list of the original JSONPath expressions found.
pub fn parse_expression(text: &str) -> (String, Vec<String>) {
    let mut result = text.to_string();
    let mut jsonpaths = Vec::new();

    for cap in JSONPATH_REGEX.find_iter(text) {
        let jsonpath = cap.as_str();
        let variable = jsonpath_to_variable(jsonpath);
        result = result.replace(jsonpath, &variable);
        jsonpaths.push(jsonpath.to_string());
    }

    (result, jsonpaths)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_jsonpath_to_variable() {
        assert_eq!(jsonpath_to_variable("$.payload"), "JSON__payload");
        assert_eq!(
            jsonpath_to_variable("$.payload.temperature"),
            "JSON__payload_temperature"
        );
    }

    #[test]
    fn test_variable_to_jsonpath() {
        assert_eq!(variable_to_jsonpath("JSON__payload"), "$.payload");
        assert_eq!(
            variable_to_jsonpath("JSON__payload_temperature"),
            "$.payload.temperature"
        );
    }

    #[test]
    fn test_parse_expression() {
        let (expr, paths) = parse_expression("32 + ($.payload * 9 / 5)");
        assert_eq!(expr, "32 + (JSON__payload * 9 / 5)");
        assert_eq!(paths, vec!["$.payload"]);
    }
}
