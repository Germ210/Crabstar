use crate::ast::Ast;

#[derive(Debug,Clone)]
pub enum Type {
  Int,
  Float,
  Bool,
  String,
  Function {
    params: Vec<Self>,
    ret_type: Box<Self>
  },
  Unknown,
  Union(Vec<Self>),
  Null,
  Heap(Box<Self>),
  Array(Box<Self>)
}

pub struct TypeChecker {
  ast: Vec<Ast>,
}

impl TypeChecker {
 
}
