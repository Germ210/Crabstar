use crate::{
  ast::{
    ArgList, AstNode, BehaviorDef, BinaryExpr, CallExpr, ConstructorList, ConstructorParamList,
    ElseClause, FieldAccess, FnExpr, Ident, InExpr, LetExpr, Literal, MatchBranch, MatchBranches,
    MatchExpr, MethodCall, MethodList, NewExpr, ParamList, PrefixExpr, RequirementList, StructDef,
    StructFieldList, TypeApp, TypeDecl, TypeExpr, WhenClause, WithClause, WithExpr,
  },
  syntax::{SyntaxKind, SyntaxNode},
  types::{BehaviorType, FuncType, Type, UnionConstructor},
};
use std::collections::HashMap;

pub struct TypeChecker {
  env: Vec<HashMap<String, SyntaxNode>>,
  call_stack: Vec<(String, FuncType)>,
  checked_nodes: HashMap<(SyntaxNode, Vec<Type>), Type>,
  in_generic_context: bool,
  current_args: Vec<Type>,
  pub behaviors: HashMap<String, BehaviorType>,
  pub declared_types: HashMap<String, Type>,
  behavior_defs: HashMap<String, SyntaxNode>,
}

impl TypeChecker {
  pub fn new() -> Self {
    TypeChecker {
      env: vec![HashMap::new()],
      call_stack: Vec::new(),
      checked_nodes: HashMap::new(),
      in_generic_context: false,
      current_args: Vec::new(),
      behaviors: HashMap::new(),
      declared_types: HashMap::new(),
      behavior_defs: HashMap::new(),
    }
  }

