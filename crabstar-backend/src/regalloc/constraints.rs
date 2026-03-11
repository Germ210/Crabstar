#[derive(Clone, Debug)]
pub enum RegConstraint<R> {
  Any,
  Fixed(R),
  SameAsOperand(usize),
}

#[derive(Clone, Debug)]
pub struct InstrConstraints<R> {
  pub operand_constraints: Vec<RegConstraint<R>>,
  pub def_constraint: RegConstraint<R>,
  pub clobbers: Vec<R>,
}

impl<R> InstrConstraints<R> {
  pub fn unconstrained(n_operands: usize) -> Self {
    Self {
      operand_constraints: (0..n_operands).map(|_| RegConstraint::Any).collect(),
      def_constraint: RegConstraint::Any,
      clobbers: vec![],
    }
  }
}
