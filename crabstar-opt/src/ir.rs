use crabstar_frontend::typechecker::Type;
use std::collections::HashMap;
use generational_arena::Arena;
use crate::pass::{InstrEdit, OptPass};

pub type Temp = generational_arena::Index;
pub type InstrId = usize;

#[derive(Debug)]
pub enum Expr {
  Int(u64),
  Float(f64),
  Bool(bool),
  HeapAlloc(Temp),
  Get(Temp),
  GetParam(String),
  GetGlobal(String),
  Block(Vec<Temp>),
  Call {
    callee: String,
    args: Vec<Temp>
  },
  Cast {
    from: Type,
    to: Type
  },
  Closure {
    instrs: Vec<Self>
  },
  Select {
    index: Temp,
    // Practically, it should always be a Closure
    choices: Vec<Temp>
  },
} 

#[derive(Debug)]
pub struct Instr {
  pub ty: Type,
  pub expr: Box<Expr>
}

impl Instr {
  pub fn new(ty: Type, expr: Box<Expr>) -> Self {
    Self { ty, expr }
  }
}


#[derive(Debug, Clone)]
pub struct Function {
  pub params: Vec<(String, Type)>,
  pub return_type: Type,
  pub body: Vec<Temp>,
}

#[derive(Debug, Clone)]
pub struct Global {
  pub ty: Type,
  pub value: Temp,
}

#[derive(Debug)]
pub struct IrModule {
  pub functions: HashMap<String, Function>,
  pub globals: HashMap<String, Global>,
  pub instrs: Arena<Instr>,
}

impl IrModule {
  pub fn run_pass(&mut self, mut pass: Box<dyn OptPass>) {
    let edits = pass.run(&self);
    for edit in edits {
      match edit {
        InstrEdit::RemoveInstr { func, id } => {
          let func = self.functions.get_mut(&func).expect("Function: {func} does not exist");
          func.body.remove(id);
        },
        InstrEdit::Insert { func, id, new } => {
          let func = self.functions.get_mut(&func).expect("Function: {func} does not exist");
          let new = self.instrs.insert(new);
          func.body.insert(id, new);
        },
        InstrEdit::ReplaceExpr { func, id, new } => {
          let func = self.functions.get_mut(&func).expect(format!("Function: {func} does not exist").as_str());
          let index = func.body.get_mut(id).expect("Index: {id} is out of bounds");
          let instr = self.instrs.get_mut(*index).expect("Index: {index} is out of bounds");
          instr.expr = Box::new(new);
        }
      };
    }
  }
}
