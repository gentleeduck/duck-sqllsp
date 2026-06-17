//! sql588: `ALTER TABLE t ADD PRIMARY KEY (...)` / `ADD UNIQUE (...)` -- adding
//! a primary-key or unique constraint builds its backing index while holding
//! an ACCESS EXCLUSIVE lock, blocking writes (and the build itself) for the
//! whole duration. On a large table, build the index off-lock first --
//! `CREATE UNIQUE INDEX CONCURRENTLY ...` -- then `ADD CONSTRAINT ... USING
//! INDEX ...`. (Skipped when it already attaches a prebuilt index.)

use crate::clause_scan::is_word;
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql588"
  }
  fn default_severity(&self) -> Severity {
    Severity::Warning
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, _body, upper) = crate::stmt_body_upper(stmt, source);
    if !upper.contains("ALTER TABLE") || !upper.contains("ADD") || upper.contains("USING INDEX") {
      return;
    }
    let ub = upper.as_bytes();
    // `PRIMARY KEY (` or `UNIQUE (` -- the inline form that builds an index.
    for kw in [&b"PRIMARY KEY"[..], &b"UNIQUE"[..]] {
      if let Some(at) = key_with_paren(ub, kw) {
        out.push(Diagnostic {
          code: "sql588",
          severity: Severity::Warning,
          message: "ALTER ... ADD PRIMARY KEY / UNIQUE builds the index under an ACCESS EXCLUSIVE lock -- build it with CREATE UNIQUE INDEX CONCURRENTLY, then ADD CONSTRAINT ... USING INDEX".into(),
          range: crate::range_at(start + at, start + at + kw.len()),
        });
        return;
      }
    }
  }
}

/// Offset of a word-bounded `kw` immediately followed (after whitespace) by `(`.
fn key_with_paren(ub: &[u8], kw: &[u8]) -> Option<usize> {
  let n = ub.len();
  let mut i = 0usize;
  while i + kw.len() <= n {
    if ub[i..i + kw.len()] == *kw
      && (i == 0 || !is_word(ub[i - 1] as char))
      && (i + kw.len() == n || !is_word(ub[i + kw.len()] as char))
    {
      let mut p = i + kw.len();
      while p < n && ub[p].is_ascii_whitespace() {
        p += 1;
      }
      if ub.get(p) == Some(&b'(') {
        return Some(i);
      }
    }
    i += 1;
  }
  None
}
