use crate::ast::*;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Type {
  Int8,
  Int16,
  Int32,
  Int64,
  Float32,
  Float64,
  Bool,
  String,
  Null,
  Ref(Box<Type>),
  App {
    constructor: String,
    args: Vec<Type>,
  },
  Fn {
    params: Vec<Type>,
    return_type: Box<Type>,
  },
  Struct {
    name: String,
    fields: Vec<(String, Type)>,
  },
  Union {
    name: String,
    variants: Vec<(String, Vec<Type>)>,
  },
  Generic,
  Var(String),
  Error,
}

#[derive(Debug, PartialEq, Eq)]
pub struct FuncType {
  pub params: Vec<Type>,
  pub return_type: Type,
}

impl Type {
  pub fn from_type_expr(type_expr: &TypeExpr) -> Self {
    let inner = type_expr.inner_type();
    if let Some(node) = inner.as_node() {
      if let Some(ref_type) = RefType::cast(node.clone()) {
        let ref_kw = ref_type.ref_keyword();
        if ref_kw
          .as_node()
          .map(|n| n.children().count() > 0)
          .unwrap_or(false)
        {
          let type_app = ref_type.type_app();
          if let Some(app_node) = type_app.as_node() {
            if let Some(app) = TypeApp::cast(app_node.clone()) {
              return Type::Ref(Box::new(Self::from_type_app(&app)));
            }
          }
        } else {
          let type_app = ref_type.type_app();
          if let Some(app_node) = type_app.as_node() {
            if let Some(app) = TypeApp::cast(app_node.clone()) {
              return Self::from_type_app(&app);
            }
          }
        }
      } else if let Some(type_app) = TypeApp::cast(node.clone()) {
        return Self::from_type_app(&type_app);
      }
    }
    Type::Generic
  }

  fn from_type_app(type_app: &TypeApp) -> Self {
    let base = type_app.base_type();
    let base_name = if let Some(node) = base.as_node() {
      if let Some(ident) = Ident::cast(node.clone()) {
        ident
          .name()
          .as_token()
          .map(|t| t.text().trim().to_string())
          .unwrap_or_default()
      } else {
        String::new()
      }
    } else {
      String::new()
    };
    match base_name.as_str() {
      "int8" => Type::Int8,
      "int16" => Type::Int16,
      "int32" => Type::Int32,
      "int64" => Type::Int64,
      "float32" => Type::Float32,
      "float64" => Type::Float64,
      "bool" => Type::Bool,
      "string" => Type::String,
      "null" => Type::Null,
      _ => {
        let args_node = type_app.type_args();
        if let Some(args) = args_node.as_node() {
          if let Some(arg_list) = TypeArgList::cast(args.clone()) {
            let type_args: Vec<Type> = arg_list
              .children()
              .filter_map(|child| TypeArg::cast(child))
              .map(|arg| {
                let te = arg.type_expr();
                if let Some(te_node) = te.as_node() {
                  if let Some(type_expr) = TypeExpr::cast(te_node.clone()) {
                    Type::from_type_expr(&type_expr)
                  } else {
                    Type::Generic
                  }
                } else {
                  Type::Generic
                }
              })
              .collect();
            if type_args.is_empty() {
              Type::Var(base_name)
            } else {
              Type::App {
                constructor: base_name,
                args: type_args,
              }
            }
          } else {
            Type::Var(base_name)
          }
        } else {
          Type::Var(base_name)
        }
      }
    }
  }
}
