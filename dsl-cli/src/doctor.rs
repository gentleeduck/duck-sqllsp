//! `duck-sqllsp doctor` -- report what the server can actually see.
//!
//! Every "the LSP isn't working" report comes down to one of a small set
//! of environment questions, and none of them are visible from inside
//! the editor:
//!
//!   * Is the external `sql-formatter` on `PATH`? Without it formatting
//!     silently degrades to the built-in alignment pass -- output still
//!     changes, just far less than expected, and nothing says why.
//!   * Which config file was found, and did its settings actually apply?
//!   * Where did the workspace root land, and how many `.sql` files did
//!     the offline scan reach? A root resolved too high scans a
//!     directory tree that isn't the project; too low finds nothing.
//!   * Is a connection configured, and does it answer?
//!   * Which dialect won, given that it can come from config or be
//!     inferred from the connection URL?
//!
//! Each check prints `ok` / `warn` / `fail` with the *observed value*,
//! not just a verdict, so the output is worth pasting into a bug report.
//! Exit status is 0 unless something is actually broken; warnings do not
//! fail, because "no connection configured" is a perfectly normal way to
//! run this server.

use std::path::{Path, PathBuf};

/// How a single check came out.
enum Status {
  Ok,
  Warn,
  Fail,
}

impl Status {
  fn label(&self) -> &'static str {
    match self {
      Status::Ok => "ok  ",
      Status::Warn => "warn",
      Status::Fail => "FAIL",
    }
  }
}

struct Report {
  failures: usize,
  warnings: usize,
}

impl Report {
  fn new() -> Self {
    Self { failures: 0, warnings: 0 }
  }

  /// Print one check. `detail` lines are indented under it.
  fn check(&mut self, status: Status, name: &str, value: &str) {
    match status {
      Status::Fail => self.failures += 1,
      Status::Warn => self.warnings += 1,
      Status::Ok => {},
    }
    println!("[{}] {name:<22} {value}", status.label());
  }

  fn detail(&self, text: &str) {
    println!("       {text}");
  }
}

pub fn run(path: Option<String>) -> anyhow::Result<()> {
  let start = match path {
    Some(p) => PathBuf::from(p),
    None => std::env::current_dir()?,
  };

  println!("duck-sqllsp {} -- checking {}\n", env!("CARGO_PKG_VERSION"), start.display());
  let mut r = Report::new();

  check_formatter(&mut r);
  let cfg = check_config(&mut r, &start);
  check_dialect(&mut r, &cfg);
  let root = check_workspace(&mut r, &start);
  check_offline_catalog(&mut r, root.as_deref());
  check_connection(&mut r, &cfg);

  println!();
  if r.failures > 0 {
    println!("{} problem(s), {} warning(s).", r.failures, r.warnings);
    std::process::exit(1);
  }
  if r.warnings > 0 {
    println!("No problems. {} warning(s) -- see above for whether they matter to you.", r.warnings);
  } else {
    println!("No problems.");
  }
  Ok(())
}

/// The external formatter is optional, and its absence is invisible from
/// the editor: formatting still "works", it just does much less.
fn check_formatter(r: &mut Report) {
  match dsl_format::external::locate_binary() {
    Some(path) => r.check(Status::Ok, "sql-formatter", &path),
    None => {
      r.check(Status::Warn, "sql-formatter", "not found on PATH");
      r.detail("Formatting falls back to the built-in passes. Still applied:");
      r.detail("  tabWidth, keywordCase, singleLine, compactClauses, and every");
      r.detail("  style.createTable option.");
      r.detail("Silently inactive without the binary:");
      r.detail("  dataTypeCase, functionCase, expressionWidth, denseOperators,");
      r.detail("  linesBetweenQueries, newlineBeforeSemicolon, logicalOperatorNewline.");
      r.detail("Install with:  npm i -g sql-formatter");
    },
  }
}

