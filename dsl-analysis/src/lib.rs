//! Diagnostic engine for duck-sqllsp.
//!
//! Each rule is a [`LintRule`] impl in [`rules`]; [`run`] fans out every
//! statement across every registered rule and returns the flat diagnostic
//! list. Rules are tagged with stable codes (sql000..sql099) so users can
//! disable individual rules through configuration.

pub mod ct_model;
pub mod diagnostic;
pub mod rules;
pub mod typing;

pub use diagnostic::{Diagnostic, Severity};

use dsl_catalog::Catalog;
use dsl_parse::{Dialect, ParseError, ParsedFile, Statement};
use dsl_resolve::Scope;

/// Diagnostic codes that detect MySQL syntax inside a PG buffer. When
/// the buffer's `Dialect` is MySql these are irrelevant -- the syntax
/// they flag is correct for the actual target -- so we skip them.
const MYSQL_PORT_CODES: &[&str] = &[
  "sql276", // INTERVAL literal needs quotes
  "sql313", // inline COMMENT in CREATE TABLE
  "sql314", // AUTO_INCREMENT
  "sql315", // ENGINE=
  "sql316", // TINYINT / MEDIUMINT / LONGTEXT / DATETIME / BLOB
];

/// MSSQL/T-SQL port-detection codes. Skipped on MSSQL buffers (we
/// route those through Generic since we have no MSSQL dialect yet).
const MSSQL_PORT_CODES: &[&str] = &["sql317", "sql318", "sql321", "sql322"];

/// Oracle port-detection codes.
const ORACLE_PORT_CODES: &[&str] = &["sql323", "sql324", "sql325", "sql326"];

/// Cross-dialect codes (ISNULL/NVL/IFNULL, GETDATE/SYSDATE) -- skip on
/// any non-PG buffer since the rewrite suggestion is dialect-specific.
const CROSS_DIALECT_CODES: &[&str] = &["sql319", "sql320"];

fn skip_for_dialect(dialect: Dialect, code: &str) -> bool {
  match dialect {
    Dialect::Postgres => false,
    Dialect::MySql => MYSQL_PORT_CODES.contains(&code) || CROSS_DIALECT_CODES.contains(&code),
    Dialect::MsSql => MSSQL_PORT_CODES.contains(&code) || CROSS_DIALECT_CODES.contains(&code),
    Dialect::SQLite | Dialect::Generic => {
      MYSQL_PORT_CODES.contains(&code)
        || MSSQL_PORT_CODES.contains(&code)
        || ORACLE_PORT_CODES.contains(&code)
        || CROSS_DIALECT_CODES.contains(&code)
    }
  }
}

/// Per-statement parser errors -> sql000 diagnostics. Always run.
fn parser_diags(errors: &[ParseError]) -> Vec<Diagnostic> {
  errors
    .iter()
    .map(|e| Diagnostic {
      code: "sql000",
      severity: Severity::Error,
      message: format!("syntax error: {}", e.message),
      range: e.range,
    })
    .collect()
}

pub trait LintRule: Send + Sync {
  fn code(&self) -> &'static str;
  fn default_severity(&self) -> Severity;
  fn check(&self, source: &str, stmt: &Statement, scope: &Scope, catalog: &Catalog, out: &mut Vec<Diagnostic>);
}

pub fn run(source: &str, file: &ParsedFile, scopes: &[Scope], catalog: &Catalog) -> Vec<Diagnostic> {
  run_with_dialect(source, file, scopes, catalog, Dialect::Postgres)
}

/// Like [`run`] but skips port-detection rules that don't apply when
/// the buffer's dialect already matches the would-be foreign syntax.
/// e.g. on a MySQL buffer, the AUTO_INCREMENT-is-MySQL hint is wrong.
pub fn run_with_dialect(
  source: &str,
  file: &ParsedFile,
  scopes: &[Scope],
  catalog: &Catalog,
  dialect: Dialect,
) -> Vec<Diagnostic> {
  let mut out = parser_diags(&file.errors);
  let registered = rules::all();
  for (stmt, scope) in file.statements.iter().zip(scopes.iter()) {
    let trimmed = trim_stmt_range(stmt, source);
    for rule in &registered {
      if skip_for_dialect(dialect, rule.code()) { continue }
      rule.check(source, &trimmed, scope, catalog, &mut out);
    }
  }
  // Belt-and-suspenders: some rules emit a different diagnostic code
  // than their `rule.code()` value (e.g. composite rules). Drop any
  // produced diagnostic whose emitted code is dialect-skipped.
  out.retain(|d| !skip_for_dialect(dialect, d.code));
  apply_suppressions(source, &mut out);
  out
}

