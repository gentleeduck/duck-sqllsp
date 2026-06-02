//! sql043: `DELETE FROM <tbl>` without WHERE inside a function body.
//!
//! The base `sql013` rule catches DELETE-without-WHERE at top level
//! already. This rule narrows the focus: inside a PL/pgSQL function the
//! mistake is even more likely to wipe the table on every call. Warn.

use crate::{Diagnostic, LintRule, Severity};
use crate::textutil::is_word;
use dsl_catalog::Catalog;
use dsl_parse::{Statement, StatementKind};
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql043"
  }
  fn default_severity(&self) -> Severity {
    Severity::Warning
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    if !matches!(stmt.kind, StatementKind::Unknown { .. }) {
      return;
    }
    let (start, body, upper) = crate::stmt_body_upper(stmt, source);
    if !upper.contains("CREATE") || !upper.contains("FUNCTION") {
      return;
    }
    let Some(body_text) = dollar_body(body) else { return };
    let upper_body = body_text.to_ascii_uppercase();
    let stripped = strip_comments(&upper_body);

    // For each top-level DELETE FROM ... ; check whether the segment
    // from DELETE to the next `;` contains WHERE.
    let bytes = stripped.as_bytes();
    let n = bytes.len();
    let mut i = 0;
    while i + 11 <= n {
      if &stripped[i..i + 11] == "DELETE FROM" {
        let prev_ok = i == 0 || !is_word(bytes[i - 1] as char);
        if prev_ok {
          let mut j = i + 11;
          let mut has_where = false;
          while j < n && bytes[j] != b';' {
            if j + 5 <= n
              && bytes[j..j + 5].eq_ignore_ascii_case(b"WHERE")
              && (j + 5 == n || !is_word(bytes[j + 5] as char))
              && (j == 0 || !is_word(bytes[j - 1] as char))
            {
              has_where = true;
              break;
            }
            j += 1;
          }
          if !has_where {
            let base = source.find(body_text).unwrap_or(start);
            let abs_start = base + i;
            let abs_end = base + (j.min(n));
            out.push(Diagnostic {
              code: "sql043",
              severity: Severity::Warning,
              message: "DELETE without WHERE inside function -- will wipe the whole table on every call".into(),
              range: crate::range_at(abs_start, abs_end),
            });
          }
        }
        i += 11;
      } else {
        i += 1;
      }
    }
  }
}

fn dollar_body(text: &str) -> Option<&str> {
  let start = text.find("$$")?;
  let after = start + 2;
  let end_rel = text[after..].find("$$")?;
  Some(&text[after..after + end_rel])
}

fn strip_comments(s: &str) -> String {
  let mut out = String::with_capacity(s.len());
  let bytes = s.as_bytes();
  let n = bytes.len();
  let mut i = 0;
  while i < n {
    if i + 1 < n && bytes[i] == b'-' && bytes[i + 1] == b'-' {
      while i < n && bytes[i] != b'\n' {
        i += 1;
      }
    } else if i + 1 < n && bytes[i] == b'/' && bytes[i + 1] == b'*' {
      i += 2;
      while i + 1 < n && !(bytes[i] == b'*' && bytes[i + 1] == b'/') {
        i += 1;
      }
      i = (i + 2).min(n);
    } else {
      out.push(bytes[i] as char);
      i += 1;
    }
  }
  out
}

