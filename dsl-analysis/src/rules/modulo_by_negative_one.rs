//! sql741: `x % -1` / `MOD(x, -1)` -- the remainder of any integer divided by
//! -1 is always 0 (just like dividing by 1), so the expression is a constant.
//! Usually a typo for a real modulus. (Companion to sql677 modulo_by_one.)

use crate::clause_scan::is_word;
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql741"
  }
  fn default_severity(&self) -> Severity {
    Severity::Hint
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, _body, upper) = crate::stmt_body_upper(stmt, source);
    let ub = upper.as_bytes();
    let n = ub.len();

    let mut i = 0usize;
    while i < n {
      match ub[i] {
        b'\'' => {
          i += 1;
          while i < n && ub[i] != b'\'' {
            i += 1;
          }
        },
        // `<expr> % -1`
        b'%' => {
          let m = skip_ws(ub, i + 1);
          if ub.get(m) == Some(&b'-') {
            let d = skip_ws(ub, m + 1);
            if is_one(ub, d) {
              out.push(diag(start + i, start + d + 1));
            }
          }
        },
        // `MOD(x, -1)`
        b'M' if word_at(ub, i, b"MOD") => {
          let p = skip_ws(ub, i + 3);
          if ub.get(p) == Some(&b'(')
            && let Some(close) = match_paren(ub, p)
            && let Some(comma) = top_level_comma(ub, p + 1, close)
          {
            let a = skip_ws(ub, comma + 1);
            if ub.get(a) == Some(&b'-') {
              let d = skip_ws(ub, a + 1);
              if is_one(ub, d) && skip_ws(ub, d + 1) == close {
                out.push(diag(start + a, start + d + 1));
                i = close + 1;
                continue;
              }
            }
          }
        },
        _ => {},
      }
      i += 1;
    }
  }
}

fn diag(s: usize, e: usize) -> Diagnostic {
  Diagnostic {
    code: "sql741",
    severity: Severity::Hint,
    message: "modulo by -1 is always 0 -- check the modulus".into(),
    range: crate::range_at(s, e),
  }
}

fn is_one(ub: &[u8], i: usize) -> bool {
  ub.get(i) == Some(&b'1') && !matches!(ub.get(i + 1), Some(c) if c.is_ascii_digit() || *c == b'.' || *c == b'E')
}

fn top_level_comma(ub: &[u8], from: usize, to: usize) -> Option<usize> {
  let mut depth = 0i32;
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
      b',' if depth == 0 => return Some(i),
      _ => {},
    }
    i += 1;
  }
  None
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
