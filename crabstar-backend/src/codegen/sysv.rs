use crate::codegen::Codegen;
use crate::codegen::x86_64::*;
use crate::ir::graph::{Instr, Operand, Terminator, Val};
use crate::regalloc::regalloc::{AllocState, RegSet};
use crate::regalloc::x86_64::{PhysReg, SysV};
use object::{Architecture, BinaryFormat, Endianness};

fn emit_const(buf: &mut Vec<u8>, dst: PhysReg, imm: i64) {
  emit_movabs(buf, dst, imm);
}

fn resolve(op: &Operand, buf: &mut Vec<u8>, scratch: PhysReg, state: &AllocState<SysV>) -> PhysReg {
  match op {
    Operand::Val(v) => state.reg_of(*v),
    Operand::Imm(i) => {
      emit_const(buf, scratch, *i);
      scratch
    }
  }
}

impl Codegen for SysV {
  fn emit_instr(buf: &mut Vec<u8>, def: Val, instr: &Instr, state: &AllocState<Self>) {
    let dst = state.reg_of(def);
    let s = PhysReg::R11;
    match instr {
      Instr::Const(i) => emit_const(buf, dst, *i),
      Instr::Add(a, b) => {
        let (r0, r1) = (resolve(a, buf, s, state), resolve(b, buf, s, state));
        emit_add(buf, r0, r1, dst);
      }
      Instr::Sub(a, b) => {
        let (r0, r1) = (resolve(a, buf, s, state), resolve(b, buf, s, state));
        emit_sub(buf, r0, r1, dst);
      }
      Instr::Mul(a, b) => {
        let (r0, r1) = (resolve(a, buf, s, state), resolve(b, buf, s, state));
        emit_imul(buf, r0, r1, dst);
      }
      Instr::Div(_, b) => {
        let r1 = resolve(b, buf, s, state);
        if r1 == PhysReg::Rdx {
          emit_mov(buf, PhysReg::Rdx, s);
          emit_idiv(buf, s);
        } else {
          emit_idiv(buf, r1);
        }
      }
      Instr::Eq(a, b) => {
        let (r0, r1) = (resolve(a, buf, s, state), resolve(b, buf, s, state));
        emit_cmp(buf, r0, r1);
        emit_sete(buf, dst);
      }
      Instr::Ne(a, b) => {
        let (r0, r1) = (resolve(a, buf, s, state), resolve(b, buf, s, state));
        emit_cmp(buf, r0, r1);
        emit_setne(buf, dst);
      }
      Instr::Lt(a, b) => {
        let (r0, r1) = (resolve(a, buf, s, state), resolve(b, buf, s, state));
        emit_cmp(buf, r0, r1);
        emit_setl(buf, dst);
      }
      Instr::Le(a, b) => {
        let (r0, r1) = (resolve(a, buf, s, state), resolve(b, buf, s, state));
        emit_cmp(buf, r0, r1);
        emit_setle(buf, dst);
      }
      Instr::Gt(a, b) => {
        let (r0, r1) = (resolve(a, buf, s, state), resolve(b, buf, s, state));
        emit_cmp(buf, r0, r1);
        emit_setg(buf, dst);
      }
      Instr::Ge(a, b) => {
        let (r0, r1) = (resolve(a, buf, s, state), resolve(b, buf, s, state));
        emit_cmp(buf, r0, r1);
        emit_setge(buf, dst);
      }
      Instr::Not(a) => {
        let r0 = resolve(a, buf, s, state);
        emit_not(buf, r0, dst);
      }
      Instr::Neg(a) => {
        let r0 = resolve(a, buf, s, state);
        emit_neg(buf, r0, dst);
      }
      Instr::Call(_, _) => unimplemented!("call"),
    }
  }

  fn emit_ret(buf: &mut Vec<u8>, _: PhysReg) {
    emit_ret(buf);
  }

  fn emit_mov(buf: &mut Vec<u8>, dst: PhysReg, src: PhysReg) {
    emit_mov(buf, src, dst);
  }

  fn emit_terminator(
    buf: &mut Vec<u8>,
    terminator: &Terminator,
    state: &AllocState<Self>,
    jump_relocs: &mut Vec<(usize, u32)>,
  ) {
    match terminator {
      Terminator::Jump(j) => {
        emit_jmp_rel32(buf, 0);
        jump_relocs.push((buf.len() - 4, j.target));
      }
      Terminator::CondJump {
        cond,
        then_jump,
        else_jump,
      } => {
        let cond_reg = match cond {
          Operand::Val(v) => state.reg_of(*v),
          Operand::Imm(_) => unimplemented!(),
        };
        let scratch = PhysReg::R11;
        let zero_reg = resolve(&Operand::Imm(0), buf, scratch, state);
        emit_cmp(buf, cond_reg, zero_reg);
        emit_jne_rel32(buf, 0);
        jump_relocs.push((buf.len() - 4, then_jump.target));
        emit_jmp_rel32(buf, 0);
        jump_relocs.push((buf.len() - 4, else_jump.target));
      }
      Terminator::Return(val) => {
        if let Some(Operand::Val(v)) = val {
          let r = state.reg_of(*v);
          let ret = SysV::return_reg();
          if r != ret {
            emit_mov(buf, r, ret);
          }
        }
        emit_ret(buf);
      }
    }
  }

  fn binary_format() -> BinaryFormat {
    BinaryFormat::Elf
  }
  fn architecture() -> Architecture {
    Architecture::X86_64
  }
  fn endianness() -> Endianness {
    Endianness::Little
  }
}
