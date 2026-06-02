//! sql324: `ROWNUM` -- Oracle pseudo-column. PG has no ROWNUM;
//! use `LIMIT N` (paging top-N) or `ROW_NUMBER() OVER (...)`
//! (ranking) instead.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql324"
  }
  fn default_severity(&self) -> Severity {
    Severity::Error
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, _body, upper) = crate::stmt_body_upper(stmt, source);
    let bytes = upper.as_bytes();
    let mut from = 0usize;
    while let Some(rel) = upper[from..].find("ROWNUM") {
      let at = from + rel;
      let prev_ok = at == 0
        || !{
          let p = bytes[at - 1] as char;
          p.is_ascii_alphanumeric() || p == '_'
        };
      let after = at + "ROWNUM".len();
      let after_ok = after >= bytes.len()
        || !{
          let p = bytes[after] as char;
          p.is_ascii_alphanumeric() || p == '_'
        };
      if !prev_ok || !after_ok {
        from = after;
        continue;
      }
      let abs_s = start + at;
      let abs_e = abs_s + "ROWNUM".len();
      out.push(Diagnostic {
        code: "sql324",
        severity: Severity::Error,
        message: "`ROWNUM` is Oracle's pseudo-column -- PG uses `LIMIT N` (top-N) or `ROW_NUMBER() OVER (ORDER BY ...)` (ranking)".into(),
        range: crate::range_at(abs_s, abs_e),
      });
      from = after;
    }
  }
}
