use crabstar_backend::{
  abi::{
    types::FfiCif,
    types::{AbiType, CallingConvention, FfiType},
    x86_64_windows::{RetLocation, Win64Abi, Win64CifData},
  },
  codegen::generate_code,
  ir::builder::FunctionBuilder,
  regalloc::x86_64::Win64,
};
use std::fs;

fn main() {
  let (mut fb, params) = FunctionBuilder::new(&[AbiType::I64, AbiType::I64]);
  let [x, y] = params.as_slice() else { panic!() };
  let cmp = fb.gt(*x, *y);
  let result = fb.if_else(
    cmp,
    &[*x, *y],
    |fb, inputs| fb.div(inputs[0], inputs[1]),
    |fb, inputs| fb.div(inputs[1], inputs[0]),
  );
  fb.ret(result);
  let cfg = fb.finish();
  dbg!(&cfg);
  let mut cif = FfiCif::new(
    Win64Abi::Win64,
    vec![
      FfiType {
        size: 8,
        alignment: 8,
        ty: AbiType::I64,
        elements: vec![],
      },
      FfiType {
        size: 8,
        alignment: 8,
        ty: AbiType::I64,
        elements: vec![],
      },
    ],
    FfiType {
      size: 8,
      alignment: 8,
      ty: AbiType::I64,
      elements: vec![],
    },
    Win64CifData {
      ret_location: RetLocation::Rax,
      arg_locations: Vec::new(),
    },
  );
  Win64::prep(&mut cif);
  let obj_bytes = generate_code::<Win64>(&cfg, &cif, "divmax");
  fs::write("target/out.o", &obj_bytes).unwrap();
}
