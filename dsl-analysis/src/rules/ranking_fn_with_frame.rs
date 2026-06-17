//! sql674: a ranking window function with an explicit frame clause, e.g.
//! `ROW_NUMBER() OVER (ORDER BY x ROWS BETWEEN ...)`. ROW_NUMBER, RANK,
//! DENSE_RANK, PERCENT_RANK, CUME_DIST and NTILE assign a value from the
//! whole partition and ignore the frame -- PG rejects the frame outright
//! ("window function ... cannot have a window frame"). Drop the ROWS / RANGE
//! / GROUPS clause; plain `OVER (PARTITION BY ... ORDER BY ...)` is enough.

use crate::clause_scan::is_word;
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

const RANKING_FNS: &[&[u8]] = &[b"ROW_NUMBER", b"RANK", b"DENSE_RANK", b"PERCENT_RANK", b"CUME_DIST", b"NTILE"];

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql674"
  }
  fn default_severity(&self) -> Severity {
    Severity::Error
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, _body, upper) = crate::stmt_body_upper(stmt, source);
    let ub = upper.as_bytes();
    let n = ub.len();

    let mut i = 0usize;
    while i < n {
      let Some(len) = RANKING_FNS.iter().copied().find(|f| word_at(ub, i, f)).map(<[u8]>::len) else {
        i += 1;
        continue;
      };
      // <fn> ( args ) OVER ( ... frame ... )
      let p = skip_ws(ub, i + len);
      if ub.get(p) != Some(&b'(') {
        i += len;
        continue;
      }
      let Some(close1) = match_paren(ub, p) else { break };
      let q = skip_ws(ub, close1 + 1);
      if !word_at(ub, q, b"OVER") {
        i = close1 + 1;
        continue;
      }
      let r = skip_ws(ub, q + 4);
      if ub.get(r) != Some(&b'(') {
        i = close1 + 1;
        continue;
      }
      let Some(close2) = match_paren(ub, r) else { break };
      if let Some(kw) = frame_keyword(ub, r + 1, close2) {
        out.push(Diagnostic {
          code: "sql674",
          severity: Severity::Error,
          message: "ranking window function cannot have a frame clause -- drop the ROWS/RANGE/GROUPS frame".into(),
          range: crate::range_at(start + kw.0, start + kw.1),
        });
      }
      i = close2 + 1;
    }
  }
}

/// Position of a top-level ROWS / RANGE / GROUPS frame keyword inside the
/// OVER parens (`from`..`to`), skipping nested parens and string literals.
fn frame_keyword(ub: &[u8], from: usize, to: usize) -> Option<(usize, usize)> {
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
      _ if depth == 0 => {
        for kw in [&b"ROWS"[..], &b"RANGE"[..], &b"GROUPS"[..]] {
          if word_at(ub, i, kw) {
            return Some((i, i + kw.len()));
          }
        }
      },
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
