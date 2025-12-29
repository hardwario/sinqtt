//! Expression parsing and evaluation module.

mod evaluator;
mod parser;

pub use evaluator::evaluate_expression;
pub use parser::{jsonpath_to_variable, parse_expression, variable_to_jsonpath};
