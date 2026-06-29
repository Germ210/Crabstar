use crate::{
  ast::{AstNode, CallExpr, DestructureExpr, Expr, FnExpr, LetExpr, MatchExpr, Param, Pattern},
  err::TypeError,
  syntax::{SyntaxKind, SyntaxNode, SyntaxToken},
  types::{
    FreshCounters, Row, Scheme, Struct, Type, TypeApp, TypeArena, TypeID, VarID, WithBehavior,
    array_type, find_root, float_type, fn_type, format_type, fresh_row_var, fresh_type_var,
    freshen, int_type, int32_type, mut_type, null_type, owned_type, ref_type, string_type,
  },
};
use rowan::GreenNode;
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone)]
pub struct InferenceResult {
  pub assumptions: HashMap<String, TypeID>,
  pub ty: TypeID,
  pub errs: Vec<TypeError>,
}

// Because overloading try is unstable, I don't wanna risk it breaking for no reason and I'd rather use a macro
// Will it ever happen? I don't know
macro_rules! unwrap_or_err {
  ($expr:expr, $self:expr) => {
    match $expr {
      Some(v) => v,
      None => return InferenceResult::new(HashMap::new(), $self.new_err(), vec![]),
    }
  };
}

pub(crate) use unwrap_or_err;

impl InferenceResult {
  pub fn new(assumptions: HashMap<String, TypeID>, ty: TypeID, errs: Vec<TypeError>) -> Self {
    Self {
      assumptions,
      ty,
      errs,
    }
  }
}

#[derive(Debug)]
pub struct Inferencer {
  pub types: TypeArena,
  pub fresh_counter: FreshCounters,
  pub node_types: HashMap<SyntaxNode, TypeID>,
  // This works because if the node is the exact same, it's assumptions about it's environment is the same
  // The actual type gets checked against the environment later
  pub inference_results: HashMap<GreenNode, InferenceResult>,
  pub constructor_names: HashSet<String>,
  pub env: HashMap<String, TypeID>,
}

impl Inferencer {
  pub fn new() -> Self {
    Self {
      types: TypeArena::new(),
      fresh_counter: FreshCounters::new(),
      node_types: HashMap::new(),
      inference_results: HashMap::new(),
      constructor_names: HashSet::new(),
      env: HashMap::new(),
    }
  }

  pub fn format_type(&mut self, ty: TypeID) -> String {
    let mut var_names = HashMap::new();
    let mut counter = 0;
    format_type(&mut self.types, ty, &mut var_names, &mut counter)
  }

  pub fn default_types(&mut self, result: &mut InferenceResult) {
    let int32 = int32_type(&mut self.types);
    for id in self.types.types.keys().collect::<Vec<_>>() {
      let root = find_root(&mut self.types, id);
      if let Type::TypeCons(c) = &self.types.types[root] {
        if Self::is_int_cons(&c.name.clone()) && c.name != "int32" {
          self.types.types[root] = Type::Link(int32);
        }
      }
    }
    result.ty = find_root(&mut self.types, result.ty);
  }

