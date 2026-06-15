//! sql559: `CREATE INDEX idx ON t (a, b, a)` -- the same column (or
//! expression) listed twice in an index. Postgres rejects it with 42701
//! ("column \"a\" specified more than once"). The repeat is dead weight even
//! when accepted; almost always a typo for a different column.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;
use std::collections::HashSet;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql559"
  }
  fn default_severity(&self) -> Severity {
    Severity::Error
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, body, upper) = crate::stmt_body_upper(stmt, source);
    if !upper.contains("CREATE") || !upper.contains("INDEX") {
      return;
    }
    let ub = upper.as_bytes();
    // The index keyword, then its ON, then the first column-list paren.
    let Some(idx) = upper.find("INDEX") else { return };
    let Some(on_rel) = find_word(&ub[idx..], b"ON") else { return };
    let on = idx + on_rel;
    let Some(open) = ub[on..].iter().position(|&b| b == b'(').map(|p| p + on) else { return };
    let Some(close) = match_paren(ub, open) else { return };

    let mut seen: HashSet<String> = HashSet::new();
    for (s, e) in split_top_level(open + 1, close, ub) {
      // Normalise the key (collapse whitespace, lowercase) for comparison.
      let key: String = body[s..e].split_whitespace().collect::<Vec<_>>().join(" ").to_ascii_lowercase();
      if key.is_empty() {
        continue;
      }
      if !seen.insert(key) {
        let raw = &body[s..e];
        let lead = raw.len() - raw.trim_start().len();
        out.push(Diagnostic {
          code: "sql559",
          severity: Severity::Error,
          message: format!("`{}` is listed more than once in the index (PG error 42701)", raw.trim()),
          range: crate::range_at(start + s + lead, start + s + raw.trim_end().len()),
        });
      }
    }
  }
}

fn find_word(ub: &[u8], kw: &[u8]) -> Option<usize> {
  let n = ub.len();
  let m = kw.len();
  let mut i = 0usize;
  while i + m <= n {
    if ub[i..i + m] == *kw
      && (i == 0 || !is_word(ub[i - 1]))
      && (i + m == n || !is_word(ub[i + m]))
    {
      return Some(i);
    }
    i += 1;
  }
  None
}

fn is_word(b: u8) -> bool {
  b.is_ascii_alphanumeric() || b == b'_'
}

fn split_top_level(from: usize, to: usize, ub: &[u8]) -> Vec<(usize, usize)> {
  let mut out = Vec::new();
  let mut depth = 0i32;
  let mut last = from;
  let mut i = from;
  while i < to {
    match ub[i] {
      b'(' | b'[' => depth += 1,
      b')' | b']' => depth -= 1,
      b'\'' => {
        i += 1;
        while i < to && ub[i] != b'\'' {
          i += 1;
        }
      },
      b',' if depth == 0 => {
        out.push((last, i));
        last = i + 1;
      },
      _ => {},
    }
    i += 1;
  }
  out.push((last, to));
  out
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
