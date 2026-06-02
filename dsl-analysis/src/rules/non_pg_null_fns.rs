//! sql319: `ISNULL(x, y)` (MSSQL/MySQL) / `NVL(x, y)` (Oracle) /
//! `IFNULL(x, y)` (MySQL) -- non-PG NULL-coalesce functions. PG
//! has `COALESCE(x, y, ...)` (SQL standard) and `NULLIF(x, y)`.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

const FNS: &[(&str, &str)] = &[("ISNULL", "MSSQL/MySQL"), ("NVL", "Oracle"), ("IFNULL", "MySQL")];

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql319"
  }
  fn default_severity(&self) -> Severity {
    Severity::Error
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, _body, upper) = crate::stmt_body_upper(stmt, source);
    let bytes = upper.as_bytes();
    for (fname, dialect) in FNS {
      let needle = format!("{fname}(");
      let mut from = 0usize;
      while let Some(rel) = upper[from..].find(&needle) {
        let at = from + rel;
        let prev_ok = at == 0
          || !{
            let p = bytes[at - 1] as char;
            p.is_ascii_alphanumeric() || p == '_'
          };
        if !prev_ok {
          from = at + needle.len();
          continue;
        }
        // Skip `IS NULL` (different from ISNULL fn) -- needle requires `(`.
        let abs_s = start + at;
        let abs_e = abs_s + fname.len();
        out.push(Diagnostic {
          code: "sql319",
          severity: Severity::Error,
          message: format!("`{fname}(...)` is {dialect} syntax -- PG uses `COALESCE(x, y, ...)` (SQL standard, n-ary)"),
          range: crate::range_at(abs_s, abs_e),
        });
        from = at + needle.len();
      }
    }
  }
}
