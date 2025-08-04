#[derive(Debug, Clone)]
pub enum Ast {
  Dummy,
  Num(f64),
  Bool(bool),
  Ident(String),
  Unary(String, Box<Ast>),
  Binary(String, Box<Ast>, Box<Ast>),
  Block(Vec<Ast>),
  Let {
    name: String,
    value: Box<Ast>,
  }
}
