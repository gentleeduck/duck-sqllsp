//! sql252: `SELECT * FROM (SELECT ... ORDER BY x) sub` -- the
//! outer SELECT is free to re-order, so the inner ORDER BY is a
//! no-op unless paired with LIMIT/OFFSET/FETCH. The author probably
//! wanted to sort the final result, not the intermediate.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql252"
  }
  fn default_severity(&self) -> Severity {
    Severity::Hint
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, body) = crate::stmt_body(stmt, source);
    let bytes = body.as_bytes();
    let mut i = 0usize;
    while i < bytes.len() {
      if bytes[i] != b'(' {
        i += 1;
        continue;
      }
      let open = i;
      let Some(close) = find_matching_paren(body, open) else { break };
      let inner = &body[open + 1..close];
      let inner_upper = inner.to_ascii_uppercase();
      // Only flag `ORDER BY` at depth 0 *inside* the subquery -- not
      // ORDER BY nested in `WITHIN GROUP (ORDER BY ...)` aggregate
      // syntax or in a window-function `OVER (ORDER BY ...)` frame.
      let has_top_order_by = contains_at_depth_zero(&inner_upper, "ORDER BY");
      let has_top_limit = contains_at_depth_zero(&inner_upper, "LIMIT")
        || contains_at_depth_zero(&inner_upper, "OFFSET")
        || contains_at_depth_zero(&inner_upper, "FETCH");
      // Walk back to find what precedes the `(` -- skip when it's
      // `ARRAY` (constructor preserves order) or aggregate functions
      // like `array_agg`/`jsonb_agg` that consume the inner ordering.
      let prev_word_upper = preceding_word(body, open);
      if matches!(prev_word_upper.as_str(), "ARRAY") {
        i = close + 1;
        continue;
      }
      if inner_upper.trim_start().starts_with("SELECT") && has_top_order_by && !has_top_limit {
        // Outer must wrap further SQL (the subquery is in a context, not a top stmt).
        let prefix_upper = body[..open].to_ascii_uppercase();
        if prefix_upper.contains("FROM ") || prefix_upper.contains("JOIN ") {
          out.push(Diagnostic {
            code: "sql252",
            severity: Severity::Hint,
            message: "ORDER BY in subquery without LIMIT/OFFSET -- outer query may reorder, sort is wasted; move ORDER BY to outer or add LIMIT".into(),
            range: crate::range_at(start + open, start + close + 1),
          });
        }
      }
      i = close + 1;
    }
  }
}

fn contains_at_depth_zero(haystack_upper: &str, needle_upper: &str) -> bool {
  let bytes = haystack_upper.as_bytes();
  let n = bytes.len();
  let nlen = needle_upper.len();
  let mut depth = 0i32;
  let mut i = 0usize;
  while i + nlen <= n {
    match bytes[i] {
      b'(' => {
        depth += 1;
        i += 1;
        continue;
      },
      b')' => {
        depth -= 1;
        i += 1;
        continue;
      },
      b'\'' => {
        i += 1;
        while i < n && bytes[i] != b'\'' {
          i += 1
        }
        if i < n {
          i += 1
        }
        continue;
      },
      _ => {},
    }
    if depth == 0 && haystack_upper[i..i + nlen].eq_ignore_ascii_case(needle_upper) {
      let prev_ok = i == 0 || !(bytes[i - 1].is_ascii_alphanumeric() || bytes[i - 1] == b'_');
      let next_ok = i + nlen == n || !(bytes[i + nlen].is_ascii_alphanumeric() || bytes[i + nlen] == b'_');
      if prev_ok && next_ok {
        return true;
      }
    }
    i += 1;
  }
  false
}

fn preceding_word(body: &str, at: usize) -> String {
  let bytes = body.as_bytes();
  let mut i = at;
  while i > 0 && bytes[i - 1].is_ascii_whitespace() {
    i -= 1
  }
  let word_end = i;
  while i > 0 && (bytes[i - 1].is_ascii_alphanumeric() || bytes[i - 1] == b'_') {
    i -= 1
  }
  body[i..word_end].to_ascii_uppercase()
}

fn find_matching_paren(s: &str, open: usize) -> Option<usize> {
  let bytes = s.as_bytes();
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
