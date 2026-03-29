use crate::ir::graph::{Instr, Operand, Terminator, Val};
use crate::regalloc::constraints::InstrConstraints;
use std::collections::HashMap;
use std::fmt;

pub trait RegSet {
  type Reg: Copy + Eq + fmt::Display + fmt::Debug + std::hash::Hash + 'static;
  fn caller_saved() -> &'static [Self::Reg];
  fn callee_saved() -> &'static [Self::Reg];
  fn return_reg() -> Self::Reg;
  fn constraints(instr: &Instr) -> InstrConstraints<Self::Reg>;
}

pub struct AllocState<R: RegSet> {
  pub assignments: HashMap<Val, R::Reg>,
  pub free: Vec<R::Reg>,
}

impl<R: RegSet> AllocState<R> {
  pub fn new() -> Self {
    Self {
      assignments: HashMap::new(),
      free: R::caller_saved().iter().rev().cloned().collect(),
    }
  }

  pub fn alloc_any(&mut self, val: Val) -> R::Reg {
    if let Some(reg) = self.free.pop() {
      self.assignments.insert(val, reg);
      reg
    } else {
      panic!("spill needed for {:?} — todo", val);
    }
  }

  fn is_live(&self, val: Val, live_set: &[Val]) -> bool {
    live_set.contains(&val)
  }

  pub fn alloc_fixed(
    &mut self,
    val: Val,
    reg: R::Reg,
    live_set: &[Val],
  ) -> (R::Reg, Option<(Val, R::Reg)>) {
    if self.assignments.get(&val) == Some(&reg) {
      return (reg, None);
    }

    if self.free.contains(&reg) {
      self.free.retain(|r| *r != reg);
      self.assignments.insert(val, reg);
      return (reg, None);
    }

    let evicted_val = self
      .assignments
      .iter()
      .find(|(_, r)| **r == reg)
      .map(|(v, _)| *v);

    if let Some(evicted) = evicted_val {
      self.assignments.remove(&evicted);
      self.assignments.insert(val, reg);

      if self.is_live(evicted, live_set) {
        let new_reg = self.free.pop().expect("spill needed during eviction");
        self.assignments.insert(evicted, new_reg);
        (reg, Some((evicted, new_reg)))
      } else {
        (reg, None)
      }
    } else {
      self.assignments.insert(val, reg);
      (reg, None)
    }
  }

  pub fn clobber(&mut self, reg: R::Reg, live_set: &[Val]) -> Option<(Val, R::Reg)> {
    let evicted_val = self
      .assignments
      .iter()
      .find(|(_, r)| **r == reg)
      .map(|(v, _)| *v);

    if let Some(evicted) = evicted_val {
      self.assignments.remove(&evicted);
      self.free.retain(|r| *r != reg);

      if self.is_live(evicted, live_set) {
        let new_reg = self.free.pop().expect("spill not implemented");
        self.assignments.insert(evicted, new_reg);
        Some((evicted, new_reg))
      } else {
        None
      }
    } else {
      self.free.retain(|r| *r != reg);
      None
    }
  }

  pub fn free_reg(&mut self, reg: R::Reg) {
    self.free.push(reg);
  }

  pub fn free_if_dead(&mut self, val: Val, live: &[Operand]) {
    let is_live = live
      .iter()
      .any(|op| matches!(op, Operand::Val(v) if *v == val));
    if !is_live {
      if let Some(reg) = self.assignments.remove(&val) {
        self.free.push(reg);
        self.free.sort_by_key(|r| {
          R::caller_saved()
            .iter()
            .rev()
            .position(|x| x == r)
            .unwrap_or(usize::MAX)
        });
      }
    }
  }

  pub fn reg_of(&self, val: Val) -> R::Reg {
    *self
      .assignments
      .get(&val)
      .unwrap_or_else(|| panic!("no register for {:?}", val))
  }
}

pub fn terminator_args(term: &Terminator) -> Vec<Operand> {
  match term {
    Terminator::Jump(j) => j.args.clone(),
    Terminator::CondJump {
      cond,
      then_jump,
      else_jump,
    } => {
      let mut args = vec![cond.clone()];
      args.extend(then_jump.args.iter().cloned());
      args.extend(else_jump.args.iter().cloned());
      args
    }
    Terminator::Return(Some(op)) => vec![op.clone()],
    Terminator::Return(None) => vec![],
  }
}

pub fn instr_operands(instr: &Instr) -> Vec<Operand> {
  match instr {
    Instr::IAdd(_, a, b)
    | Instr::ISub(_, a, b)
    | Instr::IMul(_, a, b)
    | Instr::IDiv(_, a, b)
    | Instr::IEq(_, a, b)
    | Instr::INe(_, a, b)
    | Instr::ILt(_, a, b)
    | Instr::ILe(_, a, b)
    | Instr::IGt(_, a, b)
    | Instr::IGe(_, a, b)
    | Instr::IShl(_, a, b)
    | Instr::IShr(_, a, b)
    | Instr::FAdd(_, a, b)
    | Instr::FSub(_, a, b)
    | Instr::FMul(_, a, b)
    | Instr::FDiv(_, a, b)
    | Instr::FEq(_, a, b)
    | Instr::FNe(_, a, b)
    | Instr::FLt(_, a, b)
    | Instr::FLe(_, a, b)
    | Instr::FGt(_, a, b)
    | Instr::FGe(_, a, b) => vec![a.clone(), b.clone()],
    Instr::INot(_, a) | Instr::INeg(_, a) | Instr::FNeg(_, a) => vec![a.clone()],
    Instr::Load { ptr, .. } => vec![ptr.clone()],
    Instr::Store { ptr, value, .. } => vec![ptr.clone(), value.clone()],
    Instr::FieldPtr { base, .. } => vec![base.clone()],
    Instr::Call(_, args) => args.clone(),
    Instr::IConst(_, _) | Instr::FConst(_, _) | Instr::StackAlloc { .. } => vec![],
  }
}
