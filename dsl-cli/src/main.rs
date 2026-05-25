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
  /// List every registered lint rule (code + default severity).
  Rules {
    /// Emit machine-readable JSON instead of the human table.
    #[arg(long)]
    json: bool,
    /// Only list rules with this default severity (error/warning/info/hint).
    #[arg(long)]
    severity: Option<String>,
  },
  /// Lint one or more .sql files; emit diagnostics to stdout.
  ///
  /// Exit status: 0 = no errors (warnings/hints OK); 1 = at least one
  /// error-level diagnostic; 2 = an input file could not be read.
  Lint {
    /// File paths to lint. Use `-` to read SQL from stdin.
    files: Vec<String>,
    /// Output format: text (human, default) or json (one row per diagnostic).
    #[arg(long, default_value = "text")]
    format: String,
    /// Treat warnings as errors (exit 1 if any warnings found).
    #[arg(long)]
    warnings_as_errors: bool,
    /// SQL dialect: postgres (default), mysql, sqlite, generic.
    #[arg(long, default_value = "postgres")]
    dialect: String,
  },
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
    Cmd::Rules { json, severity } => {
      let filter = severity.as_deref().map(|s| s.to_ascii_lowercase());
      let mut rules: Vec<(String, &'static str)> = dsl_analysis::rules::all()
        .into_iter()
        .map(|r| (r.code().to_string(), match r.default_severity() {
          dsl_analysis::Severity::Error => "error",
          dsl_analysis::Severity::Warning => "warning",
          dsl_analysis::Severity::Info => "info",
          dsl_analysis::Severity::Hint => "hint",
        }))
        .filter(|(_, sev)| filter.as_deref().is_none_or(|f| *sev == f))
        .collect();
      rules.sort_by(|a, b| a.0.cmp(&b.0));
      if json {
        print!("[");
        for (i, (code, sev)) in rules.iter().enumerate() {
          if i > 0 { print!(","); }
          print!("{{\"code\":\"{code}\",\"default_severity\":\"{sev}\"}}");
        }
        println!("]");
        return Ok(());
      }
      let mut by_sev: std::collections::BTreeMap<&str, usize> = Default::default();
      println!("{:6}  {:8}", "code", "severity");
      for (code, sev) in &rules {
        println!("{:6}  {:8}", code, sev);
        *by_sev.entry(sev).or_insert(0) += 1;
      }
      println!();
      println!("total: {} rules", rules.len());
      for (sev, n) in by_sev { println!("  {sev}: {n}"); }
      Ok(())
    },
    Cmd::Lint { files, format, warnings_as_errors, dialect } => {
      let dialect = match dialect.to_ascii_lowercase().as_str() {
        "postgres" | "pg" => dsl_parse::Dialect::Postgres,
        "mysql" => dsl_parse::Dialect::MySql,
        "sqlite" => dsl_parse::Dialect::SQLite,
        "mssql" | "tsql" | "sqlserver" => dsl_parse::Dialect::MsSql,
        "generic" => dsl_parse::Dialect::Generic,
        other => {
          eprintln!("error: unknown dialect '{other}'; valid: postgres, mysql, sqlite, mssql, generic");
          std::process::exit(2);
        }
      };
      let json = matches!(format.as_str(), "json");
      let mut error_count = 0usize;
      let mut warning_count = 0usize;
      if json { print!("["); }
      let mut json_first = true;
      let inputs: Vec<String> = if files.is_empty() { vec!["-".to_string()] } else { files };
      for path in &inputs {
        let source = if path == "-" {
          use std::io::Read;
          let mut buf = String::new();
          std::io::stdin().read_to_string(&mut buf).map_err(|e| {
            eprintln!("error reading stdin: {e}");
            anyhow::anyhow!("stdin read failed")
          })?;
          buf
        } else {
          match std::fs::read_to_string(path) {
            Ok(s) => s,
            Err(e) => { eprintln!("error reading {path}: {e}"); std::process::exit(2); }
          }
        };
        let parsed = dsl_parse::parse(&source, dialect);
        let scopes = dsl_resolve::resolve_with_source(&parsed.statements, &source);
        let catalog = dsl_completion::source_tables::from_source(&parsed, &source);
        let diags = dsl_analysis::run_with_dialect(&source, &parsed, &scopes, &catalog, dialect);
        for d in &diags {
          let sev_str = match d.severity {
            dsl_analysis::Severity::Error => { error_count += 1; "error" }
            dsl_analysis::Severity::Warning => { warning_count += 1; "warning" }
            dsl_analysis::Severity::Info => "info",
            dsl_analysis::Severity::Hint => "hint",
          };
          let s: u32 = d.range.start().into();
          let e: u32 = d.range.end().into();
          let (line, col) = byte_to_line_col(&source, s as usize);
          if json {
            if !json_first { print!(","); }
            json_first = false;
            let msg_esc = d.message.replace('\\', "\\\\").replace('"', "\\\"");
            print!(
              "{{\"file\":\"{}\",\"line\":{},\"col\":{},\"start\":{},\"end\":{},\"severity\":\"{}\",\"code\":\"{}\",\"message\":\"{}\"}}",
              path, line + 1, col + 1, s, e, sev_str, d.code, msg_esc,
            );
          } else {
            println!("{path}:{}:{}: {sev_str} [{code}] {msg}", line + 1, col + 1, code = d.code, msg = d.message);
          }
        }
      }
      if json { println!("]"); }
      if error_count > 0 || (warnings_as_errors && warning_count > 0) {
        std::process::exit(1);
      }
      Ok(())
    },
  }
}

fn byte_to_line_col(src: &str, off: usize) -> (usize, usize) {
  let mut line = 0usize;
  let mut col = 0usize;
  for (i, b) in src.bytes().enumerate() {
    if i >= off { break }
    if b == b'\n' { line += 1; col = 0 } else { col += 1 }
  }
  (line, col)
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
