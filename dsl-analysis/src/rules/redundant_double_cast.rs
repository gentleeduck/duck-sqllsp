//! sql739: `x::int::int` / `(a || b)::text::text` -- two adjacent casts to the
//! same type. The outer cast is a no-op; drop it. Purely syntactic (unlike
//! sql415 cast_same_type, which compares against the column's catalog type),
//! so it fires regardless of what `x` is.

use crate::clause_scan::is_word;
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql739"
  }
  fn default_severity(&self) -> Severity {
    Severity::Hint
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, _body, upper) = crate::stmt_body_upper(stmt, source);
    let ub = upper.as_bytes();
    let n = ub.len();

    let mut i = 0usize;
    while i + 1 < n {
      if ub[i] == b'\'' {
        i += 1;
        while i < n && ub[i] != b'\'' {
          i += 1;
        }
        i += 1;
        continue;
      }
      if ub[i] != b':' || ub[i + 1] != b':' {
        i += 1;
        continue;
      }
      // First cast: `:: <type>`.
      let t1s = skip_ws(ub, i + 2);
      let Some(t1e) = read_type(ub, t1s) else {
        i += 2;
        continue;
      };
      // Second cast immediately after?
      let c2 = skip_ws(ub, t1e);
      if c2 + 1 < n && ub[c2] == b':' && ub[c2 + 1] == b':' {
        let t2s = skip_ws(ub, c2 + 2);
        if let Some(t2e) = read_type(ub, t2s)
          && upper[t1s..t1e] == upper[t2s..t2e]
        {
          out.push(Diagnostic {
            code: "sql739",
            severity: Severity::Hint,
            message: "redundant double cast to the same type -- drop the outer cast".into(),
            range: crate::range_at(start + c2, start + t2e),
          });
          i = t2e;
          continue;
        }
      }
      i = t1e;
    }
  }
}

/// Read a type name at `i`: word chars, then optional `(...)` modifier, then
/// any number of `[]` array markers. Returns the end offset.
fn read_type(ub: &[u8], i: usize) -> Option<usize> {
  let n = ub.len();
  let s = i;
  let mut j = i;
  while j < n && (is_word(ub[j] as char)) {
    j += 1;
  }
  if j == s {
    return None;
  }
  if j < n && ub[j] == b'(' {
    j = match_paren(ub, j)? + 1;
  }
  loop {
    let k = skip_ws(ub, j);
    if k < n && ub[k] == b'[' {
      let mut m = k + 1;
      while m < n && ub[m] != b']' {
        m += 1;
      }
      if m >= n {
        break;
      }
      j = m + 1;
    } else {
      break;
    }
  }
  Some(j)
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

fn skip_ws(ub: &[u8], mut i: usize) -> usize {
  while i < ub.len() && ub[i].is_ascii_whitespace() {
    i += 1;
  }
  i
}