fn apply_suppressions(source: &str, diags: &mut Vec<Diagnostic>) {
  let suppressions = collect_suppressions(source);
  diags.retain(|d| {
    let line = line_of(source, u32::from(d.range.start()) as usize);
    !suppressions.iter().any(|(suppr_line, codes)| {
      *suppr_line == line && (codes.is_empty() || codes.iter().any(|c| *c == d.code))
    })
  });
}

/// Walk every line; for each `-- duck-sqllsp: ignore[-next-line] [<code>...]`
/// emit a `(target_line, codes)` tuple. Empty `codes` means "every
/// diagnostic on the target line".
fn collect_suppressions(source: &str) -> Vec<(usize, Vec<&'static str>)> {
  let mut out: Vec<(usize, Vec<&'static str>)> = Vec::new();
  for (idx, line) in source.lines().enumerate() {
    let Some(at) = line.to_ascii_lowercase().find("-- duck-sqllsp:") else { continue };
    let payload = line[at + "-- duck-sqllsp:".len()..].trim().to_ascii_lowercase();
    let next_line = payload.starts_with("ignore-next-line");
    let same_line = payload.starts_with("ignore");
    if !next_line && !same_line {
      continue;
    }
    let after_kw = if next_line { "ignore-next-line".len() } else { "ignore".len() };
    let codes_raw = payload[after_kw..].trim();
    let codes: Vec<&'static str> = codes_raw
      .split(|c: char| c.is_whitespace() || c == ',')
      .filter(|s| !s.is_empty())
      .filter_map(|s| static_code_name(s))
      .collect();
    let target = if next_line { idx + 1 } else { idx };
    out.push((target, codes));
  }
  out
}

/// Diagnostic.code is &'static str; only allow well-formed sqlNNN /
/// custom-named codes that fit into a leaked static slot. Returning
/// None for anything else (typo / garbage) is safer than allocating
/// per-comment.
fn static_code_name(raw: &str) -> Option<&'static str> {
  let trimmed = raw.trim();
  if !trimmed.starts_with("sql") {
    return None;
  }
  // Allocate a 'static slot for each unique code seen. Cheap: there
  // are only ~150 distinct codes in the rule registry.
  use std::collections::HashMap;
  use std::sync::Mutex;
  use std::sync::OnceLock;
  static CACHE: OnceLock<Mutex<HashMap<String, &'static str>>> = OnceLock::new();
  let cache = CACHE.get_or_init(|| Mutex::new(HashMap::new()));
  let mut guard = cache.lock().ok()?;
  if let Some(s) = guard.get(trimmed) {
    return Some(*s);
  }
  let leaked: &'static str = Box::leak(trimmed.to_string().into_boxed_str());
  guard.insert(trimmed.to_string(), leaked);
  Some(leaked)
}

fn line_of(source: &str, byte: usize) -> usize {
  source[..byte.min(source.len())].bytes().filter(|b| *b == b'\n').count()
}

/// Build a `Statement` clone whose range starts at the first non-
/// whitespace byte. Per-rule arithmetic on stmt.range then maps to
/// the actual statement body instead of leading whitespace.
fn trim_stmt_range(stmt: &dsl_parse::Statement, source: &str) -> dsl_parse::Statement {
  let s: u32 = stmt.range.start().into();
  let e: u32 = stmt.range.end().into();
  let mut start = s as usize;
  let end = (e as usize).min(source.len());
  let bytes = source.as_bytes();
  while start < end && bytes[start].is_ascii_whitespace() {
    start += 1;
  }
  let mut out = stmt.clone();
  out.range = text_size::TextRange::new((start as u32).into(), (end as u32).into());
  out
}
