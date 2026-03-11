use crate::abi::types::AbiType;
use crate::ir::graph::{Block, Cfg, Instr, Jump, Operand, Param, Terminator, Val};
use std::collections::HashSet;

pub struct FunctionBuilder {
  next_val: u32,
  next_block: u32,
  blocks: Vec<BlockData>,
  current_block: u32,
  merge_target: Option<u32>,
  defined_vals: HashSet<Val>,
  used_outer_vals: Option<HashSet<Val>>,
}

struct BlockData {
  params: Vec<Param>,
  instr: Option<(Val, Instr)>,
  terminator: Option<Terminator>,
}

impl BlockData {
  fn new(params: Vec<Param>) -> Self {
    Self {
      params,
      instr: None,
      terminator: None,
    }
  }
}

fn collect_operand_vals(op: &Operand, out: &mut HashSet<Val>) {
  if let Operand::Val(v) = op {
    out.insert(*v);
  }
}

fn collect_instr_vals(instr: &Instr, out: &mut HashSet<Val>) {
  match instr {
    Instr::Add(a, b)
    | Instr::Sub(a, b)
    | Instr::Mul(a, b)
    | Instr::Div(a, b)
    | Instr::Eq(a, b)
    | Instr::Ne(a, b)
    | Instr::Lt(a, b)
    | Instr::Le(a, b)
    | Instr::Gt(a, b)
    | Instr::Ge(a, b) => {
      collect_operand_vals(a, out);
      collect_operand_vals(b, out);
    }
    Instr::Not(a) | Instr::Neg(a) => collect_operand_vals(a, out),
    Instr::Call(_, args) => args.iter().for_each(|a| collect_operand_vals(a, out)),
    Instr::Const(_) => {}
  }
}

impl FunctionBuilder {
  pub fn new(param_types: &[AbiType]) -> (Self, Vec<Operand>) {
    let mut next_val = 0u32;
    let params: Vec<Param> = param_types
      .iter()
      .map(|ty| {
        let v = Val(next_val);
        next_val += 1;
        Param {
          val: v,
          ty: ty.clone(),
        }
      })
      .collect();
    let operands = params.iter().map(|p| Operand::Val(p.val)).collect();
    let mut defined_vals = HashSet::new();
    for p in &params {
      defined_vals.insert(p.val);
    }
    let entry = BlockData::new(params);
    let b = Self {
      next_val,
      next_block: 1,
      blocks: vec![entry],
      current_block: 0,
      merge_target: None,
      defined_vals,
      used_outer_vals: None,
    };
    (b, operands)
  }

  fn fresh_val(&mut self) -> Val {
    let v = Val(self.next_val);
    self.next_val += 1;
    v
  }

  fn fresh_block(&mut self, params: Vec<Param>) -> u32 {
    let id = self.next_block;
    self.next_block += 1;
    self.blocks.push(BlockData::new(params));
    id
  }

  fn track_instr_uses(&mut self, instr: &Instr) {
    if let Some(ref mut used) = self.used_outer_vals {
      let mut refs = HashSet::new();
      collect_instr_vals(instr, &mut refs);
      for v in refs {
        if self.defined_vals.contains(&v) {
          used.insert(v);
        }
      }
    }
  }

  fn emit_instr(&mut self, instr: Instr) -> Operand {
    self.track_instr_uses(&instr);
    let def = self.fresh_val();
    if let Some(merge) = self.merge_target.take() {
      let current = &mut self.blocks[self.current_block as usize];
      current.instr = Some((def, instr));
      current.terminator = Some(Terminator::Jump(Jump {
        target: merge,
        args: vec![Operand::Val(def)],
      }));
    } else {
      let mut live_vals: Vec<Val> = self.defined_vals.iter().copied().collect();
      live_vals.sort_by_key(|v| v.0);

      let next_params: Vec<Param> = std::iter::once(Param {
        val: def,
        ty: AbiType::I64,
      })
      .chain(live_vals.iter().map(|v| Param {
        val: *v,
        ty: AbiType::I64,
      }))
      .collect();

      let next_block = self.fresh_block(next_params);

      let jump_args: Vec<Operand> = std::iter::once(Operand::Val(def))
        .chain(live_vals.iter().map(|v| Operand::Val(*v)))
        .collect();

      let current = &mut self.blocks[self.current_block as usize];
      current.instr = Some((def, instr));
      current.terminator = Some(Terminator::Jump(Jump {
        target: next_block,
        args: jump_args,
      }));
      self.current_block = next_block;

      for v in &live_vals {
        self.defined_vals.insert(*v);
      }
    }
    self.defined_vals.insert(def);
    Operand::Val(def)
  }

