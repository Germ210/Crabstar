use chumsky::prelude::SimpleSpan;
use crate::typechecker::Type;

#[derive(Debug, Clone, Default)]
pub enum AstKind {
  #[default]
  Dummy,
  Int(u64),
  Float(f64),
  Bool(bool),
  String(String),
  Ident(String),
  Unary(String, Box<Ast>),
  Binary(String, Box<Ast>, Box<Ast>),
  Block(Vec<Ast>),
  Let {
    name: String,
    mutable: bool,
    recursive: bool,
    args: Option<Vec<Ast>>,
    ret_type: Option<Box<Ast>>,
    value: Box<Ast>,
    next: Option<Box<Ast>>
  },
  Call {
    callee: Box<Ast>, 
    args: Vec<Ast>
  },
  If {
    cond: Box<Ast>,
    then_expr: Box<Ast>,
    else_expr: Option<Box<Ast>>,
  },
  HeapAlloc {
    class: String,
    expr: Box<Ast>
  },
  Match {
    scrutinee: Box<Ast>,
    branches: Vec<Ast>,
    else_expr: Option<Box<Ast>>,
  },
  MatchBranch {
    match_guard: Option<Box<Ast>>,
    expr: Box<Ast>,
    body: Box<Ast>,
  },
  Array(Vec<Ast>),
  Index {
    array: Box<Ast>,
    index: Box<Ast>
  },
  FieldAccess {
    object: Box<Ast>,
    field: String
  },
  Assign {
    target: Box<Ast>,
    value: Box<Ast>
  },
  MethodCall {
    object: Box<Ast>,
    method: String,
    args: Vec<Ast>
  }
}

#[derive(Debug, Clone)]
pub struct MatchBranch {
  pub match_guard: Option<Box<Ast>>,
  pub expr: Box<Ast>,
}

#[derive(Debug, Clone)]
pub struct TypedAst {
  pub ty: Type,
  pub node: AstKind,
}

pub type Ast = (SimpleSpan, TypedAst);