  pub fn into_types(self) -> HashMap<(SyntaxNode, Vec<Type>), Type> {
    self.checked_nodes
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

  pub fn register_type(&mut self, name: String, ty: Type) {
    self.declared_types.insert(name, ty);
  }

  pub fn register_behavior(&mut self, name: String, behavior: BehaviorType) {
    self.behaviors.insert(name, behavior);
  }

  pub fn resolve_all_types(&mut self) {
    let type_names: Vec<String> = self.declared_types.keys().cloned().collect();

    for type_name in type_names {
      let ty = self.declared_types.get(&type_name).unwrap().clone();
      let resolved = self.deep_resolve_type(&ty);
      self.declared_types.insert(type_name, resolved);
    }
  }

  fn deep_resolve_type(&self, ty: &Type) -> Type {
    match ty {
      Type::Var(name) => self.resolve_type_name(name),
      Type::Struct { fields } => {
        let resolved_fields = fields
          .iter()
          .map(|(fname, ftype)| (fname.clone(), self.deep_resolve_type(ftype)))
          .collect();
        Type::Struct {
          fields: resolved_fields,
        }
      }
      Type::Ref(inner) => Type::Ref(Box::new(self.deep_resolve_type(inner))),
      _ => ty.clone(),
    }
  }
  pub fn check_type_decl(&mut self, node: &TypeDecl) {
    let name_node = node.name();
    let name = if let Some(ident_node) = name_node.as_node() {
      if let Some(ident) = Ident::cast(ident_node.clone()) {
        ident.name().as_token().unwrap().text().to_string()
      } else {
        return;
      }
    } else {
      return;
    };

    let body_node = node.body();
    let body = body_node.as_node().unwrap();

    if let Some(struct_def) = StructDef::cast(body.clone()) {
      let fields_node = struct_def.fields();
      let fields_node = fields_node.as_node().unwrap();
      let field_list = StructFieldList::cast(fields_node.clone()).unwrap();

      let mut fields = Vec::new();
      for field in field_list.fields() {
        let field_name_node = field.name();
        let field_name_node = field_name_node.as_node().unwrap();
        let field_name = Ident::cast(field_name_node.clone()).unwrap();
        let field_name_str = field_name.name().as_token().unwrap().text().to_string();

        let field_type_node = field.value();
        let field_type_node = field_type_node.as_node().unwrap();
        let field_type = self.resolve_type_expr(field_type_node);

        fields.push((field_name_str, field_type));
      }

      self.register_type(name.clone(), Type::Struct { fields });
    } else if let Some(constructor_list) = ConstructorList::cast(body.clone()) {
      let mut constructors = Vec::new();

      for ctor in constructor_list.constructors() {
        let type_ctor_node = ctor.type_constructor();
        let type_ctor_node = type_ctor_node.as_node().unwrap();
        let type_ctor = crate::ast::TypeConstructor::cast(type_ctor_node.clone()).unwrap();

        let ctor_name_node = type_ctor.name();
        let ctor_name_node = ctor_name_node.as_node().unwrap();
        let ctor_name = Ident::cast(ctor_name_node.clone()).unwrap();
        let ctor_name_str = ctor_name.name().as_token().unwrap().text().to_string();

        let params_node = type_ctor.params();
        let params_node = params_node.as_node().unwrap();
        let param_list = ConstructorParamList::cast(params_node.clone()).unwrap();

        let mut params = Vec::new();
        for param in param_list.params() {
          let type_name = param.type_name();
          let type_name = type_name.as_node().unwrap();
          let type_name = Ident::cast(type_name.clone()).unwrap();
          let type_name = type_name.name();
          let type_name = type_name.as_token().unwrap().text();

          let param_type = self.resolve_type_name(type_name);
          params.push(param_type);
        }

        let return_types_node = type_ctor.return_types();
        let return_types_node = return_types_node.as_node().unwrap();
        let return_type_list = ConstructorParamList::cast(return_types_node.clone()).unwrap();

        let mut return_types = Vec::new();
        for return_param in return_type_list.params() {
          let type_name = return_param.type_name();
          let type_name = type_name.as_node().unwrap();
          let type_name = Ident::cast(type_name.clone()).unwrap();
          let type_name = type_name.name();
          let type_name = type_name.as_token().unwrap().text();

          let return_type = self.resolve_type_name(type_name);
          return_types.push(return_type);
        }

        constructors.push(UnionConstructor {
          name: ctor_name_str,
          params,
          return_types,
        });
      }

      self.register_type(name.clone(), Type::Union { constructors });
    }
  }

  fn resolve_type_expr(&self, type_expr_node: &SyntaxNode) -> Type {
    if let Some(type_expr) = TypeExpr::cast(type_expr_node.clone()) {
      let inner = type_expr.inner_type();
      if let Some(inner_node) = inner.as_node() {
        if let Some(type_app) = TypeApp::cast(inner_node.clone()) {
          let base = type_app.base_type();
          if let Some(node) = base.as_node() {
            if let Some(ident) = Ident::cast(node.clone()) {
              let name = ident.name().as_token().unwrap().text().to_string();
              return Type::Var(name);
            }
          }
        }
      }
    }
    Type::Generic
  }

  pub fn check_behavior_def(&mut self, node: &BehaviorDef) {
    let name_node = node.name();
    let name = if let Some(ident_node) = name_node.as_node() {
      if let Some(ident) = Ident::cast(ident_node.clone()) {
        ident.name().as_token().unwrap().text().to_string()
      } else {
        return;
      }
    } else {
      return;
    };

    let requirements_node = node.requirements();
    let requirements_node = requirements_node.as_node().unwrap();
    let requirement_list = RequirementList::cast(requirements_node.clone()).unwrap();

    let mut requirements = HashMap::new();
    for req in requirement_list.fields() {
      let req_name_node = req.name();
      let req_name_node = req_name_node.as_node().unwrap();
      let req_name = Ident::cast(req_name_node.clone()).unwrap();
      let req_name_str = req_name.name().as_token().unwrap().text().to_string();

      let req_type_node = req.type_expr();
      let req_type_node = req_type_node.as_node().unwrap();
      let req_type = self.parse_type_expr(req_type_node);

      requirements.insert(req_name_str, req_type);
    }

    let methods_node = node.methods();
    let methods_node = methods_node.as_node().unwrap();
    let method_list = MethodList::cast(methods_node.clone()).unwrap();

    let mut methods = HashMap::new();
    for method in method_list.methods() {
      let method_name_node = method.name();
      let method_name_node = method_name_node.as_node().unwrap();
      let method_name = Ident::cast(method_name_node.clone()).unwrap();
      let method_name_str = method_name.name().as_token().unwrap().text().to_string();

      let param_list_node = method.param_list();
      let param_list_node = param_list_node.as_node().unwrap();
      let param_list = ParamList::cast(param_list_node.clone()).unwrap();

      let mut param_types = Vec::new();
      for param in param_list.params() {
        let type_expr_node = param.type_expr();
        let param_type = if let Some(type_node) = type_expr_node.as_node() {
          self.parse_type_expr(type_node)
        } else {
          Type::Generic
        };
        param_types.push(param_type);
      }

      let declared_return_type_node = method.return_type();
      let declared_return_type = if let Some(ret_type_node) = declared_return_type_node.as_node() {
        self.parse_type_expr(ret_type_node)
      } else {
        Type::Generic
      };

      methods.insert(
        method_name_str,
        FuncType {
          params: param_types,
          return_type: declared_return_type,
        },
      );
    }

    self.register_behavior(
      name.clone(),
      BehaviorType {
        requirements,
        methods,
      },
    );
    self
      .behavior_defs
      .insert(name.clone(), node.syntax().clone());
  }

  fn resolve_type_name(&self, type_name: &str) -> Type {
    match type_name {
      "int32" => Type::Int32,
      "int64" => Type::Int64,
      "float32" => Type::Float32,
      "float64" => Type::Float64,
      "bool" => Type::Bool,
      "string" => Type::String,
      "null" => Type::Null,
      _ => {
        if let Some(ty) = self.declared_types.get(type_name) {
          ty.clone()
        } else {
          Type::Generic
        }
      }
    }
  }

  pub fn check(&mut self, node: &SyntaxNode) -> Type {
    if !self.in_generic_context {
      let key = (node.clone(), self.current_args.clone());
      if let Some(cached) = self.checked_nodes.get(&key) {
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
      Some(AstNode::ParenExpr(node)) => self.check(node.inner().as_node().unwrap()),
      Some(AstNode::PrefixExpr(node)) => self.check_prefix_expr(&node),
      Some(AstNode::MatchExpr(node)) => self.check_match_expr(&node),
      Some(AstNode::NewExpr(node)) => self.check_new_expr(&node),
      Some(AstNode::FieldAccess(node)) => self.check_field_access(&node),
      Some(AstNode::WithExpr(node)) => self.check_with_expr(&node),
      Some(AstNode::Param(_)) => Type::Generic,
      _ => todo!(
        "Finish implementing type checking for type: {:?}",
        node.kind()
      ),
    };

    if !self.in_generic_context {
      let key = (node.clone(), self.current_args.clone());
      self.checked_nodes.insert(key, ty.clone());
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
      let ty = self.check(&def_node);
      ty
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
      if is_top_level {
        panic!("Top level in expression not allowed");
      }
      let in_expr = InExpr::cast(in_expr.clone()).unwrap();
      self.check(in_expr.expr().as_node().unwrap())
    };

    if !is_top_level {
      self.pop_scope();
    }

    result
  }

  fn parse_type_expr(&self, type_expr_node: &SyntaxNode) -> Type {
    if type_expr_node.first_child().is_none() {
      return Type::Generic;
    }

    if let Some(type_expr) = TypeExpr::cast(type_expr_node.clone()) {
      let inner = type_expr.inner_type();
      if let Some(inner_node) = inner.as_node() {
        if let Some(type_app) = TypeApp::cast(inner_node.clone()) {
          let base_type = type_app.base_type();
          if let Some(base_node) = base_type.as_node() {
            if let Some(ident) = Ident::cast(base_node.clone()) {
              let name = ident.name();
              let type_name = name.as_token().unwrap().text();
              return self.resolve_type_name(type_name);
            }
          }
        } else if let Some(ident) = Ident::cast(inner_node.clone()) {
          let name = ident.name();
          let type_name = name.as_token().unwrap().text();
          return self.resolve_type_name(type_name);
        }
      }
    }
    Type::Generic
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
      let param_name_str = param_name.name();
      let param_name_str = param_name_str.as_token().unwrap().text();

      let type_expr = param.type_expr();
      let param_type = if let Some(type_node) = type_expr.as_node() {
        self.parse_type_expr(type_node)
      } else {
        Type::Generic
      };

      self.new_var(param_name_str, param.syntax().clone());
      param_types.push(param_type);
    }

    let return_type_binding = node.return_type();
    let declared_return_type = if let Some(return_type_node) = return_type_binding.as_node() {
      self.parse_type_expr(return_type_node)
    } else {
      Type::Generic
    };

    let body_ty = self.check(node.body().as_node().unwrap());

    let final_return_type = if declared_return_type != Type::Generic {
      declared_return_type
    } else {
      body_ty
    };

    self.pop_scope();
    self.in_generic_context = was_generic;

    Type::Fn {
      params: param_types,
      return_type: Box::new(final_return_type),
      source_node: Some(node.syntax().clone()),
    }
  }

  fn check_call_expr(&mut self, node: &CallExpr) -> Type {
    let callee = node.callee();
    let callee = callee.as_node().unwrap();
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

    let func_node_opt = if let Some(AstNode::Ident(ident)) = AstNode::cast(callee.clone()) {
      let name = ident.name();
      let name = name.as_token().unwrap().text();
      self.lookup_var(name).cloned()
    } else {
      None
    };

    if let Some(func_node) = func_node_opt.clone() {
      let start = func_node.text_range().start();
      let base_key = format!("{:?}", start);

      for (key, existing) in self.call_stack.iter() {
        if key.starts_with(&base_key) {
          if existing.params != arg_types {
            panic!("Impossible monomorphization!");
          }
          return existing.return_type.clone();
        }
      }

      if let Some(AstNode::FnExpr(func_expr)) = AstNode::cast(func_node.clone()) {
        let callee_key = format!("{:?}@{:?}", start, arg_types);
        let func_type = FuncType {
          params: arg_types.clone(),
          return_type: Type::Generic,
        };

        self.call_stack.push((callee_key, func_type));

        let was_generic = self.in_generic_context;
        let prev_args = self.current_args.clone();
        self.in_generic_context = false;
        self.current_args = arg_types.clone();
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
        self.in_generic_context = was_generic;
        self.current_args = prev_args;
        self.pop_scope();
        self.call_stack.pop();
        return return_ty;
      }
    }

    let callee_ty = self.check(callee);

    match callee_ty {
      Type::Fn {
        params,
        return_type,
        source_node,
      } => {
        if params.len() != arg_types.len() {
          return Type::Error;
        }

        for (param_ty, arg_ty) in params.iter().zip(arg_types.iter()) {
          if param_ty != &Type::Generic && arg_ty != &Type::Generic && param_ty != arg_ty {
            return Type::Error;
          }
        }

        if let Some(func_node) = source_node {
          if let Some(AstNode::FnExpr(func_expr)) = AstNode::cast(func_node.clone()) {
            let start = func_node.text_range().start();
            let callee_key = format!("{:?}@{:?}", start, arg_types);
            let func_type = FuncType {
              params: arg_types.clone(),
              return_type: Type::Generic,
            };

            self.call_stack.push((callee_key, func_type));

            let was_generic = self.in_generic_context;
            let prev_args = self.current_args.clone();
            self.in_generic_context = false;
            self.current_args = arg_types.clone();
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
            self.in_generic_context = was_generic;
            self.current_args = prev_args;
            self.pop_scope();
            self.call_stack.pop();
            return return_ty;
          }
        }

        *return_type
      }
      Type::Error => Type::Error,
      Type::Generic => Type::Generic,
      _ => Type::Error,
    }
  }

  fn check_method_call(&mut self, node: &MethodCall) -> Type {
    let lhs_binding = node.lhs();
    let lhs = lhs_binding.as_node().unwrap();
    let lhs_ty = self.check(lhs);

    let method_name_binding = node.method_name();
    let method_name_node = method_name_binding.as_node().unwrap();
    let method_name_ident = Ident::cast(method_name_node.clone()).unwrap();
    let method_name = method_name_ident.name();
    let method_name = method_name.as_token().unwrap().text();

    let args_binding = node.args();
    let args_node = args_binding.as_node().unwrap();
    let args_list = ArgList::cast(args_node.clone()).unwrap();

    let mut arg_types = vec![lhs_ty.clone()];
    let mut arg_nodes = vec![lhs.clone()];

    for arg in args_list.args() {
      let arg_expr_binding = arg.arg_expr();
      let arg_expr = arg_expr_binding.as_node().unwrap();
      arg_types.push(self.check(arg_expr));
      arg_nodes.push(arg_expr.clone());
    }

    self.check_method_with_nodes(&lhs_ty, method_name, &arg_types, &arg_nodes)
  }

  fn check_method_with_nodes(
    &mut self,
    ty: &Type,
    method: &str,
    args: &[Type],
    arg_nodes: &[SyntaxNode],
  ) -> Type {
    match ty {
      Type::WithBehavior { behavior_name, .. } => {
        let Some(behavior) = self.behaviors.get(behavior_name).cloned() else {
          return Type::Error;
        };

        let Some(method_sig) = behavior.methods.get(method).cloned() else {
          return Type::Error;
        };

        if method_sig.params.len() != args.len() {
          return Type::Error;
        }

        for (param_ty, arg_ty) in method_sig.params.iter().zip(args.iter()) {
          if param_ty != &Type::Generic && arg_ty != &Type::Generic && param_ty != arg_ty {
            return Type::Error;
          }
        }

        if method_sig.return_type != Type::Generic {
          return method_sig.return_type;
        }

        let Some(behavior_def_node) = self.behavior_defs.get(behavior_name).cloned() else {
          return Type::Generic;
        };

        let behavior_def = BehaviorDef::cast(behavior_def_node).unwrap();
        let methods_binding = behavior_def.methods();
        let methods_node = methods_binding.as_node().unwrap();
        let method_list = MethodList::cast(methods_node.clone()).unwrap();

        for method_def in method_list.methods() {
          let method_name_binding = method_def.name();
          let method_name_node = method_name_binding.as_node().unwrap();
          let method_ident = Ident::cast(method_name_node.clone()).unwrap();
          let method_name_token = method_ident.name();
          let method_name_str = method_name_token.as_token().unwrap().text();

          if method_name_str == method {
            self.push_scope();

            let param_list_binding = method_def.param_list();
            let param_list_node = param_list_binding.as_node().unwrap();
            let param_list = ParamList::cast(param_list_node.clone()).unwrap();

            for (param, arg_node) in param_list.params().zip(arg_nodes.iter()) {
              let param_name_binding = param.param_name();
              let param_name_node = param_name_binding.as_node().unwrap();
              let param_ident = Ident::cast(param_name_node.clone()).unwrap();
              let param_name_token = param_ident.name();
              let param_name = param_name_token.as_token().unwrap().text();
              self.new_var(param_name, arg_node.clone());
            }

            let body_binding = method_def.body();
            let body_node = body_binding.as_node().unwrap();
            let return_ty = self.check(body_node);

            self.pop_scope();
            return return_ty;
          }
        }

        Type::Generic
      }
      _ => self.check_method(ty, method, args),
    }
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
        "-" if args.is_empty() => Type::Int32,
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
        "not" if args.is_empty() => Type::Bool,
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

  fn check_prefix_expr(&mut self, node: &PrefixExpr) -> Type {
    let rhs_ty = self.check(node.rhs().as_node().unwrap());
    let op = node.operator();
    let op = op.as_node().unwrap();
    let op = op.children_with_tokens().nth(1).unwrap();
    let op = op.as_token().unwrap().text();
    self.check_method(&rhs_ty, op, &[])
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

  fn check_match_expr(&mut self, node: &MatchExpr) -> Type {
    let discriminant = node.discriminant();
    let discriminant = discriminant.as_node().unwrap();
    let discriminant_ty = self.check(discriminant);

    let branches = node.match_branches();
    let branches = branches.as_node().unwrap();
    let branches = MatchBranches::cast(branches.clone()).unwrap();

    let mut branch_types = Vec::new();

    for branch in branches.children() {
      if let Some(branch) = MatchBranch::cast(branch) {
        let pattern = branch.pattern();
        let pattern = pattern.as_node().unwrap();
        let pattern_ty = self.check(pattern);

        if pattern_ty != Type::Generic
          && discriminant_ty != Type::Generic
          && pattern_ty != discriminant_ty
        {
          return Type::Error;
        }

        let when_clause = branch.when_clause();
        let when_clause = when_clause.as_node().unwrap();
        let when_clause = WhenClause::cast(when_clause.clone()).unwrap();
        let guard = when_clause.guard_clause();
        let guard = guard.as_node().unwrap();
        if guard.first_child().is_some() {
          let guard_ty = self.check(guard);
          if guard_ty != Type::Bool && guard_ty != Type::Generic {
            return Type::Error;
          }
        }

        let expr = branch.expr();
        let expr = expr.as_node().unwrap();
        branch_types.push(self.check(expr));
      }
    }

    let else_clause = node.else_clause();
    let else_clause = else_clause.as_node().unwrap();
    if else_clause.first_child().is_some() {
      if let Some(else_clause) = ElseClause::cast(else_clause.clone()) {
        branch_types.push(self.check(else_clause.expr().as_node().unwrap()));
      }
    }

    let first = branch_types.first().cloned().unwrap_or(Type::Error);

    if branch_types
      .iter()
      .all(|t| t == &first || t == &Type::Generic)
    {
      first
    } else {
      Type::Error
    }
  }

  fn check_with_expr(&mut self, node: &WithExpr) -> Type {
    let lhs_ty = self.check(node.lhs().as_node().unwrap());

    let with_clause = node.with_clause();
    let with_clause = with_clause.as_node().unwrap();
    let with_clause = WithClause::cast(with_clause.clone()).unwrap();
    let behavior_node = with_clause.behavior();
    let behavior_node = behavior_node.as_node().unwrap();
    let behavior_ident = Ident::cast(behavior_node.clone()).unwrap();
    let behavior_name_token = behavior_ident.name();
    let behavior_name = behavior_name_token.as_token().unwrap().text();

    if let Some(behavior) = self.behaviors.get(behavior_name) {
      if self.type_satisfies_requirements(&lhs_ty, &behavior.requirements) {
        return Type::WithBehavior {
          base_type: Box::new(lhs_ty),
          behavior_name: behavior_name.to_string(),
        };
      }
    }
    Type::Error
  }

  fn type_satisfies_requirements(&self, ty: &Type, requirements: &HashMap<String, Type>) -> bool {
    let struct_ty = match ty {
      Type::Struct { fields, .. } => fields,
      Type::WithBehavior { base_type, .. } => {
        if let Type::Struct { fields, .. } = base_type.as_ref() {
          fields
        } else {
          return false;
        }
      }
      _ => return false,
    };

    for (req_field, req_type) in requirements {
      let has_field = struct_ty
        .iter()
        .any(|(fname, ftype)| fname == req_field && ftype == req_type);
      if !has_field {
        return false;
      }
    }
    true
  }

  fn check_new_expr(&mut self, node: &NewExpr) -> Type {
    let struct_name = node.struct_name();
    let struct_name = struct_name.as_node().unwrap();
    let struct_name = Ident::cast(struct_name.clone()).unwrap();
    let struct_name = struct_name.name();
    let struct_name = struct_name.as_token().unwrap().text();

    if let Some(ty) = self.declared_types.get(struct_name) {
      return ty.clone();
    }
    Type::Error
  }

  fn check_field_access(&mut self, node: &FieldAccess) -> Type {
    let struct_ty = self.check(node.structure().as_node().unwrap());
    let field_node = node.field();
    let field_node = field_node.as_node().unwrap();
    let field_ident = Ident::cast(field_node.clone()).unwrap();
    let field_name = field_ident.name();
    let field_name = field_name.as_token().unwrap().text();

    let base_struct = match struct_ty {
      Type::Struct { fields, .. } => fields,
      Type::WithBehavior { base_type, .. } => match base_type.as_ref() {
        Type::Struct { fields, .. } => fields.clone(),
        Type::WithBehavior {
          base_type: inner_base,
          ..
        } => match inner_base.as_ref() {
          Type::Struct { fields, .. } => fields.clone(),
          _ => return Type::Error,
        },
        _ => return Type::Error,
      },
      _ => return Type::Error,
    };

    for (fname, ftype) in base_struct {
      if fname == field_name {
        return ftype;
      }
    }
    Type::Error
  }
}
