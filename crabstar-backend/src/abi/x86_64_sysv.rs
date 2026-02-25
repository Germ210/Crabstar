use crate::abi::types::{AbiType, CallingConvention, FfiCif, FfiStatus, FfiType};

const MAX_GPR_REGS: usize = 6;
const MAX_SSE_REGS: usize = 8;
const MAX_CLASSES: usize = 4;

#[derive(Debug, Clone, Copy, PartialEq)]
enum RegClass {
  No,
  Integer,
  IntegerSi,
  Sse,
  SseSf,
  SseDf,
  SseUp,
  X87,
  X87Up,
  ComplexX87,
  Memory,
}

fn merge_classes(a: RegClass, b: RegClass) -> RegClass {
  if a == b {
    return a;
  }
  if a == RegClass::No {
    return b;
  }
  if b == RegClass::No {
    return a;
  }
  if a == RegClass::Memory || b == RegClass::Memory {
    return RegClass::Memory;
  }
  if (a == RegClass::IntegerSi && b == RegClass::SseSf)
    || (b == RegClass::IntegerSi && a == RegClass::SseSf)
  {
    return RegClass::IntegerSi;
  }
  if matches!(a, RegClass::Integer | RegClass::IntegerSi)
    || matches!(b, RegClass::Integer | RegClass::IntegerSi)
  {
    return RegClass::Integer;
  }
  if matches!(a, RegClass::X87 | RegClass::X87Up | RegClass::ComplexX87)
    || matches!(b, RegClass::X87 | RegClass::X87Up | RegClass::ComplexX87)
  {
    return RegClass::Memory;
  }
  RegClass::Sse
}

fn classify_argument(
  ty: &FfiType,
  classes: &mut [RegClass; MAX_CLASSES],
  byte_offset: usize,
) -> usize {
  match ty.ty {
    AbiType::U8
    | AbiType::I8
    | AbiType::U16
    | AbiType::I16
    | AbiType::U32
    | AbiType::I32
    | AbiType::U64
    | AbiType::I64
    | AbiType::Int
    | AbiType::Pointer => {
      let size = byte_offset + ty.size;
      if size <= 4 {
        classes[0] = RegClass::IntegerSi;
        1
      } else if size <= 8 {
        classes[0] = RegClass::Integer;
        1
      } else if size <= 12 {
        classes[0] = RegClass::Integer;
        classes[1] = RegClass::IntegerSi;
        2
      } else if size <= 16 {
        classes[0] = RegClass::Integer;
        classes[1] = RegClass::Integer;
        2
      } else {
        0
      }
    }

    AbiType::Float => {
      classes[0] = if byte_offset % 8 == 0 {
        RegClass::SseSf
      } else {
        RegClass::Sse
      };
      1
    }

    AbiType::Double => {
      classes[0] = RegClass::SseDf;
      1
    }

    AbiType::LongDouble => {
      classes[0] = RegClass::X87;
      classes[1] = RegClass::X87Up;
      2
    }

    AbiType::Void => {
      classes[0] = RegClass::No;
      1
    }

    AbiType::Struct => {
      const UNITS_PER_WORD: usize = 8;
      let words = (ty.size + byte_offset + UNITS_PER_WORD - 1) / UNITS_PER_WORD;

      if ty.size > 32 {
        return 0;
      }

      if words == 0 {
        classes[0] = RegClass::No;
        return 1;
      }

      for i in 0..words {
        classes[i] = RegClass::No;
      }

      let mut field_offset = byte_offset;
      for elem in &ty.elements {
        field_offset = align_up(field_offset, elem.alignment as usize);
        let mut subclasses = [RegClass::No; MAX_CLASSES];
        let num = classify_argument(elem, &mut subclasses, field_offset % 8);
        if num == 0 {
          return 0;
        }
        let pos = field_offset / 8;
        for i in 0..num {
          if i + pos < words {
            classes[i + pos] = merge_classes(subclasses[i], classes[i + pos]);
          }
        }
        field_offset += elem.size;
      }

      if words > 2 {
        if classes[0] != RegClass::Sse {
          return 0;
        }
        for i in 1..words {
          if classes[i] != RegClass::SseUp {
            return 0;
          }
        }
      }

      for i in 0..words {
        if classes[i] == RegClass::Memory {
          return 0;
        }
        if i > 0
          && classes[i] == RegClass::SseUp
          && classes[i - 1] != RegClass::Sse
          && classes[i - 1] != RegClass::SseUp
        {
          classes[i] = RegClass::Sse;
        }
        if i > 0 && classes[i] == RegClass::X87Up && classes[i - 1] != RegClass::X87 {
          return 0;
        }
      }

      words
    }

    AbiType::Complex => {
      let inner = ty.elements.first().expect("complex has no inner type");
      match inner.ty {
        AbiType::Int
        | AbiType::U8
        | AbiType::I8
        | AbiType::U16
        | AbiType::I16
        | AbiType::U32
        | AbiType::I32
        | AbiType::U64
        | AbiType::I64 => {
          let size = byte_offset + ty.size;
          if size <= 4 {
            classes[0] = RegClass::IntegerSi;
            1
          } else if size <= 8 {
            classes[0] = RegClass::Integer;
            1
          } else if size <= 12 {
            classes[0] = RegClass::Integer;
            classes[1] = RegClass::IntegerSi;
            2
          } else {
            classes[0] = RegClass::Integer;
            classes[1] = RegClass::Integer;
            2
          }
        }
        AbiType::Float => {
          classes[0] = RegClass::Sse;
          if byte_offset % 8 != 0 {
            classes[1] = RegClass::SseSf;
            2
          } else {
            1
          }
        }
        AbiType::Double => {
          classes[0] = RegClass::SseDf;
          classes[1] = RegClass::SseDf;
          2
        }
        AbiType::LongDouble => {
          classes[0] = RegClass::ComplexX87;
          1
        }
        _ => 0,
      }
    }
  }
}

