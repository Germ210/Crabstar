use crate::ir::{Expr, Instr, IrModule, InstrId};

pub enum InstrEdit {
  ReplaceExpr {
    func: String,
    id: InstrId,
    new: Expr,
  },
  RemoveInstr {
    func: String,
    id: InstrId,
  },
  Insert {
    func: String,
    id: InstrId,
    new: Instr,
  }
}

pub trait OptPass {
  fn run(&mut self, ir_mod: & IrModule) -> Vec<InstrEdit>;
}

