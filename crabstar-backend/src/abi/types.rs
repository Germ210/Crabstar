use std::marker::PhantomData;

#[derive(Debug, Clone, PartialEq, Hash, Eq)]
pub enum AbiType {
  Void,
  Int,
  Float,
  Double,
  LongDouble,
  U8,
  I8,
  U16,
  I16,
  U32,
  I32,
  U64,
  I64,
  Struct,
  Pointer,
  Complex,
}

pub struct FfiType {
  pub size: usize,
  pub alignment: u16,
  pub ty: AbiType,
  pub elements: Vec<FfiType>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum FfiStatus {
  Ok,
  BadTypedef,
  BadAbi,
  BadArgType,
}

pub trait CallingConvention {
  type Abi;
  type CifData;
  type PhysReg: Copy + PartialEq;
  fn prep(cif: &mut FfiCif<Self>) -> FfiStatus
  where
    Self: Sized;
  fn arg_reg(data: &Self::CifData, idx: usize) -> Option<Self::PhysReg>;
  fn ret_reg(data: &Self::CifData) -> Self::PhysReg;
}

pub struct FfiCif<CC: CallingConvention> {
  pub abi: CC::Abi,
  pub nargs: u32,
  pub arg_types: Vec<FfiType>,
  pub rtype: FfiType,
  pub bytes: u32,
  pub flags: u32,
  pub cc_data: CC::CifData,
  _cc: PhantomData<CC>,
}

impl<CC: CallingConvention> FfiCif<CC> {
  pub fn new(abi: CC::Abi, arg_types: Vec<FfiType>, rtype: FfiType, cc_data: CC::CifData) -> Self {
    let nargs = arg_types.len() as u32;
    Self {
      abi,
      nargs,
      arg_types,
      rtype,
      bytes: 0,
      flags: 0,
      cc_data,
      _cc: PhantomData,
    }
  }
}
