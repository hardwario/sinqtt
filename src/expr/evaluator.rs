//! Expression evaluation using evalexpr.

use crate::error::ExpressionError;
use evalexpr::{eval_with_context, ContextWithMutableVariables, HashMapContext, Value};
use std::collections::HashMap;

use super::parser::parse_expression;

/// Evaluate a mathematical expression with the given variables.
///
/// The expression should start with `=` (which is stripped before evaluation).
/// JSONPath expressions in the input are converted to variables.
pub fn evaluate_expression(
    expression: &str,
    variables: &HashMap<String, f64>,
) -> Result<f64, ExpressionError> {
    // Strip leading `=` and whitespace
    let expr = expression.trim_start_matches('=').trim();

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
    fn test_simple_expression() {
        let vars = HashMap::new();
        let result = evaluate_expression("= 1 + 2", &vars).unwrap();
        assert!((result - 3.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_expression_with_variable() {
        let mut vars = HashMap::new();
        vars.insert("JSON__payload".to_string(), 100.0);
        let result = evaluate_expression("= $.payload * 2", &vars).unwrap();
        assert!((result - 200.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_celsius_to_fahrenheit() {
        let mut vars = HashMap::new();
        vars.insert("JSON__payload".to_string(), 0.0);
        let result = evaluate_expression("= 32 + ($.payload * 9 / 5)", &vars).unwrap();
        assert!((result - 32.0).abs() < f64::EPSILON);
    }
}
