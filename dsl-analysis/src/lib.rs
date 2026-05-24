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
    for rule in &registered {
      rule.check(source, stmt, scope, catalog, &mut out);
    }
  }
  out
}
