use crate::epath::ir::{Block, BlockId, EPath, Expr, ExprId, Path};
use crate::ir::graph::{Cfg, FloatSize, Instr, IntSize, Operand, Terminator, Val};
use hash_cons::Hc;
use std::collections::HashMap;

pub fn from_cfg(cfg: &Cfg) -> EPath {
  let mut epath = EPath::new();
  let mut val_map: HashMap<Val, ExprId> = HashMap::new();
  let mut idx_to_block: HashMap<u32, BlockId> = HashMap::new();

  for (idx, block) in cfg.blocks.iter().enumerate() {
    for (i, param) in block.params.iter().enumerate() {
      let expr = val_map
        .get(&param.val)
        .cloned()
        .unwrap_or_else(|| epath.expr(Expr::Param(i as u32)));
      val_map.insert(param.val, expr);
    }

    let expr = match &block.instr {
      Some((val, instr)) => {
        let e = translate_instr(instr, &val_map, &mut epath);
        val_map.insert(*val, e.clone());
        e
      }
      None => epath.expr(Expr::Param(0)),
    };

    let params: Vec<_> = block
      .params
      .iter()
      .map(|p| val_map[&p.val].clone())
      .collect();

    let b = epath.block(Block { params, expr });
    idx_to_block.insert(idx as u32, b);

    if let Terminator::Jump(j) = &block.terminator {
      let target = &cfg.blocks[j.target as usize];
      for (param, arg) in target.params.iter().zip(j.args.iter()) {
        let arg_expr = translate_operand(arg, &val_map, &mut epath);
        val_map.insert(param.val, arg_expr);
      }
    }
  }

  for (idx, block) in cfg.blocks.iter().enumerate() {
    let term = translate_terminator(&block.terminator, &val_map, &mut epath, &idx_to_block);
    let block_id = idx_to_block[&(idx as u32)].clone();
    epath.block_terminators.insert(block_id, term);
  }

  let mut current_path = vec![];
  dfs_paths(
    &idx_to_block,
    &epath.block_terminators.clone(),
    0,
    &mut current_path,
    &mut epath.paths,
  );

  epath
}

fn translate_operand(op: &Operand, val_map: &HashMap<Val, ExprId>, epath: &mut EPath) -> ExprId {
  match op {
    Operand::Val(v) => val_map[v].clone(),
    Operand::I8(x) => epath.expr(Expr::IConst(IntSize::I8, *x as i64)),
    Operand::I16(x) => epath.expr(Expr::IConst(IntSize::I16, *x as i64)),
    Operand::I32(x) => epath.expr(Expr::IConst(IntSize::I32, *x as i64)),
    Operand::I64(x) => epath.expr(Expr::IConst(IntSize::I64, *x)),
    Operand::U8(x) => epath.expr(Expr::IConst(IntSize::U8, *x as i64)),
    Operand::U16(x) => epath.expr(Expr::IConst(IntSize::U16, *x as i64)),
    Operand::U32(x) => epath.expr(Expr::IConst(IntSize::U32, *x as i64)),
    Operand::U64(x) => epath.expr(Expr::IConst(IntSize::U64, *x as i64)),
    Operand::F32(x) => {
      let mut bytes = [0u8; 8];
      bytes[..4].copy_from_slice(&x.to_bits().to_ne_bytes());
      epath.expr(Expr::FConst(FloatSize::F32, bytes))
    }
    Operand::F64(x) => epath.expr(Expr::FConst(FloatSize::F64, x.to_bits().to_ne_bytes())),
    Operand::Mem(m) => epath.expr(Expr::Param(m.0)),
  }
}

