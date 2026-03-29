pub mod sysv;
pub mod win64;
pub mod x86_64;

use crate::abi::types::{CallingConvention, FfiCif};
use crate::ir::graph::{Cfg, Instr, Operand, Terminator, Val};
use crate::regalloc::constraints::RegConstraint;
use crate::regalloc::regalloc::{AllocState, RegSet, instr_operands, terminator_args};
use object::write::{Object, Relocation, StandardSection, Symbol, SymbolSection};
use object::{
  Architecture, BinaryFormat, Endianness, RelocationEncoding, RelocationFlags, RelocationKind,
  SymbolFlags, SymbolKind, SymbolScope,
};
use std::collections::HashMap;

pub trait Codegen: RegSet + CallingConvention<PhysReg = <Self as RegSet>::Reg> {
  fn emit_instr(buf: &mut Vec<u8>, def: Val, instr: &Instr, state: &AllocState<Self>)
  where
    Self: Sized;
  fn emit_ret(buf: &mut Vec<u8>, reg: Self::Reg);
  fn emit_mov(buf: &mut Vec<u8>, dst: Self::Reg, src: Self::Reg);
  fn emit_terminator(
    buf: &mut Vec<u8>,
    terminator: &Terminator,
    state: &AllocState<Self>,
    jump_relocs: &mut Vec<(usize, u32)>,
  ) where
    Self: Sized;
  fn binary_format() -> BinaryFormat;
  fn architecture() -> Architecture;
  fn endianness() -> Endianness;
}

