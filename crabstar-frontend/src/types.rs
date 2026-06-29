use std::collections::HashMap;

use slotmap::{SlotMap, new_key_type};

new_key_type! { pub struct TypeID; }

pub type VarID = u32;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TypeVar {
  pub id: VarID,
}

impl TypeVar {
  pub fn new(id: VarID) -> Self {
    TypeVar { id }
  }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RowVar {
  pub id: VarID,
}

impl RowVar {
  pub fn new(id: VarID) -> Self {
    RowVar { id }
  }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TypeCons {
  pub name: String,
}

impl TypeCons {
  pub fn new(name: impl Into<String>) -> Self {
    TypeCons { name: name.into() }
  }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TypeApp {
  pub head: TypeID,
  pub args: Vec<TypeID>,
}

impl TypeApp {
  pub fn new(head: TypeID, args: Vec<TypeID>) -> Self {
    TypeApp { head, args }
  }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Row {
  pub fields: Vec<(String, TypeID)>,
  pub rest: Option<TypeID>,
}

impl Row {
  pub fn new(fields: Vec<(String, TypeID)>, rest: Option<TypeID>) -> Self {
    Row { fields, rest }
  }

  pub fn empty(rest: Option<TypeID>) -> Self {
    Row {
      fields: Vec::new(),
      rest,
    }
  }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct WithBehavior {
  pub inner: TypeID,
  pub behavior: String,
  pub methods: Row,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Struct {
  pub fields: Row,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Scheme {
  pub vars: Vec<VarID>,
  pub ty: TypeID,
}

impl Scheme {
  pub fn mono_type(ty: TypeID) -> Self {
    Scheme { vars: vec![], ty }
  }

  pub fn poly_type(vars: Vec<VarID>, ty: TypeID) -> Self {
    Scheme { vars, ty }
  }

  pub fn is_mono_type(&self) -> bool {
    self.vars.is_empty()
  }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Type {
  TypeVar(TypeVar),
  RowVar(RowVar),
  TypeCons(TypeCons),
  TypeApp(TypeApp),
  Row(Row),
  WithBehavior(WithBehavior),
  Struct(Struct),
  Sum(Row),
  Scheme(Scheme),
  Link(TypeID),
  Error,
}

#[derive(Debug)]
pub struct TypeArena {
  pub types: SlotMap<TypeID, Type>,
}

impl TypeArena {
  pub fn new() -> Self {
    TypeArena {
      types: SlotMap::with_key(),
    }
  }

  pub fn alloc(&mut self, ty: Type) -> TypeID {
    self.types.insert(ty)
  }
}

#[derive(Debug, Default)]
pub struct FreshCounters {
  pub type_var: VarID,
  pub row_var: VarID,
}

impl FreshCounters {
  pub fn new() -> Self {
    FreshCounters::default()
  }
}

pub fn fresh_type_var(arena: &mut TypeArena, counters: &mut FreshCounters) -> TypeID {
  counters.type_var += 1;
  arena.alloc(Type::TypeVar(TypeVar::new(counters.type_var)))
}

pub fn fresh_row_var(arena: &mut TypeArena, counters: &mut FreshCounters) -> TypeID {
  counters.row_var += 1;
  arena.alloc(Type::RowVar(RowVar::new(counters.row_var)))
}

pub fn find_root(arena: &mut TypeArena, id: TypeID) -> TypeID {
  let mut current = id;
  while let Some(Type::Link(next)) = arena.types.get(current) {
    current = *next;
  }
  let root = current;
  current = id;
  while let Some(Type::Link(next)) = arena.types.get(current) {
    let temp = *next;
    if temp == root {
      break;
    }
    arena.types[current] = Type::Link(root);
    current = temp;
  }
  root
}

#[derive(Debug, Clone)]
pub struct TypeOption(pub Option<TypeID>);

pub fn int_type(arena: &mut TypeArena) -> TypeID {
  arena.alloc(Type::TypeCons(TypeCons::new("int")))
}

pub fn int8_type(arena: &mut TypeArena) -> TypeID {
  arena.alloc(Type::TypeCons(TypeCons::new("int8")))
}

pub fn int16_type(arena: &mut TypeArena) -> TypeID {
  arena.alloc(Type::TypeCons(TypeCons::new("int16")))
}

pub fn int32_type(arena: &mut TypeArena) -> TypeID {
  arena.alloc(Type::TypeCons(TypeCons::new("int32")))
}

pub fn int64_type(arena: &mut TypeArena) -> TypeID {
  arena.alloc(Type::TypeCons(TypeCons::new("int64")))
}

pub fn float_type(arena: &mut TypeArena) -> TypeID {
  arena.alloc(Type::TypeCons(TypeCons::new("float")))
}

pub fn float32_type(arena: &mut TypeArena) -> TypeID {
  arena.alloc(Type::TypeCons(TypeCons::new("float32")))
}

pub fn float64_type(arena: &mut TypeArena) -> TypeID {
  arena.alloc(Type::TypeCons(TypeCons::new("float64")))
}

pub fn bool_type(arena: &mut TypeArena) -> TypeID {
  arena.alloc(Type::TypeCons(TypeCons::new("bool")))
}

pub fn string_type(arena: &mut TypeArena) -> TypeID {
  arena.alloc(Type::TypeCons(TypeCons::new("String")))
}

pub fn null_type(arena: &mut TypeArena) -> TypeID {
  arena.alloc(Type::TypeCons(TypeCons::new("null")))
}

pub fn ref_type(arena: &mut TypeArena, inner: TypeID) -> TypeID {
  let cons = arena.alloc(Type::TypeCons(TypeCons::new("ref")));
  arena.alloc(Type::TypeApp(TypeApp::new(cons, vec![inner])))
}

pub fn mut_type(arena: &mut TypeArena, inner: TypeID) -> TypeID {
  let cons = arena.alloc(Type::TypeCons(TypeCons::new("mut")));
  arena.alloc(Type::TypeApp(TypeApp::new(cons, vec![inner])))
}

pub fn owned_type(arena: &mut TypeArena, inner: TypeID) -> TypeID {
  let cons = arena.alloc(Type::TypeCons(TypeCons::new("owned")));
  arena.alloc(Type::TypeApp(TypeApp::new(cons, vec![inner])))
}

pub fn fn_type(arena: &mut TypeArena, mut args: Vec<TypeID>, ret: TypeID) -> TypeID {
  let cons = arena.alloc(Type::TypeCons(TypeCons::new("fn")));
  args.push(ret);
  arena.alloc(Type::TypeApp(TypeApp::new(cons, args)))
}

pub fn array_type(arena: &mut TypeArena, array_type: TypeID) -> TypeID {
  let cons = arena.alloc(Type::TypeCons(TypeCons::new("array")));
  arena.alloc(Type::TypeApp(TypeApp::new(cons, vec![array_type])))
}

fn var_name(
  id: u32,
  var_names: &mut std::collections::HashMap<u32, String>,
  counter: &mut u32,
) -> String {
  if let Some(name) = var_names.get(&id) {
    return name.clone();
  }
  let name = format!("'{}", (b'a' + (*counter % 26) as u8) as char);
  *counter += 1;
  var_names.insert(id, name.clone());
  name
}

pub fn format_type(
  arena: &mut TypeArena,
  id: TypeID,
  var_names: &mut std::collections::HashMap<u32, String>,
  counter: &mut u32,
) -> String {
  let root = find_root(arena, id);
  let ty = arena.types[root].clone();
  match ty {
    Type::TypeVar(v) => var_name(v.id, var_names, counter),
    Type::RowVar(_) => "...".into(),
    Type::TypeCons(c) => c.name.clone(),
    Type::Link(inner) => format_type(arena, inner, var_names, counter),
    Type::Error => "<error>".to_string(),
    Type::TypeApp(app) => {
      let head = format_type(arena, app.head, var_names, counter);
      if head == "fn" {
        let args: Vec<_> = app.args[..app.args.len() - 1]
          .iter()
          .map(|a| format_type(arena, *a, var_names, counter))
          .collect();
        let ret = format_type(arena, *app.args.last().unwrap(), var_names, counter);
        format!("fn ({}) -> {}", args.join(", "), ret)
      } else {
        let args: Vec<_> = app
          .args
          .iter()
          .map(|a| format_type(arena, *a, var_names, counter))
          .collect();
        if args.len() == 0 {
          format!("{}", head)
        } else {
          format!("{}({})", head, args.join(", "))
        }
      }
    }
    Type::Row(row) => {
      let mut parts: Vec<_> = row
        .fields
        .iter()
        .map(|(n, t)| format!("{}: {}", n, format_type(arena, *t, var_names, counter)))
        .collect();
      if row.rest.is_some() {
        parts.push("...".to_string());
      }
      format!("{{{}}}", parts.join(", "))
    }
    Type::Struct(s) => {
      let mut parts: Vec<_> = s
        .fields
        .fields
        .iter()
        .map(|(n, t)| format!("{}: {}", n, format_type(arena, *t, var_names, counter)))
        .collect();
      if let Some(rest) = s.fields.rest {
        parts.push(format!(
          "...{}",
          format_type(arena, rest, var_names, counter)
        ));
      }
      format!("struct {{{}}}", parts.join(", "))
    }
    Type::WithBehavior(wb) => {
      let inner = format_type(arena, wb.inner, var_names, counter);
      format!("{} with {}", inner, wb.behavior)
    }
    Type::Scheme(scheme) => {
      let vars: Vec<_> = scheme
        .vars
        .iter()
        .map(|v| var_name(*v, var_names, counter))
        .collect();
      let ty = format_type(arena, scheme.ty, var_names, counter);
      format!("forall {}. {}", vars.join(" "), ty)
    }
    Type::Sum(row) => {
      let fields = row.fields.clone();
      let has_rest = row.rest.is_some();
      let parts: Vec<_> = fields
        .iter()
        .map(|(n, t)| {
          let t_root = find_root(arena, *t);
          let t_ty = arena.types[t_root].clone();
          match t_ty {
            Type::TypeApp(app) if app.args.len() == 1 => {
              let arg_root = find_root(arena, app.args[0]);
              let arg_ty = arena.types[arg_root].clone();
              match arg_ty {
                Type::TypeCons(c) if c.name == "null" => n.clone(),
                _ => format!(
                  "{}({})",
                  n,
                  format_type(arena, app.args[0], var_names, counter)
                ),
              }
            }
            _ => n.clone(),
          }
        })
        .collect();
      if has_rest {
        format!("{} or ...", parts.join(" or "))
      } else {
        parts.join(" or ")
      }
    }
  }
}

pub fn freshen(
  arena: &mut TypeArena,
  counters: &mut FreshCounters,
  ty: TypeID,
  type_vars: &mut HashMap<VarID, TypeID>,
  row_vars: &mut HashMap<VarID, TypeID>,
) -> TypeID {
  let ty_val = arena.types[ty].clone();

  match ty_val {
    Type::TypeVar(v) => *type_vars
      .entry(v.id)
      .or_insert_with(|| fresh_type_var(arena, counters)),

    Type::RowVar(v) => *row_vars
      .entry(v.id)
      .or_insert_with(|| fresh_row_var(arena, counters)),

    Type::TypeCons(_) => ty,

    Type::TypeApp(app) => {
      let head = freshen(arena, counters, app.head, type_vars, row_vars);
      let args = app
        .args
        .into_iter()
        .map(|t| freshen(arena, counters, t, type_vars, row_vars))
        .collect();
      arena.alloc(Type::TypeApp(TypeApp::new(head, args)))
    }

    Type::Row(row) => {
      let fields = row
        .fields
        .into_iter()
        .map(|(n, t)| (n, freshen(arena, counters, t, type_vars, row_vars)))
        .collect();
      let rest = row
        .rest
        .map(|t| freshen(arena, counters, t, type_vars, row_vars));
      arena.alloc(Type::Row(Row::new(fields, rest)))
    }

    Type::WithBehavior(wb) => {
      let inner = freshen(arena, counters, wb.inner, type_vars, row_vars);
      let methods = Row {
        fields: wb
          .methods
          .fields
          .into_iter()
          .map(|(n, t)| (n, freshen(arena, counters, t, type_vars, row_vars)))
          .collect(),
        rest: wb
          .methods
          .rest
          .map(|t| freshen(arena, counters, t, type_vars, row_vars)),
      };
      arena.alloc(Type::WithBehavior(WithBehavior {
        inner,
        behavior: wb.behavior,
        methods,
      }))
    }

    Type::Struct(s) => {
      let fields = Row {
        fields: s
          .fields
          .fields
          .into_iter()
          .map(|(n, t)| (n, freshen(arena, counters, t, type_vars, row_vars)))
          .collect(),
        rest: s
          .fields
          .rest
          .map(|t| freshen(arena, counters, t, type_vars, row_vars)),
      };
      arena.alloc(Type::Struct(Struct { fields }))
    }
    Type::Sum(row) => {
      let fields = row
        .fields
        .into_iter()
        .map(|(n, t)| (n, freshen(arena, counters, t, type_vars, row_vars)))
        .collect();
      let rest = row
        .rest
        .map(|t| freshen(arena, counters, t, type_vars, row_vars));
      arena.alloc(Type::Sum(Row::new(fields, rest)))
    }
    Type::Scheme(scheme) => {
      let ty = freshen(arena, counters, scheme.ty, type_vars, row_vars);
      arena.alloc(Type::Scheme(Scheme {
        vars: scheme.vars,
        ty,
      }))
    }
    Type::Link(id) => freshen(arena, counters, id, type_vars, row_vars),

    Type::Error => ty,
  }
}
