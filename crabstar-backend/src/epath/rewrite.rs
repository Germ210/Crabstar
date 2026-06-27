use crate::epath::ir::{BlockId, EPath, Expr, ExprId, Path, PathId, PathSlice};

#[macro_export]
macro_rules! build {
  ($epath:expr, $variant:ident($($args:tt)*)) => {{
    build_args!($epath, $variant, [], $($args)*)
  }};
}

#[macro_export]
macro_rules! build_args {
  ($epath:expr, $variant:ident, [$($built:expr),*]) => {{
    $epath.expr($crate::epath::ir::Expr::$variant($($built),*))
  }};
  ($epath:expr, $variant:ident, [$($built:expr),*], $Nested:ident($($inner:tt)*) $(, $($rest:tt)*)?) => {{
    let __nested = build!($epath, $Nested($($inner)*));
    build_args!($epath, $variant, [$($built,)* __nested] $(, $($rest)*)?)
  }};
  ($epath:expr, $variant:ident, [$($built:expr),*], $arg:expr $(, $($rest:tt)*)?) => {{
    build_args!($epath, $variant, [$($built,)* $arg] $(, $($rest)*)?)
  }};
}

#[macro_export]
macro_rules! smatch {
  ($slice:expr, $epath:expr, { $($arms:tt)* }) => {
    match $crate::epath::rewrite::ControlStructure::from_path(
      &$epath.paths[$slice.path.0],
      &$epath
    ) {
      $($arms)*
    }
  }
}

pub struct RewriteEngine {
  pub rules: Vec<Box<dyn Fn(&[BlockId], PathSlice, &mut EPath)>>,
}

impl RewriteEngine {
  pub fn new() -> Self {
    Self { rules: vec![] }
  }

  pub fn add_rule(&mut self, rule: impl Fn(&[BlockId], PathSlice, &mut EPath) + 'static) {
    self.rules.push(Box::new(rule));
  }

  pub fn run(&self, epath: &mut EPath) {
    loop {
      let before = epath.equalities.len();
      let path_count = epath.paths.len();

      for path_idx in 0..path_count {
        let path_len = epath.paths[path_idx].blocks.len();

        for block_idx in 0..path_len {
          let slice = PathSlice {
            path: PathId(path_idx),
            start: block_idx,
            end: path_len,
          };

          if epath.equalities.contains_key(&slice) {
            continue;
          }

          let blocks = epath.paths[path_idx].blocks[block_idx..].to_vec();

          for rule in &self.rules {
            rule(&blocks, slice.clone(), epath);
          }

          epath
            .equalities
            .entry(slice)
            .or_insert_with(std::collections::HashSet::new);
        }
      }

      if epath.equalities.len() == before {
        break;
      }
    }
  }
}

pub struct Sequence {
  pub body: Vec<BlockId>,
}

impl Sequence {
  pub fn new(body: Vec<BlockId>) -> Self {
    Self { body }
  }
}

pub enum ControlStructure {
  Sequence { body: Sequence },
  Loop { header: BlockId, body: Sequence },
}

impl ControlStructure {
  pub fn from_path(path: &Path, epath: &EPath) -> ControlStructure {
    for (i, block_id) in path.blocks.iter().enumerate() {
      let term = &epath.block_terminators[block_id];
      if let crate::epath::ir::Terminator::Jump(j) = &**term {
        if let Some(pos) = path.blocks[..i].iter().position(|b| b == &j.target) {
          return ControlStructure::Loop {
            header: j.target.clone(),
            body: Sequence::new(path.blocks[pos..=i].to_vec()),
          };
        }
      }
    }
    ControlStructure::Sequence {
      body: Sequence::new(path.blocks.clone()),
    }
  }
}

pub fn get_invariants(body: &Sequence, header: &BlockId) -> Vec<(BlockId, ExprId)> {
  let header_params: std::collections::HashSet<ExprId> = (*header).params.iter().cloned().collect();
  body
    .body
    .iter()
    .filter_map(|block_id| {
      let expr = (*block_id).expr.clone();
      if !references_any(&expr, &header_params) {
        Some((block_id.clone(), expr))
      } else {
        None
      }
    })
    .collect()
}

fn references_any(expr: &ExprId, params: &std::collections::HashSet<ExprId>) -> bool {
  if params.contains(expr) {
    return true;
  }
  match expr.as_expr() {
    Expr::IAdd(_, a, b)
    | Expr::ISub(_, a, b)
    | Expr::IMul(_, a, b)
    | Expr::IDiv(_, a, b)
    | Expr::IShl(_, a, b)
    | Expr::IShr(_, a, b)
    | Expr::IEq(_, a, b)
    | Expr::INe(_, a, b)
    | Expr::ILt(_, a, b)
    | Expr::ILe(_, a, b)
    | Expr::IGt(_, a, b)
    | Expr::IGe(_, a, b)
    | Expr::FAdd(_, a, b)
    | Expr::FSub(_, a, b)
    | Expr::FMul(_, a, b)
    | Expr::FDiv(_, a, b)
    | Expr::FEq(_, a, b)
    | Expr::FNe(_, a, b)
    | Expr::FLt(_, a, b)
    | Expr::FLe(_, a, b)
    | Expr::FGt(_, a, b)
    | Expr::FGe(_, a, b) => references_any(a, params) || references_any(b, params),
    Expr::INot(_, a) | Expr::INeg(_, a) | Expr::FNeg(_, a) | Expr::FieldPtr(a, _) => {
      references_any(a, params)
    }
    _ => false,
  }
}

pub fn hoist(invariant: BlockId, loop_path: PathSlice, epath: &mut EPath) -> PathSlice {
  let mut new_blocks = epath.paths[loop_path.path.0].blocks.clone();
  new_blocks.retain(|b| b != &invariant);
  new_blocks.insert(0, invariant.clone());
  let new_path_id = epath.add_path(Path {
    blocks: new_blocks,
    origin: Some(loop_path.clone()),
  });
  PathSlice {
    path: new_path_id,
    start: loop_path.start,
    end: loop_path.end,
  }
}
