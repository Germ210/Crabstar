use crate::{
  ast::{AstNode, Expr, Root},
  err::TypeError,
  inference::Inferencer,
  types::{Row, Scheme, Type, TypeCons, TypeID, find_root, fn_type},
};
use std::collections::HashMap;

#[derive(Debug)]
pub struct Module {
  // TODO: Have a binding type that isn't just a raw string
  pub bindings: HashMap<String, TypeID>,
  pub inferencer: Inferencer,
  pub errs: Vec<TypeError>,
  pub constructor_env: HashMap<String, Vec<TypeID>>,
}

impl Module {
  pub fn new() -> Self {
    Self {
      bindings: HashMap::new(),
      inferencer: Inferencer::new(),
      errs: vec![],
      constructor_env: HashMap::new(),
    }
  }

  pub fn infer_module(&mut self, root: Root) {
    for type_decl in root.type_decls() {
      let Some(name_token) = type_decl.name() else {
        continue;
      };
      let name = name_token.text().to_string();
      let ty = if let Some(struct_body) = type_decl.struct_body() {
        let fields: Vec<(String, TypeID)> = struct_body
          .fields()
          .filter_map(|f| {
            let field_name = f.name()?.text().to_string();
            let field_ty = f
              .type_expr()
              .map(|t| {
                t.lower(
                  &mut self.inferencer.types,
                  &mut self.inferencer.fresh_counter,
                )
              })
              .unwrap_or_else(|| self.inferencer.fresh_type_var());
            Some((field_name, field_ty))
          })
          .collect();
        let row = crate::types::Row::new(fields, None);
        self
          .inferencer
          .new_type(Type::Struct(crate::types::Struct { fields: row }))
      } else if let Some(variant_body) = type_decl.variant_body() {
        let nominal_ty = self
          .inferencer
          .new_type(Type::TypeCons(TypeCons::new(&name)));

        let type_params: HashMap<String, TypeID> = type_decl
          .type_param_list()
          .map(|pl| {
            pl.params()
              .filter_map(|p| {
                let param_name = p.name()?.text().to_string();
                let ty = self.inferencer.fresh_type_var();
                Some((param_name, ty))
              })
              .collect()
          })
          .unwrap_or_default();

        let param_var_ids: Vec<_> = type_params
          .values()
          .map(|ty| {
            let root = find_root(&mut self.inferencer.types, *ty);
            match &self.inferencer.types.types[root] {
              Type::TypeVar(tv) => tv.id,
              _ => unreachable!(),
            }
          })
          .collect();

        let adt_ty = if type_params.is_empty() {
          nominal_ty
        } else {
          let args: Vec<TypeID> = type_params.values().cloned().collect();
          self
            .inferencer
            .new_type(Type::TypeApp(crate::types::TypeApp::new(nominal_ty, args)))
        };

        let mut sum_fields: Vec<(String, TypeID)> = Vec::new();
        for constructor in variant_body.constructors() {
          let Some(constructor_name_token) = constructor.name() else {
            continue;
          };
          let constructor_name = constructor_name_token.text().to_string();
          let qualified_name = format!("{}\\{}", name, constructor_name);
          let params: Vec<TypeID> = constructor
            .param_list()
            .map(|pl| {
              pl.params()
                .filter_map(|p| p.type_expr())
                .map(|t| {
                  let lowered = t.lower(
                    &mut self.inferencer.types,
                    &mut self.inferencer.fresh_counter,
                  );
                  self
                    .inferencer
                    .substitute_type_params(lowered, &type_params)
                })
                .collect()
            })
            .unwrap_or_default();
          let constructor_ty = if params.is_empty() {
            adt_ty
          } else {
            crate::types::fn_type(&mut self.inferencer.types, params, adt_ty)
          };
          let scheme_ty = if param_var_ids.is_empty() {
            constructor_ty
          } else {
            self.inferencer.new_type(Type::Scheme(Scheme::poly_type(
              param_var_ids.clone(),
              constructor_ty,
            )))
          };
          self.bindings.insert(qualified_name.clone(), scheme_ty);
          self.bindings.insert(constructor_name.clone(), scheme_ty);
          self
            .constructor_env
            .entry(constructor_name.clone())
            .or_default()
            .push(scheme_ty);
          self
            .inferencer
            .constructor_names
            .insert(constructor_name.clone());
          self
            .inferencer
            .env
            .insert(constructor_name.clone(), scheme_ty);
          sum_fields.push((constructor_name, scheme_ty));
        }
        let sum_ty = self
          .inferencer
          .new_type(Type::Sum(crate::types::Row::new(sum_fields, None)));
        let scheme_sum = if param_var_ids.is_empty() {
          sum_ty
        } else {
          self.inferencer.new_type(Type::Scheme(Scheme::poly_type(
            param_var_ids.clone(),
            sum_ty,
          )))
        };
        self.inferencer.unify(
          nominal_ty,
          scheme_sum,
          root.syntax().clone(),
          root.syntax().clone(),
        );
        nominal_ty
      } else {
        self
          .inferencer
          .new_type(Type::TypeCons(TypeCons::new(&name)))
      };
      let nominal_ty = self
        .inferencer
        .new_type(Type::TypeCons(TypeCons::new(&name)));
      self
        .inferencer
        .unify(nominal_ty, ty, root.syntax().clone(), root.syntax().clone());
      self.bindings.insert(name, nominal_ty);
    }

    self.inferencer.env.extend(self.bindings.clone());

    for behavior_def in root.behavior_defs() {
      let Some(name_token) = behavior_def.name() else {
        continue;
      };
      let name = name_token.text().to_string();

      let req_fields: Vec<(String, TypeID)> = behavior_def
        .requirement_list()
        .map(|rl| {
          rl.fields()
            .filter_map(|f| {
              let field_name = f.name()?.text().to_string();
              let field_ty = f.type_expr()?.lower(
                &mut self.inferencer.types,
                &mut self.inferencer.fresh_counter,
              );
              Some((field_name, field_ty))
            })
            .collect()
        })
        .unwrap_or_default();

      for (field_name, field_ty) in &req_fields {
        self.inferencer.env.insert(field_name.clone(), *field_ty);
      }

      let mut method_fields: Vec<(String, TypeID)> = Vec::new();
      if let Some(method_list) = behavior_def.method_list() {
        for method in method_list.methods() {
          let Some(method_name_token) = method.name() else {
            continue;
          };
          let method_name = method_name_token.text().to_string();
          let Some(body) = method.body() else {
            continue;
          };
          let mut body_res = self.inferencer.infer_with_cache(body);
          self.inferencer.default_types(&mut body_res);

          for (req_name, req_ty) in &req_fields {
            if let Some(assumed_ty) = body_res.assumptions.get(req_name).cloned() {
              self.inferencer.unify(
                assumed_ty,
                *req_ty,
                root.syntax().clone(),
                root.syntax().clone(),
              );
            }
          }

          let mut param_tys: Vec<TypeID> = Vec::new();
          let mut method_assumptions = body_res.assumptions;
          for param in method.params() {
            let Some(pname_token) = param.name() else {
              continue;
            };
            let pname = pname_token.text().to_string();
            let pty = if let Some(ty) = method_assumptions.remove(&pname) {
              ty
            } else if let Some(type_expr) = param.type_expr() {
              type_expr.lower(
                &mut self.inferencer.types,
                &mut self.inferencer.fresh_counter,
              )
            } else {
              self.inferencer.fresh_type_var()
            };
            param_tys.push(pty);
          }

          let method_ty = fn_type(&mut self.inferencer.types, param_tys, body_res.ty);
          self
            .bindings
            .insert(format!("{}\\{}", name, method_name), method_ty);
          self.inferencer.env.insert(method_name.clone(), method_ty);
          method_fields.push((method_name, method_ty));
        }
      }

      let req_row = Row::new(req_fields.clone(), None);
      let inner = self
        .inferencer
        .new_type(Type::Struct(crate::types::Struct { fields: req_row }));
      let behavior_ty = self
        .inferencer
        .new_type(Type::WithBehavior(crate::types::WithBehavior {
          inner,
          behavior: name.clone(),
          methods: crate::types::Row::new(method_fields, None),
        }));
      self.bindings.insert(name, behavior_ty);
    }

    let mut results: Vec<(String, TypeID, HashMap<String, TypeID>)> = Vec::new();
    for let_expr in root.let_exprs() {
      let Some(name_token) = let_expr.name() else {
        continue;
      };
      let name = name_token.text().to_string();
      let mut result = self.inferencer.infer_with_cache(Expr::LetExpr(let_expr));
      self.inferencer.default_types(&mut result);
      self.errs.extend(result.errs);
      results.push((name.clone(), result.ty, result.assumptions));
      self.bindings.insert(name, result.ty);
    }

    for (_, _, assumptions) in &results {
      for (name, ty) in assumptions {
        if let Some(bound_ty) = self.bindings.get(name).cloned() {
          if let (Some(err), _) =
            self
              .inferencer
              .unify(bound_ty, *ty, root.syntax().clone(), root.syntax().clone())
          {
            self.errs.push(err);
          }
        }
      }
    }

    for (name, ty, _) in &mut results {
      *ty = self.inferencer.generalize_free(*ty);
      self.bindings.insert(name.clone(), *ty);
    }

    for (name, ty, _) in &mut results {
      let root_ty = find_root(&mut self.inferencer.types, *ty);
      match self.inferencer.types.types[root_ty].clone() {
        Type::TypeCons(ref c) if c.name.contains('\\') => {
          let parts: Vec<&str> = c.name.split('\\').collect();
          let adt_name = parts[parts.len() - 2].to_string();
          if let Some(adt_ty) = self.bindings.get(&adt_name).cloned() {
            *ty = adt_ty;
          }
        }
        Type::TypeApp(ref app) => {
          let head_root = find_root(&mut self.inferencer.types, app.head);
          if let Type::TypeCons(ref c) = self.inferencer.types.types[head_root].clone() {
            if c.name.contains('\\') {
              let parts: Vec<&str> = c.name.split('\\').collect();
              let adt_name = parts[parts.len() - 2].to_string();
              if let Some(adt_ty) = self.bindings.get(&adt_name).cloned() {
                *ty = adt_ty;
              }
            }
          }
        }
        _ => {}
      }
      self.bindings.insert(name.clone(), *ty);
    }

    for (_, _, assumptions) in &results {
      for (name, ty) in assumptions {
        if let Some(bound_ty) = self.bindings.get(name).cloned() {
          if let (Some(err), _) =
            self
              .inferencer
              .unify(bound_ty, *ty, root.syntax().clone(), root.syntax().clone())
          {
            self.errs.push(err);
          }
        }
      }
    }
  }

  pub fn resolve_constructor(&self, name: &str) -> Vec<TypeID> {
    let suffix = format!("\\{}", name);
    self
      .bindings
      .iter()
      .filter(|(k, _)| k.ends_with(&suffix))
      .map(|(_, v)| *v)
      .collect()
  }
}