/// Which config file was found and whether it parsed. Reports the
/// *effective* values so a file that parsed into nothing is visible.
fn check_config(r: &mut Report, start: &Path) -> dsl_server::config::DuckSqllspConfig {
  let found = find_config_file(start);
  match &found {
    Some(p) => r.check(Status::Ok, "config file", &p.display().to_string()),
    None => {
      r.check(Status::Warn, "config file", "none found");
      r.detail("Looked for .duck-sqllsp.toml / .duck-sqllsp.json walking up from here.");
      r.detail("Defaults are in use. See dsl-server/docs/configuration.md.");
    },
  }

  let cfg = dsl_server::config::load_project_config(start).unwrap_or_default();
  if let Some(p) = &found {
    // A file that exists but yields a wholly default config is the
    // signature of a parse that silently produced nothing.
    let looks_empty =
      cfg.connections.is_empty() && cfg.active_connection.is_none() && cfg.rules.is_empty() && cfg.dialect.is_none();
    let has_bytes = std::fs::read_to_string(p).map(|s| !s.trim().is_empty()).unwrap_or(false);
    if looks_empty && has_bytes {
      r.check(Status::Warn, "config contents", "parsed, but set nothing recognisable");
      r.detail("Every key fell back to its default. Check the key names against");
      r.detail("dsl-server/docs/configuration.md -- unknown keys are ignored silently.");
    } else {
      let mut parts = Vec::new();
      if !cfg.connections.is_empty() {
        parts.push(format!("{} connection(s)", cfg.connections.len()));
      }
      if !cfg.rules.is_empty() {
        parts.push(format!("{} rule override(s)", cfg.rules.len()));
      }
      if cfg.dialect.is_some() {
        parts.push("dialect".into());
      }
      if parts.is_empty() {
        parts.push("style only".into());
      }
      r.check(Status::Ok, "config contents", &parts.join(", "));
    }
  }
  cfg
}

fn check_dialect(r: &mut Report, cfg: &dsl_server::config::DuckSqllspConfig) {
  let effective = cfg.effective_dialect();
  let source = if cfg.dialect.is_some() {
    "from config"
  } else if cfg.active().is_some() {
    "inferred from the active connection"
  } else {
    "default"
  };
  r.check(Status::Ok, "dialect", &format!("{effective:?} ({source})"));
}

/// Where the workspace root lands decides what the offline scan reaches.
fn check_workspace(r: &mut Report, start: &Path) -> Option<PathBuf> {
  let root = derive_root(start);
  match &root {
    Some(p) => {
      // A root at the filesystem root means no marker was found, and the
      // scan would crawl far more than the project.
      if p.parent().is_none() {
        r.check(Status::Fail, "workspace root", &format!("{} (filesystem root)", p.display()));
        r.detail("No .duck-sqllsp.toml / .git / Cargo.toml / package.json marker was found");
        r.detail("above this directory, so the offline scan would walk the whole filesystem.");
        r.detail("Add a marker file at the project root.");
      } else {
        r.check(Status::Ok, "workspace root", &p.display().to_string());
      }
    },
    None => r.check(Status::Warn, "workspace root", "could not be derived"),
  }
  root
}

/// What the offline scan actually finds. Zero tables in a project that
/// has `.sql` files usually means the root is wrong.
fn check_offline_catalog(r: &mut Report, root: Option<&Path>) {
  let Some(root) = root else { return };
  let mut files = 0usize;
  let mut tables = 0usize;
  let mut functions = 0usize;
  walk_sql(root, &mut files, &mut |text| {
    let parsed = dsl_parse::parse(text, dsl_parse::Dialect::Postgres);
    let cat = dsl_completion::source_tables::from_source(&parsed, text);
    tables += cat.tables().count();
    functions += cat.functions.len();
  });

  let summary = format!("{files} .sql file(s), {tables} table(s), {functions} function(s)");
  if files == 0 {
    r.check(Status::Warn, "offline catalog", &summary);
    r.detail("No .sql files under the workspace root. Completion and hover will only");
    r.detail("know about the buffer you have open, plus any live connection.");
  } else if tables == 0 {
    r.check(Status::Warn, "offline catalog", &summary);
    r.detail("Files were found but no CREATE TABLE was derived from them.");
  } else {
    r.check(Status::Ok, "offline catalog", &summary);
  }
}