  pub fn infer(&mut self, expr: Expr) -> InferenceResult {
    let inference_res = match expr {
      Expr::Ident(ref ident) => {
        let name = ident.name().as_ref().unwrap().text().to_owned();
        let ty = self.fresh_type_var();
        InferenceResult::new(Self::new_variable(&name, ty), ty, vec![])
      }
      Expr::Literal(ref literal) => {
        let ty = match literal.token() {
          Some(token) => match SyntaxToken::from(token).kind() {
            SyntaxKind::Int => int_type(&mut self.types),
            SyntaxKind::Float => float_type(&mut self.types),
            SyntaxKind::String => string_type(&mut self.types),
            SyntaxKind::KwNull => null_type(&mut self.types),
            _ => unreachable!(),
          },
          // Hopefully unreachable? It should be but I'm not 100% sure
          None => unreachable!(),
        };
        InferenceResult::new(HashMap::new(), ty, vec![])
      }
      Expr::LetExpr(ref let_expr) => self.infer_let(let_expr.clone()),
      Expr::FnExpr(ref fn_expr) => self.infer_fn(fn_expr.clone()),
      Expr::CallExpr(ref call_expr) => self.infer_call(call_expr.clone()),
      Expr::ParenExpr(ref paren) => self.infer(unwrap_or_err!(paren.expr(), self)),
      Expr::DoThenExpr(ref do_then) => {
        let do_res = self.infer(unwrap_or_err!(do_then.do_expr(), self));
        let then_res = self.infer(unwrap_or_err!(do_then.then_expr(), self));
        let mut errs = do_res.errs;
        errs.extend(then_res.errs);
        let (merged, merge_errs) = self.merge(
          do_res.assumptions,
          then_res.assumptions,
          do_then.syntax().clone(),
          do_then.syntax().clone(),
        );
        errs.extend(merge_errs);
        InferenceResult::new(merged, then_res.ty, errs)
      }
      Expr::FieldAccess(ref field_access) => {
        let expr = unwrap_or_err!(field_access.expr(), self);
        let field = unwrap_or_err!(field_access.field(), self)
          .text()
          .to_string();
        let expr_res = self.infer(expr.clone());
        let field_ty = self.fresh_type_var();
        let rest = self.fresh_row_var();
        let row_ty = self
          .types
          .alloc(Type::Row(Row::new(vec![(field, field_ty)], Some(rest))));
        let mut errs = expr_res.errs;
        let (err, _) = self.unify(
          expr_res.ty,
          row_ty,
          expr.syntax().clone(),
          field_access.syntax().clone(),
        );
        if let Some(err) = err {
          errs.push(err);
        }
        InferenceResult::new(expr_res.assumptions, field_ty, errs)
      }
      Expr::Array(ref array) => {
        let mut elem_ty = self.fresh_type_var();
        let mut assumptions = HashMap::new();
        let mut errs = Vec::new();
        for element in array.elements() {
          let expr = unwrap_or_err!(element.expr(), self);
          let res = self.infer(expr.clone());
          errs.extend(res.errs);
          let (err, unified_ty) = self.unify(
            elem_ty,
            res.ty,
            array.syntax().clone(),
            expr.syntax().clone(),
          );
          if let Some(err) = err {
            errs.push(err);
          }
          elem_ty = unified_ty;
          let (merged, merge_errs) = self.merge(
            assumptions,
            res.assumptions,
            array.syntax().clone(),
            expr.syntax().clone(),
          );
          assumptions = merged;
          errs.extend(merge_errs);
        }
        let ty = array_type(&mut self.types, elem_ty);
        InferenceResult::new(assumptions, ty, errs)
      }
      Expr::PrefixExpr(ref prefix) => {
        let expr = unwrap_or_err!(prefix.expr(), self);
        let expr_res = self.infer(expr.clone());
        let ty = match unwrap_or_err!(prefix.op(), self).kind() {
          SyntaxKind::KwRef => ref_type(&mut self.types, expr_res.ty),
          SyntaxKind::KwMut => mut_type(&mut self.types, expr_res.ty),
          SyntaxKind::KwOwned => owned_type(&mut self.types, expr_res.ty),
          SyntaxKind::KwNot | SyntaxKind::Minus => todo!("Add inference for operators"),
          _ => unreachable!(),
        };
        InferenceResult::new(expr_res.assumptions, ty, expr_res.errs)
      }
      Expr::NewExpr(ref new_expr) => {
        let name = unwrap_or_err!(new_expr.name(), self).text().to_string();
        let mut assumptions = HashMap::new();
        let mut errs = Vec::new();
        let mut fields = Vec::new();
        for field in new_expr.fields() {
          let field_name = unwrap_or_err!(field.name(), self).text().to_string();
          let expr = unwrap_or_err!(field.value(), self);
          let res = self.infer(expr.clone());
          errs.extend(res.errs);
          let (merged, merge_errs) = self.merge(
            assumptions,
            res.assumptions,
            new_expr.syntax().clone(),
            expr.syntax().clone(),
          );
          assumptions = merged;
          errs.extend(merge_errs);
          fields.push((field_name, res.ty));
        }
        let provided_struct = self.types.alloc(Type::Struct(Struct {
          fields: Row::new(fields, None),
        }));
        assumptions.insert(name, provided_struct);
        InferenceResult::new(assumptions, provided_struct, errs)
      }
      Expr::DestructureExpr(ref destructure) => self.infer_destructure(destructure.clone()),
      Expr::MatchExpr(ref match_expr) => self.infer_match(match_expr.clone()),
      Expr::WithExpr(ref with_expr) => {
        let expr = unwrap_or_err!(with_expr.expr(), self);
        let behavior_name = unwrap_or_err!(with_expr.behavior(), self)
          .text()
          .to_string();
        let expr_res = self.infer(expr.clone());
        let with_ty = self.types.alloc(Type::WithBehavior(WithBehavior {
          inner: expr_res.ty,
          behavior: behavior_name.clone(),
          methods: Row::new(vec![], None),
        }));
        let mut assumptions = expr_res.assumptions;
        assumptions.insert(behavior_name, expr_res.ty);
        InferenceResult::new(assumptions, with_ty, expr_res.errs)
      }
      Expr::MethodCall(ref method_call) => {
        let receiver = unwrap_or_err!(method_call.receiver(), self);
        let method_name = unwrap_or_err!(method_call.method(), self)
          .text()
          .to_string();
        let receiver_res = self.infer(receiver.clone());
        let arg_results: Vec<InferenceResult> = method_call
          .arg_list()
          .map(|al| {
            al.args()
              .filter_map(|a| a.expr())
              .map(|e| self.infer(e))
              .collect()
          })
          .unwrap_or_default();

        let arg_types: Vec<TypeID> = arg_results.iter().map(|r| r.ty).collect();
        let ret_ty = self.fresh_type_var();
        let expected_fn_ty = fn_type(&mut self.types, arg_types, ret_ty);

        let mut errs = receiver_res.errs;
        let mut assumptions = receiver_res.assumptions;

        for arg_res in &arg_results {
          errs.extend(arg_res.errs.clone());
          let (merged, merge_errs) = self.merge(
            assumptions,
            arg_res.assumptions.clone(),
            receiver.syntax().clone(),
            method_call.syntax().clone(),
          );
          assumptions = merged;
          errs.extend(merge_errs);
        }

        if let Some(method_ty) = self.env.get(&method_name).cloned() {
          let (err, _) = self.unify(
            method_ty,
            expected_fn_ty,
            receiver.syntax().clone(),
            method_call.syntax().clone(),
          );
          if let Some(err) = err {
            errs.push(err);
          }
        } else {
          assumptions.insert(method_name, expected_fn_ty);
        }

        InferenceResult::new(assumptions, ret_ty, errs)
      }
      _ => todo!("Implement type inference for type: {:#?}", expr),
    };

    self
      .node_types
      .insert(expr.syntax().clone(), inference_res.ty);
    self
      .inference_results
      .insert(expr.syntax().green().into_owned(), inference_res.clone());
    inference_res
  }

