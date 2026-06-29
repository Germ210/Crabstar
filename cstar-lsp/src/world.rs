use std::{
  path::PathBuf,
  sync::{Arc, Mutex},
};

use crabstar_frontend::module::Module;
use dashmap::DashMap;
use ropey::Rope;
use tower_lsp_server::ls_types::Uri;
use vfs::{PhysicalFS, VfsPath};

#[derive(Debug)]
pub struct CrabstarDocument {
  pub source: Rope,
  pub module: Arc<Mutex<Module>>,
}

unsafe impl Send for CrabstarDocument {}
unsafe impl Sync for CrabstarDocument {}

#[derive(Debug)]
pub struct World {
  pub open_docs: DashMap<Uri, CrabstarDocument>,
  pub root: VfsPath,
}

impl World {
  pub fn new(workspace: PathBuf) -> Self {
    Self {
      open_docs: DashMap::new(),
      root: PhysicalFS::new(workspace).into(),
    }
  }
}
