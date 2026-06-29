use tracing_subscriber::{EnvFilter, fmt};

pub fn init_logging() {
  let file = tracing_appender::rolling::never(
    std::env::current_dir().unwrap().join("testing"),
    "cstar-lsp.log",
  );
  fmt()
    .with_env_filter(EnvFilter::new("info"))
    .with_writer(file)
    .with_ansi(false)
    .init();
}
