use std::collections::HashMap;

use crate::{ir::{IrModule, Temp}, pass::{InstrEdit, OptPass}};

pub struct Lifetime {
  end: usize,
}

pub struct LifetimeAnalyzer {
  captures: HashMap<Temp, Vec<Lifetime>>,
}

impl LifetimeAnalyzer {
  pub fn new() -> Box<Self> {
    Box::new(Self { captures: HashMap::new() })
  }

  fn build_lifetimes(&mut self, ir: &IrModule) {
    println!("Doing shit");
    for (_, func) in &ir.functions {
      println!("Running pass in func: {:?}", func) 
    } 
  }
}

impl OptPass for LifetimeAnalyzer {
  fn run(&mut self, ir: &IrModule) -> Vec<InstrEdit> {
    self.build_lifetimes(ir);
    vec![]
  }

}
