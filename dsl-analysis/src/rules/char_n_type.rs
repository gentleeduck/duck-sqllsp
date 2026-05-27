//! sql104: `CHAR(n)` / `CHARACTER(n)` -- fixed-width type that
//! right-pads with spaces. PG docs explicitly recommend VARCHAR or TEXT.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql104"
  }
  fn default_severity(&self) -> Severity {
    Severity::Hint
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let start: usize = u32::from(stmt.range.start()) as usize;
    let end: usize = (u32::from(stmt.range.end()) as usize).min(source.len());
    let body = &source[start..end];
    let upper = body.to_ascii_uppercase();
    let bytes = upper.as_bytes();
    let n = bytes.len();
    // Only flag inside CREATE TABLE / ALTER TABLE / CAST. Skip
    // CHARACTER VARYING (== varchar, fine).
    if !upper.contains("CREATE TABLE")
      && !upper.contains("ALTER TABLE")
      && !upper.contains("CAST(")
      && !upper.contains("::")
    {
      return;
    }
    let mut i = 0;
    while i + 4 <= n {
      // Match CHARACTER first (longer prefix) -- if followed by
      // VARYING, skip; else treat like CHAR(n).
      let mut matched: Option<usize> = None;
      if i + 9 <= n
        && &upper[i..i + 9] == "CHARACTER"
        && (i == 0 || !is_word(bytes[i - 1] as char))
        && !is_word(bytes[i + 9] as char)
      {
        let mut j = i + 9;
        while j < n && bytes[j].is_ascii_whitespace() {
          j += 1;
        }
        if j + 7 <= n && &upper[j..j + 7] == "VARYING" {
          i = j + 7;
          continue;
        }
        matched = Some(i + 9);
      } else if i + 6 <= n
        && &upper[i..i + 6] == "BPCHAR"
        && (i == 0 || !is_word(bytes[i - 1] as char))
        && (i + 6 == n || !is_word(bytes[i + 6] as char))
      {
        // bpchar is the internal PG name for blank-padded char.
        matched = Some(i + 6);
      } else if i + 4 <= n
        && &upper[i..i + 4] == "CHAR"
        && (i == 0 || !is_word(bytes[i - 1] as char))
        && (i + 4 == n || !is_word(bytes[i + 4] as char))
      {
        matched = Some(i + 4);
      }
      if let Some(after) = matched {
        let mut j = after;
        while j < n && bytes[j].is_ascii_whitespace() {
          j += 1;
        }
        if j < n && bytes[j] == b'(' {
          // CHAR(n) form -- find matching ).
          let close = body[j..].find(')').map(|p| j + p);
          if let Some(c) = close {
            let abs_start = start + i;
            let abs_end = start + c + 1;
            out.push(Diagnostic {
              code: "sql104",
              severity: Severity::Hint,
              message: "CHAR(n) / CHARACTER(n) right-pads with spaces -- prefer VARCHAR(n) or TEXT".into(),
              range: text_size::TextRange::new((abs_start as u32).into(), (abs_end as u32).into()),
            });
            return;
          }
        }
      }
      i += 1;
    }
  }
}

fn is_word(c: char) -> bool {
  c.is_alphanumeric() || c == '_'
}
