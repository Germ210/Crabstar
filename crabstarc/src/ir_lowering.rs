use crabstar_backend::{
  abi::types::{AbiType, FfiCif, FfiType},
  ir::{
    builder::FunctionBuilder,
    graph::{Cfg, Operand, Terminator},
  },
};
use crabstar_frontend::{
  ast::{AstNode, BinaryExpr, CallExpr, Ident, LetExpr, Literal, PrefixExpr},
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

  pub fn build_with_ffi<C>(&mut self, let_expr: &LetExpr, ty: &Type) -> Option<(Cfg, FfiCif<C>)>
  where
    C: crabstar_backend::abi::types::CallingConvention,
    C::CifData: Default,
    C::Abi: Default,
  {
    let cfg_opt = self.compile_let_expr_fn(let_expr, ty)?;
    let (cfg, param_types) = cfg_opt;

    let ffi_params: Vec<FfiType> = param_types
      .iter()
      .map(|ty| FfiType {
        size: 8,
        alignment: 8,
        ty: ty.clone(),
        elements: vec![],
      })
      .collect();

    let ret_abi = cfg
      .blocks
      .last()
      .and_then(|b| match &b.terminator {
        Terminator::Return(Some(Operand::Val(v))) => cfg
          .blocks
          .iter()
          .flat_map(|blk| blk.params.iter())
          .find(|p| p.val == *v)
          .map(|p| p.ty.clone()),
        Terminator::Return(None) => Some(AbiType::Void),
        _ => Some(AbiType::Void),
      })
      .unwrap_or(AbiType::Void);

    let ffi_ret = FfiType {
      size: 8,
      alignment: 8,
      ty: ret_abi,
      elements: vec![],
    };

    let mut cif = FfiCif::new(C::Abi::default(), ffi_params, ffi_ret, Default::default());
    C::prep(&mut cif);

    Some((cfg, cif))
  }

  pub fn type_to_abi(ty: &Type) -> AbiType {
    dbg!(&ty);
    match ty {
      Type::Int32 => AbiType::I32,
      Type::Int64 => AbiType::I64,
      Type::Float32 => AbiType::Float,
      Type::Float64 => AbiType::Double,
      _ => unimplemented!(),
    }
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
      if let Some(param_list_node) = fn_expr.param_list().as_node() {
        if let Some(param_list) = crabstar_frontend::ast::ParamList::cast(param_list_node.clone()) {
          for (param, op) in param_list.params().zip(param_ops.iter()) {
            if let Some(ident) = param
              .param_name()
              .as_node()
              .map(|n| n.clone())
              .and_then(|n| Ident::cast(n))
            {
              let name_binding = ident.name();
              let name = name_binding.as_token().unwrap().text();
              self.bind_var(name, *op);
            }
          }
        }
      }

      if let Some(body_node) = fn_expr.body().as_node() {
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
      Some(AstNode::MatchExpr(_)) => unimplemented!(),
      _ => unimplemented!(),
    }
  }

  fn compile_literal(&mut self, builder: &mut FunctionBuilder, lit: &Literal) -> Operand {
    use crabstar_frontend::syntax::SyntaxKind;
    match lit.literal().kind() {
      SyntaxKind::Int => {
        let text = lit.literal();
        let text = text.as_token().unwrap().text();
        let val = text.parse::<i64>().unwrap();
        builder.iconst(val)
      }
      _ => unimplemented!(),
    }
  }

  fn compile_ident(&mut self, _builder: &mut FunctionBuilder, ident: &Ident) -> Operand {
    let name_binding = ident.name();
    let name = name_binding.as_token().unwrap().text();
    self.lookup_var(name).unwrap_or_else(|| unimplemented!())
  }

  fn compile_binary(&mut self, builder: &mut FunctionBuilder, binop: &BinaryExpr) -> Operand {
    let lhs_op = self.compile_expr(builder, binop.lhs().as_node().unwrap());
    let rhs_op = self.compile_expr(builder, binop.rhs().as_node().unwrap());

    let op_node = binop.operator().as_node().unwrap().clone();
    let op_token_binding = op_node.children_with_tokens().nth(1).unwrap();
    let op_token = op_token_binding.as_token().unwrap();
    let op_text = op_token.text();

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
      _ => unimplemented!(),
    }
  }

  fn compile_prefix(&mut self, builder: &mut FunctionBuilder, prefix: &PrefixExpr) -> Operand {
    let rhs_op = self.compile_expr(builder, prefix.rhs().as_node().unwrap());

    let op_node = prefix.operator().as_node().unwrap().clone();
    let op_token_binding = op_node.children_with_tokens().nth(1).unwrap();
    let op_token = op_token_binding.as_token().unwrap();
    let op_text = op_token.text();

    match op_text {
      "-" => builder.neg(rhs_op),
      _ => unimplemented!(),
    }
  }

  fn compile_call(&mut self, builder: &mut FunctionBuilder, call: &CallExpr) -> Operand {
    let callee_node = call.callee().as_node().unwrap().clone();
    let args_node = call.args().as_node().unwrap().clone();
    let args_list = crabstar_frontend::ast::ArgList::cast(args_node).unwrap();
    let arg_ops: Vec<Operand> = args_list
      .args()
      .map(|arg| self.compile_expr(builder, arg.arg_expr().as_node().unwrap()))
      .collect();

    if let Some(AstNode::Ident(ident)) = AstNode::cast(callee_node) {
      let name_binding = ident.name();
      let name = name_binding.as_token().unwrap().text();
      builder.call(name, arg_ops)
    } else {
      unimplemented!()
    }
  }

  fn compile_let(&mut self, builder: &mut FunctionBuilder, let_expr: &LetExpr) -> Operand {
    let ident_node =
      crabstar_frontend::ast::Ident::cast(let_expr.name().into_node().unwrap().clone()).unwrap();
    let name_binding = ident_node.name();
    let name = name_binding.as_token().unwrap().text();

    let val = self.compile_expr(builder, let_expr.expr().as_node().unwrap());
    self.bind_var(name, val);

    let in_expr_node = let_expr.in_expr().as_node().unwrap().clone();
    if in_expr_node.first_child().is_none() {
      unimplemented!()
    } else {
      let expr_node = crabstar_frontend::ast::InExpr::cast(in_expr_node).unwrap();
      let expr_node = expr_node.expr();
      let expr_node = expr_node.as_node().unwrap();
      self.compile_expr(builder, expr_node)
    }
  }

  pub fn compile_let_expr_fn(
    &mut self,
    let_expr: &LetExpr,
    ty: &Type,
  ) -> Option<(Cfg, Vec<AbiType>)> {
    if let Some(expr_node) = let_expr.expr().as_node() {
      if let Some(AstNode::FnExpr(_)) = AstNode::cast(expr_node.clone()) {
        if let Type::Fn { params, .. } = ty {
          let param_types: Vec<AbiType> = params.iter().map(|ty| Self::type_to_abi(ty)).collect();
          let cfg = self.compile_function(&expr_node, &param_types);
          return Some((cfg, param_types));
        }
      }
    }
    None
  }
}
