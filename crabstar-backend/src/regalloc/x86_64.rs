use crate::ir::graph::Instr;
use crate::regalloc::constraints::{InstrConstraints, RegConstraint};
use crate::regalloc::regalloc::RegSet;
use std::fmt;
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PhysReg {
  Rax,
  Rcx,
  Rdx,
  Rsi,
  Rdi,
  Rsp,
  R8,
  R9,
  R10,
  R11,
  Rbx,
  Rbp,
  R12,
  R13,
  R14,
  R15,
}
impl PhysReg {
  pub fn is_extended(self) -> bool {
    matches!(
      self,
      PhysReg::R8
        | PhysReg::R9
        | PhysReg::R10
        | PhysReg::R11
        | PhysReg::R12
        | PhysReg::R13
        | PhysReg::R14
        | PhysReg::R15
    )
  }
}
impl fmt::Display for PhysReg {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    match self {
      PhysReg::Rax => write!(f, "rax"),
      PhysReg::Rcx => write!(f, "rcx"),
      PhysReg::Rdx => write!(f, "rdx"),
      PhysReg::Rsi => write!(f, "rsi"),
      PhysReg::Rdi => write!(f, "rdi"),
      PhysReg::Rsp => write!(f, "rsp"),
      PhysReg::R8 => write!(f, "r8"),
      PhysReg::R9 => write!(f, "r9"),
      PhysReg::R10 => write!(f, "r10"),
      PhysReg::R11 => write!(f, "r11"),
      PhysReg::Rbx => write!(f, "rbx"),
      PhysReg::Rbp => write!(f, "rbp"),
      PhysReg::R12 => write!(f, "r12"),
      PhysReg::R13 => write!(f, "r13"),
      PhysReg::R14 => write!(f, "r14"),
      PhysReg::R15 => write!(f, "r15"),
    }
  }
}
const SYSV_CALLER_SAVED: &[PhysReg] = &[
  PhysReg::Rax,
  PhysReg::Rcx,
  PhysReg::Rdx,
  PhysReg::Rsi,
  PhysReg::Rdi,
  PhysReg::R8,
  PhysReg::R9,
  PhysReg::R10,
  PhysReg::R11,
];
const SYSV_CALLEE_SAVED: &[PhysReg] = &[
  PhysReg::Rbx,
  PhysReg::Rbp,
  PhysReg::R12,
  PhysReg::R13,
  PhysReg::R14,
  PhysReg::R15,
];
const WIN64_CALLER_SAVED: &[PhysReg] = &[
  PhysReg::Rax,
  PhysReg::Rcx,
  PhysReg::Rdx,
  PhysReg::R8,
  PhysReg::R9,
  PhysReg::R10,
  PhysReg::R11,
];
const WIN64_CALLEE_SAVED: &[PhysReg] = &[
  PhysReg::Rbx,
  PhysReg::Rbp,
  PhysReg::Rsi,
  PhysReg::Rdi,
  PhysReg::R12,
  PhysReg::R13,
  PhysReg::R14,
  PhysReg::R15,
];
fn binary_unconstrained() -> InstrConstraints<PhysReg> {
  InstrConstraints::unconstrained(2)
}
fn mul_constraints() -> InstrConstraints<PhysReg> {
  InstrConstraints {
    operand_constraints: vec![RegConstraint::Any, RegConstraint::Any],
    def_constraint: RegConstraint::SameAsOperand(0),
    clobbers: vec![],
  }
}
fn div_constraints() -> InstrConstraints<PhysReg> {
  InstrConstraints {
    operand_constraints: vec![RegConstraint::Fixed(PhysReg::Rax), RegConstraint::Any],
    def_constraint: RegConstraint::Fixed(PhysReg::Rax),
    clobbers: vec![PhysReg::Rdx],
  }
}
fn x64_constraints(instr: &Instr) -> InstrConstraints<PhysReg> {
  match instr {
    Instr::Mul(_, _) => mul_constraints(),
    Instr::Div(_, _) => div_constraints(),
    Instr::Add(_, _)
    | Instr::Sub(_, _)
    | Instr::Eq(_, _)
    | Instr::Ne(_, _)
    | Instr::Lt(_, _)
    | Instr::Le(_, _)
    | Instr::Gt(_, _)
    | Instr::Ge(_, _) => binary_unconstrained(),
    Instr::Not(_) | Instr::Neg(_) => InstrConstraints::unconstrained(1),
    Instr::Const(_) => InstrConstraints::unconstrained(0),
    Instr::Call(_, args) => InstrConstraints::unconstrained(args.len()),
  }
}
pub struct SysV;
pub struct Win64;
impl RegSet for SysV {
  type Reg = PhysReg;
  fn caller_saved() -> &'static [PhysReg] {
    SYSV_CALLER_SAVED
  }
  fn callee_saved() -> &'static [PhysReg] {
    SYSV_CALLEE_SAVED
  }
  fn return_reg() -> PhysReg {
    PhysReg::Rax
  }
  fn constraints(instr: &Instr) -> InstrConstraints<PhysReg> {
    x64_constraints(instr)
  }
}
impl RegSet for Win64 {
  type Reg = PhysReg;
  fn caller_saved() -> &'static [PhysReg] {
    WIN64_CALLER_SAVED
  }
  fn callee_saved() -> &'static [PhysReg] {
    WIN64_CALLEE_SAVED
  }
  fn return_reg() -> PhysReg {
    PhysReg::Rax
  }
  fn constraints(instr: &Instr) -> InstrConstraints<PhysReg> {
    x64_constraints(instr)
  }
}
