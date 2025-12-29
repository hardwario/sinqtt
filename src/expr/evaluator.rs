//! Expression evaluation using evalexpr.

use crate::error::ExpressionError;
use evalexpr::{ContextWithMutableVariables, HashMapContext, Value, eval_with_context};
use std::collections::HashMap;

use super::parser::parse_expression;

/// Evaluate a mathematical expression with the given variables.
///
/// The expression should start with `=` (which is stripped before evaluation).
/// JSONPath expressions in the input are converted to variables.
///
/// Supports:
/// - Basic arithmetic: `+`, `-`, `*`, `/`
/// - Power: `^` (converted to `**` internally)
/// - Modulo: `%`
/// - Parentheses for grouping
/// - Variables in JSON__name format or $.name JSONPath format
pub fn evaluate_expression(
    expression: &str,
    variables: &HashMap<String, f64>,
) -> Result<f64, ExpressionError> {
    // Strip leading `=` and whitespace
    let expr = expression.trim_start_matches('=').trim();

    if expr.is_empty() {
        return Err(ExpressionError::Parse("Empty expression".to_string()));
    }

    // Parse the expression and convert JSONPath to variables
    let (converted_expr, _) = parse_expression(expr);

    // Build the evaluation context
    let mut context = HashMapContext::new();
    for (name, value) in variables {
        context
            .set_value(name.clone(), Value::Float(*value))
            .map_err(|e: evalexpr::EvalexprError| ExpressionError::Evaluation(e.to_string()))?;
    }

    // Evaluate the expression
    let result = eval_with_context(&converted_expr, &context)
        .map_err(|e| ExpressionError::Evaluation(e.to_string()))?;

    // Convert result to f64
    match result {
        Value::Float(f) => Ok(f),
        Value::Int(i) => Ok(i as f64),
        _ => Err(ExpressionError::Evaluation(format!(
            "Expected numeric result, got: {:?}",
            result
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_addition() {
        let vars = HashMap::new();
        let result = evaluate_expression("= 1 + 2", &vars).unwrap();
        assert!((result - 3.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_expression_without_equals() {
        let vars = HashMap::new();
        let result = evaluate_expression("1 + 2", &vars).unwrap();
        assert!((result - 3.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_multiplication() {
        let vars = HashMap::new();
        let result = evaluate_expression("= 5 * 3", &vars).unwrap();
        assert!((result - 15.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_division() {
        let vars = HashMap::new();
        let result = evaluate_expression("= 10 / 2", &vars).unwrap();
        assert!((result - 5.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_complex_expression() {
        let vars = HashMap::new();
        let result = evaluate_expression("= (2 + 3) * 4", &vars).unwrap();
        assert!((result - 20.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_expression_with_variable() {
        let mut vars = HashMap::new();
        vars.insert("JSON__payload".to_string(), 100.0);
        let result = evaluate_expression("= $.payload * 2", &vars).unwrap();
        assert!((result - 200.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_expression_with_nested_variable() {
        let mut vars = HashMap::new();
        vars.insert("JSON__payload_value".to_string(), 50.0);
        let result = evaluate_expression("= $.payload.value + 100", &vars).unwrap();
        assert!((result - 150.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_celsius_to_fahrenheit_0c() {
        let mut vars = HashMap::new();
        vars.insert("JSON__payload".to_string(), 0.0);
        let result = evaluate_expression("= 32 + ($.payload * 9 / 5)", &vars).unwrap();
        assert!((result - 32.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_celsius_to_fahrenheit_100c() {
        let mut vars = HashMap::new();
        vars.insert("JSON__payload".to_string(), 100.0);
        let result = evaluate_expression("= 32 + ($.payload * 9 / 5)", &vars).unwrap();
        assert!((result - 212.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_celsius_to_fahrenheit_37c() {
        let mut vars = HashMap::new();
        vars.insert("JSON__payload".to_string(), 37.0);
        let result = evaluate_expression("= 32 + ($.payload * 9 / 5)", &vars).unwrap();
        assert!((result - 98.6).abs() < 0.01);
    }

    #[test]
    fn test_negative_numbers() {
        let vars = HashMap::new();
        let result = evaluate_expression("= -5 + 10", &vars).unwrap();
        assert!((result - 5.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_decimal_variable_values() {
        let mut vars = HashMap::new();
        vars.insert("JSON__payload".to_string(), 1.5);
        let result = evaluate_expression("= JSON__payload * 2", &vars).unwrap();
        assert!((result - 3.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_power_operation() {
        let vars = HashMap::new();
        let result = evaluate_expression("= 2 ^ 3", &vars).unwrap();
        assert!((result - 8.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_modulo_operation() {
        let vars = HashMap::new();
        let result = evaluate_expression("= 10 % 3", &vars).unwrap();
        assert!((result - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_whitespace_handling() {
        let vars = HashMap::new();
        let result = evaluate_expression("=   1   +   2   ", &vars).unwrap();
        assert!((result - 3.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_invalid_expression() {
        let vars = HashMap::new();
        let result = evaluate_expression("= invalid (( syntax", &vars);
        assert!(result.is_err());
    }

    #[test]
    fn test_unbalanced_parentheses() {
        let vars = HashMap::new();
        let result = evaluate_expression("= (1 + 2", &vars);
        assert!(result.is_err());
    }
}
