use crate::abi::types::{AbiType, CallingConvention, FfiCif, FfiType};
use crate::regalloc::x86_64::SysV;
use crate::regalloc::x86_64::Win64;
use std::any::Any;
use target_lexicon::{Architecture, OperatingSystem, Triple};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Val(pub u32);

#[derive(Clone, Debug, Copy, PartialEq, Eq)]
pub enum Operand {
  Val(Val),
  Imm(i64),
}

#[derive(Clone, Debug)]
pub enum Instr {
  Const(i64),
  Add(Operand, Operand),
  Sub(Operand, Operand),
  Mul(Operand, Operand),
  Div(Operand, Operand),
  Eq(Operand, Operand),
  Ne(Operand, Operand),
  Lt(Operand, Operand),
  Le(Operand, Operand),
  Gt(Operand, Operand),
  Ge(Operand, Operand),
  Not(Operand),
  Neg(Operand),
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
