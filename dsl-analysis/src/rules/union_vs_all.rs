//! sql056: `UNION` (deduplicates) is often slower than `UNION ALL` and
//! used by mistake.
//!
//! Hint: when the author wrote `UNION` without explicit `DISTINCT`
//! reasoning, suggest considering `UNION ALL` for cases where duplicate
//! rows are impossible (different tables, disjoint predicates, etc.).
//! We can't fully prove disjointness, so this is a soft Hint that
//! reminds the author to think about it.

use crate::{Diagnostic, LintRule, Severity};
use crate::textutil::is_word;
use dsl_catalog::Catalog;
use dsl_parse::{Statement, StatementKind};
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql056"
  }
  fn default_severity(&self) -> Severity {
    Severity::Hint
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    if !matches!(stmt.kind, StatementKind::Select(_)) {
      return;
    }
    let (start, body) = crate::stmt_body(stmt, source);
    let stripped = strip_quoted_and_comments(body);
    let upper = stripped.to_ascii_uppercase();
    // Find UNION not followed by ALL / DISTINCT.
    let bytes = upper.as_bytes();
    let n = bytes.len();
    let mut i = 0;
    while i + 5 <= n {
      if &upper[i..i + 5] == "UNION" {
        let prev_ok = i == 0 || !is_word(bytes[i - 1] as char);
        let next_ok = i + 5 == n || !is_word(bytes[i + 5] as char);
        if prev_ok && next_ok {
          // Look at the next non-whitespace token.
          let mut j = i + 5;
          while j < n && bytes[j].is_ascii_whitespace() {
            j += 1;
          }
          let upper_rest = &upper[j..];
          if upper_rest.starts_with("ALL") || upper_rest.starts_with("DISTINCT") {
            i = j;
            continue;
          }
          let abs_start = start + i;
          let abs_end = start + i + 5;
          out.push(Diagnostic {
            code: "sql056",
            severity: Severity::Hint,
            message:
              "plain `UNION` deduplicates; use `UNION ALL` when duplicates are impossible (faster, clearer intent)"
                .into(),
            range: crate::range_at(abs_start, abs_end),
          });
          return;
        }
      }
      i += 1;
    }
  }
}

/// Space-preserving strip: emit spaces in place of stripped bytes so
/// indices in the output map 1:1 to the input.
fn strip_quoted_and_comments(s: &str) -> String {
  let mut out = String::with_capacity(s.len());
  let bytes = s.as_bytes();
  let n = bytes.len();
  let mut i = 0;
  while i < n {
    if i + 1 < n && bytes[i] == b'-' && bytes[i + 1] == b'-' {
      while i < n && bytes[i] != b'\n' {
        out.push(' ');
        i += 1;
      }
    } else if i + 1 < n && bytes[i] == b'/' && bytes[i + 1] == b'*' {
      out.push(' ');
      out.push(' ');
      i += 2;
      while i + 1 < n && !(bytes[i] == b'*' && bytes[i + 1] == b'/') {
        out.push(' ');
        i += 1;
      }
      if i + 1 < n {
        out.push(' ');
        out.push(' ');
        i += 2;
      } else {
        while i < n {
          out.push(' ');
          i += 1;
        }
      }
    } else if bytes[i] == b'\'' {
      out.push(' ');
      i += 1;
      while i < n && bytes[i] != b'\'' {
        out.push(' ');
        i += 1;
      }
      if i < n {
        out.push(' ');
        i += 1;
      }
    } else if bytes[i].is_ascii() {
      out.push(bytes[i] as char);
      i += 1;
    } else {
      out.push(' ');
      i += 1;
    }
  }
  out
}

