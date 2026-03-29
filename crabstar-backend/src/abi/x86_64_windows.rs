use crate::{
  abi::types::{AbiType, CallingConvention, FfiCif, FfiStatus, FfiType},
  regalloc::x86_64::Win64,
};

#[derive(Debug, Default)]
pub enum Win64Abi {
  #[default]
  Win64,
  GnuW64,
}

#[derive(Debug)]
pub struct PackedField {
  pub bit_offset: u8,
  pub size: u8,
}

#[derive(Debug)]
pub enum RetReg {
  Rax,
  Xmm0,
}

#[derive(Debug, Default)]
pub enum RetLocation {
  #[default]
  Rax,
  Xmm0,
  SretRcx,
  Packed {
    register: RetReg,
    fields: Vec<PackedField>,
  },
}

#[derive(Debug)]
pub enum IntReg {
  Rcx,
  Rdx,
  R8,
  R9,
}

#[derive(Debug)]
pub enum FloatReg {
  Xmm0,
  Xmm1,
  Xmm2,
  Xmm3,
}

#[derive(Debug)]
pub enum ArgLocation {
  Int(IntReg),
  Float(FloatReg),
  Stack { offset: u32 },
  ByRef(IntReg),
}

#[derive(Debug, Default)]
pub struct Win64CifData {
  pub ret_location: RetLocation,
  pub arg_locations: Vec<ArgLocation>,
}

impl CallingConvention for Win64 {
  type Abi = Win64Abi;
  type CifData = Win64CifData;
  type PhysReg = crate::regalloc::x86_64::PhysReg;

  fn prep(cif: &mut FfiCif<Self>) -> FfiStatus {
    let is_gnu = matches!(cif.abi, Win64Abi::GnuW64);
    let ret_location = classify_ret(&cif.rtype, is_gnu);
    let sret = matches!(ret_location, RetLocation::SretRcx);

    let mut arg_locations = Vec::with_capacity(cif.arg_types.len());
    let mut reg_idx = if sret { 1u32 } else { 0u32 };

    for arg in &cif.arg_types {
      let loc = classify_arg(arg, reg_idx);
      reg_idx += 1;
      arg_locations.push(loc);
    }

    let n = cif.nargs + if sret { 1 } else { 0 };
    let stack_slots = if n > 4 { n - 4 } else { 0 };
    cif.bytes = (32 + stack_slots * 8 + 15) & !15;

    cif.cc_data = Win64CifData {
      ret_location,
      arg_locations,
    };
    FfiStatus::Ok
  }

  fn arg_reg(data: &Win64CifData, idx: usize) -> Option<crate::regalloc::x86_64::PhysReg> {
    use crate::regalloc::x86_64::PhysReg;
    match data.arg_locations.get(idx) {
      Some(ArgLocation::Int(IntReg::Rcx)) => Some(PhysReg::Rcx),
      Some(ArgLocation::Int(IntReg::Rdx)) => Some(PhysReg::Rdx),
      Some(ArgLocation::Int(IntReg::R8)) => Some(PhysReg::R8),
      Some(ArgLocation::Int(IntReg::R9)) => Some(PhysReg::R9),
      _ => None,
    }
  }

  fn ret_reg(_data: &Win64CifData) -> crate::regalloc::x86_64::PhysReg {
    crate::regalloc::x86_64::PhysReg::Rax
  }
}

fn int_reg(idx: u32) -> Option<IntReg> {
  match idx {
    0 => Some(IntReg::Rcx),
    1 => Some(IntReg::Rdx),
    2 => Some(IntReg::R8),
    3 => Some(IntReg::R9),
    _ => None,
  }
}

fn float_reg(idx: u32) -> Option<FloatReg> {
  match idx {
    0 => Some(FloatReg::Xmm0),
    1 => Some(FloatReg::Xmm1),
    2 => Some(FloatReg::Xmm2),
    3 => Some(FloatReg::Xmm3),
    _ => None,
  }
}

fn stack_offset(idx: u32) -> u32 {
  32 + (idx - 4) * 8
}

fn classify_arg(ty: &FfiType, reg_idx: u32) -> ArgLocation {
  match ty.ty {
    AbiType::Float | AbiType::Double => match float_reg(reg_idx) {
      Some(r) => ArgLocation::Float(r),
      None => ArgLocation::Stack {
        offset: stack_offset(reg_idx),
      },
    },
    AbiType::Struct | AbiType::Complex => match ty.size {
      1 | 2 | 4 | 8 => match int_reg(reg_idx) {
        Some(r) => ArgLocation::Int(r),
        None => ArgLocation::Stack {
          offset: stack_offset(reg_idx),
        },
      },
      _ => match int_reg(reg_idx) {
        Some(r) => ArgLocation::ByRef(r),
        None => ArgLocation::Stack {
          offset: stack_offset(reg_idx),
        },
      },
    },
    _ => match int_reg(reg_idx) {
      Some(r) => ArgLocation::Int(r),
      None => ArgLocation::Stack {
        offset: stack_offset(reg_idx),
      },
    },
  }
}

fn pack_fields(ty: &FfiType) -> Vec<PackedField> {
  let mut fields = Vec::new();
  let mut bit_offset = 0u8;
  for elem in &ty.elements {
    let size = elem.size as u8;
    fields.push(PackedField {
      bit_offset,
      size: size * 8,
    });
    bit_offset += size * 8;
  }
  fields
}

fn classify_ret(ty: &FfiType, is_gnu: bool) -> RetLocation {
  match ty.ty {
    AbiType::Void => RetLocation::Rax,
    AbiType::Float | AbiType::Double => RetLocation::Xmm0,
    AbiType::LongDouble => {
      if is_gnu {
        RetLocation::SretRcx
      } else {
        RetLocation::Xmm0
      }
    }
    AbiType::Complex | AbiType::Struct => match ty.size {
      1 | 2 | 4 | 8 => RetLocation::Packed {
        register: RetReg::Rax,
        fields: pack_fields(ty),
      },
      _ => RetLocation::SretRcx,
    },
    _ => RetLocation::Rax,
  }
}
