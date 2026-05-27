//! sql288: `CREATE INDEX ON t (col)` -- PG auto-generates a name
//! like `t_col_idx`, but the name is hard to reference for later
//! DROP / REINDEX and gets ugly with expression indexes. Hint:
//! name the index explicitly.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql288"
  }
  fn default_severity(&self) -> Severity {
    Severity::Hint
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let start: usize = u32::from(stmt.range.start()) as usize;
    let end: usize = (u32::from(stmt.range.end()) as usize).min(source.len());
    let raw = &source[start..end];
    let body_owned = crate::textutil::strip_noise_full(raw);
    let body = body_owned.as_str();
    let upper = body.to_ascii_uppercase();
    let trim = upper.trim_start();
    if !(trim.starts_with("CREATE INDEX") || trim.starts_with("CREATE UNIQUE INDEX")) {
      return;
    }
    // Skip CONCURRENTLY for the keyword scan.
    let needle = if trim.starts_with("CREATE UNIQUE INDEX") { "CREATE UNIQUE INDEX" } else { "CREATE INDEX" };
    let after = upper.find(needle).unwrap() + needle.len();
    let rest = body[after..].trim_start();
    let rest_upper = rest.to_ascii_uppercase();
    // optional CONCURRENTLY / IF NOT EXISTS modifiers
    let mut head = rest_upper.as_str();
    let mut off = 0usize;
    if head.starts_with("CONCURRENTLY ") {
      off += "CONCURRENTLY ".len();
      head = &head[off..];
    }
    if head.starts_with("IF NOT EXISTS ") {
      off += "IF NOT EXISTS ".len();
      head = &head[off..];
    }
    // Now: either ON ... (auto-named) or <name> ON ...
    if head.starts_with("ON ") {
      let lead = body.len() - body.trim_start().len();
      let abs_s = start + lead;
      let abs_e = start + body.find(';').unwrap_or(body.len());
      out.push(Diagnostic {
        code: "sql288",
        severity: Severity::Hint,
        message: "CREATE INDEX without explicit name -- PG auto-generates one; future DROP / REINDEX needs the auto-name to be looked up. Prefer `CREATE INDEX idx_<table>_<col> ON ...`".into(),
        range: text_size::TextRange::new((abs_s as u32).into(), (abs_e as u32).into()),
      });
    }
  }
}
