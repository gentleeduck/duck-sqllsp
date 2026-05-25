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
  out
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
