//! sql520: `WHERE lower(col) = 'ABC'` / `WHERE upper(col) LIKE 'abc%'` -- a
//! case-folding function compared against a string literal of the opposite
//! case. `lower(...)` only ever returns lowercase, so it can never equal a
//! literal containing an uppercase ASCII letter (and vice-versa for
//! `upper(...)`). The predicate is dead: it matches zero rows. Almost always
//! a bug -- the literal should have been written in the folded case.

use crate::clause_scan::is_word;
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql520"
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
    while i < n {
      let want_lower = ub[i..].starts_with(b"LOWER");
      let want_upper = ub[i..].starts_with(b"UPPER");
      if !(want_lower || want_upper) || (i > 0 && is_word(ub[i - 1] as char)) {
        i += 1;
        continue;
      }
      // Must be a function call: `LOWER (` immediately after the name.
      let mut p = i + 5;
      while p < n && bytes[p].is_ascii_whitespace() {
        p += 1;
      }
      if bytes.get(p) != Some(&b'(') {
        i += 5;
        continue;
      }
      let Some(close) = match_paren(bytes, p) else { break };
      // Operator after the call: `=` or `LIKE`.
      let mut q = close + 1;
      while q < n && bytes[q].is_ascii_whitespace() {
        q += 1;
      }
      let lit_at = if bytes.get(q) == Some(&b'=') && bytes.get(q + 1) != Some(&b'=') {
        skip_ws(bytes, q + 1)
      } else if ub[q..].starts_with(b"LIKE") && q + 4 <= n && !is_word(*ub.get(q + 4).unwrap_or(&b' ') as char) {
        skip_ws(bytes, q + 4)
      } else {
        i = close + 1;
        continue;
      };
      // The compared operand must be a string literal.
      if bytes.get(lit_at) != Some(&b'\'') {
        i = close + 1;
        continue;
      }
      let Some((content, lit_end)) = read_string(bytes, lit_at) else {
        i = close + 1;
        continue;
      };
      let func = if want_lower { "lower" } else { "upper" };
      let impossible = if want_lower {
        content.bytes().any(|b| b.is_ascii_uppercase())
      } else {
        content.bytes().any(|b| b.is_ascii_lowercase())
      };
      if impossible {
        let opposite = if want_lower { "uppercase" } else { "lowercase" };
        out.push(Diagnostic {
          code: "sql520",
          severity: Severity::Warning,
          message: format!(
            "`{func}(...)` compared to `'{content}'`, which has {opposite} letters -- \
             `{func}()` never returns that, so this matches zero rows"
          ),
          range: crate::range_at(start + i, start + lit_end),
        });
      }
      i = lit_end;
    }
  }
}

fn skip_ws(bytes: &[u8], mut i: usize) -> usize {
  while i < bytes.len() && bytes[i].is_ascii_whitespace() {
    i += 1;
  }
  i
}

/// Read a single-quoted string starting at `open` (a `'`). Returns the
/// content (with `''` collapsed to `'`) and the index just past the closing
/// quote. None if unterminated.
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