  pub fn infer_with_cache(&mut self, expr: Expr) -> InferenceResult {
    let key = expr.syntax().green().into_owned();
    if let Some(mut inference_res) = self.inference_results.remove(&key) {
      inference_res.ty = self.freshen(inference_res.ty);
      for val in inference_res.assumptions.values_mut() {
        *val = self.freshen(val.clone());
      }
      inference_res.errs = inference_res
        .errs
        .into_iter()
        .map(|err| match err {
          TypeError::TypeMismatch {
            expected, actual, ..
          } => {
            let node = self.find_origin_for_error(
              &TypeError::TypeMismatch {
                expected: expected.clone(),
                actual: actual.clone(),
                node: expr.syntax().clone(),
                secondary: expr.syntax().clone(),
              },
              expr.clone(),
            );
            TypeError::TypeMismatch {
              expected,
              actual,
              node: node.clone(),
              secondary: node,
            }
          }
          TypeError::MissingField {
            field,
            expected_row,
            actual_row,
            ..
          } => {
            let node = self.find_origin_for_error(
              &TypeError::MissingField {
                field: field.clone(),
                expected_row: expected_row.clone(),
                actual_row: actual_row.clone(),
                node: expr.syntax().clone(),
                secondary: expr.syntax().clone(),
              },
              expr.clone(),
            );
            TypeError::MissingField {
              field,
              expected_row,
              actual_row,
              node: node.clone(),
              secondary: node,
            }
          }
          TypeError::NotPolymorphic { actual, .. } => {
            let node = self.find_origin_for_error(
              &TypeError::NotPolymorphic {
                actual: actual.clone(),
                node: expr.syntax().clone(),
                secondary: expr.syntax().clone(),
              },
              expr.clone(),
            );
            TypeError::NotPolymorphic {
              actual,
              node: node.clone(),
              secondary: node,
            }
          }
          // TODO: Handle other error cases
          e => e,
        })
        .collect();
      self.inference_results.insert(key, inference_res.clone());
      return inference_res;
    }
    self.infer(expr)
  }

  pub fn find_origin_for_error(&mut self, err: &TypeError, expr: Expr) -> SyntaxNode {
    match expr {
      Expr::LetExpr(ref let_expr) => {
        let value = let_expr.value().unwrap();
        let value_green = value.syntax().green().into_owned();
        if self
          .inference_results
          .get(&value_green)
          .map_or(false, |r| r.errs.contains(err))
        {
          return self.find_origin_for_error(err, value);
        }
        if let Some(in_expr) = let_expr.in_expr() {
          let in_green = in_expr.syntax().green().into_owned();
          if self
            .inference_results
            .get(&in_green)
            .map_or(false, |r| r.errs.contains(err))
          {
            return self.find_origin_for_error(err, in_expr);
          }
        }
        expr.syntax().clone()
      }
      Expr::FnExpr(ref fn_expr) => {
        let body = fn_expr.body().unwrap();
        let body_green = body.syntax().green().into_owned();
        if self
          .inference_results
          .get(&body_green)
          .map_or(false, |r| r.errs.contains(err))
        {
          return self.find_origin_for_error(err, body);
        }
        expr.syntax().clone()
      }
      Expr::CallExpr(ref call_expr) => {
        let callee = call_expr.callee().unwrap();
        let callee_green = callee.syntax().green().into_owned();
        if self
          .inference_results
          .get(&callee_green)
          .map_or(false, |r| r.errs.contains(err))
        {
          return self.find_origin_for_error(err, callee);
        }
        if let Some(al) = call_expr.arg_list() {
          for arg in al.args().filter_map(|a| a.expr()) {
            let arg_green = arg.syntax().green().into_owned();
            if self
              .inference_results
              .get(&arg_green)
              .map_or(false, |r| r.errs.contains(err))
            {
              return self.find_origin_for_error(err, arg);
            }
          }
        }
        expr.syntax().clone()
      }
      Expr::ParenExpr(ref paren) => {
        let inner = paren.expr().unwrap();
        let inner_green = inner.syntax().green().into_owned();
        if self.inference_results[&inner_green].errs.contains(err) {
          return self.find_origin_for_error(err, inner);
        }
        expr.syntax().clone()
      }
      Expr::DoThenExpr(ref do_then) => {
        if let Some(do_expr) = do_then.do_expr() {
          let do_green = do_expr.syntax().green().into_owned();
          if self
            .inference_results
            .get(&do_green)
            .map_or(false, |r| r.errs.contains(err))
          {
            return self.find_origin_for_error(err, do_expr);
          }
        }
        if let Some(then_expr) = do_then.then_expr() {
          let then_green = then_expr.syntax().green().into_owned();
          if self
            .inference_results
            .get(&then_green)
            .map_or(false, |r| r.errs.contains(err))
          {
            return self.find_origin_for_error(err, then_expr);
          }
        }
        expr.syntax().clone()
      }
      Expr::FieldAccess(ref fa) => {
        if let Some(inner) = fa.expr() {
          let inner_green = inner.syntax().green().into_owned();
          if self
            .inference_results
            .get(&inner_green)
            .map_or(false, |r| r.errs.contains(err))
          {
            return self.find_origin_for_error(err, inner);
          }
        }
        expr.syntax().clone()
      }
      Expr::Array(ref array) => {
        for elem in array.elements().filter_map(|e| e.expr()) {
          let elem_green = elem.syntax().green().into_owned();
          if self
            .inference_results
            .get(&elem_green)
            .map_or(false, |r| r.errs.contains(err))
          {
            return self.find_origin_for_error(err, elem);
          }
        }
        expr.syntax().clone()
      }
      Expr::PrefixExpr(ref prefix) => {
        if let Some(inner) = prefix.expr() {
          let inner_green = inner.syntax().green().into_owned();
          if self
            .inference_results
            .get(&inner_green)
            .map_or(false, |r| r.errs.contains(err))
          {
            return self.find_origin_for_error(err, inner);
          }
        }
        expr.syntax().clone()
      }
      Expr::NewExpr(ref new_expr) => {
        for field in new_expr.fields().filter_map(|f| f.value()) {
          let field_green = field.syntax().green().into_owned();
          if self
            .inference_results
            .get(&field_green)
            .map_or(false, |r| r.errs.contains(err))
          {
            return self.find_origin_for_error(err, field);
          }
        }
        expr.syntax().clone()
      }
      Expr::DestructureExpr(ref destructure) => {
        if let Some(scrutinee) = destructure.expr() {
          let scrutinee_green = scrutinee.syntax().green().into_owned();
          if self
            .inference_results
            .get(&scrutinee_green)
            .map_or(false, |r| r.errs.contains(err))
          {
            return self.find_origin_for_error(err, scrutinee);
          }
        }
        if let Some(body) = destructure.body() {
          let body_green = body.syntax().green().into_owned();
          if self
            .inference_results
            .get(&body_green)
            .map_or(false, |r| r.errs.contains(err))
          {
            return self.find_origin_for_error(err, body);
          }
        }
        expr.syntax().clone()
      }
      Expr::MatchExpr(ref match_expr) => {
        if let Some(scrutinee) = match_expr.scrutinee() {
          let scrutinee_green = scrutinee.syntax().green().into_owned();
          if self
            .inference_results
            .get(&scrutinee_green)
            .map_or(false, |r| r.errs.contains(err))
          {
            return self.find_origin_for_error(err, scrutinee);
          }
        }
        for branch in match_expr.branches() {
          if let Some(body) = branch.body() {
            let body_green = body.syntax().green().into_owned();
            if self
              .inference_results
              .get(&body_green)
              .map_or(false, |r| r.errs.contains(err))
            {
              return self.find_origin_for_error(err, body);
            }
          }
        }
        if let Some(else_clause) = match_expr.else_clause() {
          if let Some(else_body) = else_clause.body() {
            let else_green = else_body.syntax().green().into_owned();
            if self
              .inference_results
              .get(&else_green)
              .map_or(false, |r| r.errs.contains(err))
            {
              return self.find_origin_for_error(err, else_body);
            }
          }
        }
        expr.syntax().clone()
      }
      Expr::WithExpr(ref with_expr) => {
        if let Some(inner) = with_expr.expr() {
          let inner_green = inner.syntax().green().into_owned();
          if self
            .inference_results
            .get(&inner_green)
            .map_or(false, |r| r.errs.contains(err))
          {
            return self.find_origin_for_error(err, inner);
          }
        }
        expr.syntax().clone()
      }
      Expr::MethodCall(ref method_call) => {
        if let Some(receiver) = method_call.receiver() {
          let receiver_green = receiver.syntax().green().into_owned();
          if self
            .inference_results
            .get(&receiver_green)
            .map_or(false, |r| r.errs.contains(err))
          {
            return self.find_origin_for_error(err, receiver);
          }
        }
        if let Some(al) = method_call.arg_list() {
          for arg in al.args().filter_map(|a| a.expr()) {
            let arg_green = arg.syntax().green().into_owned();
            if self
              .inference_results
              .get(&arg_green)
              .map_or(false, |r| r.errs.contains(err))
            {
              return self.find_origin_for_error(err, arg);
            }
          }
        }
        expr.syntax().clone()
      }
      Expr::Ident(_) | Expr::Literal(_) => expr.syntax().clone(),
      _ => todo!("find_origin_for_error for {:?}", expr),
    }
  }

