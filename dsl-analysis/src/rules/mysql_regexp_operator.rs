//! sql597: `col REGEXP 'pat'` / `col RLIKE 'pat'` -- MySQL's regex-match
//! operators. PostgreSQL doesn't have them; use the POSIX regex operators
//! `~` (case-sensitive), `~*` (case-insensitive), and `!~` / `!~*` for the
//! negated forms. Word-bounded so `regexp_match` / `regexp_replace` (real PG
//! functions) aren't touched.

use crate::clause_scan::is_word;
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql597"
  }
  fn default_severity(&self) -> Severity {
    Severity::Error
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, _body, upper) = crate::stmt_body_upper(stmt, source);
    let ub = upper.as_bytes();
    let n = ub.len();
    for (kw, len) in [(&b"REGEXP"[..], 6usize), (&b"RLIKE"[..], 5usize)] {
      let mut i = 0usize;
      while i + len <= n {
        if ub[i..i + len] == *kw
          && (i == 0 || !is_word(ub[i - 1] as char))
          && ub.get(i + len).is_none_or(|&b| !is_word(b as char))
        {
          let name = std::str::from_utf8(kw).unwrap();
          out.push(Diagnostic {
            code: "sql597",
            severity: Severity::Error,
            message: format!("`{name}` is a MySQL operator -- PostgreSQL uses `~` (case-sensitive) or `~*` (case-insensitive)"),
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
