use crate::epath::ir::{Block, Expr};

#[derive(Clone)]
pub struct LoopSymbol(pub usize);

#[derive(Clone)]
pub enum CostExpr {
  Const(u64),
  Mul(Box<CostExpr>, Box<CostExpr>),
  Add(Box<CostExpr>, Box<CostExpr>),
  Symbol(LoopSymbol),
}

impl CostExpr {
  pub fn dominates(&self, other: &CostExpr) -> bool {
    match (self.evaluate(), other.evaluate()) {
      (Some(a), Some(b)) => a <= b,
      (Some(_), None) => true,
      (None, Some(_)) => false,
      (None, None) => match (self, other) {
        (CostExpr::Mul(s1, a), CostExpr::Mul(s2, b)) => {
          if let (CostExpr::Symbol(ls1), CostExpr::Symbol(ls2)) = (s1.as_ref(), s2.as_ref()) {
            if ls1.0 == ls2.0 {
              return a.dominates(b);
            }
          }
          false
        }
        _ => false,
      },
    }
  }

  pub fn evaluate(&self) -> Option<u64> {
    match self {
      CostExpr::Const(n) => Some(*n),
      CostExpr::Add(a, b) => Some(a.evaluate()? + b.evaluate()?),
      CostExpr::Mul(a, b) => Some(a.evaluate()? * b.evaluate()?),
      CostExpr::Symbol(_) => None,
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
