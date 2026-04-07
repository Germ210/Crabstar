use crate::epath::ir::{Block, Expr};

pub struct LoopSymbol(pub usize);

pub enum CostExpr {
  Const(u64),
  Mul(Box<CostExpr>, Box<CostExpr>),
  Add(Box<CostExpr>, Box<CostExpr>),
  Symbol(LoopSymbol),
}

impl CostExpr {
  pub fn dominates(&self, other: &CostExpr) -> bool {
    match (self, other) {
      (CostExpr::Const(a), CostExpr::Const(b)) => a <= b,
      (CostExpr::Const(_), CostExpr::Mul(sym, _)) => {
        matches!(sym.as_ref(), CostExpr::Symbol(_))
      }
      (CostExpr::Mul(_, _), CostExpr::Const(_)) => false,
      (CostExpr::Mul(s1, a), CostExpr::Mul(s2, b)) => {
        if let (CostExpr::Symbol(ls1), CostExpr::Symbol(ls2)) = (s1.as_ref(), s2.as_ref()) {
          if ls1.0 == ls2.0 {
            return a.dominates(b);
          }
        }
        false
      }
      (CostExpr::Add(a1, a2), CostExpr::Add(b1, b2)) => a1.dominates(b1) && a2.dominates(b2),
      _ => false,
    }
  }
}

pub trait Cost {
  fn expr_cost(&self, expr: &Expr) -> CostExpr;
  fn block_cost(&self, block: &Block) -> CostExpr {
    self.expr_cost(&block.expr)
  }
}

pub struct AstSizeCost;

impl Cost for AstSizeCost {
  fn expr_cost(&self, _expr: &Expr) -> CostExpr {
    CostExpr::Const(1)
  }
}
