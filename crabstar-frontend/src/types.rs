#[derive(Debug, Clone, PartialEq)]
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
  Array(Box<Self>),
  TypeVar(u64),
  Realloc(Box<Self>),
  Mut(Box<Self>),
}

#[derive(Debug, Clone)]
pub enum Constraint {
  TypesAreEqual(Type, Type),
  IsOverload(Vec<Type>, Type)
}
