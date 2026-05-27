//! sql313: `CREATE TABLE t (...) COMMENT 'msg'` -- MySQL inline-
//! comment syntax. PG requires `COMMENT ON TABLE t IS 'msg'` as a
//! separate statement. Catches the common port mistake.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql313"
  }
  fn default_severity(&self) -> Severity {
    Severity::Error
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let start: usize = u32::from(stmt.range.start()) as usize;
    let end: usize = (u32::from(stmt.range.end()) as usize).min(source.len());
    let body = &source[start..end];
    let upper = body.to_ascii_uppercase();
    if !upper.contains("CREATE TABLE") {
      return;
    }
    // Need: `) ... COMMENT '...'` or `) ... COMMENT='...'`.
    let Some(close_paren) = body.rfind(')') else { return };
    let after = body[close_paren + 1..].to_ascii_uppercase();
    let Some(c_at) = after.find("COMMENT") else { return };
    let post = after[c_at + "COMMENT".len()..].trim_start();
    if !post.starts_with('\'') && !post.starts_with('=') {
      return;
    }
    let abs_s = start + close_paren + 1 + c_at;
    let abs_e = abs_s + "COMMENT".len();
    out.push(Diagnostic {
      code: "sql313",
      severity: Severity::Error,
      message: "Inline COMMENT in CREATE TABLE is MySQL syntax -- PG needs `COMMENT ON TABLE <name> IS '<msg>'` as a separate statement".into(),
      range: text_size::TextRange::new((abs_s as u32).into(), (abs_e as u32).into()),
    });
  }
}