  fn infer_let(&mut self, let_expr: LetExpr) -> InferenceResult {
    let value = unwrap_or_err!(let_expr.value(), self);
    let node = let_expr.syntax().clone();
    let name = unwrap_or_err!(let_expr.name(), self).text().to_string();
    let mut value_res = self.infer(value.clone());

    match let_expr.in_expr() {
      Some(in_expr) => {
        let in_res = self.infer(in_expr.clone());
        let mut errs = value_res.errs;
        errs.extend(in_res.errs);
        let mut assumptions = in_res.assumptions;
        if let Some(assumed_ty) = assumptions.remove(&name) {
          let (err, unified_ty) = self.unify(
            assumed_ty,
            value_res.ty,
            node.clone(),
            value.syntax().clone(),
          );
          if let Some(err) = err {
            errs.push(err);
            value_res.ty = unified_ty;
          }
        }
        let (merged, merge_errs) = self.merge(
          value_res.assumptions,
          assumptions,
          node,
          in_expr.syntax().clone(),
        );
        errs.extend(merge_errs);
        InferenceResult::new(merged, in_res.ty, errs)
      }
      None => {
        let mut assumptions = value_res.assumptions;
        assumptions.insert(name, value_res.ty);
        InferenceResult::new(assumptions, value_res.ty, value_res.errs)
      }
    }
  }

