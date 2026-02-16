use crate::{
  ast::{
    ArgList, AstNode, BinaryExpr, CallExpr, FnExpr, Ident, InExpr, LetExpr, Literal, MethodCall,
    ParamList,
  },
  syntax::{SyntaxKind, SyntaxNode},
  types::{FuncType, Type},
};
use std::collections::HashMap;

pub struct TypeChecker {
  env: Vec<HashMap<String, SyntaxNode>>,
  call_stack: Vec<(String, FuncType)>,
  checked_nodes: HashMap<SyntaxNode, Type>,
  in_generic_context: bool,
}

impl TypeChecker {
  pub fn new() -> Self {
    TypeChecker {
      env: vec![HashMap::new()],
      call_stack: Vec::new(),
      checked_nodes: HashMap::new(),
      in_generic_context: false,
    }
  }

  fn push_scope(&mut self) {
    self.env.push(HashMap::new());
  }

  fn pop_scope(&mut self) {
    self.env.pop();
  }

  fn lookup_var(&self, name: &str) -> Option<&SyntaxNode> {
    for scope in self.env.iter().rev() {
      if let Some(node) = scope.get(name) {
        return Some(node);
      }
    }
    None
  }

  fn new_var(&mut self, name: &str, node: SyntaxNode) {
    self.env.last_mut().unwrap().insert(name.to_string(), node);
  }

  pub fn check(&mut self, node: &SyntaxNode) -> Type {
    if !self.in_generic_context {
      if let Some(cached) = self.checked_nodes.get(node) {
        return cached.clone();
      }
    }

    let ty = match AstNode::cast(node.clone()) {
      Some(AstNode::Literal(node)) => self.check_literal(&node),
      Some(AstNode::Ident(node)) => self.check_ident(&node),
      Some(AstNode::LetExpr(node)) => self.check_let_expr(&node),
      Some(AstNode::FnExpr(node)) => self.check_fn_expr(&node),
      Some(AstNode::CallExpr(node)) => self.check_call_expr(&node),
      Some(AstNode::MethodCall(node)) => self.check_method_call(&node),
      Some(AstNode::BinaryExpr(node)) => self.check_binary_expr(&node),
      _ => todo!(
        "Finish implementing type checking for type: {:?}",
        node.kind()
      ),
    };

    if !self.in_generic_context {
      self.checked_nodes.insert(node.clone(), ty.clone());
    }

    ty
  }

  fn check_literal(&mut self, node: &Literal) -> Type {
    match node.literal().kind() {
      SyntaxKind::Int => Type::Int32,
      SyntaxKind::Float => Type::Float64,
      SyntaxKind::KwTrue | SyntaxKind::KwFalse => Type::Bool,
      SyntaxKind::String => Type::String,
      SyntaxKind::KwNull => Type::Null,
      _ => unreachable!(),
    }
  }

  fn check_ident(&mut self, node: &Ident) -> Type {
    let name = node.name();
    let name = name.as_token().unwrap().text();

    if let Some(def_node) = self.lookup_var(name).cloned() {
      self.check(&def_node)
    } else {
      Type::Error
    }
  }

  fn check_let_expr(&mut self, node: &LetExpr) -> Type {
    let is_top_level = self.env.len() == 1;

    if !is_top_level {
      self.push_scope();
    }

    let name = node.name();
    let name = Ident::cast(name.into_node().unwrap()).unwrap().name();
    let name = name.as_token().unwrap().text();
    let expr = node.expr();
    let expr = expr.as_node().unwrap();
    self.new_var(name, expr.clone());
    let in_expr = node.in_expr();
    let in_expr = in_expr.as_node().unwrap();

    let result = if in_expr.first_child().is_none() {
      Type::Null
    } else {
      let in_expr = InExpr::cast(in_expr.clone()).unwrap();
      self.check(in_expr.expr().as_node().unwrap())
    };

    if !is_top_level {
      self.pop_scope();
    }

    result
  }

  fn check_fn_expr(&mut self, node: &FnExpr) -> Type {
    let was_generic = self.in_generic_context;
    self.in_generic_context = true;

    self.push_scope();

    let param_list = node.param_list();
    let param_list = param_list.as_node().unwrap();
    let param_list = ParamList::cast(param_list.clone()).unwrap();
    let mut param_types = Vec::new();

    for param in param_list.params() {
      let param_name = param.param_name();
      let param_name = param_name.as_node().unwrap();
      let param_name = Ident::cast(param_name.clone()).unwrap();
      let param_name = param_name.name();
      let param_name = param_name.as_token().unwrap().text();
      self.new_var(param_name, param.syntax().clone());
      param_types.push(Type::Generic);
    }

    let body_ty = self.check(node.body().as_node().unwrap());

    self.pop_scope();

    self.in_generic_context = was_generic;

    Type::Fn {
      params: param_types,
      return_type: Box::new(body_ty),
    }
  }