fn examine_argument(
  ty: &FfiType,
  classes: &mut [RegClass; MAX_CLASSES],
  in_return: bool,
  ngpr: &mut usize,
  nsse: &mut usize,
) -> usize {
  let n = classify_argument(ty, classes, 0);
  if n == 0 {
    return 0;
  }

  *ngpr = 0;
  *nsse = 0;

  for i in 0..n {
    match classes[i] {
      RegClass::Integer | RegClass::IntegerSi => *ngpr += 1,
      RegClass::Sse | RegClass::SseSf | RegClass::SseDf => *nsse += 1,
      RegClass::No | RegClass::SseUp => {}
      RegClass::X87 | RegClass::X87Up | RegClass::ComplexX87 => {
        if !in_return {
          return 0;
        }
      }
      RegClass::Memory => return 0,
    }
  }

  n
}

fn align_up(val: usize, align: usize) -> usize {
  (val + align - 1) & !(align - 1)
}

fn sse_class(c: RegClass) -> bool {
  matches!(
    c,
    RegClass::Sse | RegClass::SseSf | RegClass::SseDf | RegClass::SseUp
  )
}

pub enum SysVAbi {
  Unix64,
}

#[derive(Debug, PartialEq)]
pub enum GprReg {
  Rdi,
  Rsi,
  Rdx,
  Rcx,
  R8,
  R9,
}

#[derive(Debug, PartialEq)]
pub enum SseReg {
  Xmm0,
  Xmm1,
  Xmm2,
  Xmm3,
  Xmm4,
  Xmm5,
  Xmm6,
  Xmm7,
}

#[derive(Debug, PartialEq)]
pub enum RetLocation {
  Void,
  Rax,
  RaxRdx,
  Xmm0,
  Xmm0Xmm1,
  RaxXmm0,
  Xmm0Rax,
  X87,
  X87Two,
  Sret,
}

#[derive(Debug, PartialEq)]
pub enum ArgLocation {
  Gpr(GprReg),
  Sse(SseReg),
  GprPair(GprReg, GprReg),
  SsePair(SseReg, SseReg),
  GprSse(GprReg, SseReg),
  SseGpr(SseReg, GprReg),
  Stack { offset: u32 },
}

pub struct SysVCifData {
  pub ret_location: RetLocation,
  pub arg_locations: Vec<ArgLocation>,
}

pub struct SysV;

fn gpr_reg(idx: usize) -> Option<GprReg> {
  match idx {
    0 => Some(GprReg::Rdi),
    1 => Some(GprReg::Rsi),
    2 => Some(GprReg::Rdx),
    3 => Some(GprReg::Rcx),
    4 => Some(GprReg::R8),
    5 => Some(GprReg::R9),
    _ => None,
  }
}

fn sse_reg(idx: usize) -> Option<SseReg> {
  match idx {
    0 => Some(SseReg::Xmm0),
    1 => Some(SseReg::Xmm1),
    2 => Some(SseReg::Xmm2),
    3 => Some(SseReg::Xmm3),
    4 => Some(SseReg::Xmm4),
    5 => Some(SseReg::Xmm5),
    6 => Some(SseReg::Xmm6),
    7 => Some(SseReg::Xmm7),
    _ => None,
  }
}

