//! sql514: empty expression parentheses where an expression is required.
//!
//! Catches the post-refactor pattern where a `WHEN`, `IF`, `WHERE`,
//! `NOT`, `AND`, `OR`, `IN`, `ANY`, `ALL` etc. ends up with an empty
//! `()` group -- almost always a typo or half-deleted condition.
//!
//!   IF NOT () THEN ...         -- meant `IF NOT (cond)`
//!   CASE WHEN () THEN ...      -- empty WHEN clause
//!   WHERE id IN ()             -- PG rejects empty IN list at runtime
//!   foo BETWEEN () AND ()      -- empty BETWEEN bound
//!
//! Text-scanned (cheap + works regardless of AST shape). Skips bodies
//! of string literals + comments via strip_comments_strings.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql514"
  }
  fn default_severity(&self) -> Severity {
    Severity::Error
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, raw) = crate::stmt_body(stmt, source);
    let body_owned = crate::textutil::strip_comments_strings(raw);
    let body = body_owned.as_str();
    let bytes = body.as_bytes();
    let n = bytes.len();
    // Walk every `(` and check whether the matching `)` immediately
    // follows (only whitespace between them) AND the preceding token
    // is one of the keywords that demand an expression inside.
    let mut i = 0usize;
    while i < n {
      if bytes[i] != b'(' {
        i += 1;
        continue;
      }
      // Find matching `)`.
      let mut depth = 1i32;
      let mut j = i + 1;
      while j < n {
        match bytes[j] {
          b'(' => depth += 1,
          b')' => {
            depth -= 1;
            if depth == 0 {
              break;
            }
          },
          _ => {},
        }
        j += 1;
      }
      if j >= n {
        break;
      }
      // Is the interior empty (only whitespace)?
      let inner = &body[i + 1..j];
      if !inner.trim().is_empty() {
        i = j + 1;
        continue;
      }
      // Look back for the preceding keyword (skip whitespace).
      let prev_token = preceding_token(body, i);
      let kw = prev_token.to_ascii_uppercase();
      let demanded = matches!(
        kw.as_str(),
        "WHEN"
          | "IF"
          | "WHERE"
          | "NOT"
          | "AND"
          | "OR"
          | "HAVING"
          | "IN"
          | "ANY"
          | "ALL"
          | "EXISTS"
          | "USING"
          | "BETWEEN"
      );
      if demanded {
        let msg = format!(
          "empty `()` after `{kw}` -- expression required (likely a typo or half-deleted condition)"
        );
        out.push(Diagnostic {
          code: "sql514",
          severity: Severity::Error,
          message: msg,
          range: crate::range_at(start + i, start + j + 1),
        });
      }
      i = j + 1;
    }
  }
}

/// Return the previous non-whitespace word ending right before `paren_at`.
/// Skips over any preceding punctuation (e.g. `=`, `,`, `(`) and stops
/// at the first word boundary.
fn preceding_token(body: &str, paren_at: usize) -> String {
  let bytes = body.as_bytes();
  let mut k = paren_at;
  // Skip whitespace.
  while k > 0 && bytes[k - 1].is_ascii_whitespace() {
    k -= 1;
  }
  let end = k;
  while k > 0 && is_word_char(bytes[k - 1]) {
    k -= 1;
  }
  body[k..end].to_string()
}

fn is_word_char(b: u8) -> bool {
  b.is_ascii_alphanumeric() || b == b'_'
}
