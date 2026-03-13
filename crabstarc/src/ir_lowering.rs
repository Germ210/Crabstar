use crabstar_backend::{
  abi::types::AbiType,
  ir::{
    builder::FunctionBuilder,
    graph::{Cfg, Operand},
  },
};
use crabstar_frontend::{
  ast::{AstNode, BinaryExpr, CallExpr, Ident, LetExpr, Literal, MatchExpr, PrefixExpr},
  syntax::SyntaxNode,
  types::Type,
};
use std::collections::HashMap;

pub struct Compiler {
  env: Vec<HashMap<String, Operand>>,
}

impl Compiler {
  pub fn new() -> Self {
    Self {
      env: vec![HashMap::new()],
    }
  }

  pub fn type_to_abi(ty: &Type) -> AbiType {
    match ty {
      Type::Int32 => AbiType::I64,
      _ => panic!("type not yet supported in IR: {:?}", ty),
    }
  }

  fn push_scope(&mut self) {
    self.env.push(HashMap::new());
  }

  fn pop_scope(&mut self) {
    self.env.pop();
  }

  fn lookup_var(&self, name: &str) -> Option<Operand> {
    for scope in self.env.iter().rev() {
      if let Some(op) = scope.get(name) {
        return Some(*op);
      }
    }
    None
  }

  fn bind_var(&mut self, name: &str, op: Operand) {
    self.env.last_mut().unwrap().insert(name.to_string(), op);
  }

  pub fn compile_function(&mut self, node: &SyntaxNode, param_types: &[AbiType]) -> Cfg {
    let (mut builder, param_ops) = FunctionBuilder::new(param_types);

    if let Some(AstNode::FnExpr(fn_expr)) = AstNode::cast(node.clone()) {
      let param_list = fn_expr.param_list();
      if let Some(param_list_node) = param_list.as_node() {
        if let Some(param_list) = crabstar_frontend::ast::ParamList::cast(param_list_node.clone()) {
          for (param, op) in param_list.params().zip(param_ops.iter()) {
            let param_name = param.param_name();
            if let Some(param_name_node) = param_name.as_node() {
              if let Some(ident) = Ident::cast(param_name_node.clone()) {
                let name = ident.name();
                let name = name.as_token().unwrap().text();
                self.bind_var(name, *op);
              }
            }
          }
        }
      }

      let body = fn_expr.body();
      if let Some(body_node) = body.as_node() {
        let result = self.compile_expr(&mut builder, body_node);
        builder.ret(result);
      }
    }

    builder.finish()
  }

  fn compile_expr(&mut self, builder: &mut FunctionBuilder, node: &SyntaxNode) -> Operand {
    match AstNode::cast(node.clone()) {
      Some(AstNode::Literal(lit)) => self.compile_literal(builder, &lit),
      Some(AstNode::Ident(ident)) => self.compile_ident(builder, &ident),
      Some(AstNode::BinaryExpr(binop)) => self.compile_binary(builder, &binop),
      Some(AstNode::PrefixExpr(prefix)) => self.compile_prefix(builder, &prefix),
      Some(AstNode::CallExpr(call)) => self.compile_call(builder, &call),
      Some(AstNode::LetExpr(let_expr)) => self.compile_let(builder, &let_expr),
      Some(AstNode::MatchExpr(match_expr)) => self.compile_match(builder, &match_expr),
      Some(AstNode::ParenExpr(paren)) => {
        let inner = paren.inner();
        if let Some(inner_node) = inner.as_node() {
          self.compile_expr(builder, inner_node)
        } else {
          builder.iconst(0)
        }
      }
      _ => builder.iconst(0),
    }
  }

  fn compile_literal(&mut self, builder: &mut FunctionBuilder, lit: &Literal) -> Operand {
    use crabstar_frontend::syntax::SyntaxKind;
    match lit.literal().kind() {
      SyntaxKind::Int => {
        let token = lit.literal();
        let token = token.as_token().unwrap();
        let text = token.text();
        let val = text.parse::<i64>().unwrap_or(0);
        builder.iconst(val)
      }
      SyntaxKind::KwTrue | SyntaxKind::KwFalse => {
        unimplemented!("bool literals not yet supported in IR")
      }
      SyntaxKind::KwNull => {
        unimplemented!("null literals not yet supported in IR")
      }
      _ => builder.iconst(0),
    }
  }

  fn compile_ident(&mut self, builder: &mut FunctionBuilder, ident: &Ident) -> Operand {
    let name = ident.name();
    let name = name.as_token().unwrap().text();
    self.lookup_var(name).unwrap_or(builder.iconst(0))
  }

  fn compile_binary(&mut self, builder: &mut FunctionBuilder, binop: &BinaryExpr) -> Operand {
    let lhs = binop.lhs();
    let lhs_node = lhs.as_node().unwrap();
    let lhs_op = self.compile_expr(builder, lhs_node);

    let rhs = binop.rhs();
    let rhs_node = rhs.as_node().unwrap();
    let rhs_op = self.compile_expr(builder, rhs_node);

    let op = binop.operator();
    let op_node = op.as_node().unwrap();
    let op_token = op_node.children_with_tokens().nth(1).unwrap();
    let op_text = op_token.as_token().unwrap().text();

    match op_text {
      "+" => builder.add(lhs_op, rhs_op),
      "-" => builder.sub(lhs_op, rhs_op),
      "*" => builder.mul(lhs_op, rhs_op),
      "/" => builder.div(lhs_op, rhs_op),
      "=" => builder.eq(lhs_op, rhs_op),
      "!=" => builder.ne(lhs_op, rhs_op),
      "<" => builder.lt(lhs_op, rhs_op),
      "<=" => builder.le(lhs_op, rhs_op),
      ">" => builder.gt(lhs_op, rhs_op),
      ">=" => builder.ge(lhs_op, rhs_op),
      "and" => {
        let zero = builder.iconst(0);
        let lhs_bool = builder.ne(lhs_op, zero);
        let zero2 = builder.iconst(0);
        let rhs_bool = builder.ne(rhs_op, zero2);
        let result = builder.mul(lhs_bool, rhs_bool);
        let zero3 = builder.iconst(0);
        builder.ne(result, zero3)
      }
      "or" => {
        let zero = builder.iconst(0);
        let lhs_bool = builder.ne(lhs_op, zero);
        let zero2 = builder.iconst(0);
        let rhs_bool = builder.ne(rhs_op, zero2);
        builder.add(lhs_bool, rhs_bool)
      }
      _ => builder.iconst(0),
    }
  }

