//! sql044: `EXIT` / `CONTINUE` used outside a LOOP / WHILE / FOR block.
//!
//! Postgres rejects this with `EXIT cannot be used outside a loop` at
//! parse time on the server. We surface it sooner so the user sees the
//! red squiggle inside the editor.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::{Statement, StatementKind};
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql044"
  }
  fn default_severity(&self) -> Severity {
    Severity::Error
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    if !matches!(stmt.kind, StatementKind::Unknown { .. }) {
      return;
    }
    let start: usize = u32::from(stmt.range.start()) as usize;
    let end: usize = (u32::from(stmt.range.end()) as usize).min(source.len());
    let body = &source[start..end];
    let upper = body.to_ascii_uppercase();
    if !upper.contains("CREATE") || !upper.contains("FUNCTION") {
      return;
    }
    let Some(body_text) = dollar_body(body) else { return };
    let body_upper = body_text.to_ascii_uppercase();
    let stripped = strip_comments(&body_upper);

    // Walk tokens in `stripped`, tracking LOOP / FOR / WHILE depth.
    // Use byte positions so the diagnostic can pinpoint the
    // offending `EXIT`/`CONTINUE` keyword rather than the whole
    // function statement.
    let body_offset = source.find(body_text).unwrap_or(start);
    let bytes = stripped.as_bytes();
    let n = bytes.len();
    let mut depth = 0i32;
    let mut i = 0;
    while i < n {
      if !bytes[i].is_ascii_alphabetic() && bytes[i] != b'_' {
        i += 1;
        continue;
      }
      let s = i;
      while i < n && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_') {
        i += 1;
      }
      let word = &stripped[s..i];
      match word {
        "LOOP" | "FOR" | "WHILE" => depth += 1,
        "EXIT" | "CONTINUE" if depth == 0 => {
          let abs_start = body_offset + s;
          let abs_end = body_offset + i;
          out.push(Diagnostic {
            code: "sql044",
            severity: Severity::Error,
            message: format!("`{word}` used outside a LOOP / FOR / WHILE block"),
            range: text_size::TextRange::new((abs_start as u32).into(), (abs_end as u32).into()),
          });
          return;
        },
        _ => {},
      }
    }
    let _ = depth;
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
  let mut i = 0;
  let n = bytes.len();
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

fn tokenize(s: &str) -> Vec<String> {
  let mut out = Vec::new();
  let bytes = s.as_bytes();
  let n = bytes.len();
  let mut i = 0;
  while i < n {
    let c = bytes[i] as char;
    if c.is_alphabetic() || c == '_' {
      let start = i;
      while i < n && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_') {
        i += 1;
      }
      out.push(s[start..i].to_string());
    } else {
      i += 1;
    }
  }
  out
}