fn check_connection(r: &mut Report, cfg: &dsl_server::config::DuckSqllspConfig) {
  let Some(active) = cfg.active() else {
    r.check(Status::Warn, "connection", "none active");
    r.detail("This is fine -- the server works offline from the workspace scan.");
    r.detail("Catalog-backed diagnostics (sql001, sql002) stay quiet unless you set");
    r.detail("requireConnection = false.");
    return;
  };

  if active.driver() == "unknown" {
    r.check(Status::Fail, "connection", &format!("{} -- unrecognised URL scheme", active.name));
    r.detail("The driver is chosen from the URL scheme: postgres:// postgresql://");
    r.detail("mysql:// mariadb:// sqlite:// sqlite:  -- a bare path is not enough.");
    return;
  }
  r.check(Status::Ok, "connection", &format!("{} ({})", active.name, active.driver()));

  match dsl_conn::build(active) {
    Err(e) => {
      r.check(Status::Fail, "connection build", &e.to_string());
    },
    Ok(driver) => {
      let rt = match tokio::runtime::Runtime::new() {
        Ok(rt) => rt,
        Err(e) => {
          r.check(Status::Fail, "connection probe", &e.to_string());
          return;
        },
      };
      match rt.block_on(driver.introspect()) {
        Ok(cat) => {
          let tables = cat.tables().count();
          let cols: usize = cat.tables().map(|t| t.columns.len()).sum();
          r.check(
            Status::Ok,
            "introspect",
            &format!("{tables} table(s), {cols} column(s), {} function(s)", cat.functions.len()),
          );
        },
        Err(e) => {
          r.check(Status::Fail, "introspect", &e.to_string());
          r.detail("The server falls back to the offline catalog when this fails.");
        },
      }
    },
  }
}

/// Same marker list the server uses when the editor sends no root.
fn find_config_file(start: &Path) -> Option<PathBuf> {
  let mut dir = if start.is_dir() { start.to_path_buf() } else { start.parent()?.to_path_buf() };
  loop {
    for name in [".duck-sqllsp.toml", ".duck-sqllsp.json"] {
      let candidate = dir.join(name);
      if candidate.is_file() {
        return Some(candidate);
      }
    }
    if !dir.pop() {
      return None;
    }
  }
}

fn derive_root(start: &Path) -> Option<PathBuf> {
  let mut dir = if start.is_dir() { start.to_path_buf() } else { start.parent()?.to_path_buf() };
  let immediate = dir.clone();
  loop {
    for marker in [".duck-sqllsp.toml", ".duck-sqllsp.json", ".git", "Cargo.toml", "package.json"] {
      if dir.join(marker).exists() {
        return Some(dir);
      }
    }
    if !dir.pop() {
      return Some(immediate);
    }
  }
}

