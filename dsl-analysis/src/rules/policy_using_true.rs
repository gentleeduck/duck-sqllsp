//! sql575: `CREATE POLICY p ON t USING (true)` (or `WITH CHECK (true)`) -- a
//! row-level-security policy whose qualifier is trivially true grants access
//! to every row, which defeats the point of enabling RLS. Usually a
//! placeholder that was never filled in with a real ownership/tenant check.

use crate::clause_scan::is_word;
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql575"
  }
  fn default_severity(&self) -> Severity {
    Severity::Warning
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, body, upper) = crate::stmt_body_upper(stmt, source);
    if !upper.contains("POLICY") {
      return;
    }
    let ub = upper.as_bytes();
    let n = ub.len();
    for kw in [&b"USING"[..], &b"CHECK"[..]] {
      let mut i = 0usize;
      while i + kw.len() <= n {
        if ub[i..i + kw.len()] != *kw || (i > 0 && is_word(ub[i - 1] as char)) {
          i += 1;
          continue;
        }
        let mut p = i + kw.len();
        while p < n && ub[p].is_ascii_whitespace() {
          p += 1;
        }
        if ub.get(p) == Some(&b'(')
          && let Some(close) = match_paren(ub, p)
        {
          if is_tautology(body[p + 1..close].trim()) {
            out.push(Diagnostic {
              code: "sql575",
              severity: Severity::Warning,
              message: "RLS policy qualifier is always true -- it allows every row, defeating row-level security".into(),
              range: crate::range_at(start + i, start + close + 1),
            });
          }
          i = close + 1;
          continue;
        }
        i += kw.len();
      }
    }
  }
}

fn is_tautology(s: &str) -> bool {
  let t: String = s.chars().filter(|c| !c.is_whitespace()).collect::<String>().to_ascii_lowercase();
  matches!(t.as_str(), "true" | "1=1" | "1" | "(true)")
}

fn match_paren(bytes: &[u8], open: usize) -> Option<usize> {
  let mut depth = 0i32;
  let mut i = open;
  while i < bytes.len() {
    match bytes[i] {
      b'(' => depth += 1,
      b')' => {
        depth -= 1;
        if depth == 0 {
          return Some(i);
        }
      },
      b'\'' => {
        i += 1;
        while i < bytes.len() && bytes[i] != b'\'' {
          i += 1
        }
      },
      _ => {},
    }
    i += 1;
  }
  None
}
