//! sql068: BEGIN / COMMIT pair wrapping a single statement -- the
//! transaction adds nothing. Each statement already runs in its own
//! implicit transaction. Hint.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql068"
  }
  fn default_severity(&self) -> Severity {
    Severity::Hint
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    // Fire only when this stmt is the FIRST statement in the source
    // -- avoids emitting the same diagnostic for every nested
    // statement inside the txn block.
    let start: usize = u32::from(stmt.range.start()) as usize;
    let prefix = &source[..start];
    if !prefix.trim().is_empty() {
      return;
    }

    let upper = strip_quoted_and_comments(&source.to_ascii_uppercase());
    let bytes = upper.as_bytes();
    let n = bytes.len();

    // Find leading BEGIN.
    let mut i = 0;
    while i < n && (bytes[i] as char).is_ascii_whitespace() {
      i += 1;
    }
    if i + 5 > n || &upper[i..i + 5] != "BEGIN" {
      return;
    }
    let next_ok = i + 5 == n || !is_word(bytes[i + 5] as char);
    if !next_ok {
      return;
    }
    i += 5;

    // Walk to next COMMIT/END/ROLLBACK at depth 0, counting `;`.
    let mut depth = 0i32;
    let mut count = 0usize;
    while i < n {
      let c = bytes[i];
      match c {
        b'(' => depth += 1,
        b')' => depth -= 1,
        b'\'' => {
          i += 1;
          while i < n && bytes[i] != b'\'' {
            i += 1;
          }
        },
        b';' if depth == 0 => count += 1,
        _ => {},
      }
      if depth == 0 && c.is_ascii_alphabetic() {
        let s = i;
        while i < n && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_') {
          i += 1;
        }
        let word = &upper[s..i];
        if word == "COMMIT" || word == "END" || word == "ROLLBACK" {
          if count == 2 {
            // Narrow to the leading `BEGIN` keyword.
            let upper_src = source.to_ascii_uppercase();
            let begin_off = upper_src.find("BEGIN").unwrap_or(0);
            out.push(Diagnostic {
              code: "sql068",
              severity: Severity::Hint,
              message: "transaction wraps a single statement -- per-statement implicit txn already handles atomicity"
                .into(),
              range: text_size::TextRange::new((begin_off as u32).into(), ((begin_off + 5) as u32).into()),
            });
          }
          return;
        }
        continue;
      }
      i += 1;
    }
  }
}

fn strip_quoted_and_comments(s: &str) -> String {
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
    } else if bytes[i] == b'\'' {
      i += 1;
      while i < n && bytes[i] != b'\'' {
        i += 1;
      }
      if i < n {
        i += 1;
      }
    } else {
      out.push(bytes[i] as char);
      i += 1;
    }
  }
  out
}

fn is_word(c: char) -> bool {
  c.is_alphanumeric() || c == '_'
}
