//! `duck-sqllsp` binary entry point.
//!
//! Defaults to `server` (LSP over stdio). Future subcommands (`lint`,
//! `format`, `introspect`, `version`) are sketched but not yet wired.

mod server;

use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(name = "duck-sqllsp", version, about, long_about = None)]
struct Cli {
  #[command(subcommand)]
  cmd: Option<Cmd>,
}

#[derive(Subcommand, Debug)]
enum Cmd {
  /// Run the language server over stdio (default).
  Server,
  /// Print version and capability info.
  Version,
}

fn main() -> anyhow::Result<()> {
  init_tracing();
  let cli = Cli::parse();
  match cli.cmd.unwrap_or(Cmd::Server) {
    Cmd::Server => server::run(),
    Cmd::Version => {
      println!("duck-sqllsp {}", env!("CARGO_PKG_VERSION"));
      Ok(())
    },
  }
}

fn init_tracing() {
  // Log to stderr so stdout stays clean for JSON-RPC.
  use tracing_subscriber::EnvFilter;
  let _ = tracing_subscriber::fmt()
    .with_env_filter(EnvFilter::try_from_env("DUCK_SQLLSP_LOG").unwrap_or_else(|_| EnvFilter::new("info")))
    .with_writer(std::io::stderr)
    .with_ansi(false)
    .try_init();
}