/// Bounded `.sql` walk mirroring the server's own scan limits.
fn walk_sql(root: &Path, count: &mut usize, f: &mut impl FnMut(&str)) {
  const MAX_FILES: usize = 5000;
  const MAX_FILE_BYTES: u64 = 4 * 1024 * 1024;
  if *count >= MAX_FILES {
    return;
  }
  let Ok(rd) = std::fs::read_dir(root) else { return };
  for entry in rd.flatten() {
    if *count >= MAX_FILES {
      return;
    }
    let path = entry.path();
    if let Some(name) = path.file_name().and_then(|s| s.to_str())
      && (name.starts_with('.') || matches!(name, "node_modules" | "target" | "dist" | "build" | "vendor" | "out"))
    {
      continue;
    }
    if path.is_dir() {
      walk_sql(&path, count, f);
    } else if let Some(ext) = path.extension().and_then(|s| s.to_str())
      && matches!(ext.to_ascii_lowercase().as_str(), "sql" | "pgsql" | "psql")
    {
      let Ok(meta) = std::fs::metadata(&path) else { continue };
      if meta.len() > MAX_FILE_BYTES {
        continue;
      }
      if let Ok(text) = std::fs::read_to_string(&path) {
        *count += 1;
        f(&text);
      }
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  /// Scratch directory, removed and recreated per test.
  fn scratch(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("duck-sqllsp-doctor-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("scratch dir");
    dir
  }

  #[test]
  fn find_config_walks_upward() {
    let root = scratch("cfg-up");
    std::fs::write(root.join(".duck-sqllsp.toml"), "[duck_sqllsp]\n").unwrap();
    let nested = root.join("a/b/c");
    std::fs::create_dir_all(&nested).unwrap();
    let found = find_config_file(&nested).expect("should walk up to the root config");
    assert_eq!(found, root.join(".duck-sqllsp.toml"));
    let _ = std::fs::remove_dir_all(&root);
  }

  #[test]
  fn find_config_prefers_toml_over_json() {
    let root = scratch("cfg-pref");
    std::fs::write(root.join(".duck-sqllsp.toml"), "[duck_sqllsp]\n").unwrap();
    std::fs::write(root.join(".duck-sqllsp.json"), "{}").unwrap();
    assert!(find_config_file(&root).unwrap().to_string_lossy().ends_with(".toml"));
    let _ = std::fs::remove_dir_all(&root);
  }

  #[test]
  fn derive_root_stops_at_a_marker() {
    let root = scratch("root-marker");
    std::fs::create_dir_all(root.join(".git")).unwrap();
    let nested = root.join("db/migrations");
    std::fs::create_dir_all(&nested).unwrap();
    assert_eq!(derive_root(&nested).unwrap(), root);
    let _ = std::fs::remove_dir_all(&root);
  }

  #[test]
  fn walk_sql_finds_nested_files_and_skips_noise() {
    let root = scratch("walk");
    std::fs::create_dir_all(root.join("migrations")).unwrap();
    std::fs::create_dir_all(root.join("node_modules")).unwrap();
    std::fs::create_dir_all(root.join(".hidden")).unwrap();
    std::fs::write(root.join("migrations/a.sql"), "CREATE TABLE t (id int);").unwrap();
    std::fs::write(root.join("b.PSQL"), "CREATE TABLE u (id int);").unwrap();
    std::fs::write(root.join("notes.md"), "not sql").unwrap();
    std::fs::write(root.join("node_modules/dep.sql"), "CREATE TABLE skipme (id int);").unwrap();
    std::fs::write(root.join(".hidden/h.sql"), "CREATE TABLE alsoskip (id int);").unwrap();

    let mut count = 0usize;
    let mut seen = Vec::new();
    walk_sql(&root, &mut count, &mut |text| seen.push(text.to_string()));

    assert_eq!(count, 2, "expected a.sql and b.PSQL only, saw {seen:?}");
    assert!(seen.iter().any(|s| s.contains("TABLE t")));
    assert!(seen.iter().any(|s| s.contains("TABLE u")), "extension match must be case-insensitive");
    assert!(!seen.iter().any(|s| s.contains("skipme")), "node_modules must be skipped");
    assert!(!seen.iter().any(|s| s.contains("alsoskip")), "hidden dirs must be skipped");
    let _ = std::fs::remove_dir_all(&root);
  }

  #[test]
  fn walk_sql_on_a_missing_directory_is_not_an_error() {
    let mut count = 0usize;
    walk_sql(Path::new("/definitely/not/here"), &mut count, &mut |_| {});
    assert_eq!(count, 0);
  }

  #[test]
  fn report_counts_failures_and_warnings_separately() {
    let mut r = Report::new();
    r.check(Status::Ok, "a", "");
    r.check(Status::Warn, "b", "");
    r.check(Status::Warn, "c", "");
    r.check(Status::Fail, "d", "");
    assert_eq!((r.failures, r.warnings), (1, 2));
  }
}