  pub fn add(&mut self, a: Operand, b: Operand) -> Operand {
    self.emit_instr(Instr::Add(a, b))
  }
  pub fn sub(&mut self, a: Operand, b: Operand) -> Operand {
    self.emit_instr(Instr::Sub(a, b))
  }
  pub fn mul(&mut self, a: Operand, b: Operand) -> Operand {
    self.emit_instr(Instr::Mul(a, b))
  }
  pub fn div(&mut self, a: Operand, b: Operand) -> Operand {
    self.emit_instr(Instr::Div(a, b))
  }
  pub fn eq(&mut self, a: Operand, b: Operand) -> Operand {
    self.emit_instr(Instr::Eq(a, b))
  }
  pub fn ne(&mut self, a: Operand, b: Operand) -> Operand {
    self.emit_instr(Instr::Ne(a, b))
  }
  pub fn lt(&mut self, a: Operand, b: Operand) -> Operand {
    self.emit_instr(Instr::Lt(a, b))
  }
  pub fn le(&mut self, a: Operand, b: Operand) -> Operand {
    self.emit_instr(Instr::Le(a, b))
  }
  pub fn gt(&mut self, a: Operand, b: Operand) -> Operand {
    self.emit_instr(Instr::Gt(a, b))
  }
  pub fn ge(&mut self, a: Operand, b: Operand) -> Operand {
    self.emit_instr(Instr::Ge(a, b))
  }
  pub fn not(&mut self, a: Operand) -> Operand {
    self.emit_instr(Instr::Not(a))
  }
  pub fn neg(&mut self, a: Operand) -> Operand {
    self.emit_instr(Instr::Neg(a))
  }
  pub fn iconst(&mut self, i: i64) -> Operand {
    self.emit_instr(Instr::Const(i))
  }
  pub fn call(&mut self, name: &str, args: Vec<Operand>) -> Operand {
    self.emit_instr(Instr::Call(name.to_string(), args))
  }

  pub fn if_else(
    &mut self,
    cond: Operand,
    inputs: &[Operand],
    then_fn: impl FnOnce(&mut FunctionBuilder, &[Operand]) -> Operand,
    else_fn: impl FnOnce(&mut FunctionBuilder, &[Operand]) -> Operand,
  ) -> Operand {
    let saved_merge = self.merge_target.take();
    let saved_defined = self.defined_vals.clone();

    let merge_val = self.fresh_val();
    let merge_block = self.fresh_block(vec![Param {
      val: merge_val,
      ty: AbiType::I64,
    }]);

    let input_vals: Vec<Val> = inputs
      .iter()
      .filter_map(|op| match op {
        Operand::Val(v) => Some(*v),
        _ => None,
      })
      .collect();

    let then_params: Vec<Param> = input_vals
      .iter()
      .map(|v| Param {
        val: *v,
        ty: AbiType::I64,
      })
      .collect();

    let else_params: Vec<Param> = input_vals
      .iter()
      .map(|v| Param {
        val: *v,
        ty: AbiType::I64,
      })
      .collect();

    let then_block_id = self.fresh_block(then_params);
    let else_block_id = self.fresh_block(else_params);

    self.blocks[self.current_block as usize].terminator = Some(Terminator::CondJump {
      cond,
      then_jump: Jump {
        target: then_block_id,
        args: inputs.to_vec(),
      },
      else_jump: Jump {
        target: else_block_id,
        args: inputs.to_vec(),
      },
    });

    self.current_block = then_block_id;
    self.defined_vals = saved_defined.clone();
    for v in &input_vals {
      self.defined_vals.insert(*v);
    }
    self.merge_target = Some(merge_block);
    let _then_val = then_fn(self, inputs);

    self.current_block = else_block_id;
    self.defined_vals = saved_defined.clone();
    for v in &input_vals {
      self.defined_vals.insert(*v);
    }
    self.merge_target = Some(merge_block);
    let _else_val = else_fn(self, inputs);

    self.merge_target = saved_merge;
    self.defined_vals = saved_defined;
    self.defined_vals.insert(merge_val);
    self.current_block = merge_block;
    Operand::Val(merge_val)
  }

  pub fn ret(&mut self, val: Operand) {
    let current = &mut self.blocks[self.current_block as usize];
    current.terminator = Some(Terminator::Return(Some(val)));
  }

  pub fn finish(self) -> Cfg {
    Cfg {
      blocks: self
        .blocks
        .into_iter()
        .map(|b| Block {
          params: b.params,
          instr: b.instr,
          terminator: b.terminator.expect("block missing terminator"),
        })
        .collect(),
    }
  }
}
