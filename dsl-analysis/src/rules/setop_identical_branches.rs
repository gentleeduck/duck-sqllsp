//! sql532: `SELECT ... UNION SELECT ...` where two branches of a set
//! operation are textually identical. `UNION` dedups the duplicate away
//! (so it reduces to one branch) and `UNION ALL` repeats every row twice;
//! either way it's almost always a copy-paste slip where one branch should
//! have differed. Also covers INTERSECT / EXCEPT.

use crate::clause_scan::is_word;
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql532"
  }
  fn default_severity(&self) -> Severity {
    Severity::Warning
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, body, upper) = crate::stmt_body_upper(stmt, source);
    let ub = upper.as_bytes();

    let segments = split_setops(ub);
    if segments.len() < 2 {
      return;
    }
    let mut seen: Vec<(String, usize, usize)> = Vec::new();
    for (s, e) in segments {
      let (bs, be) = strip_branch(body, s, e);
      if bs >= be {
        continue;
      }
      let key = body[bs..be].trim().to_string();
      if key.is_empty() {
        continue;
      }
      if seen.iter().any(|(k, _, _)| *k == key) {
        out.push(Diagnostic {
          code: "sql532",
          severity: Severity::Warning,
          message: "duplicate branch in a set operation -- this SELECT is identical to an earlier one".into(),
          range: crate::range_at(start + bs, start + body[..be].trim_end().len()),
        });
      } else {
        seen.push((key, bs, be));
      }
    }
  }
}

/// Split the statement on top-level `UNION` / `INTERSECT` / `EXCEPT` keywords,
/// returning the `(start, end)` byte range of each branch.
fn split_setops(ub: &[u8]) -> Vec<(usize, usize)> {
  let n = ub.len();
  let kws: [&[u8]; 3] = [b"UNION", b"INTERSECT", b"EXCEPT"];
  let mut cuts = Vec::new();
  let mut depth = 0i32;
  let mut i = 0usize;
  while i < n {
    match ub[i] {
      b'(' => depth += 1,
      b')' => depth -= 1,
      b'\'' => {
        i += 1;
        while i < n && ub[i] != b'\'' {
          i += 1;
        }
      },
      _ if depth == 0 => {
        for kw in kws {
          if i + kw.len() <= n
            && ub[i..i + kw.len()] == *kw
            && (i == 0 || !is_word(ub[i - 1] as char))
            && (i + kw.len() == n || !is_word(ub[i + kw.len()] as char))
          {
            cuts.push((i, i + kw.len()));
            i += kw.len();
            break;
          }
        }
      },
      _ => {},
    }
    i += 1;
  }
  if cuts.is_empty() {
    return Vec::new();
  }
  let mut segs = Vec::new();
  let mut prev_end = 0usize;
  for (cs, ce) in &cuts {
    segs.push((prev_end, *cs));
    prev_end = *ce;
  }
  segs.push((prev_end, n));
  segs
}

/// Drop a leading `ALL` / `DISTINCT` (the set-op modifier) and surrounding
/// whitespace / wrapping parens from a branch, returning the tight range.
fn strip_branch(body: &str, mut s: usize, mut e: usize) -> (usize, usize) {
  let bytes = body.as_bytes();
  let trim = |b: &[u8], mut s: usize, mut e: usize| {
    while s < e && b[s].is_ascii_whitespace() {
      s += 1;
    }
    while e > s && b[e - 1].is_ascii_whitespace() {
      e -= 1;
    }
    (s, e)
  };
  (s, e) = trim(bytes, s, e);
  for kw in ["ALL", "DISTINCT"] {
    let k = kw.as_bytes();
    if e - s >= k.len()
      && body[s..s + k.len()].eq_ignore_ascii_case(kw)
      && (s + k.len() == e || bytes[s + k.len()].is_ascii_whitespace())
    {
      s += k.len();
      (s, e) = trim(bytes, s, e);
      break;
    }
  }
  // Unwrap a single pair of parens around the whole branch.
  while s < e && bytes[s] == b'(' && bytes[e - 1] == b')' {
    s += 1;
    e -= 1;
    (s, e) = trim(bytes, s, e);
  }
  (s, e)
}
