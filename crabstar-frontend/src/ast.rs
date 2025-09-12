use chumsky::prelude::SimpleSpan;
use crate::types::{Constraint, Type};

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
    next: Option<Box<Ast>>,
    constraints: Vec<Constraint>
  },
  Const {
    name: String,
    recursive: bool,
    args: Option<Vec<Ast>>,
    ret_type: Option<Box<Ast>>,
    value: Box<Ast>,
    constraints: Vec<Constraint>
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
pub struct TypedAst {
  pub ty: Type,
  pub node: AstKind,
}

pub type Ast = (SimpleSpan, TypedAst);

pub fn traverse_ast<F: FnMut(&mut Ast)>(ast: &mut Ast, f: &mut F)  {
  f(ast);

  match &mut ast.1.node {
    AstKind::Unary(_, expr) => traverse_ast(expr, f),
    AstKind::Binary(_, left, right) => {
      traverse_ast(left, f);
      traverse_ast(right, f);
    },
    AstKind::Block(exprs) | AstKind::Array(exprs) => {
      for expr in exprs { traverse_ast(expr, f); }
    },
    AstKind::Let { value, next, args, ret_type, .. } => {
      traverse_ast(value, f);
      if let Some(next) = next { traverse_ast(next, f) }
      if let Some(ret_type) = ret_type { traverse_ast(ret_type, f) }
      if let Some(args) = args {
        for arg in args { traverse_ast(arg, f) }
      }
    },
    AstKind::Const { value, args, ret_type, .. } => {
        traverse_ast(value, f);
        if let Some(ret_type) = ret_type { traverse_ast(ret_type, f) }
        if let Some(args) = args {
          for arg in args { traverse_ast(arg, f) }
        }
    }
    AstKind::Call { callee, args } => {
      traverse_ast(callee, f);
      for arg in args { traverse_ast(arg, f) }
    },
    AstKind::If { cond, then_expr, else_expr } => {
      traverse_ast(cond, f);
      traverse_ast(then_expr, f);
      if let Some(else_expr) = else_expr { traverse_ast(else_expr, f) }
    },
    AstKind::HeapAlloc { expr, .. } => traverse_ast(expr, f),
    AstKind::Match { scrutinee, branches, else_expr } => {
      traverse_ast(scrutinee, f);
      for branch in branches { traverse_ast(branch, f); }
      if let Some(else_expr) = else_expr { traverse_ast(else_expr, f) }
    },
    AstKind::MatchBranch { match_guard, expr, body } => {
      if let Some(guard) = match_guard { traverse_ast(guard, f) }
      traverse_ast(expr, f);
      traverse_ast(body, f);
    },
    AstKind::Index { array, index } => {
      traverse_ast(array, f);
      traverse_ast(index, f);
    },
    AstKind::FieldAccess { object, .. } | AstKind::Assign { target: object, value: _ } | AstKind::MethodCall { object, args: _ , .. } => {
      traverse_ast(object, f);
    },
    AstKind::Int(_) | AstKind::Float(_) | AstKind::Bool(_) | AstKind::String(_) | AstKind::Ident(_) | AstKind::Dummy => {}
  }
}
