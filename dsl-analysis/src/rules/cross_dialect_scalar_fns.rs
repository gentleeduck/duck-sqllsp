//! sql628: scalar functions from Oracle / SQL Server / MySQL that don't exist
//! in PostgreSQL -- `LISTAGG`, `INSTR`, `CHARINDEX`, `IIF`, `NVL2`, `LEN`. PG
//! raises 42883; each has a standard PG counterpart. Complements sql596 / sql620
//! / sql622 (other non-PG functions) and the NVL / ISNULL / IFNULL lint
//! (sql628 adds the three-argument `NVL2`, which that rule doesn't cover).

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

const FNS: &[(&str, &str)] = &[
  ("listagg(", "string_agg(...)"),
  ("instr(", "position(sub in str) / strpos(str, sub)"),
  ("charindex(", "position(sub in str) / strpos(str, sub)"),
  ("iif(", "a CASE expression"),
  ("nvl2(", "a CASE expression (CASE WHEN x IS NOT NULL THEN a ELSE b END)"),
  ("len(", "length(...) / char_length(...)"),
];

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql628"
  }
  fn default_severity(&self) -> Severity {
    Severity::Error
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, body) = crate::stmt_body(stmt, source);
    let lower = body.to_ascii_lowercase();
    let bytes = body.as_bytes();
    for &(needle, pg) in FNS {
      let mut from = 0usize;
      while let Some(rel) = lower[from..].find(needle) {
        let at = from + rel;
        from = at + needle.len();
        if at > 0 && (bytes[at - 1].is_ascii_alphanumeric() || bytes[at - 1] == b'_') {
          continue;
        }
        let name = needle.trim_end_matches('(');
        out.push(Diagnostic {
          code: "sql628",
          severity: Severity::Error,
          message: format!("`{name}` is a non-PostgreSQL function (Oracle/SQL Server/MySQL) -- use `{pg}`"),
          range: crate::range_at(start + at, start + at + name.len()),
        });
      }
    }
  }
}