fn classify_ret(ty: &FfiType, classes: &mut [RegClass; MAX_CLASSES]) -> RetLocation {
  let mut ngpr = 0usize;
  let mut nsse = 0usize;
  let n = examine_argument(ty, classes, true, &mut ngpr, &mut nsse);

  if n == 0 {
    return RetLocation::Sret;
  }

  match ty.ty {
    AbiType::Void => RetLocation::Void,
    AbiType::Float | AbiType::Double => RetLocation::Xmm0,
    AbiType::LongDouble => RetLocation::X87,

    AbiType::Complex => {
      let inner = ty.elements.first().expect("complex has no inner type");
      match inner.ty {
        AbiType::Float => RetLocation::Xmm0,
        AbiType::Double => RetLocation::Xmm0Xmm1,
        AbiType::LongDouble => RetLocation::X87Two,
        _ => RetLocation::RaxRdx,
      }
    }

    AbiType::Struct => {
      let sse0 = sse_class(classes[0]);
      let rtype_size = ty.size;
      if rtype_size == 4 && sse0 {
        RetLocation::Xmm0
      } else if rtype_size == 8 {
        if sse0 {
          RetLocation::Xmm0
        } else {
          RetLocation::Rax
        }
      } else {
        let sse1 = n == 2 && sse_class(classes[1]);
        match (sse0, sse1) {
          (true, true) => RetLocation::Xmm0Xmm1,
          (true, false) => RetLocation::Xmm0Rax,
          (false, true) => RetLocation::RaxXmm0,
          (false, false) => RetLocation::RaxRdx,
        }
      }
    }

    _ => RetLocation::Rax,
  }
}

fn assign_arg_registers(
  classes: &[RegClass; MAX_CLASSES],
  n: usize,
  gprcount: &mut usize,
  ssecount: &mut usize,
) -> ArgLocation {
  if n == 1 {
    if sse_class(classes[0]) {
      let r = sse_reg(*ssecount).unwrap();
      *ssecount += 1;
      ArgLocation::Sse(r)
    } else {
      let r = gpr_reg(*gprcount).unwrap();
      *gprcount += 1;
      ArgLocation::Gpr(r)
    }
  } else {
    let c0_sse = sse_class(classes[0]);
    let c1_sse = sse_class(classes[1]);
    match (c0_sse, c1_sse) {
      (false, false) => {
        let r0 = gpr_reg(*gprcount).unwrap();
        let r1 = gpr_reg(*gprcount + 1).unwrap();
        *gprcount += 2;
        ArgLocation::GprPair(r0, r1)
      }
      (true, true) => {
        let r0 = sse_reg(*ssecount).unwrap();
        let r1 = sse_reg(*ssecount + 1).unwrap();
        *ssecount += 2;
        ArgLocation::SsePair(r0, r1)
      }
      (false, true) => {
        let g = gpr_reg(*gprcount).unwrap();
        let s = sse_reg(*ssecount).unwrap();
        *gprcount += 1;
        *ssecount += 1;
        ArgLocation::GprSse(g, s)
      }
      (true, false) => {
        let s = sse_reg(*ssecount).unwrap();
        let g = gpr_reg(*gprcount).unwrap();
        *ssecount += 1;
        *gprcount += 1;
        ArgLocation::SseGpr(s, g)
      }
    }
  }
}

impl CallingConvention for SysV {
  type Abi = SysVAbi;
  type CifData = SysVCifData;

  fn prep(cif: &mut FfiCif<Self>) -> FfiStatus {
    if !matches!(cif.abi, SysVAbi::Unix64) {
      return FfiStatus::BadAbi;
    }

    let mut classes = [RegClass::No; MAX_CLASSES];
    let ret_location = classify_ret(&cif.rtype, &mut classes);
    let mut gprcount = if ret_location == RetLocation::Sret {
      1
    } else {
      0
    };
    let mut ssecount = 0usize;
    let mut stack_bytes = 0usize;
    let mut arg_locations = Vec::with_capacity(cif.arg_types.len());

    for arg in &cif.arg_types {
      let mut classes = [RegClass::No; MAX_CLASSES];
      let mut ng = 0usize;
      let mut ns = 0usize;
      let n = examine_argument(arg, &mut classes, false, &mut ng, &mut ns);

      if n == 0 || gprcount + ng > MAX_GPR_REGS || ssecount + ns > MAX_SSE_REGS {
        let align = usize::max(arg.alignment as usize, 8);
        stack_bytes = align_up(stack_bytes, align);
        arg_locations.push(ArgLocation::Stack {
          offset: stack_bytes as u32,
        });
        stack_bytes += arg.size;
      } else {
        let loc = assign_arg_registers(&classes, n, &mut gprcount, &mut ssecount);
        arg_locations.push(loc);
      }
    }

    cif.bytes = align_up(stack_bytes, 8) as u32;
    cif.cc_data = SysVCifData {
      ret_location,
      arg_locations,
    };

    FfiStatus::Ok
  }
}
