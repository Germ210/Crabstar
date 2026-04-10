pub mod ir;
#[macro_use]
pub mod rewrite;
pub mod cost;
pub mod extractor;
pub mod translate_cfg;

#[cfg(test)]
mod tests {
  use crate::abi::types::AbiType;
  use crate::build;
  use crate::epath::cost::{Cost, CostExpr};
  use crate::epath::ir::{ExprId, PathId, PathSlice};
  use crate::epath::rewrite::{ControlStructure, RewriteEngine, get_invariants, hoist};
  use crate::epath::translate_cfg::from_cfg;
  use crate::ir::builder::FunctionBuilder;
  use crate::{
    epath::ir::{EPath, Expr},
    ir::graph::IntSize,
  };
  use ematch_macro::ematch;

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
    ematch!(expr.as_expr(), epath, {
      IAdd(_sz, a, b) if a == b => {
        fired = true;
      }
    });
    assert!(fired);
  }

  #[test]
  fn test_ematch_no_match() {
    let mut epath = EPath::new();
    let a = epath.expr(Expr::Param(0));
    let b = epath.expr(Expr::Param(1));
    let expr = epath.expr(Expr::IAdd(IntSize::I64, a, b));
    let mut fired = false;
    ematch!(expr.as_expr(), epath, {
      IAdd(_sz, a, b) if a == b => {
        fired = true;
      }
    });
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
    ematch!(outer.as_expr(), epath, {
      IAdd(_sz, IAdd(_sz2, _a, _b), _c) => {
        fired = true;
      }
    });
    assert!(fired);
  }

  #[test]
  fn test_ematch_nested_first_position() {
    let mut epath = EPath::new();
    let a = epath.expr(Expr::Param(0));
    let b = epath.expr(Expr::Param(1));
    let inner = ExprId(
      epath
        .exprs
        .hashcons(Expr::IAdd(IntSize::I64, a.clone(), b.clone())),
    );
    let outer = ExprId(epath.exprs.hashcons(Expr::INeg(IntSize::I64, inner)));
    let mut fired = false;
    ematch!(outer.as_expr(), epath, {
      INeg(_sz, _inner) => {
        fired = true;
      }
    });
    assert!(fired);
  }

  #[test]
  fn test_ematch_nested_third_position() {
    let mut epath = EPath::new();
    let a = epath.expr(Expr::Param(0));
    let b = epath.expr(Expr::Param(1));
    let inner = ExprId(
      epath
        .exprs
        .hashcons(Expr::IAdd(IntSize::I64, a.clone(), b.clone())),
    );
    let outer = ExprId(
      epath
        .exprs
        .hashcons(Expr::IAdd(IntSize::I64, a.clone(), inner)),
    );
    let mut fired = false;
    ematch!(outer.as_expr(), epath, {
      IAdd(_sz, _a, _inner) => {
        fired = true;
      }
    });
    assert!(fired);
  }

  #[test]
  fn test_build_flat() {
    let mut epath = EPath::new();
    let result = build!(epath, IConst(IntSize::I64, 42));
    assert_eq!(result, epath.expr(Expr::IConst(IntSize::I64, 42)));
  }

  #[test]
  fn test_build_nested() {
    let mut epath = EPath::new();
    let a = epath.expr(Expr::Param(0));
    let result = build!(
      epath,
      IShl(IntSize::I64, a.clone(), IConst(IntSize::I64, 1))
    );
    let expected = {
      let inner = epath.expr(Expr::IConst(IntSize::I64, 1));
      epath.expr(Expr::IShl(IntSize::I64, a.clone(), inner))
    };
    assert_eq!(result, expected);
  }

  #[test]
  fn test_build_deep() {
    let mut epath = EPath::new();
    let a = epath.expr(Expr::Param(0));
    let result = build!(
      epath,
      IAdd(
        IntSize::I64,
        IShl(IntSize::I64, a.clone(), IConst(IntSize::I64, 1)),
        IConst(IntSize::I64, 2)
      )
    );
    let expected = {
      let one = epath.expr(Expr::IConst(IntSize::I64, 1));
      let shl = epath.expr(Expr::IShl(IntSize::I64, a.clone(), one));
      let two = epath.expr(Expr::IConst(IntSize::I64, 2));
      epath.expr(Expr::IAdd(IntSize::I64, shl, two))
    };
    assert_eq!(result, expected);
  }

  #[test]
  fn test_loop_detection() {
    let (mut builder, args) = FunctionBuilder::new(&[AbiType::I64]);
    let (header, _loop_params) = builder.begin_loop(vec![]);
    let _val = builder.add(args[0], args[0]);
    builder.loop_back(header, vec![]);
    let cfg = builder.finish();
    let epath = from_cfg(&cfg);
    let structure = ControlStructure::from_path(&epath.paths[0], &epath);
    assert!(matches!(structure, ControlStructure::Loop { .. }));
  }

  #[test]
  fn test_smatch_loop() {
    let (mut builder, args) = FunctionBuilder::new(&[AbiType::I64]);
    let (header, _loop_params) = builder.begin_loop(vec![]);
    let _val = builder.add(args[0], args[0]);
    builder.loop_back(header, vec![]);
    let cfg = builder.finish();
    let epath = from_cfg(&cfg);
    let fired: bool;
    let slice = PathSlice {
      path: PathId(0),
      start: 0,
      end: 1,
    };
    smatch!(slice, epath, {
      ControlStructure::Loop { .. } => {
        fired = true;
      }
      ControlStructure::Sequence { .. } => {
        fired = false;
      }
    });
    assert!(fired);
  }

  #[test]
  fn test_smatch_sequence() {
    let (mut builder, args) = FunctionBuilder::new(&[AbiType::I64]);
    let sum = builder.add(args[0], args[0]);
    builder.ret(sum);
    let cfg = builder.finish();
    let epath = from_cfg(&cfg);
    let fired: bool;
    let slice = PathSlice {
      path: PathId(0),
      start: 0,
      end: 1,
    };
    smatch!(slice, epath, {
      ControlStructure::Sequence { .. } => {
        fired = true;
      }
      ControlStructure::Loop { .. } => {
        fired = false;
      }
    });
    assert!(fired);
  }

  #[test]
  fn test_get_invariants_with_induction() {
    let (mut builder, _args) = FunctionBuilder::new(&[AbiType::I64]);
    let zero = builder.iconst(0);
    let (header, loop_params) = builder.begin_loop(vec![zero]);
    let i = loop_params[0];
    let _invariant = builder.iconst(42);
    let one = builder.iconst(1);
    let next_i = builder.add(i, one);
    builder.loop_back(header, vec![next_i]);
    let cfg = builder.finish();
    let epath = from_cfg(&cfg);
    let structure = ControlStructure::from_path(&epath.paths[0], &epath);
    if let ControlStructure::Loop { header, body } = structure {
      let invariants = get_invariants(&body, &header);
      assert!(
        invariants
          .iter()
          .any(|(_, expr)| matches!(expr.as_expr(), Expr::IConst(..)))
      );
      assert!(
        !invariants
          .iter()
          .any(|(_, expr)| matches!(expr.as_expr(), Expr::IAdd(..)))
      );
    } else {
      panic!("expected loop");
    }
  }

  #[test]
  fn test_hoist() {
    let (mut builder, _args) = FunctionBuilder::new(&[AbiType::I64]);
    let zero = builder.iconst(0);
    let (header, loop_params) = builder.begin_loop(vec![zero]);
    let i = loop_params[0];
    let _invariant = builder.iconst(42);
    let one = builder.iconst(1);
    let next_i = builder.add(i, one);
    builder.loop_back(header, vec![next_i]);
    let cfg = builder.finish();
    let mut epath = from_cfg(&cfg);
    let structure = ControlStructure::from_path(&epath.paths[0], &epath);
    if let ControlStructure::Loop { header, body } = structure {
      let invariants = get_invariants(&body, &header);
      let (inv_block, _) = invariants
        .iter()
        .find(|(_, expr)| matches!(expr.as_expr(), Expr::IConst(..)))
        .unwrap()
        .clone();
      let loop_slice = PathSlice {
        path: PathId(0),
        start: 0,
        end: epath.paths[0].blocks.len(),
      };
      let new_slice = hoist(inv_block.clone(), loop_slice.clone(), &mut epath);
      let new_path = &epath.paths[new_slice.path.0];
      assert_eq!(new_path.blocks[0], inv_block);
      assert_eq!(new_path.origin, Some(loop_slice));
      assert!(!new_path.blocks[1..].contains(&inv_block));
    } else {
      panic!("expected loop");
    }
  }

  #[test]
  fn test_loop_extraction() {
    use crate::epath::rewrite::hoist;
    use crate::epath::{cost::AstSizeCost, extractor::Extractor};
    let (mut builder, _args) = FunctionBuilder::new(&[AbiType::I64]);
    let zero = builder.iconst(0);
    let (header, loop_params) = builder.begin_loop(vec![zero]);
    let i = loop_params[0];
    let _invariant = builder.iconst(42);
    let one = builder.iconst(1);
    let next_i = builder.add(i, one);
    builder.loop_back(header, vec![next_i]);
    let cfg = builder.finish();
    let mut epath = from_cfg(&cfg);
    let structure = ControlStructure::from_path(&epath.paths[0], &epath);
    if let ControlStructure::Loop { header, body } = structure {
      let invariants = get_invariants(&body, &header);
      for (inv_block, _) in &invariants {
        let loop_slice = PathSlice {
          path: PathId(0),
          start: 0,
          end: epath.paths[0].blocks.len(),
        };
        let new_slice = hoist(inv_block.clone(), loop_slice.clone(), &mut epath);
        epath.record_eq(loop_slice, new_slice);
      }
    }
    let extractor = Extractor::new(AstSizeCost);
    let extracted = extractor.extract(&epath);
    let first_block = &extracted[0].blocks[0];
    assert!(matches!(&*(**first_block).expr, Expr::IConst(..)));
  }

  #[test]
  fn test_rewrite_engine() {
    let (mut builder, args) = FunctionBuilder::new(&[AbiType::I64]);
    let sum = builder.add(args[0], args[0]);
    builder.ret(sum);
    let cfg = builder.finish();
    let mut epath = from_cfg(&cfg);
    let mut engine = RewriteEngine::new();
    engine.add_rule(|blocks, slice, epath| {
      let expr = &(*blocks[0]).expr;
      ematch!(&**expr, epath, {
        IAdd(sz, a, b) if a == b => {
          let new_expr = build!(epath, IShl(*sz, a.clone(), IConst(*sz, 1)));
          let new_slice = epath.rewrite_expr(slice.clone(), new_expr);
          epath.record_eq(slice, new_slice);
        }
      })
    });
    engine.run(&mut epath);
  }

  #[test]
  fn test_mul_div_cancel() {
    use crate::epath::rewrite::RewriteEngine;
    let (mut builder, args) = FunctionBuilder::new(&[AbiType::I64]);
    let two = builder.iconst(2);
    let mul = builder.mul(args[0], two);
    let div = builder.div(mul, two);
    builder.ret(div);
    let cfg = builder.finish();
    let mut epath = from_cfg(&cfg);
    let mut engine = RewriteEngine::new();
    engine.add_rule(|blocks, slice, epath| {
      let expr = &(*blocks[0]).expr;
      ematch!(&**expr, epath, {
        IDiv(_sz, IMul(_sz2, a, b), c) => {
          if b == c {
            let new_slice = epath.rewrite_expr(slice.clone(), a.clone());
            epath.record_eq(slice.clone(), new_slice);
          }
        }
      })
    });
    engine.add_rule(|blocks, slice, epath| {
      let expr = &(*blocks[0]).expr;
      ematch!(&**expr, epath, {
        IDiv(sz, IConst(_, va), IConst(_, vb)) => {
          let new_expr = build!(epath, IConst(*sz, va / vb));
          let new_slice = epath.rewrite_expr(slice.clone(), new_expr);
          epath.record_eq(slice.clone(), new_slice);
        }
      })
    });
    engine.add_rule(|blocks, slice, epath| {
      let expr = &(*blocks[0]).expr;
      ematch!(&**expr, epath, {
        IMul(_sz, a, IConst(_, 1)) => {
          let new_slice = epath.rewrite_expr(slice.clone(), a.clone());
          epath.record_eq(slice.clone(), new_slice);
        }
      })
    });
    engine.run(&mut epath);

    let original_slice = epath
      .equalities
      .keys()
      .find(|s| {
        let block = epath.paths[s.path.0].blocks[s.start].clone();
        matches!(&*(*block).expr, Expr::IDiv(..))
      })
      .unwrap()
      .clone();

    let rewritten_slice = epath.equalities[&original_slice]
      .iter()
      .find(|s| {
        let block = epath.paths[s.path.0].blocks[s.start].clone();
        matches!(&*(*block).expr, Expr::Param(_))
      })
      .unwrap()
      .clone();

    let original_block = epath.paths[original_slice.path.0].blocks[original_slice.start].clone();
    let rewritten_block = epath.paths[rewritten_slice.path.0].blocks[rewritten_slice.start].clone();

    assert!(matches!(&*(*original_block).expr, Expr::IDiv(..)));
    assert!(matches!(&*(*rewritten_block).expr, Expr::Param(_)));
    assert_ne!(original_block, rewritten_block);

    let param_expr = (*rewritten_block).expr.clone();
    let original_param = epath.expr(Expr::Param(0));
    assert_eq!(param_expr, original_param);
  }

  #[test]
  fn test_extraction() {
    use crate::epath::extractor::Extractor;

    struct TestCost;

    impl Cost for TestCost {
      fn expr_cost(&self, expr: &Expr) -> CostExpr {
        match expr {
          Expr::IShl(_, _, _) => CostExpr::Const(1),
          _ => CostExpr::Const(5),
        }
      }
    }
    let (mut builder, args) = FunctionBuilder::new(&[AbiType::I64]);
    let sum = builder.add(args[0], args[0]);
    builder.ret(sum);
    let cfg = builder.finish();
    let mut epath = from_cfg(&cfg);
    let mut engine = RewriteEngine::new();
    engine.add_rule(|blocks, slice, epath| {
      let expr = &(*blocks[0]).expr;
      ematch!(&**expr, epath, {
        IAdd(sz, a, b) if a == b => {
          let new_expr = build!(epath, IShl(*sz, a.clone(), IConst(*sz, 1)));
          let new_slice = epath.rewrite_expr(slice.clone(), new_expr);
          epath.record_eq(slice, new_slice);
        }
      })
    });
    engine.run(&mut epath);
    let extractor = Extractor::new(TestCost);
    let extracted = extractor.extract(&epath);

    let found_shl = extracted.iter().any(|path| {
      path
        .blocks
        .iter()
        .any(|block| matches!(&*(**block).expr, Expr::IShl(..)))
    });
    assert!(found_shl);
  }
}