  fn infer_fn(&mut self, fn_expr: FnExpr) -> InferenceResult {
    let body = unwrap_or_err!(fn_expr.body(), self);
    let body_res = self.infer(body);
    let mut assumptions = body_res.assumptions;
    let mut errs = body_res.errs;

    let param_types: Vec<(TypeID, Param)> = fn_expr
      .params()
      .filter_map(|param| {
        let name = param.name()?.text().to_string();
        let ty = if let Some(ty) = assumptions.remove(&name) {
          if let Some(type_expr) = param.type_expr() {
            let annotated = type_expr.lower(&mut self.types, &mut self.fresh_counter);
            let (err, unified_ty) = self.unify(
              ty,
              annotated,
              fn_expr.syntax().clone(),
              param.syntax().clone(),
            );
            if let Some(err) = err {
              errs.push(err);
            }
            unified_ty
          } else {
            ty
          }
        } else if let Some(type_expr) = param.type_expr() {
          type_expr.lower(&mut self.types, &mut self.fresh_counter)
        } else {
          self.fresh_type_var()
        };
        Some((ty, param))
      })
      .collect();
    for (param_ty, param) in &param_types {
      let root = find_root(&mut self.types, *param_ty);
      if let Type::TypeApp(app) = self.types.types[root].clone() {
        let head_root = find_root(&mut self.types, app.head);
        if let Type::TypeCons(c) = self.types.types[head_root].clone() {
          if c.name == "fn" {
            let inner_param_tys = app.args[..app.args.len() - 1].to_vec();
            let has_scheme_param = inner_param_tys.iter().any(|t| {
              let r = find_root(&mut self.types, *t);
              matches!(&self.types.types[r], Type::Scheme(s) if !s.vars.is_empty())
            });
            if !has_scheme_param {
              errs.push(TypeError::not_polymorphic(
                self.get_type(root).clone(),
                fn_expr.syntax().clone(),
                param.syntax().clone(),
              ));
            }
          }
        }
      }
    }

    let ret_ty = match fn_expr.return_type() {
      Some(type_expr) => {
        let annotated = type_expr.lower(&mut self.types, &mut self.fresh_counter);
        let (err, unified_ty) = self.unify(
          body_res.ty,
          annotated,
          fn_expr.syntax().clone(),
          fn_expr.syntax().clone(),
        );
        if let Some(err) = err {
          errs.push(err);
        }
        unified_ty
      }
      None => body_res.ty,
    };

    let fn_ty = fn_type(
      &mut self.types,
      param_types.iter().map(|(ty, _)| *ty).collect(),
      ret_ty,
    );
    let assumption_roots: Vec<TypeID> = assumptions
      .values()
      .map(|a| find_root(&mut self.types, *a))
      .collect();

    let mut scheme_vars: Vec<VarID> = Vec::new();
    if assumption_roots.is_empty() {
      for (ty, _) in &param_types {
        let root = find_root(&mut self.types, *ty);
        if let Type::TypeVar(tv) = &self.types.types[root] {
          scheme_vars.push(tv.id);
        }
      }
    }

    let result_ty = if scheme_vars.is_empty() {
      fn_ty
    } else {
      self
        .types
        .alloc(Type::Scheme(Scheme::poly_type(scheme_vars, fn_ty)))
    };

    InferenceResult::new(assumptions, result_ty, errs)
  }

  fn infer_match(&mut self, match_expr: MatchExpr) -> InferenceResult {
    let scrutinee = unwrap_or_err!(match_expr.scrutinee(), self);
    let scrutinee_res = self.infer(scrutinee.clone());

    let mut assumptions = scrutinee_res.assumptions;
    let mut errs = scrutinee_res.errs;
    let mut branch_body_tys: Vec<TypeID> = Vec::new();
    let mut branch_assumption_sets: Vec<HashMap<String, TypeID>> = Vec::new();

    for branch in match_expr.branches() {
      let body = unwrap_or_err!(branch.body(), self);
      let body_res = self.infer(body.clone());
      errs.extend(body_res.errs);

      let mut branch_assumptions = body_res.assumptions;

      if let Some(pattern) = branch.pattern() {
        match pattern {
          Pattern::Ident(ident) => {
            let name = unwrap_or_err!(ident.name(), self).text().to_string();
            if let Some(bound_ty) = branch_assumptions.remove(&name) {
              let (err, _) = self.unify(
                bound_ty,
                scrutinee_res.ty,
                scrutinee.syntax().clone(),
                match_expr.syntax().clone(),
              );
              if let Some(err) = err {
                errs.push(err);
              }
            }
          }
          Pattern::Literal(literal) => {
            let lit_res = self.infer(Expr::Literal(literal));
            let (err, _) = self.unify(
              scrutinee_res.ty,
              lit_res.ty,
              scrutinee.syntax().clone(),
              match_expr.syntax().clone(),
            );
            if let Some(err) = err {
              errs.push(err);
            }
          }
          Pattern::VariantPattern(variant) => {
            let variant_name = unwrap_or_err!(variant.name(), self).text().to_string();
            let payload_ty = self.fresh_type_var();
            let adt_ty = self.fresh_type_var();

            if let Some(binding_list) = variant.binding_list() {
              let bindings: Vec<_> = binding_list.bindings().collect();
              if bindings.len() == 1 {
                if let Some(Pattern::Ident(ident)) = bindings[0].pattern() {
                  let name = unwrap_or_err!(ident.name(), self).text().to_string();
                  if let Some(bound_ty) = branch_assumptions.remove(&name) {
                    let (err, _) = self.unify(
                      bound_ty,
                      payload_ty,
                      match_expr.syntax().clone(),
                      match_expr.syntax().clone(),
                    );
                    if let Some(err) = err {
                      errs.push(err);
                    }
                  }
                }
              }
            }

            let (err, _) = self.unify(
              scrutinee_res.ty,
              adt_ty,
              scrutinee.syntax().clone(),
              match_expr.syntax().clone(),
            );
            if let Some(err) = err {
              errs.push(err);
            }

            let constructor_fn_ty = fn_type(&mut self.types, vec![payload_ty], adt_ty);
            if let Some(env_ty) = self.env.get(&variant_name).cloned() {
              let (err, _) = self.unify(
                env_ty,
                constructor_fn_ty,
                match_expr.syntax().clone(),
                match_expr.syntax().clone(),
              );
              if let Some(err) = err {
                errs.push(err);
              }
            } else {
              branch_assumptions.insert(variant_name, constructor_fn_ty);
            }
          }
          Pattern::DestructureStruct(destructure) => {
            let mut fields = Vec::new();
            for field in destructure.fields() {
              let binding_name = unwrap_or_err!(field.name(), self).text().to_string();
              let field_expr = unwrap_or_err!(field.value(), self);
              let field_name = field_expr.syntax().text().to_string().trim().to_string();
              let field_ty = if let Some(bound_ty) = branch_assumptions.remove(&binding_name) {
                bound_ty
              } else {
                self.fresh_type_var()
              };
              fields.push((field_name, field_ty));
            }
            let rest = self.fresh_row_var();
            let row_ty = self.types.alloc(Type::Row(Row::new(fields, Some(rest))));
            let (err, _) = self.unify(
              scrutinee_res.ty,
              row_ty,
              scrutinee.syntax().clone(),
              match_expr.syntax().clone(),
            );
            if let Some(err) = err {
              errs.push(err);
            }
          }
        }
      }

      branch_body_tys.push(body_res.ty);
      branch_assumption_sets.push(branch_assumptions);
    }

    for branch_assumptions in branch_assumption_sets {
      let (merged, merge_errs) = self.merge(
        assumptions,
        branch_assumptions,
        match_expr.syntax().clone(),
        match_expr.syntax().clone(),
      );
      assumptions = merged;
      errs.extend(merge_errs);
    }

    let mut ret_ty = self.fresh_type_var();
    for body_ty in branch_body_tys {
      let (err, unified_ret) = self.unify(
        ret_ty,
        body_ty,
        match_expr.syntax().clone(),
        match_expr.syntax().clone(),
      );
      if let Some(err) = err {
        errs.push(err);
      }
      ret_ty = unified_ret;
    }

    if let Some(else_clause) = match_expr.else_clause() {
      let else_body = unwrap_or_err!(else_clause.body(), self);
      let else_res = self.infer(else_body.clone());
      errs.extend(else_res.errs);
      let (err, unified_ret) = self.unify(
        ret_ty,
        else_res.ty,
        match_expr.syntax().clone(),
        else_body.syntax().clone(),
      );
      if let Some(err) = err {
        errs.push(err);
      }
      ret_ty = unified_ret;
      let (merged, merge_errs) = self.merge(
        assumptions,
        else_res.assumptions,
        match_expr.syntax().clone(),
        else_body.syntax().clone(),
      );
      assumptions = merged;
      errs.extend(merge_errs);
    }

    InferenceResult::new(assumptions, ret_ty, errs)
  }

