//! Diagnostic engine for duck-sqllsp.
//!
//! Each rule is a [`LintRule`] impl in [`rules`]; [`run`] fans out every
//! statement across every registered rule and returns the flat diagnostic
//! list. Rules are tagged with stable codes (sql000..sql099) so users can
//! disable individual rules through configuration.

pub mod ct_model;
pub mod diagnostic;
pub mod rules;

pub use diagnostic::{Diagnostic, Severity};

use dsl_catalog::Catalog;
use dsl_parse::{ParseError, ParsedFile, Statement};
use dsl_resolve::Scope;

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
  let mut out = parser_diags(&file.errors);
  let registered = rules::all();
  for (stmt, scope) in file.statements.iter().zip(scopes.iter()) {
    // pg_query / sqlparser sometimes include leading whitespace
    // (the gap after the prior `;`) in stmt.range. That extra
    // span shifts every offset rules derive from stmt.range -- so
    // diagnostics land on the prior statement's last line. Trim
    // the start to the first non-whitespace byte before passing
    // the statement down.
    let trimmed = trim_stmt_range(stmt, source);
    for rule in &registered {
      rule.check(source, &trimmed, scope, catalog, &mut out);
    }
  }
  // Apply suppression comments. `-- duck-sqllsp: ignore` on the same
  // line silences every diagnostic on that line; appending a rule
  // code (`ignore sql001`) narrows to that one. `-- duck-sqllsp:
  // ignore-next-line [code]` silences the line after the comment.
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
