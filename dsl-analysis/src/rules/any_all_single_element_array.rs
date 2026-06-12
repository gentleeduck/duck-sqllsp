//! sql521: `col = ANY(ARRAY[1])` / `col <> ALL(ARRAY['x'])` -- the array has
//! a single element, so the quantifier is pointless: `op ANY(ARRAY[v])` and
//! `op ALL(ARRAY[v])` both reduce to `col op v`. Usually a list templated
//! down to one value. Suggests the direct comparison. (Parallels sql515 for
//! the `IN (v)` spelling.)

use crate::clause_scan::is_word;
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql521"
  }
  fn default_severity(&self) -> Severity {
    Severity::Hint
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, body, upper) = crate::stmt_body_upper(stmt, source);
    let ub = upper.as_bytes();
    let bytes = body.as_bytes();
    let n = ub.len();

    let mut i = 0usize;
    while i < n {
      let kw = if ub[i..].starts_with(b"ANY") {
        "ANY"
      } else if ub[i..].starts_with(b"ALL") {
        "ALL"
      } else {
        i += 1;
        continue;
      };
      let kw_len = 3;
      if (i > 0 && is_word(ub[i - 1] as char)) || is_word(*ub.get(i + kw_len).unwrap_or(&b' ') as char) {
        i += 1;
        continue;
      }
      // `( ARRAY [ ... ] )`
      let mut p = skip_ws(bytes, i + kw_len);
      if bytes.get(p) != Some(&b'(') {
        i += kw_len;
        continue;
      }
      let Some(call_close) = match_pair(bytes, p, b'(', b')') else { break };
      p = skip_ws(bytes, p + 1);
      if !ub[p..].starts_with(b"ARRAY") {
        i = call_close + 1;
        continue;
      }
      p = skip_ws(bytes, p + 5);
      if bytes.get(p) != Some(&b'[') {
        i = call_close + 1;
        continue;
      }
      let Some(rbracket) = match_pair(bytes, p, b'[', b']') else {
        i = call_close + 1;
        continue;
      };
      let content = body[p + 1..rbracket].trim();
      if content.is_empty() || has_top_level_comma(content) {
        i = call_close + 1;
        continue;
      }
      out.push(Diagnostic {
        code: "sql521",
        severity: Severity::Hint,
        message: format!("single-element `{kw}(ARRAY[{content}])` -- compare against `{content}` directly"),
        range: crate::range_at(start + i, start + call_close + 1),
      });
      i = call_close + 1;
    }
  }
}

fn skip_ws(bytes: &[u8], mut i: usize) -> usize {
  while i < bytes.len() && bytes[i].is_ascii_whitespace() {
    i += 1;
  }
  i
}

fn has_top_level_comma(s: &str) -> bool {
  let bytes = s.as_bytes();
  let mut depth = 0i32;
  let mut i = 0usize;
  while i < bytes.len() {
    match bytes[i] {
      b'(' | b'[' => depth += 1,
      b')' | b']' => depth -= 1,
      b',' if depth == 0 => return true,
      b'\'' => {
        i += 1;
        while i < bytes.len() && bytes[i] != b'\'' {
          i += 1;
        }
      },
      _ => {},
    }
    i += 1;
  }
  false
}

/// Index of the `close` byte matching the `open` byte at `from`, honoring
/// nesting of the same pair and skipping single-quoted strings.
fn match_pair(bytes: &[u8], from: usize, open: u8, close: u8) -> Option<usize> {
  let mut depth = 0i32;
  let mut i = from;
  while i < bytes.len() {
    let b = bytes[i];
    if b == open {
      depth += 1;
    } else if b == close {
      depth -= 1;
      if depth == 0 {
        return Some(i);
      }
    } else if b == b'\'' {
      i += 1;
      while i < bytes.len() && bytes[i] != b'\'' {
        i += 1;
      }
    }
    i += 1;
  }
  None
}
