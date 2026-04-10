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
        let n = path.blocks.len();
        let mut dp: Vec<Option<(CostExpr, Vec<BlockId>)>> = vec![None; n + 1];
        dp[0] = Some((CostExpr::Const(0), vec![]));

        for i in 0..n {
          if dp[i].is_none() {
            continue;
          }
          let base_cost = dp[i].as_ref().unwrap().0.clone();
          let base_blocks = dp[i].as_ref().unwrap().1.clone();

          let try_span =
            |_start: usize, _end: usize, span_blocks: Vec<BlockId>| -> (CostExpr, Vec<BlockId>) {
              let span_cost = span_blocks.iter().fold(CostExpr::Const(0), |acc, b| {
                let ls = self.loop_symbol_for(&structure, b, path_idx);
                CostExpr::Add(Box::new(acc), Box::new(self.total_cost(b, ls.as_ref())))
              });
              let total = CostExpr::Add(Box::new(base_cost.clone()), Box::new(span_cost));
              let mut new_blocks = base_blocks.clone();
              new_blocks.extend(span_blocks);
              (total, new_blocks)
            };

          let orig_block = path.blocks[i].clone();
          let ls = self.loop_symbol_for(&structure, &orig_block, path_idx);
          let orig_cost = CostExpr::Add(
            Box::new(base_cost.clone()),
            Box::new(self.total_cost(&orig_block, ls.as_ref())),
          );
          let mut orig_blocks = base_blocks.clone();
          orig_blocks.push(orig_block);
          match &dp[i + 1] {
            None => dp[i + 1] = Some((orig_cost, orig_blocks)),
            Some((c, _)) if orig_cost.dominates(c) => dp[i + 1] = Some((orig_cost, orig_blocks)),
            _ => {}
          }

          let slice = PathSlice {
            path: PathId(path_idx),
            start: i,
            end: path.blocks.len(),
          };
          if let Some(equivalents) = epath.equalities.get(&slice) {
            for eq_slice in equivalents {
              let eq_path = &epath.paths[eq_slice.path.0];
              let span_blocks: Vec<BlockId> = (eq_slice.start..eq_slice.end)
                .map(|k| eq_path.blocks[k].clone())
                .collect();
              let advance = eq_slice.end - eq_slice.start;
              let j = i + advance;
              if j > n {
                continue;
              }
              let (total, new_blocks) = try_span(i, j, span_blocks);
              match &dp[j] {
                None => dp[j] = Some((total, new_blocks)),
                Some((c, _)) if total.dominates(c) => dp[j] = Some((total, new_blocks)),
                _ => {}
              }
            }
          }
        }

        Path {
          blocks: dp[n].take().map(|(_, b)| b).unwrap_or_default(),
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
}
