use std::collections::{HashMap, HashSet};

use hash_cons::{Hc, HcTable};

use crate::{
  abi::types::AbiType,
  ir::graph::{FloatSize, IntSize, MemToken},
};

#[derive(Hash, PartialEq, Eq, Clone, Debug)]
pub struct ExprId(pub Hc<Expr>);
pub type BlockId = Hc<Block>;

impl ExprId {
  pub fn as_expr(&self) -> &Expr {
    &self.0
  }
}

impl std::ops::Deref for ExprId {
  type Target = Expr;
  fn deref(&self) -> &Expr {
    &self.0
  }
}

#[derive(Hash, PartialEq, Eq, Clone, Debug)]
pub struct PathId(pub usize);

#[derive(Hash, PartialEq, Eq, Clone, Debug)]
pub enum Expr {
  IConst(IntSize, i64),
  // 64 bits used here because Rust float's aren't Hash
  FConst(FloatSize, [u8; 8]),
  Param(u32),

  IAdd(IntSize, ExprId, ExprId),
  ISub(IntSize, ExprId, ExprId),
  IMul(IntSize, ExprId, ExprId),
  IDiv(IntSize, ExprId, ExprId),
  IShl(IntSize, ExprId, ExprId),
  IShr(IntSize, ExprId, ExprId),

  FAdd(FloatSize, ExprId, ExprId),
  FSub(FloatSize, ExprId, ExprId),
  FMul(FloatSize, ExprId, ExprId),
  FDiv(FloatSize, ExprId, ExprId),

  IEq(IntSize, ExprId, ExprId),
  INe(IntSize, ExprId, ExprId),
  ILt(IntSize, ExprId, ExprId),
  ILe(IntSize, ExprId, ExprId),
  IGt(IntSize, ExprId, ExprId),
  IGe(IntSize, ExprId, ExprId),

  FEq(FloatSize, ExprId, ExprId),
  FNe(FloatSize, ExprId, ExprId),
  FLt(FloatSize, ExprId, ExprId),
  FLe(FloatSize, ExprId, ExprId),
  FGt(FloatSize, ExprId, ExprId),
  FGe(FloatSize, ExprId, ExprId),

  INot(IntSize, ExprId),
  INeg(IntSize, ExprId),
  FNeg(FloatSize, ExprId),

  FieldPtr(ExprId, usize),
  Load(ExprId, AbiType, MemToken),
  Store(ExprId, ExprId, AbiType, MemToken),
  StackAlloc(usize, usize, MemToken),
  Call(String, Vec<ExprId>, MemToken),
}

#[derive(Hash, PartialEq, Eq, Clone, Debug)]
pub struct Jump {
  pub target: BlockId,
  pub args: Vec<ExprId>,
}

#[derive(Hash, PartialEq, Eq, Clone, Debug)]
pub enum Terminator {
  Jump(Jump),
  CondJump {
    cond: ExprId,
    then_jump: Jump,
    else_jump: Jump,
  },
  Return(Option<ExprId>),
}

#[derive(Hash, PartialEq, Eq, Clone, Debug)]
pub struct Block {
  pub params: Vec<ExprId>,
  pub expr: ExprId,
}

pub struct EPath {
  pub exprs: HcTable<Expr>,
  pub blocks: HcTable<Block>,
  pub jumps: HcTable<Terminator>,
  pub block_terminators: HashMap<BlockId, Hc<Terminator>>,
  pub paths: Vec<Path>,
  pub equalities: HashMap<PathSlice, HashSet<PathSlice>>,
}

impl EPath {
  pub fn new() -> Self {
    Self {
      exprs: HcTable::new(),
      blocks: HcTable::new(),
      jumps: HcTable::new(),
      block_terminators: HashMap::new(),
      paths: Vec::new(),
      equalities: HashMap::new(),
    }
  }

  pub fn expr(&mut self, e: Expr) -> ExprId {
    ExprId(self.exprs.hashcons(e))
  }

  pub fn block(&mut self, b: Block) -> BlockId {
    self.blocks.hashcons(b)
  }

  pub fn jump(&mut self, j: Terminator) -> Hc<Terminator> {
    self.jumps.hashcons(j)
  }

  pub fn add_path(&mut self, path: Path) -> PathId {
    let id = PathId(self.paths.len());
    self.paths.push(path);
    id
  }

  pub fn record_eq(&mut self, a: PathSlice, b: PathSlice) {
    self
      .equalities
      .entry(a.clone())
      .or_default()
      .insert(b.clone());
    self.equalities.entry(b).or_default().insert(a);
  }

  pub fn rewrite_slice(&mut self, slice: PathSlice, blocks: Vec<BlockId>) -> PathSlice {
    let mut new_blocks = self.paths[slice.path.0].blocks.clone();
    new_blocks.splice(slice.start..slice.end, blocks.iter().cloned());
    let end = slice.start + blocks.len();
    let new_path_id = self.add_path(Path {
      blocks: new_blocks,
      origin: Some(slice.clone()),
    });
    PathSlice {
      path: new_path_id,
      start: slice.start,
      end,
    }
  }

  pub fn rewrite_expr(&mut self, slice: PathSlice, expr: ExprId) -> PathSlice {
    let old_block = self.paths[slice.path.0].blocks[slice.start].clone();
    let new_block = self.blocks.hashcons(Block {
      params: (*old_block).params.clone(),
      expr,
    });
    let old_term = self.block_terminators[&old_block].clone();
    self.block_terminators.insert(new_block.clone(), old_term);
    self.rewrite_slice(slice, vec![new_block])
  }
}

#[derive(Hash, PartialEq, Eq, Clone, Debug)]
pub struct PathSlice {
  pub path: PathId,
  pub start: usize,
  pub end: usize,
}

pub struct Path {
  pub blocks: Vec<BlockId>,
  pub origin: Option<PathSlice>,
}
