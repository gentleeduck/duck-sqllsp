//! sql733: a string literal containing `password=...` -- a hardcoded
//! credential in a connection string (`dblink(...)`, `postgres_fdw` /
//! `CREATE SERVER ... OPTIONS`, `CREATE USER MAPPING`). The secret lands in
//! the server log, `pg_stat_activity`, and version control. Move it to a
//! `.pgpass` file, a user mapping created out of band, or a secret store.
//! (Companion to sql571 plaintext_password for role DDL.)

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql733"
  }
  fn default_severity(&self) -> Severity {
    Severity::Warning
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, body) = crate::stmt_body(stmt, source);
    let b = body.as_bytes();
    let n = b.len();

    let mut i = 0usize;
    while i < n {
      if b[i] != b'\'' {
        i += 1;
        continue;
      }
      let open = i;
      let mut j = i + 1;
      while j < n && b[j] != b'\'' {
        j += 1;
      }
      let content = body[open + 1..j.min(n)].to_ascii_lowercase();
      if has_password_value(&content) {
        out.push(Diagnostic {
          code: "sql733",
          severity: Severity::Warning,
          message: "hardcoded password in a connection string -- move the credential out of the SQL".into(),
          range: crate::range_at(start + open, start + (j + 1).min(n)),
        });
      }
      i = j + 1;
    }
  }
}

/// True when the (lowercased) string contains `password=` (or `pwd=`)
/// immediately followed by a real value (not empty / not just whitespace).
fn has_password_value(s: &str) -> bool {
  for key in ["password=", "pwd="] {
    if let Some(p) = s.find(key) {
      let rest = &s.as_bytes()[p + key.len()..];
      if rest.first().is_some_and(|&c| !c.is_ascii_whitespace()) {
        return true;
      }
    }
  }
  false
}
