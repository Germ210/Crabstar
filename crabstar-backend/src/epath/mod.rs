pub mod ir;
pub mod rewrite;
pub mod translate_cfg;

#[cfg(test)]
mod tests {
  use crate::abi::types::AbiType;
  use crate::ematch;
  use crate::epath::translate_cfg::from_cfg;
  use crate::ir::builder::FunctionBuilder;
  use crate::{
    epath::ir::{EPath, Expr},
    ir::graph::IntSize,
  };

  #[test]
  fn test_simple_paths() {
    let (mut builder, args) = FunctionBuilder::new(&[AbiType::I64, AbiType::I64]);
    let sum = builder.add(args[0], args[1]);
    builder.ret(sum);
    let cfg = builder.finish();
    let epath = from_cfg(&cfg);
    assert_eq!(epath.paths.len(), 1);
    assert!(!epath.paths[0].blocks.is_empty());
    assert!(epath.paths[0].origin.is_none());
  }

  #[test]
  fn test_if_else_paths() {
    let (mut builder, args) = FunctionBuilder::new(&[AbiType::I64, AbiType::I64]);
    let result = builder.if_else(
      args[0],
      &[args[1]],
      |b, inputs| b.add(inputs[0], inputs[0]),
      |b, inputs| b.sub(inputs[0], inputs[0]),
    );
    builder.ret(result);
    let cfg = builder.finish();
    let epath = from_cfg(&cfg);
    assert_eq!(epath.paths.len(), 2);
    for path in &epath.paths {
      assert!(!path.blocks.is_empty());
      assert!(path.origin.is_none());
    }
  }

  #[test]
  fn test_interning() {
    let mut ep = EPath::new();
    let a = ep.expr(Expr::IConst(IntSize::I64, 42));
    let b = ep.expr(Expr::IConst(IntSize::I64, 42));
    assert!(a == b);
  }

  #[test]
  fn test_ematch_flat() {
    let mut epath = EPath::new();
    let a = epath.expr(Expr::Param(0));
    let expr = epath.expr(Expr::IAdd(IntSize::I64, a.clone(), a.clone()));
    let mut fired = false;
    ematch!(expr.as_expr(), epath,
        IAdd(sz, a, b) if a == b => {
            fired = true;
        }
    );
    assert!(fired);
  }

  #[test]
  fn test_ematch_no_match() {
    let mut epath = EPath::new();
    let a = epath.expr(Expr::Param(0));
    let b = epath.expr(Expr::Param(1));
    let expr = epath.expr(Expr::IAdd(IntSize::I64, a, b));
    let mut fired = false;
    ematch!(expr.as_expr(), epath,
        IAdd(sz, a, b) if a == b => {
            fired = true;
        }
    );
    assert!(!fired);
  }

  #[test]
  fn test_ematch_nested() {
    let mut epath = EPath::new();
    let a = epath.expr(Expr::Param(0));
    let b = epath.expr(Expr::Param(1));
    let inner = epath.expr(Expr::IAdd(IntSize::I64, a.clone(), b.clone()));
    let outer = epath.expr(Expr::IAdd(IntSize::I64, inner, a.clone()));
    let mut fired = false;
    ematch!(outer.as_expr(), epath,
        IAdd(sz, IAdd(sz2, a, b), c) => {
            fired = true;
        }
    );
    assert!(fired);
  }
}