pub fn generate_code<C: Codegen>(cfg: &Cfg, cif: &FfiCif<C>, fn_name: &str) -> Vec<u8> {
  let mut buf: Vec<u8> = Vec::new();
  let mut block_param_regs: HashMap<u32, Vec<C::Reg>> = HashMap::new();
  let mut block_offsets: HashMap<u32, usize> = HashMap::new();
  let mut jump_relocs: Vec<(usize, u32)> = Vec::new();

  for (block_id, block) in cfg.blocks.iter().enumerate() {
    let block_id = block_id as u32;
    block_offsets.insert(block_id, buf.len());

    let mut state = AllocState::<C>::new();

    if block_id == 0 {
      for (i, param) in block.params.iter().enumerate() {
        if let Some(reg) = C::arg_reg(&cif.cc_data, i) {
          let (reg, evict) = state.alloc_fixed(param.val, reg, &[]);
          if let Some((_, new_reg)) = evict {
            C::emit_mov(&mut buf, new_reg, reg);
          }
        } else {
          state.alloc_any(param.val);
        }
      }
    } else if let Some(regs) = block_param_regs.get(&block_id) {
      for (param, reg) in block.params.iter().zip(regs.iter()) {
        state.assignments.insert(param.val, *reg);
        state.free.retain(|r| r != reg);
      }
    } else {
      for param in &block.params {
        state.alloc_any(param.val);
      }
    }

    let term_args = terminator_args(&block.terminator);

    for (def, instr) in &block.instr {
      let operands = instr_operands(instr);
      let constraints = C::constraints(instr);

      let term_vals: Vec<Val> = term_args
        .iter()
        .filter_map(|op| match op {
          Operand::Val(v) => Some(*v),
          _ => None,
        })
        .collect();

      let operand_vals: Vec<Val> = operands
        .iter()
        .filter_map(|op| match op {
          Operand::Val(v) => Some(*v),
          _ => None,
        })
        .collect();

      let mut live_for_clobber = term_vals.clone();
      live_for_clobber.extend(operand_vals.iter());

      let mut live_for_alloc = term_vals.clone();
      live_for_alloc.extend(operand_vals.iter());

      for (i, op) in operands.iter().enumerate() {
        let v = match op {
          Operand::Val(v) => v,
          _ => continue,
        };
        if let Some(RegConstraint::Fixed(r)) = constraints.operand_constraints.get(i) {
          let current = state.reg_of(*v);
          if current != *r {
            let (_, evict) = state.alloc_fixed(*v, *r, &live_for_alloc);
            if let Some((_, new_reg)) = evict {
              C::emit_mov(&mut buf, new_reg, *r);
            }
            C::emit_mov(&mut buf, *r, current);
          }
        }
      }
      for clobber in &constraints.clobbers {
        let clobbered_val = state
          .assignments
          .iter()
          .find(|(_, r)| **r == *clobber)
          .map(|(v, _)| *v);

        if let Some(v) = clobbered_val {
          if operand_vals.contains(&v) {
            continue;
          }
        }

        if let Some((_, new_reg)) = state.clobber(*clobber, &term_vals) {
          C::emit_mov(&mut buf, new_reg, *clobber);
        }
      }

      let _ = match &constraints.def_constraint {
        RegConstraint::Fixed(r) => {
          let current = state.assignments.get(def).copied();
          if current == Some(*r) {
            *r
          } else {
            let (reg, evict) = state.alloc_fixed(*def, *r, &term_vals);
            if let Some((_, new_reg)) = evict {
              C::emit_mov(&mut buf, new_reg, *r);
            }
            reg
          }
        }
        RegConstraint::SameAsOperand(i) => {
          let reg = match operands.get(*i) {
            Some(Operand::Val(v)) => state.reg_of(*v),
            _ => state.alloc_any(*def),
          };
          state.assignments.insert(*def, reg);
          reg
        }
        RegConstraint::Any => {
          let is_ret = matches!(
              &block.terminator,
              Terminator::Return(Some(Operand::Val(v))) if v == def
          );
          if is_ret {
            let (reg, evict) = state.alloc_fixed(*def, C::return_reg(), &term_vals);
            if let Some((_, new_reg)) = evict {
              C::emit_mov(&mut buf, new_reg, C::return_reg());
            }
            reg
          } else {
            state.alloc_any(*def)
          }
        }
      };

      C::emit_instr(&mut buf, *def, instr, &state);

      for op in &operands {
        if let Operand::Val(v) = op {
          state.free_if_dead(*v, &term_args);
        }
      }

      for op in &term_args {
        if let Operand::Val(v) = op {
          state.free_if_dead(*v, &term_args);
        }
      }
    }

    match &block.terminator {
      Terminator::Jump(j) => {
        let next_regs = j
          .args
          .iter()
          .map(|op| match op {
            Operand::Val(v) => state.reg_of(*v),
            _ => C::caller_saved()[0],
          })
          .collect();
        if !block_param_regs.contains_key(&j.target) {
          block_param_regs.insert(j.target, next_regs);
        }
      }
      Terminator::CondJump {
        then_jump,
        else_jump,
        ..
      } => {
        if !block_param_regs.contains_key(&then_jump.target) {
          let regs = then_jump
            .args
            .iter()
            .map(|op| match op {
              Operand::Val(v) => state.reg_of(*v),
              _ => C::caller_saved()[0],
            })
            .collect();
          block_param_regs.insert(then_jump.target, regs);
        }
        if !block_param_regs.contains_key(&else_jump.target) {
          let regs = else_jump
            .args
            .iter()
            .map(|op| match op {
              Operand::Val(v) => state.reg_of(*v),
              _ => C::caller_saved()[0],
            })
            .collect();
          block_param_regs.insert(else_jump.target, regs);
        }
      }
      Terminator::Return(_) => {}
    }

    let is_fallthrough =
      matches!(&block.terminator, Terminator::Jump(j) if j.target == block_id + 1);
    if !is_fallthrough {
      C::emit_terminator(&mut buf, &block.terminator, &state, &mut jump_relocs);
    }
  }

  let mut obj = Object::new(C::binary_format(), C::architecture(), C::endianness());
  let section = obj.section_id(StandardSection::Text);
  let offset = obj.append_section_data(section, &buf, 4);

  obj.add_symbol(Symbol {
    name: fn_name.as_bytes().to_vec(),
    value: offset,
    size: buf.len() as u64,
    kind: SymbolKind::Text,
    scope: SymbolScope::Linkage,
    weak: false,
    section: SymbolSection::Section(section),
    flags: SymbolFlags::None,
  });

  let targeted_blocks: std::collections::HashSet<u32> =
    jump_relocs.iter().map(|(_, t)| *t).collect();
  let mut block_sym_ids: HashMap<u32, object::write::SymbolId> = HashMap::new();
  for (block_id, block_offset) in &block_offsets {
    if !targeted_blocks.contains(block_id) {
      continue;
    }
    let block_sym = format!("{fn_name}__block_{block_id}");
    let sym_id = obj.add_symbol(Symbol {
      name: block_sym.as_bytes().to_vec(),
      value: offset + *block_offset as u64,
      size: 0,
      kind: SymbolKind::Label,
      scope: SymbolScope::Compilation,
      weak: false,
      section: SymbolSection::Section(section),
      flags: SymbolFlags::None,
    });
    block_sym_ids.insert(*block_id, sym_id);
  }

  for (reloc_offset, target_block) in &jump_relocs {
    let sym_id = block_sym_ids[target_block];
    obj
      .add_relocation(
        section,
        Relocation {
          offset: offset + *reloc_offset as u64,
          symbol: sym_id,
          addend: -4,
          flags: RelocationFlags::Generic {
            kind: RelocationKind::Relative,
            encoding: RelocationEncoding::Generic,
            size: 32,
          },
        },
      )
      .unwrap();
  }

  obj.write().expect("failed to write object")
}
