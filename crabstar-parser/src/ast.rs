#[derive(Debug, Clone, Default)]
pub enum Ast {
  // For when an input is invalid, but still needs an ast node
  #[default]
  Dummy,
  // Probably wondering why an integer is using an unsized integer
  // This is because a parsd integer cannot be negative, so it would be more beneficial to handle a larger range and leave it for type checking
  Int(u64),
  // 64 bit floats
  Float(f64),
  // Boolean
  Bool(bool),
  // Identifier, or a symbol, e.g foo, bar, baz
  Ident(String),
  // Operator with a single operand, e.g -x, not true
  Unary(String, Box<Self>),
  // Operator with two operands, e.g 5 + 3, 6 or 7, 12 and 16
  Binary(String, Box<Self>, Box<Self>),
  // A list of ast nodes
  Block(Vec<Self>),
  // Defining a named variable/function, e.g
  // let x: 12 + 7 in: x + 11
  // let fn :: (a, b) (
  //   a - b * 3
  // )
  // lazy evaluation, aka a function with no args 
  // let lazy => 12
  Let {
    name: String,
    args: Option<Vec<Self>>,
    value: Box<Self>,
    next: Option<Box<Self>>
  },
  // function calls
  // foo(), bar(), baz()
  Call {
    callee: Box<Self>, 
    args: Vec<Self>
  },
  // conditional branching,
  // if true: 
  //   12 
  // elif 12 + 11 < 25 (
  //   car(), 
  //   12 + truck()
  //  ) else: 
  //   16
  If {
    cond: Box<Self>,
    then_expr: Box<Self>,
    else_expr: Option<Box<Self>>,
  },
}
