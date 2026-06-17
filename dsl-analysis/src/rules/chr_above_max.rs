//! sql730: `chr(2000000)` / `chr(-1)` -- a code point outside the valid range.
//! In a UTF-8 database `chr(n)` needs n in 1..=1114111 (0x10FFFF); PostgreSQL
//! raises 54000 ("requested character too large") otherwise. Usually a bad
//! constant or a value that should have been masked. (sql698 chr_zero covers
//! the n = 0 case.)

use crate::clause_scan::is_word;
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql730"
  }
  fn default_severity(&self) -> Severity {
    Severity::Warning
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, _body, upper) = crate::stmt_body_upper(stmt, source);
    let ub = upper.as_bytes();
    let n = ub.len();

    let mut i = 0usize;
    while i < n {
      if !word_at(ub, i, b"CHR") {
        i += 1;
        continue;
      }
      let p = skip_ws(ub, i + 3);
      if ub.get(p) != Some(&b'(') {
        i += 3;
        continue;
      }
      let Some(close) = match_paren(ub, p) else { break };
      if let Some((s, e)) = trim_range(ub, p + 1, close)
        && upper[s..e].parse::<i64>().is_ok_and(|v| !(0..=1_114_111).contains(&v))
      {
        out.push(Diagnostic {
          code: "sql730",
          severity: Severity::Warning,
          message: "chr() code point is out of range (1..1114111) -- raises an error at runtime (PG 54000)".into(),
          range: crate::range_at(start + s, start + e),
        });
      }
      i = close + 1;
    }
  }
}

fn trim_range(ub: &[u8], mut s: usize, mut e: usize) -> Option<(usize, usize)> {
  while s < e && ub[s].is_ascii_whitespace() {
    s += 1;
  }
  while e > s && ub[e - 1].is_ascii_whitespace() {
    e -= 1;
  }
  (s < e).then_some((s, e))
}

fn match_paren(ub: &[u8], open: usize) -> Option<usize> {
  let mut depth = 0i32;
  let mut i = open;
  while i < ub.len() {
    match ub[i] {
      b'(' => depth += 1,
      b')' => {
        depth -= 1;
        if depth == 0 {
          return Some(i);
        }
      },
      b'\'' => {
        i += 1;
        while i < ub.len() && ub[i] != b'\'' {
          i += 1
        }
      },
      _ => {},
    }
    i += 1;
  }
  None
}

fn word_at(ub: &[u8], i: usize, w: &[u8]) -> bool {
  i + w.len() <= ub.len()
    && &ub[i..i + w.len()] == w
    && (i == 0 || !is_word(ub[i - 1] as char))
    && (i + w.len() == ub.len() || !is_word(ub[i + w.len()] as char))
}

fn skip_ws(ub: &[u8], mut i: usize) -> usize {
  while i < ub.len() && ub[i].is_ascii_whitespace() {
    i += 1;
  }
  i
}
