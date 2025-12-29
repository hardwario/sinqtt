//! Expression parsing and evaluation module.

mod evaluator;
mod parser;

pub use evaluator::evaluate_expression;
pub use parser::{extract_variables, jsonpath_to_variable, parse_expression, variable_to_jsonpath};
