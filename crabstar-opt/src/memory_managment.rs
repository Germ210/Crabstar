use std::{collections::HashMap, vec};
use crabstar_frontend::types::Type;
use crate::{ir::{Expr, Instr, IrModule, Temp}, pass::{InstrEdit, OptPass}};

#[derive(Debug)]
pub struct Lifetime {
  func_name: String,
  end: usize,
}

impl Lifetime {
  pub fn new(func_name: String, end: usize) -> Self {
    Self { func_name, end }
  }
}

#[derive(Debug)]
pub struct LifetimeAnalyzer {
  captures: HashMap<Temp, Vec<Lifetime>>,
}

impl LifetimeAnalyzer {
  pub fn new() -> Box<Self> {
    Box::new(Self { captures: HashMap::new() })
  }

  fn build_lexical_lifetimes(&mut self, ir_mod: &IrModule) {
    for (func_name, func) in &ir_mod.functions {
      for (i, instr) in func.body.iter().rev().enumerate() {
        match ir_mod.instrs.get(*instr).expect("Index: {instr} is out of bounds").ty {
          Type::Heap(_) => {
            self.captures.insert(*instr, vec![Lifetime::new(func_name.clone(), func.body.len() - i - 1)]);
          }
          _ => ()
        }
      }
    }
    println!("{:#?}\n", self);
  }

  fn build_capture_lifetimes(&mut self, ir_mod: &IrModule) {
    for (func_name, func) in &ir_mod.functions {
      for (i, instr) in func.body.iter().enumerate() {
        if self.is_capture(ir_mod, instr) {
          self.captures.get_mut(instr).unwrap().push(Lifetime::new(func_name.clone(), i));
        }
      }
    }
  }

  fn is_capture(&self, ir_mod: &IrModule, instr: &Temp) -> bool {
    false
  }
}

impl OptPass for LifetimeAnalyzer {
  fn run(&mut self, ir_mod: &IrModule) -> Vec<InstrEdit> {
    self.build_lexical_lifetimes(ir_mod);
    self.build_capture_lifetimes(ir_mod);
    vec![]
  }
}