fn translate_instr(instr: &Instr, val_map: &HashMap<Val, ExprId>, epath: &mut EPath) -> ExprId {
  match instr {
    Instr::IConst(sz, x) => epath.expr(Expr::IConst(*sz, *x)),
    Instr::FConst(sz, x) => epath.expr(Expr::FConst(*sz, x.to_bits().to_ne_bytes())),
    Instr::IAdd(sz, a, b) => {
      let a = translate_operand(a, val_map, epath);
      let b = translate_operand(b, val_map, epath);
      epath.expr(Expr::IAdd(*sz, a, b))
    }
    Instr::ISub(sz, a, b) => {
      let a = translate_operand(a, val_map, epath);
      let b = translate_operand(b, val_map, epath);
      epath.expr(Expr::ISub(*sz, a, b))
    }
    Instr::IMul(sz, a, b) => {
      let a = translate_operand(a, val_map, epath);
      let b = translate_operand(b, val_map, epath);
      epath.expr(Expr::IMul(*sz, a, b))
    }
    Instr::IDiv(sz, a, b) => {
      let a = translate_operand(a, val_map, epath);
      let b = translate_operand(b, val_map, epath);
      epath.expr(Expr::IDiv(*sz, a, b))
    }
    Instr::IShl(sz, a, b) => {
      let a = translate_operand(a, val_map, epath);
      let b = translate_operand(b, val_map, epath);
      epath.expr(Expr::IShl(*sz, a, b))
    }
    Instr::IShr(sz, a, b) => {
      let a = translate_operand(a, val_map, epath);
      let b = translate_operand(b, val_map, epath);
      epath.expr(Expr::IShr(*sz, a, b))
    }
    Instr::FAdd(sz, a, b) => {
      let a = translate_operand(a, val_map, epath);
      let b = translate_operand(b, val_map, epath);
      epath.expr(Expr::FAdd(*sz, a, b))
    }
    Instr::FSub(sz, a, b) => {
      let a = translate_operand(a, val_map, epath);
      let b = translate_operand(b, val_map, epath);
      epath.expr(Expr::FSub(*sz, a, b))
    }
    Instr::FMul(sz, a, b) => {
      let a = translate_operand(a, val_map, epath);
      let b = translate_operand(b, val_map, epath);
      epath.expr(Expr::FMul(*sz, a, b))
    }
    Instr::FDiv(sz, a, b) => {
      let a = translate_operand(a, val_map, epath);
      let b = translate_operand(b, val_map, epath);
      epath.expr(Expr::FDiv(*sz, a, b))
    }
    Instr::IEq(sz, a, b) => {
      let a = translate_operand(a, val_map, epath);
      let b = translate_operand(b, val_map, epath);
      epath.expr(Expr::IEq(*sz, a, b))
    }
    Instr::INe(sz, a, b) => {
      let a = translate_operand(a, val_map, epath);
      let b = translate_operand(b, val_map, epath);
      epath.expr(Expr::INe(*sz, a, b))
    }
    Instr::ILt(sz, a, b) => {
      let a = translate_operand(a, val_map, epath);
      let b = translate_operand(b, val_map, epath);
      epath.expr(Expr::ILt(*sz, a, b))
    }
    Instr::ILe(sz, a, b) => {
      let a = translate_operand(a, val_map, epath);
      let b = translate_operand(b, val_map, epath);
      epath.expr(Expr::ILe(*sz, a, b))
    }
    Instr::IGt(sz, a, b) => {
      let a = translate_operand(a, val_map, epath);
      let b = translate_operand(b, val_map, epath);
      epath.expr(Expr::IGt(*sz, a, b))
    }
    Instr::IGe(sz, a, b) => {
      let a = translate_operand(a, val_map, epath);
      let b = translate_operand(b, val_map, epath);
      epath.expr(Expr::IGe(*sz, a, b))
    }
    Instr::FEq(sz, a, b) => {
      let a = translate_operand(a, val_map, epath);
      let b = translate_operand(b, val_map, epath);
      epath.expr(Expr::FEq(*sz, a, b))
    }
    Instr::FNe(sz, a, b) => {
      let a = translate_operand(a, val_map, epath);
      let b = translate_operand(b, val_map, epath);
      epath.expr(Expr::FNe(*sz, a, b))
    }
    Instr::FLt(sz, a, b) => {
      let a = translate_operand(a, val_map, epath);
      let b = translate_operand(b, val_map, epath);
      epath.expr(Expr::FLt(*sz, a, b))
    }
    Instr::FLe(sz, a, b) => {
      let a = translate_operand(a, val_map, epath);
      let b = translate_operand(b, val_map, epath);
      epath.expr(Expr::FLe(*sz, a, b))
    }
    Instr::FGt(sz, a, b) => {
      let a = translate_operand(a, val_map, epath);
      let b = translate_operand(b, val_map, epath);
      epath.expr(Expr::FGt(*sz, a, b))
    }
    Instr::FGe(sz, a, b) => {
      let a = translate_operand(a, val_map, epath);
      let b = translate_operand(b, val_map, epath);
      epath.expr(Expr::FGe(*sz, a, b))
    }
    Instr::INot(sz, a) => {
      let a = translate_operand(a, val_map, epath);
      epath.expr(Expr::INot(*sz, a))
    }
    Instr::INeg(sz, a) => {
      let a = translate_operand(a, val_map, epath);
      epath.expr(Expr::INeg(*sz, a))
    }
    Instr::FNeg(sz, a) => {
      let a = translate_operand(a, val_map, epath);
      epath.expr(Expr::FNeg(*sz, a))
    }
    Instr::FieldPtr { base, offset } => {
      let base = translate_operand(base, val_map, epath);
      epath.expr(Expr::FieldPtr(base, *offset))
    }
    Instr::Load { ptr, ty, mem } => {
      let ptr = translate_operand(ptr, val_map, epath);
      epath.expr(Expr::Load(ptr, ty.clone(), *mem))
    }
    Instr::Store {
      ptr,
      value,
      ty,
      mem,
    } => {
      let ptr = translate_operand(ptr, val_map, epath);
      let value = translate_operand(value, val_map, epath);
      epath.expr(Expr::Store(ptr, value, ty.clone(), *mem))
    }
    Instr::StackAlloc { size, align } => epath.expr(Expr::StackAlloc(
      *size,
      *align,
      crate::ir::graph::MemToken(0),
    )),
    Instr::Call(name, args) => {
      let args = args
        .iter()
        .map(|a| translate_operand(a, val_map, epath))
        .collect();
      epath.expr(Expr::Call(
        name.clone(),
        args,
        crate::ir::graph::MemToken(0),
      ))
    }
  }
}

