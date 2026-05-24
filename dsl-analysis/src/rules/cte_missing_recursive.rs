//! sql124: `WITH t AS (SELECT ... FROM t)` self-references `t` but
//! lacks the `RECURSIVE` keyword. PG will refuse to execute it.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql124"
  }
  fn default_severity(&self) -> Severity {
    Severity::Hint
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let start: usize = u32::from(stmt.range.start()) as usize;
    let end: usize = (u32::from(stmt.range.end()) as usize).min(source.len());
    let body = &source[start..end];
    let upper = body.to_ascii_uppercase();
    let bytes = body.as_bytes();
    let n = bytes.len();
    // Need `WITH ` at start (modulo whitespace) and NO `RECURSIVE`
    // immediately after it.
    let trimmed = upper.trim_start();
    if !trimmed.starts_with("WITH ") {
      return;
    }
    let leading_ws = upper.len() - trimmed.len();
    let after_with = &trimmed[5..];
    let after_with_trim = after_with.trim_start();
    if after_with_trim.starts_with("RECURSIVE") {
      return;
    }
    // Find first CTE name: `WITH <name> AS (...)`.
    let mut k = leading_ws + 5;
    while k < n && bytes[k].is_ascii_whitespace() {
      k += 1;
    }
    let name_start = k;
    while k < n && (is_word(bytes[k] as char)) {
      k += 1;
    }
    if k == name_start {
      return;
    }
    let name = &body[name_start..k];
    // Skip ws + `AS` + ws + `(`.
    while k < n && bytes[k].is_ascii_whitespace() {
      k += 1;
    }
    if k + 2 > n || &upper[k..k + 2] != "AS" {
      return;
    }
    k += 2;
    while k < n && bytes[k].is_ascii_whitespace() {
      k += 1;
    }
    if k >= n || bytes[k] != b'(' {
      return;
    }
    let open = k;
    let mut depth = 1i32;
    let mut j = open + 1;
    while j < n && depth > 0 {
      match bytes[j] {
        b'(' => depth += 1,
        b')' => depth -= 1,
        b'\'' => {
          j += 1;
          while j < n && bytes[j] != b'\'' {
            j += 1;
          }
        },
        _ => {},
      }
      if depth == 0 {
        break;
      }
      j += 1;
    }
    if j >= n {
      return;
    }
    let inner = &body[open + 1..j];
    let inner_up = inner.to_ascii_uppercase();
    // Look for `<name>` as a standalone word in the body.
    if !contains_word(&inner_up, &name.to_ascii_uppercase()) {
      return;
    }
    let abs_start = start + leading_ws;
    let abs_end = start + leading_ws + 4;
    out.push(Diagnostic {
      code: "sql124",
      severity: Severity::Hint,
      message: format!(
        "CTE `{name}` self-references inside its body but `WITH` lacks `RECURSIVE` -- PG will reject the query"
      ),
      range: text_size::TextRange::new((abs_start as u32).into(), (abs_end as u32).into()),
    });
  }
}

fn is_word(c: char) -> bool {
  c.is_alphanumeric() || c == '_'
}

fn contains_word(haystack: &str, needle: &str) -> bool {
  let h = haystack.as_bytes();
  let n = h.len();
  let w = needle.len();
  let mut i = 0;
  while i + w <= n {
    if &haystack[i..i + w] == needle {
      let prev_ok = i == 0 || !is_word(h[i - 1] as char);
      let next_ok = i + w == n || !is_word(h[i + w] as char);
      if prev_ok && next_ok {
        return true;
      }
    }
    i += 1;
  }
  false
}
