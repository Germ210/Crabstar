use std::fmt;
use std::ops::Range;

use crate::{
  syntax::SyntaxNode,
  types::{Type, VarID},
};

pub type Span = Range<usize>;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Expected<T> {
  Token(T),
  Label(&'static str),
  EndOfInput,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Reason<T> {
  Expected {
    found: Option<T>,
    expected: Vec<Expected<T>>,
  },
  Custom(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ParseErr<T> {
  pub span: Span,
  pub reason: Box<Reason<T>>,
  pub context: Vec<(&'static str, Span)>,
}

impl<T: fmt::Debug + Clone + PartialEq> ParseErr<T> {
  pub fn expected_found(expected: Vec<Expected<T>>, found: Option<T>, span: Span) -> Self {
    Self {
      span,
      reason: Box::new(Reason::Expected { found, expected }),
      context: Vec::new(),
    }
  }
  pub fn custom(msg: impl Into<String>, span: Span) -> Self {
    Self {
      span,
      reason: Box::new(Reason::Custom(msg.into())),
      context: Vec::new(),
    }
  }
  pub fn with_context(mut self, label: &'static str, span: Span) -> Self {
    self.context.push((label, span));
    self
  }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum TypeError {
  TypeMismatch {
    expected: Type,
    actual: Type,
    node: SyntaxNode,
    secondary: SyntaxNode,
  },
  AmbiguousType {
    var: VarID,
    node: SyntaxNode,
  },
  UnboundVariable {
    name: String,
    node: SyntaxNode,
  },
  UnresolvedConstructor {
    name: String,
    node: SyntaxNode,
  },
  MissingField {
    field: String,
    expected_row: Type,
    actual_row: Type,
    node: SyntaxNode,
    secondary: SyntaxNode,
  },
  NotPolymorphic {
    actual: Type,
    node: SyntaxNode,
    secondary: SyntaxNode,
  },
}

impl TypeError {
  pub fn mismatch(expected: Type, actual: Type, node: SyntaxNode, secondary: SyntaxNode) -> Self {
    Self::TypeMismatch {
      expected,
      actual,
      node,
      secondary,
    }
  }
  pub fn ambiguous(var: VarID, node: SyntaxNode) -> Self {
    Self::AmbiguousType { var, node }
  }
  pub fn unbound_variable(name: impl Into<String>, node: SyntaxNode) -> Self {
    Self::UnboundVariable {
      name: name.into(),
      node,
    }
  }
  pub fn unresolved_constructor(name: impl Into<String>, node: SyntaxNode) -> Self {
    Self::UnresolvedConstructor {
      name: name.into(),
      node,
    }
  }
  pub fn missing_field(
    field: impl Into<String>,
    expected_row: Type,
    actual_row: Type,
    node: SyntaxNode,
    secondary: SyntaxNode,
  ) -> Self {
    Self::MissingField {
      field: field.into(),
      expected_row,
      actual_row,
      node,
      secondary,
    }
  }
  pub fn not_polymorphic(actual: Type, node: SyntaxNode, secondary: SyntaxNode) -> Self {
    Self::NotPolymorphic {
      actual,
      node,
      secondary,
    }
  }
}
