//! sql032: bare `RETURN;` inside a function that declares a non-void
//! return type.
//!
//! Postgres requires `RETURN <expr>;` whenever the function isn't
//! `RETURNS void`. A bare `RETURN;` is only legal in OUT-parameter
//! procedures or void functions; everywhere else it's a runtime trap.

use crate::{Diagnostic, LintRule, Severity};
use crate::textutil::is_word;
use dsl_catalog::Catalog;
use dsl_parse::{Statement, StatementKind};
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql032"
  }
  fn default_severity(&self) -> Severity {
    Severity::Error
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    if !matches!(stmt.kind, StatementKind::Unknown { .. }) {
      return;
    }
    let (start, body, upper) = crate::stmt_body_upper(stmt, source);

    if !upper.contains("CREATE") || !upper.contains("FUNCTION") {
      return;
    }
    // RETURNS void -> bare RETURN is fine.
    if upper.contains("RETURNS VOID") {
      return;
    }
    // No `RETURNS` clause at all -> not a function header we model.
    if !upper.contains("RETURNS ") {
      return;
    }

    let Some(body_text) = dollar_body(body) else { return };
    let body_offset = source.find(body_text).unwrap_or(start);
    let stripped = strip_comments(&body_text.to_ascii_uppercase());

    // Find `RETURN` followed only by `;` (whitespace allowed).
    let bytes = stripped.as_bytes();
    let n = bytes.len();
    let needle = "RETURN";
    let mut i = 0;
    while i + needle.len() <= n {
      if &stripped[i..i + needle.len()] == needle {
        let prev_ok = i == 0 || !is_word(bytes[i - 1] as char);
        let after = i + needle.len();
        let next_ok = after == n || !is_word(bytes[after] as char);
        if prev_ok && next_ok {
          // Look for next non-ws char; if it's ';', flag.
          let mut j = after;
          while j < n && bytes[j].is_ascii_whitespace() {
            j += 1;
          }
          if j < n && bytes[j] == b';' {
            let abs_start = body_offset + i;
            let abs_end = body_offset + j + 1;
            out.push(Diagnostic {
              code: "sql032",
              severity: Severity::Error,
              message: "bare `RETURN;` in non-void function -- supply a return value".into(),
              range: crate::range_at(abs_start, abs_end),
            });
            return;
          }
        }
      }
      i += 1;
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

