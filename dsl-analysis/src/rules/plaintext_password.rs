//! sql571: `CREATE ROLE app PASSWORD 'hunter2'` -- a plaintext password
//! literal in DDL. The value lands in the server log, `pg_stat_activity`,
//! `.psql_history`, and any migration file in version control. Use `\password`
//! (psql prompts and sends a pre-hashed value) or supply a precomputed
//! SCRAM-SHA-256 verifier instead.

use crate::clause_scan::is_word;
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql571"
  }
  fn default_severity(&self) -> Severity {
    Severity::Warning
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, body, upper) = crate::stmt_body_upper(stmt, source);
    let ub = upper.as_bytes();
    let bytes = body.as_bytes();
    let n = ub.len();

    let mut i = 0usize;
    while i + 8 <= n {
      if &ub[i..i + 8] != b"PASSWORD" || (i > 0 && is_word(ub[i - 1] as char)) || is_word(*ub.get(i + 8).unwrap_or(&b' ') as char) {
        i += 1;
        continue;
      }
      let mut p = i + 8;
      while p < n && bytes[p].is_ascii_whitespace() {
        p += 1;
      }
      if bytes.get(p) != Some(&b'\'') {
        i += 8;
        continue;
      }
      let Some((content, end)) = read_string(bytes, p) else {
        i += 8;
        continue;
      };
      // A precomputed verifier (SCRAM / md5 hash) is already safe-ish; the
      // concern is a human-readable secret.
      let lc = content.to_ascii_lowercase();
      if !lc.starts_with("scram-sha-256$") && !lc.starts_with("md5") {
        out.push(Diagnostic {
          code: "sql571",
          severity: Severity::Warning,
          message: "plaintext password literal -- it leaks into logs / history / VCS; use `\\password` or a precomputed SCRAM verifier".into(),
          range: crate::range_at(start + i, start + end),
        });
      }
      i = end;
    }
  }
}

fn read_string(bytes: &[u8], open: usize) -> Option<(String, usize)> {
  let mut content = String::new();
  let mut i = open + 1;
  while i < bytes.len() {
    if bytes[i] == b'\'' {
      if bytes.get(i + 1) == Some(&b'\'') {
        content.push('\'');
        i += 2;
        continue;
      }
      return Some((content, i + 1));
    }
    content.push(bytes[i] as char);
    i += 1;
  }
  None
}
