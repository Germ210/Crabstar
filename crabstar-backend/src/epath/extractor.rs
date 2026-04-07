use crate::epath::{
  cost::{Cost, CostExpr, LoopSymbol},
  ir::{BlockId, EPath, Expr, ExprId, Path, PathId, PathSlice},
  rewrite::ControlStructure,
};

pub struct Extractor<C: Cost> {
  pub cost: C,
}

impl<C: Cost> Extractor<C> {
  pub fn new(cost: C) -> Self {
    Self { cost }
  }

  pub fn extract(&self, epath: &EPath) -> Vec<Path> {
    epath
      .paths
      .iter()
      .enumerate()
      .filter(|(_, p)| p.origin.is_none())
      .map(|(path_idx, path)| {
        let structure = ControlStructure::from_path(path, epath);
        let blocks = path
          .blocks
          .iter()
          .enumerate()
          .map(|(block_idx, block_id)| {
            let slice = PathSlice {
              path: PathId(path_idx),
              start: block_idx,
              end: block_idx + 1,
            };
            let loop_symbol = self.loop_symbol_for(&structure, block_id, path_idx);
            self.best_block(block_id, &slice, loop_symbol, epath)
          })
          .collect();
        Path {
          blocks,
          origin: None,
        }
      })
      .collect()
  }

  fn total_cost(&self, block_id: &BlockId, loop_symbol: Option<&LoopSymbol>) -> CostExpr {
    let expr_cost = self.expr_total(&(**block_id).expr);
    match loop_symbol {
      Some(sym) => CostExpr::Mul(
        Box::new(CostExpr::Symbol(LoopSymbol(sym.0))),
        Box::new(expr_cost),
      ),
      None => expr_cost,
    }
  }

  fn expr_total(&self, expr: &ExprId) -> CostExpr {
    let node = self.cost.expr_cost(&**expr);
    let children = self.children_cost(expr);
    CostExpr::Add(Box::new(node), Box::new(children))
  }

  fn children_cost(&self, expr: &ExprId) -> CostExpr {
    match &**expr {
      Expr::IAdd(_, a, b)
      | Expr::ISub(_, a, b)
      | Expr::IMul(_, a, b)
      | Expr::IDiv(_, a, b)
      | Expr::IShl(_, a, b)
      | Expr::IShr(_, a, b)
      | Expr::IEq(_, a, b)
      | Expr::INe(_, a, b)
      | Expr::ILt(_, a, b)
      | Expr::ILe(_, a, b)
      | Expr::IGt(_, a, b)
      | Expr::IGe(_, a, b)
      | Expr::FAdd(_, a, b)
      | Expr::FSub(_, a, b)
      | Expr::FMul(_, a, b)
      | Expr::FDiv(_, a, b)
      | Expr::FEq(_, a, b)
      | Expr::FNe(_, a, b)
      | Expr::FLt(_, a, b)
      | Expr::FLe(_, a, b)
      | Expr::FGt(_, a, b)
      | Expr::FGe(_, a, b) => {
        CostExpr::Add(Box::new(self.expr_total(a)), Box::new(self.expr_total(b)))
      }
      Expr::INot(_, a)
      | Expr::INeg(_, a)
      | Expr::FNeg(_, a)
      | Expr::FieldPtr(a, _)
      | Expr::Load(a, _, _) => self.expr_total(a),
      Expr::Store(a, b, _, _) => {
        CostExpr::Add(Box::new(self.expr_total(a)), Box::new(self.expr_total(b)))
      }
      Expr::Call(_, args, _) => args.iter().fold(CostExpr::Const(0), |acc, a| {
        CostExpr::Add(Box::new(acc), Box::new(self.expr_total(a)))
      }),
      _ => CostExpr::Const(0),
    }
  }

  fn loop_symbol_for(
    &self,
    structure: &ControlStructure,
    block_id: &BlockId,
    path_idx: usize,
  ) -> Option<LoopSymbol> {
    match structure {
      ControlStructure::Loop { body, .. } => {
        if body.body.iter().any(|b| b == block_id) {
          Some(LoopSymbol(path_idx))
        } else {
          None
        }
      }
      _ => None,
    }
  }

  fn best_block(
    &self,
    block_id: &BlockId,
    slice: &PathSlice,
    loop_symbol: Option<LoopSymbol>,
    epath: &EPath,
  ) -> BlockId {
    let mut best_block = block_id.clone();
    let mut best_cost = self.total_cost(block_id, loop_symbol.as_ref());

    if let Some(equivalents) = epath.equalities.get(slice) {
      for eq_slice in equivalents {
        let eq_block = &epath.paths[eq_slice.path.0].blocks[eq_slice.start];
        let eq_cost = self.total_cost(eq_block, loop_symbol.as_ref());
        if eq_cost.dominates(&best_cost) {
          best_cost = eq_cost;
          best_block = eq_block.clone();
        }
      }
    }

    best_block
  }
}
