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
  Server {
    /// Accepted for compatibility with VS Code's
    /// vscode-languageclient (TransportKind.stdio appends `--stdio`
    /// to every command it spawns). We always use stdio anyway, so
    /// the flag is a no-op.
    #[arg(long, hide = true)]
    stdio: bool,
    /// Accepted for compatibility with editors that pass `--node-ipc`
    /// or `--socket=...`. Ignored.
    #[arg(long, hide = true)]
    node_ipc: bool,
    #[arg(long, hide = true)]
    socket: Option<String>,
  },
  /// Print version and capability info.
  Version,
}

fn main() -> anyhow::Result<()> {
  init_tracing();
  // Accept unknown flags gracefully so we never blow up on a transport
  // flag we didn't anticipate. clap's `Cli::parse` exits on unknown
  // args; try the strict parse first, fall back to "drop any --flag /
  // --flag=value the LSP client sent and re-parse the rest".
  let argv: Vec<String> = std::env::args().collect();
  let cli = Cli::try_parse_from(&argv).unwrap_or_else(|_| {
    let filtered: Vec<String> = argv
      .into_iter()
      .enumerate()
      .filter(|(i, a)| *i == 0 || !a.starts_with("--"))
      .map(|(_, a)| a)
      .collect();
    Cli::try_parse_from(&filtered).unwrap_or(Cli { cmd: Some(Cmd::Server { stdio: true, node_ipc: false, socket: None }) })
  });
  match cli.cmd.unwrap_or(Cmd::Server { stdio: true, node_ipc: false, socket: None }) {
    Cmd::Server { .. } => server::run(),
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