fn translate_jump(
  j: &crate::ir::graph::Jump,
  val_map: &HashMap<Val, ExprId>,
  epath: &mut EPath,
  idx_to_block: &HashMap<u32, BlockId>,
) -> crate::epath::ir::Jump {
  crate::epath::ir::Jump {
    target: idx_to_block[&j.target].clone(),
    args: j
      .args
      .iter()
      .map(|a| translate_operand(a, val_map, epath))
      .collect(),
  }
}

fn translate_terminator(
  term: &Terminator,
  val_map: &HashMap<Val, ExprId>,
  epath: &mut EPath,
  idx_to_block: &HashMap<u32, BlockId>,
) -> Hc<crate::epath::ir::Terminator> {
  let t = match term {
    Terminator::Jump(j) => {
      crate::epath::ir::Terminator::Jump(translate_jump(j, val_map, epath, idx_to_block))
    }
    Terminator::CondJump {
      cond,
      then_jump,
      else_jump,
    } => crate::epath::ir::Terminator::CondJump {
      cond: translate_operand(cond, val_map, epath),
      then_jump: translate_jump(then_jump, val_map, epath, idx_to_block),
      else_jump: translate_jump(else_jump, val_map, epath, idx_to_block),
    },
    Terminator::Return(v) => {
      crate::epath::ir::Terminator::Return(v.as_ref().map(|o| translate_operand(o, val_map, epath)))
    }
  };
  epath.jump(t)
}

fn dfs_paths(
  idx_to_block: &HashMap<u32, BlockId>,
  block_terminators: &HashMap<BlockId, Hc<crate::epath::ir::Terminator>>,
  idx: u32,
  current: &mut Vec<BlockId>,
  paths: &mut Vec<Path>,
) {
  let block_id = idx_to_block[&idx].clone();

  if current.contains(&block_id) {
    paths.push(Path {
      blocks: current.clone(),
      origin: None,
    });
    return;
  }

  current.push(block_id.clone());

  match &*block_terminators[&block_id] {
    crate::epath::ir::Terminator::Return(_) => {
      paths.push(Path {
        blocks: current.clone(),
        origin: None,
      });
    }
    crate::epath::ir::Terminator::Jump(j) => {
      let target_idx = idx_to_block
        .iter()
        .find(|(_, v)| *v == &j.target)
        .map(|(k, _)| *k)
        .unwrap();
      dfs_paths(idx_to_block, block_terminators, target_idx, current, paths);
    }
    crate::epath::ir::Terminator::CondJump {
      then_jump,
      else_jump,
      ..
    } => {
      let then_idx = idx_to_block
        .iter()
        .find(|(_, v)| *v == &then_jump.target)
        .map(|(k, _)| *k)
        .unwrap();
      let else_idx = idx_to_block
        .iter()
        .find(|(_, v)| *v == &else_jump.target)
        .map(|(k, _)| *k)
        .unwrap();
      dfs_paths(idx_to_block, block_terminators, then_idx, current, paths);
      current.pop();
      dfs_paths(idx_to_block, block_terminators, else_idx, current, paths);
    }
  }

  current.pop();
}
