use crabstar_frontend::{err::TypeError, inference::Inferencer};
use ropey::Rope;
use tower_lsp_server::ls_types::{Diagnostic, DiagnosticSeverity, Range};

use crate::server::char_index_to_position;

pub fn type_error_to_diagnostic(
  err: &TypeError,
  rope: &Rope,
  inferencer: &mut Inferencer,
) -> Diagnostic {
  let (node, message) = match err {
    TypeError::TypeMismatch {
      expected,
      actual,
      node,
      ..
    } => {
      let expected_id = inferencer.new_type(expected.clone());
      let actual_id = inferencer.new_type(actual.clone());
      let expected_str = inferencer.format_type(expected_id);
      let actual_str = inferencer.format_type(actual_id);
      (
        node,
        format!("Type mismatch: expected {expected_str}, got {actual_str}"),
      )
    }
    TypeError::MissingField { field, node, .. } => (node, format!("Missing field: {field}")),
    TypeError::NotPolymorphic { actual, node, .. } => {
      let actual_id = inferencer.new_type(actual.clone());
      let actual_str = inferencer.format_type(actual_id);
      (node, format!("Type is not polymorphic: {actual_str}"))
    }
    TypeError::AmbiguousType { node, .. } => (node, "Ambiguous type".to_string()),
    TypeError::UnboundVariable { name, node } => (node, format!("Unbound variable: {name}")),
    TypeError::UnresolvedConstructor { name, node } => {
      (node, format!("Unresolved constructor: {name}"))
    }
  };
  let text_range = node.text_range();
  let start = char_index_to_position(rope, text_range.start().into());
  let end = char_index_to_position(rope, text_range.end().into());
  Diagnostic {
    range: Range { start, end },
    severity: Some(DiagnosticSeverity::ERROR),
    message,
    ..Default::default()
  }
}