  fn compile_prefix(&mut self, builder: &mut FunctionBuilder, prefix: &PrefixExpr) -> Operand {
    let rhs = prefix.rhs();
    let rhs_node = rhs.as_node().unwrap();
    let rhs_op = self.compile_expr(builder, rhs_node);

    let op = prefix.operator();
    let op_node = op.as_node().unwrap();
    let op_token = op_node.children_with_tokens().nth(1).unwrap();
    let op_text = op_token.as_token().unwrap().text();

    match op_text {
      "-" => builder.neg(rhs_op),
      "not" => builder.not(rhs_op),
      _ => rhs_op,
    }
  }

  fn compile_call(&mut self, builder: &mut FunctionBuilder, call: &CallExpr) -> Operand {
    let callee = call.callee();
    let callee_node = callee.as_node().unwrap();

    let args_binding = call.args();
    let args_node = args_binding.as_node().unwrap();
    let args_list = crabstar_frontend::ast::ArgList::cast(args_node.clone()).unwrap();

    let mut arg_ops = Vec::new();
    for arg in args_list.args() {
      let arg_expr = arg.arg_expr();
      let arg_expr_node = arg_expr.as_node().unwrap();
      arg_ops.push(self.compile_expr(builder, arg_expr_node));
    }

    if let Some(AstNode::Ident(ident)) = AstNode::cast(callee_node.clone()) {
      let name = ident.name();
      let name = name.as_token().unwrap().text();
      builder.call(name, arg_ops)
    } else {
      builder.iconst(0)
    }
  }

  fn compile_let(&mut self, builder: &mut FunctionBuilder, let_expr: &LetExpr) -> Operand {
    let name = let_expr.name();
    let name = crabstar_frontend::ast::Ident::cast(name.into_node().unwrap())
      .unwrap()
      .name();
    let name = name.as_token().unwrap().text();

    let expr = let_expr.expr();
    let expr_node = expr.as_node().unwrap();
    let val = self.compile_expr(builder, expr_node);

    self.bind_var(name, val);

    let in_expr = let_expr.in_expr();
    let in_expr_node = in_expr.as_node().unwrap();

    if in_expr_node.first_child().is_none() {
      builder.iconst(0)
    } else {
      let in_expr = crabstar_frontend::ast::InExpr::cast(in_expr_node.clone()).unwrap();
      let expr = in_expr.expr();
      let expr_node = expr.as_node().unwrap();
      self.compile_expr(builder, expr_node)
    }
  }

  fn compile_match(&mut self, builder: &mut FunctionBuilder, match_expr: &MatchExpr) -> Operand {
    let discriminant = match_expr.discriminant();
    let discriminant_node = discriminant.as_node().unwrap();
    let discriminant_val = self.compile_expr(builder, discriminant_node);

    let branches = match_expr.match_branches();
    let branches_node = branches.as_node().unwrap();
    let branches = crabstar_frontend::ast::MatchBranches::cast(branches_node.clone()).unwrap();

    let mut result = builder.iconst(0);

    for branch in branches.children() {
      if let Some(branch) = crabstar_frontend::ast::MatchBranch::cast(branch) {
        let pattern = branch.pattern();
        let pattern_node = pattern.as_node().unwrap();
        let pattern_val = self.compile_expr(builder, pattern_node);

        let when_clause = branch.when_clause();
        let when_clause_node = when_clause.as_node().unwrap();
        let when_clause =
          crabstar_frontend::ast::WhenClause::cast(when_clause_node.clone()).unwrap();
        let guard = when_clause.guard_clause();
        let guard_node = guard.as_node().unwrap();

        let cond = if guard_node.first_child().is_some() {
          let guard_val = self.compile_expr(builder, guard_node);
          let pattern_match = builder.eq(discriminant_val, pattern_val);
          builder.mul(pattern_match, guard_val)
        } else {
          builder.eq(discriminant_val, pattern_val)
        };

        let expr = branch.expr();
        let expr_node = expr.as_node().unwrap();

        result = builder.if_else(
          cond,
          &[],
          |b, _| {
            let mut compiler = Compiler::new();
            compiler.env = self.env.clone();
            compiler.compile_expr(b, expr_node)
          },
          |b, _| b.iconst(0),
        );
      }
    }

    let else_clause = match_expr.else_clause();
    let else_clause_node = else_clause.as_node().unwrap();
    if else_clause_node.first_child().is_some() {
      if let Some(else_clause) = crabstar_frontend::ast::ElseClause::cast(else_clause_node.clone())
      {
        let expr = else_clause.expr();
        let expr_node = expr.as_node().unwrap();
        result = self.compile_expr(builder, expr_node);
      }
    }

    result
  }
}
