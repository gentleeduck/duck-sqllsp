//! sql604: `CLOB` / `NCLOB` -- Oracle's large-character-object types.
//! PostgreSQL has no length-limited character LOBs; `text` holds strings of any
//! size. (Oracle `BLOB` -> PG `bytea` is handled with the MySQL BLOB lint.)
//! Word-bounded so identifiers like `nclob_data` aren't matched.

use crate::clause_scan::is_word;
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql604"
  }
  fn default_severity(&self) -> Severity {
    Severity::Error
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, _body, upper) = crate::stmt_body_upper(stmt, source);
    let ub = upper.as_bytes();
    let n = ub.len();
    for kw in [&b"NCLOB"[..], &b"CLOB"[..]] {
      let len = kw.len();
      let mut i = 0usize;
      while i + len <= n {
        if &ub[i..i + len] == kw
          && (i == 0 || !is_word(ub[i - 1] as char))
          && (i + len == n || !is_word(ub[i + len] as char))
        {
          let name = std::str::from_utf8(kw).unwrap();
          out.push(Diagnostic {
            code: "sql604",
            severity: Severity::Error,
            message: format!("`{name}` is an Oracle LOB type -- PostgreSQL uses `text`"),
            range: crate::range_at(start + i, start + i + len),
          });
          i += len;
          continue;
        }
        i += 1;
      }
    }
  }
}
