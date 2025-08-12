use crate::ast::Ast;

#[derive(Debug,Clone)]
pub enum Type {
  Int,
  Float,
  Bool,
  Function {
    params: Vec<Self>,
    ret_type: Box<Self>
  },
  Unknown,
  Union(Vec<Self>),
  Null,
  Heap(Box<Self>)
}

pub struct TypeChecker {
  ast: Vec<Ast>,
}

impl TypeChecker {
  // returns if type checking succeded
  // if not, then it's up the caller to decide what to do
  pub fn type_check(&mut self) -> bool {
    
    true
  }
}