  pub fn generalize_free(&mut self, ty: TypeID) -> TypeID {
    let root = find_root(&mut self.types, ty);
    match self.types.types[root].clone() {
      Type::Scheme(scheme) => {
        let fn_root = find_root(&mut self.types, scheme.ty);
        if let Type::TypeApp(app) = self.types.types[fn_root].clone() {
          let mut still_vars: Vec<VarID> = Vec::new();
          for var in &scheme.vars {
            for arg in &app.args {
              let arg_root = find_root(&mut self.types, *arg);
              if let Type::TypeVar(tv) = &self.types.types[arg_root] {
                if tv.id == *var && !still_vars.contains(var) {
                  still_vars.push(*var);
                }
              }
            }
          }
          if still_vars.is_empty() {
            fn_root
          } else {
            self
              .types
              .alloc(Type::Scheme(Scheme::poly_type(still_vars, fn_root)))
          }
        } else {
          root
        }
      }
      Type::TypeApp(app) => {
        let mut vars: Vec<VarID> = Vec::new();
        let args = app.args.clone();
        for arg in &args {
          self.collect_free_vars(*arg, &mut vars);
        }
        if vars.is_empty() {
          root
        } else {
          self
            .types
            .alloc(Type::Scheme(Scheme::poly_type(vars, root)))
        }
      }
      _ => root,
    }
  }

  fn collect_free_vars(&mut self, ty: TypeID, vars: &mut Vec<VarID>) {
    let root = find_root(&mut self.types, ty);
    match self.types.types[root].clone() {
      Type::TypeVar(tv) => {
        if !vars.contains(&tv.id) {
          vars.push(tv.id);
        }
      }
      Type::TypeApp(app) => {
        let args = app.args.clone();
        for arg in args {
          self.collect_free_vars(arg, vars);
        }
      }
      _ => {}
    }
  }

  fn infer_call(&mut self, call_expr: CallExpr) -> InferenceResult {
    let callee = unwrap_or_err!(call_expr.callee(), self);
    let callee_res = self.infer(callee.clone());

    let arg_results: Vec<InferenceResult> = call_expr
      .arg_list()
      .map(|al| {
        al.args()
          .filter_map(|a| a.expr())
          .map(|e| self.infer(e))
          .collect()
      })
      .unwrap_or_default();

    let arg_types: Vec<TypeID> = arg_results.iter().map(|r| r.ty).collect();
    let mut ret_ty = self.fresh_type_var();
    let expected_fn_ty = fn_type(&mut self.types, arg_types.clone(), ret_ty);

    let mut errs = callee_res.errs;
    let mut assumptions = callee_res.assumptions;

    for arg_res in &arg_results {
      errs.extend(arg_res.errs.clone());
      let (merged, merge_errs) = self.merge(
        assumptions,
        arg_res.assumptions.clone(),
        callee.syntax().clone(),
        call_expr.syntax().clone(),
      );
      assumptions = merged;
      errs.extend(merge_errs);
    }

    let (err, unified) = self.unify(
      callee_res.ty,
      expected_fn_ty,
      callee.syntax().clone(),
      call_expr.syntax().clone(),
    );
    if let Some(err) = err {
      errs.push(err);
      ret_ty = unified;
    }

    InferenceResult::new(assumptions, ret_ty, errs)
  }

