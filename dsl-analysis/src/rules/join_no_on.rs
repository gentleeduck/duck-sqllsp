//! sql064: `... JOIN tbl` not followed by `ON`/`USING` and not
//! preceded by `CROSS` / `NATURAL`. The pg_query backend can't reliably
//! distinguish CROSS-JOIN from a missing ON, so this rule uses text
//! analysis.

use crate::{Diagnostic, LintRule, Severity};
use crate::textutil::is_word;
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql064"
  }
  fn default_severity(&self) -> Severity {
    Severity::Error
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, body) = crate::stmt_body(stmt, source);
    let stripped = strip_quoted_and_comments(body);
    let upper = stripped.to_ascii_uppercase();
    let bytes = upper.as_bytes();
    let n = bytes.len();

    let mut i = 0;
    while i + 4 <= n {
      if &upper[i..i + 4] == "JOIN"
        && (i == 0 || !is_word(bytes[i - 1] as char))
        && (i + 4 == n || !is_word(bytes[i + 4] as char))
      {
        let mut p = i;
        while p > 0 && (bytes[p - 1] as char).is_whitespace() {
          p -= 1;
        }
        let end_w = p;
        while p > 0 && is_word(bytes[p - 1] as char) {
          p -= 1;
        }
        let preceding = &upper[p..end_w];
        if preceding == "CROSS" || preceding == "NATURAL" {
          i += 4;
          continue;
        }
        let mut j = i + 4;
        let mut has_on = false;
        let mut depth = 0i32;
        while j < n {
          let c = bytes[j];
          match c {
            b'(' => depth += 1,
            b')' => depth -= 1,
            b';' if depth == 0 => break,
            _ => {},
          }
          if depth == 0 && c.is_ascii_alphabetic() {
            let start = j;
            while j < n && (bytes[j].is_ascii_alphanumeric() || bytes[j] == b'_') {
              j += 1;
            }
            let word = &upper[start..j];
            if word == "ON" || word == "USING" {
              has_on = true;
              break;
            }
            if matches!(
              word,
              "JOIN"
                | "WHERE"
                | "GROUP"
                | "ORDER"
                | "LIMIT"
                | "OFFSET"
                | "UNION"
                | "EXCEPT"
                | "INTERSECT"
                | "RETURNING"
                | "HAVING"
                | "WINDOW"
            ) {
              break;
            }
            continue;
          }
          j += 1;
        }
        if !has_on {
          let abs_start = start + i;
          let abs_end = start + i + 4;
          out.push(Diagnostic {
            code: "sql064",
            severity: Severity::Error,
            message: "JOIN without ON / USING -- add `ON a.col = b.col` or use CROSS JOIN".into(),
            range: crate::range_at(abs_start, abs_end),
          });
          return;
        }
      }
      i += 1;
    }
  }
}

/// Strips comments + quoted-string contents but preserves byte offsets
/// by emitting spaces in place of the skipped bytes. Lets the caller
/// translate positions in `out` back to positions in `s` 1:1.
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
      // Preserve length on non-ASCII bytes too.
      out.push(' ');
      i += 1;
    }
  }
  out
}

