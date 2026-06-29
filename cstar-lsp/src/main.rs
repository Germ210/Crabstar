use std::env;

use crate::{log::init_logging, server::Backend, world::World};
use tower_lsp_server::{LspService, Server};

mod diagnostics;
mod log;
mod server;
mod world;

#[tokio::main]
async fn main() {
  init_logging();
  let stdin = tokio::io::stdin();
  let stdout = tokio::io::stdout();

  let (service, socket) = LspService::new(|client| Backend {
    client,
    world: World::new(env::current_dir().expect("Failed to get current directory")),
  });
  Server::new(stdin, stdout, socket).serve(service).await;
}
