use std::collections::HashMap;

use crate::types::Type;

pub struct TypeEnv {
  params: HashMap<String, Type>,
  locals: HashMap<String, Type>,
}

pub struct ConstraintBuilder {
  env: TypeEnv
}