  fn infer_destructure(&mut self, destructure: DestructureExpr) -> InferenceResult {
    let scrutinee = unwrap_or_err!(destructure.expr(), self);
    let scrutinee_res = self.infer(scrutinee.clone());
    let body = unwrap_or_err!(destructure.body(), self);
    let body_res = self.infer(body.clone());

    let mut assumptions = body_res.assumptions;
    let mut errs = body_res.errs;
    errs.extend(scrutinee_res.errs);

    let mut fields = Vec::new();
    let struct_node = unwrap_or_err!(destructure.destructure_struct(), self);
    for field in struct_node.fields() {
      let binding_name = unwrap_or_err!(field.name(), self).text().to_string();
      let field_expr = unwrap_or_err!(field.value(), self);
      let field_name = field_expr.syntax().text().to_string().trim().to_string();
      let field_ty = if let Some(bound_ty) = assumptions.remove(&binding_name) {
        bound_ty
      } else {
        self.fresh_type_var()
      };
      fields.push((field_name, field_ty));
    }

    let rest = self.fresh_row_var();
    let row_ty = self.types.alloc(Type::Row(Row::new(fields, Some(rest))));
    if let (Some(err), _) = self.unify(
      scrutinee_res.ty,
      row_ty,
      scrutinee.syntax().clone(),
      destructure.syntax().clone(),
    ) {
      errs.push(err);
    }

    let (merged, merge_errs) = self.merge(
      assumptions,
      scrutinee_res.assumptions,
      destructure.syntax().clone(),
      scrutinee.syntax().clone(),
    );
    assumptions = merged;
    errs.extend(merge_errs);

    let ret_ty = if let Some(else_clause) = destructure.else_clause() {
      let else_body = unwrap_or_err!(else_clause.body(), self);
      let else_res = self.infer(else_body.clone());
      errs.extend(else_res.errs);
      let (merged, merge_errs) = self.merge(
        assumptions,
        else_res.assumptions,
        destructure.syntax().clone(),
        else_body.syntax().clone(),
      );
      assumptions = merged;
      errs.extend(merge_errs);
      let (err, unified_ty) = self.unify(
        body_res.ty,
        else_res.ty,
        body.syntax().clone(),
        else_body.syntax().clone(),
      );
      if let Some(err) = err {
        errs.push(err);
      }
      unified_ty
    } else {
      body_res.ty
    };

    InferenceResult::new(assumptions, ret_ty, errs)
  }

  pub fn unify(
    &mut self,
    a: TypeID,
    b: TypeID,
    a_node: SyntaxNode,
    b_node: SyntaxNode,
  ) -> (Option<TypeError>, TypeID) {
    let a = find_root(&mut self.types, a);
    let b = find_root(&mut self.types, b);

    if a == b {
      return (None, a);
    }

    let a_ty = self.types.types[a].clone();
    let b_ty = self.types.types[b].clone();

    match (a_ty, b_ty) {
      (Type::Error, _) | (_, Type::Error) => (None, self.new_err()),

      (Type::TypeVar(_), _) => {
        self.types.types[a] = Type::Link(b);
        (None, b)
      }
      (_, Type::TypeVar(_)) => {
        self.types.types[b] = Type::Link(a);
        (None, a)
      }

      (Type::TypeCons(x), Type::TypeCons(y)) => {
        if x.name == y.name {
          (None, a)
        } else if Self::is_int_cons(&x.name) && Self::is_int_cons(&y.name) {
          (None, a)
        } else {
          (
            Some(TypeError::mismatch(
              Type::TypeCons(x),
              Type::TypeCons(y),
              a_node,
              b_node,
            )),
            self.new_err(),
          )
        }
      }

      (Type::TypeApp(app), _) if app.args.is_empty() => self.unify(app.head, b, a_node, b_node),
      (_, Type::TypeApp(app)) if app.args.is_empty() => self.unify(a, app.head, a_node, b_node),

      (Type::TypeApp(a_app), Type::TypeApp(b_app)) => {
        if a_app.args.len() != b_app.args.len() {
          return (
            Some(TypeError::mismatch(
              self.get_type(a).clone(),
              self.get_type(b).clone(),
              a_node,
              b_node,
            )),
            self.new_err(),
          );
        }
        for (x, y) in a_app.args.iter().zip(&b_app.args) {
          if let (Some(err), _) = self.unify(*x, *y, a_node.clone(), b_node.clone()) {
            return (Some(err), self.new_err());
          }
        }
        (None, a)
      }

      (Type::Row(a_row), Type::Row(b_row)) => {
        let mut a_fields = HashMap::new();
        let mut b_fields = HashMap::new();
        for (name, ty) in &a_row.fields {
          a_fields.insert(name, *ty);
        }
        for (name, ty) in &b_row.fields {
          b_fields.insert(name, *ty);
        }
        for (name, a_ty) in &a_fields {
          if let Some(b_ty) = b_fields.get(name) {
            if let (Some(err), _) = self.unify(*a_ty, *b_ty, a_node.clone(), b_node.clone()) {
              return (Some(err), self.new_err());
            }
          }
        }
        for (name, _) in &a_fields {
          if !b_fields.contains_key(name) && b_row.rest.is_none() {
            return (
              Some(TypeError::missing_field(
                name.to_string(),
                Type::Row(a_row.clone()),
                Type::Row(b_row.clone()),
                a_node.clone(),
                b_node.clone(),
              )),
              self.new_err(),
            );
          }
        }
        for (name, _) in &b_fields {
          if !a_fields.contains_key(name) && a_row.rest.is_none() {
            return (
              Some(TypeError::missing_field(
                name.to_string(),
                Type::Row(b_row.clone()),
                Type::Row(a_row.clone()),
                b_node.clone(),
                a_node.clone(),
              )),
              self.new_err(),
            );
          }
        }
        if let (Some(a_r), Some(b_r)) = (a_row.rest, b_row.rest) {
          if let (Some(err), _) = self.unify(a_r, b_r, a_node, b_node) {
            return (Some(err), self.new_err());
          }
        }
        (None, a)
      }

      (Type::Struct(s), Type::Row(_)) => {
        let row = self.types.alloc(Type::Row(s.fields));
        self.unify(row, b, a_node, b_node)
      }
      (Type::Row(_), Type::Struct(s)) => {
        let row = self.types.alloc(Type::Row(s.fields));
        self.unify(a, row, a_node, b_node)
      }

      (Type::WithBehavior(behavior), _) => self.unify(behavior.inner, b, a_node, b_node),
      (_, Type::WithBehavior(behavior)) => self.unify(a, behavior.inner, a_node, b_node),

      // TODO: Add a least general bound algorithm for schemes
      // (Type::Scheme(scheme_a), Type::Scheme(scheme_b)) => {}
      (Type::Scheme(scheme), _) if !scheme.vars.is_empty() => {
        let mut subst: HashMap<VarID, TypeID> = HashMap::new();
        for var in &scheme.vars {
          subst.insert(*var, self.fresh_type_var());
        }
        let instantiated = self.instantiate(scheme.ty, &subst);
        self.unify(instantiated, b, a_node, b_node)
      }
      (_, Type::Scheme(scheme)) if !scheme.vars.is_empty() => {
        let mut subst: HashMap<VarID, TypeID> = HashMap::new();
        for var in &scheme.vars {
          subst.insert(*var, self.fresh_type_var());
        }
        let instantiated = self.instantiate(scheme.ty, &subst);
        self.unify(a, instantiated, a_node, b_node)
      }

      (Type::Struct(a_struct), Type::Struct(b_struct)) => {
        let a_row = a_struct.fields;
        let b_row = b_struct.fields;
        let mut a_fields = HashMap::new();
        let mut b_fields = HashMap::new();
        for (name, ty) in &a_row.fields {
          a_fields.insert(name.clone(), *ty);
        }
        for (name, ty) in &b_row.fields {
          b_fields.insert(name.clone(), *ty);
        }
        for (name, a_ty) in &a_fields {
          if let Some(b_ty) = b_fields.get(name) {
            if let (Some(err), _) = self.unify(*a_ty, *b_ty, a_node.clone(), b_node.clone()) {
              return (Some(err), self.new_err());
            }
          }
        }
        for (name, _) in &b_fields {
          if !a_fields.contains_key(name) {
            return (
              Some(TypeError::missing_field(
                name.clone(),
                Type::Struct(Struct {
                  fields: a_row.clone(),
                }),
                Type::Struct(Struct {
                  fields: b_row.clone(),
                }),
                a_node.clone(),
                b_node.clone(),
              )),
              self.new_err(),
            );
          }
        }
        (None, a)
      }

      _ => (
        Some(TypeError::mismatch(
          self.get_type(a).clone(),
          self.get_type(b).clone(),
          a_node,
          b_node,
        )),
        self.new_err(),
      ),
    }
  }

