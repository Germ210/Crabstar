use crate::abi::types::{AbiType, CallingConvention, FfiCif, FfiType};
use crate::regalloc::x86_64::SysV;
use crate::regalloc::x86_64::Win64;
use std::any::Any;
use target_lexicon::{Architecture, OperatingSystem, Triple};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Val(pub u32);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct MemToken(pub u32);

#[derive(Clone, Debug, Copy, PartialEq, Hash, Eq)]
pub enum IntSize {
  I8,
  I16,
  I32,
  I64,
  U8,
  U16,
  U32,
  U64,
}

#[derive(Clone, Debug, Copy, PartialEq, Hash, Eq)]
pub enum FloatSize {
  F32,
  F64,
}

#[derive(Clone, Debug, Copy, PartialEq)]
pub enum Operand {
  Val(Val),
  I8(i8),
  I16(i16),
  I32(i32),
  I64(i64),
  U8(u8),
  U16(u16),
  U32(u32),
  U64(u64),
  F32(f32),
  F64(f64),
  Mem(MemToken),
}

#[derive(Debug, Clone)]
pub enum Instr {
  IConst(IntSize, i64),
  FConst(FloatSize, f64),

  IAdd(IntSize, Operand, Operand),
  ISub(IntSize, Operand, Operand),
  IMul(IntSize, Operand, Operand),
  IDiv(IntSize, Operand, Operand),
  IShl(IntSize, Operand, Operand),
  IShr(IntSize, Operand, Operand),

  FAdd(FloatSize, Operand, Operand),
  FSub(FloatSize, Operand, Operand),
  FMul(FloatSize, Operand, Operand),
  FDiv(FloatSize, Operand, Operand),

  IEq(IntSize, Operand, Operand),
  INe(IntSize, Operand, Operand),
  ILt(IntSize, Operand, Operand),
  ILe(IntSize, Operand, Operand),
  IGt(IntSize, Operand, Operand),
  IGe(IntSize, Operand, Operand),

  FEq(FloatSize, Operand, Operand),
  FNe(FloatSize, Operand, Operand),
  FLt(FloatSize, Operand, Operand),
  FLe(FloatSize, Operand, Operand),
  FGt(FloatSize, Operand, Operand),
  FGe(FloatSize, Operand, Operand),

  INot(IntSize, Operand),
  INeg(IntSize, Operand),
  FNeg(FloatSize, Operand),

  StackAlloc {
    size: usize,
    align: usize,
  },

  Load {
    ptr: Operand,
    ty: AbiType,
    mem: MemToken,
  },

  Store {
    ptr: Operand,
    value: Operand,
    ty: AbiType,
    mem: MemToken,
  },

  FieldPtr {
    base: Operand,
    offset: usize,
  },

  Call(String, Vec<Operand>),
}

#[derive(Clone, Debug)]
pub struct Param {
  pub val: Val,
  pub ty: AbiType,
}

#[derive(Clone, Debug)]
pub struct Jump {
  pub target: BlockId,
  pub args: Vec<Operand>,
}

#[derive(Clone, Debug)]
pub enum Terminator {
  Jump(Jump),
  CondJump {
    cond: Operand,
    then_jump: Jump,
    else_jump: Jump,
  },
  Return(Option<Operand>),
}

#[derive(Clone, Debug)]
pub struct Block {
  pub params: Vec<Param>,
  pub instr: Option<(Val, Instr)>,
  pub terminator: Terminator,
}

type BlockId = u32;

#[derive(Clone, Debug)]
pub struct Cfg {
  pub blocks: Vec<Block>,
}

pub trait AnyFunction: Any {
  fn name(&self) -> &str;
  fn entry(&self) -> BlockId;
  fn cfg(&self) -> &Cfg;
  fn cif(&self) -> &dyn Any;
  fn triple(&self) -> &Triple;
}

pub struct Function<CC: CallingConvention> {
  pub name: String,
  pub cif: FfiCif<CC>,
  pub triple: Triple,
  pub entry: BlockId,
  pub cfg: Cfg,
}

impl<CC: CallingConvention + 'static> AnyFunction for Function<CC> {
  fn name(&self) -> &str {
    &self.name
  }

  fn entry(&self) -> BlockId {
    self.entry
  }

  fn cfg(&self) -> &Cfg {
    &self.cfg
  }

  fn cif(&self) -> &dyn Any {
    &self.cif
  }

  fn triple(&self) -> &Triple {
    &self.triple
  }
}

pub enum TargetCif<'a> {
  Win64(&'a FfiCif<Win64>),
  SysV(&'a FfiCif<SysV>),
}

pub fn resolve_cif<'a>(func: &'a dyn AnyFunction) -> Option<TargetCif<'a>> {
  let triple = func.triple();
  match (triple.architecture, triple.operating_system) {
    (Architecture::X86_64, OperatingSystem::Windows) => func
      .cif()
      .downcast_ref::<FfiCif<Win64>>()
      .map(TargetCif::Win64),
    (Architecture::X86_64, _) => func
      .cif()
      .downcast_ref::<FfiCif<SysV>>()
      .map(TargetCif::SysV),
    _ => None,
  }
}

pub struct Module {
  pub functions: Vec<Box<dyn AnyFunction>>,
}

impl Module {
  pub fn new() -> Self {
    Self { functions: vec![] }
  }

  pub fn add_function<CC: CallingConvention + 'static>(&mut self, f: Function<CC>) {
    self.functions.push(Box::new(f));
  }
}

pub fn make_function<CC: CallingConvention>(
  name: String,
  triple: Triple,
  arg_types: Vec<FfiType>,
  rtype: FfiType,
  abi: CC::Abi,
  entry: BlockId,
  cfg: Cfg,
) -> Function<CC>
where
  CC::CifData: Default,
{
  let mut f = Function {
    name,
    cif: FfiCif::new(abi, arg_types, rtype, CC::CifData::default()),
    triple,
    entry,
    cfg,
  };
  CC::prep(&mut f.cif);
  f
}
