//! sql109: `length(text_col)` returns *bytes*. Use `char_length` for
//! characters -- the bytes/chars distinction bites with non-ASCII.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql109"
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
    let mut i = 0;
    while i + 7 <= n {
      // Match `LENGTH(` -- skip `BIT_LENGTH`, `OCTET_LENGTH`,
      // `CHAR_LENGTH`, `CHARACTER_LENGTH`.
      if &upper[i..i + 6] == "LENGTH" && (i == 0 || !is_word(bytes[i - 1] as char)) {
        // Walk one char past the keyword (skip optional ws), check `(`.
        let mut j = i + 6;
        while j < n && bytes[j].is_ascii_whitespace() {
          j += 1;
        }
        if j < n && bytes[j] == b'(' {
          let close = body[j..].find(')').map(|p| j + p + 1);
          if let Some(c) = close {
            let abs_start = start + i;
            let abs_end = start + c;
            out.push(Diagnostic {
              code: "sql109",
              severity: Severity::Hint,
              message: "length() returns bytes -- use char_length() for character count on non-ASCII text".into(),
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