  fn check_call_expr(&mut self, node: &CallExpr) -> Type {
    let callee = node.callee();
    let callee = callee.as_node().unwrap();
    let callee_name = if let Some(ident) = Ident::cast(callee.clone()) {
      ident.name().as_token().unwrap().text().to_string()
    } else {
      return Type::Error;
    };

    let args_binding = node.args();
    let args_node = args_binding.as_node().unwrap();
    let args_list = ArgList::cast(args_node.clone()).unwrap();
    let mut arg_types = Vec::new();
    let mut arg_nodes = Vec::new();

    for arg in args_list.args() {
      let arg_expr_binding = arg.arg_expr();
      let arg_expr = arg_expr_binding.as_node().unwrap();
      arg_types.push(self.check(arg_expr));
      arg_nodes.push(arg_expr.clone());
    }

    let func_type = FuncType {
      params: arg_types.clone(),
      return_type: Type::Generic,
    };

    if let Some((_, existing)) = self
      .call_stack
      .iter()
      .find(|(name, _)| name == &callee_name)
    {
      if *existing != func_type {
        panic!("Infinite monomorphization!");
      }
      return existing.return_type.clone();
    }

    self.call_stack.push((callee_name.clone(), func_type));
    let func_def_node = self.lookup_var(&callee_name).cloned();

    if let Some(func_node) = func_def_node {
      if let Some(AstNode::FnExpr(func_expr)) = AstNode::cast(func_node) {
        self.push_scope();

        let param_list_binding = func_expr.param_list();
        let param_list = param_list_binding.as_node().unwrap();
        let param_list = ParamList::cast(param_list.clone()).unwrap();

        for (param, arg_node) in param_list.params().zip(arg_nodes.iter()) {
          let param_name_binding = param.param_name();
          let param_name = param_name_binding.as_node().unwrap();
          let param_name = Ident::cast(param_name.clone()).unwrap();
          let param_name_token = param_name.name();
          let param_name = param_name_token.as_token().unwrap().text();
          self.new_var(param_name, arg_node.clone());
        }

        let return_ty = self.check(func_expr.body().as_node().unwrap());
        self.pop_scope();
        self.call_stack.pop();
        return_ty
      } else {
        self.call_stack.pop();
        Type::Error
      }
    } else {
      self.call_stack.pop();
      Type::Error
    }
  }

  fn check_method_call(&mut self, node: &MethodCall) -> Type {
    let lhs_binding = node.lhs();
    let lhs = lhs_binding.as_node().unwrap();
    let lhs_ty = self.check(lhs);

    let method_name = node.method_name();
    let method_name = method_name.as_token().unwrap().text();

    let args_binding = node.args();
    let args_node = args_binding.as_node().unwrap();
    let args_list = ArgList::cast(args_node.clone()).unwrap();

    let mut arg_types = Vec::new();
    for arg in args_list.args() {
      let arg_expr_binding = arg.arg_expr();
      let arg_expr = arg_expr_binding.as_node().unwrap();
      arg_types.push(self.check(arg_expr));
    }

    self.check_method(&lhs_ty, method_name, &arg_types)
  }

  fn check_binary_expr(&mut self, node: &BinaryExpr) -> Type {
    let lhs_binding = node.lhs();
    let lhs = lhs_binding.as_node().unwrap();
    let lhs_ty = self.check(lhs);

    let rhs_binding = node.rhs();
    let rhs = rhs_binding.as_node().unwrap();
    let rhs_ty = self.check(rhs);

    let op = node.operator();
    let op = op.as_node().unwrap();
    let op = op.children_with_tokens().nth(1).unwrap();
    let op = op.as_token().unwrap().text();

    self.check_method(&lhs_ty, op, &vec![rhs_ty])
  }

  fn check_method(&self, ty: &Type, method: &str, args: &[Type]) -> Type {
    match ty {
      Type::Error => Type::Error,
      Type::Generic => Type::Generic,
      Type::Int32 => match method {
        "+" | "-" | "*" | "/" | "%" if args.len() == 1 && args[0] == Type::Int32 => Type::Int32,
        "<" | ">" | "<=" | ">=" | "=" | "!=" if args.len() == 1 && args[0] == Type::Int32 => {
          Type::Bool
        }
        _ => Type::Error,
      },
      Type::Float64 => match method {
        "+" | "-" | "*" | "/" if args.len() == 1 && args[0] == Type::Float64 => Type::Float64,
        "<" | ">" | "<=" | ">=" | "=" | "!=" if args.len() == 1 && args[0] == Type::Float64 => {
          Type::Bool
        }
        _ => Type::Error,
      },
      Type::Bool => match method {
        "and" | "or" if args.len() == 1 && args[0] == Type::Bool => Type::Bool,
        "=" | "!=" if args.len() == 1 && args[0] == Type::Bool => Type::Bool,
        _ => Type::Error,
      },
      Type::String => match method {
        "+" if args.len() == 1 && args[0] == Type::String => Type::String,
        "=" | "!=" if args.len() == 1 && args[0] == Type::String => Type::Bool,
        _ => Type::Error,
      },
      _ => Type::Error,
    }
  }
}