  fn instantiate(&mut self, ty: TypeID, subst: &HashMap<VarID, TypeID>) -> TypeID {
    let root = find_root(&mut self.types, ty);
    match self.types.types[root].clone() {
      Type::TypeVar(tv) => *subst.get(&tv.id).unwrap_or(&root),
      Type::TypeApp(app) => {
        let new_head = self.instantiate(app.head, subst);
        let new_args: Vec<TypeID> = app
          .args
          .iter()
          .map(|a| self.instantiate(*a, subst))
          .collect();
        self
          .types
          .alloc(Type::TypeApp(TypeApp::new(new_head, new_args)))
      }
      _ => root,
    }
  }

  fn merge(
    &mut self,
    mut a: HashMap<String, TypeID>,
    b: HashMap<String, TypeID>,
    a_node: SyntaxNode,
    b_node: SyntaxNode,
  ) -> (HashMap<String, TypeID>, Vec<TypeError>) {
    let mut errors = Vec::new();
    for (name, ty) in b {
      if let Some(existing) = a.remove(&name) {
        let (err, unified_ty) = self.unify(existing, ty, a_node.clone(), b_node.clone());
        if let Some(e) = err {
          errors.push(e);
        }
        a.insert(name, unified_ty);
      } else {
        a.insert(name, ty);
      }
    }
    (a, errors)
  }

  pub fn get_type(&self, id: TypeID) -> &Type {
    &self.types.types[id]
  }

  pub fn set_type(&mut self, id: TypeID, ty: Type) {
    self.types.types[id] = ty;
  }

  pub fn fresh_type_var(&mut self) -> TypeID {
    fresh_type_var(&mut self.types, &mut self.fresh_counter)
  }

  pub fn fresh_row_var(&mut self) -> TypeID {
    fresh_row_var(&mut self.types, &mut self.fresh_counter)
  }

  pub fn substitute_type_params(&mut self, ty: TypeID, params: &HashMap<String, TypeID>) -> TypeID {
    let root = find_root(&mut self.types, ty);
    match self.types.types[root].clone() {
      Type::TypeCons(ref c) if params.contains_key(&c.name) => params[&c.name],
      Type::TypeApp(app) => {
        let new_head = self.substitute_type_params(app.head, params);
        let new_args: Vec<TypeID> = app
          .args
          .iter()
          .map(|a| self.substitute_type_params(*a, params))
          .collect();
        self
          .types
          .alloc(Type::TypeApp(TypeApp::new(new_head, new_args)))
      }
      _ => root,
    }
  }

  pub fn new_type(&mut self, ty: Type) -> TypeID {
    self.types.alloc(ty)
  }

  pub fn new_err(&mut self) -> TypeID {
    self.new_type(Type::Error)
  }

  fn new_variable(name: &str, ty: TypeID) -> HashMap<String, TypeID> {
    let mut hashmap = HashMap::new();
    hashmap.insert(name.to_string(), ty);
    hashmap
  }

  fn freshen(&mut self, ty: TypeID) -> TypeID {
    let mut type_vars = HashMap::new();
    let mut row_vars = HashMap::new();
    freshen(
      &mut self.types,
      &mut self.fresh_counter,
      ty,
      &mut type_vars,
      &mut row_vars,
    )
  }

  fn is_int_cons(name: &str) -> bool {
    matches!(name, "int" | "int8" | "int16" | "int32" | "int64")
  }
}
