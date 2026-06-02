//! sql320: `GETDATE()` / `SYSDATE` / `GETUTCDATE()` -- non-PG
//! current-time forms. PG uses `now()` (or `CURRENT_TIMESTAMP`).

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

const FNS: &[(&str, &str)] =
  &[("GETDATE", "MSSQL"), ("GETUTCDATE", "MSSQL"), ("SYSDATETIME", "MSSQL"), ("SYSDATE", "Oracle")];

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql320"
  }
  fn default_severity(&self) -> Severity {
    Severity::Error
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, _body, upper) = crate::stmt_body_upper(stmt, source);
    let bytes = upper.as_bytes();
    for (fname, dialect) in FNS {
      let mut from = 0usize;
      while let Some(rel) = upper[from..].find(fname) {
        let at = from + rel;
        let prev_ok = at == 0
          || !{
            let p = bytes[at - 1] as char;
            p.is_ascii_alphanumeric() || p == '_'
          };
        let after = at + fname.len();
        let after_ok = after >= bytes.len()
          || !{
            let p = bytes[after] as char;
            p.is_ascii_alphanumeric() || p == '_'
          };
        if !prev_ok || !after_ok {
          from = at + fname.len();
          continue;
        }
        let abs_s = start + at;
        let abs_e = abs_s + fname.len();
        out.push(Diagnostic {
          code: "sql320",
          severity: Severity::Error,
          message: format!("`{fname}` is {dialect} -- PG uses `now()` (or `CURRENT_TIMESTAMP`)"),
          range: crate::range_at(abs_s, abs_e),
        });
        from = after;
      }
    }
  }
}
