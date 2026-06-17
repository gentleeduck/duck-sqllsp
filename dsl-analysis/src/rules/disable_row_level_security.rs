//! sql574: `ALTER TABLE t DISABLE ROW LEVEL SECURITY` -- turns off RLS
//! enforcement on the table, so every policy stops applying and all rows
//! become visible/writable to anyone with table privileges. Sometimes
//! intentional (maintenance), but it's a security-relevant change worth a
//! second look -- often a leftover from debugging.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql574"
  }
  fn default_severity(&self) -> Severity {
    Severity::Warning
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, _body, upper) = crate::stmt_body_upper(stmt, source);
    if let Some(at) = find_disable_rls(upper.as_bytes()) {
      out.push(Diagnostic {
        code: "sql574",
        severity: Severity::Warning,
        message: "DISABLE ROW LEVEL SECURITY stops all RLS policies on this table from applying -- confirm that's intended".into(),
        range: crate::range_at(start + at, start + (at + 7)),
      });
    }
  }
}

/// Offset of a `DISABLE` that is followed (whitespace-flexible) by
/// `ROW LEVEL SECURITY`.
fn find_disable_rls(ub: &[u8]) -> Option<usize> {
  let n = ub.len();
  let mut i = 0usize;
  while i + 7 <= n {
    if &ub[i..i + 7] == b"DISABLE" {
      let mut p = i + 7;
      let mut ok = true;
      for word in ["ROW", "LEVEL", "SECURITY"] {
        while p < n && ub[p].is_ascii_whitespace() {
          p += 1;
        }
        let w = word.as_bytes();
        if p + w.len() <= n && &ub[p..p + w.len()] == w {
          p += w.len();
        } else {
          ok = false;
          break;
        }
      }
      if ok {
        return Some(i);
      }
    }
    i += 1;
  }
  None
}
