//! sql084: `COUNT(1)` is equivalent to `COUNT(*)` -- prefer `COUNT(*)`
//! which reads more naturally and matches every style guide.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql084"
  }
  fn default_severity(&self) -> Severity {
    Severity::Hint
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let start: usize = u32::from(stmt.range.start()) as usize;
    let end: usize = (u32::from(stmt.range.end()) as usize).min(source.len());
    let raw = &source[start..end];
    let body_owned = crate::textutil::strip_noise_full(raw);
    let body = body_owned.as_str();
    let upper = body.to_ascii_uppercase();
    let bytes = upper.as_bytes();
    let n = bytes.len();
    let mut i = 0;
    while i + 8 <= n {
      if &upper[i..i + 5] == "COUNT" && (i == 0 || !is_word(bytes[i - 1] as char)) {
        // After `COUNT`, allow whitespace, then `(`.
        let mut j = i + 5;
        while j < n && bytes[j].is_ascii_whitespace() {
          j += 1;
        }
        if j < n && bytes[j] == b'(' {
          let inner_start = j + 1;
          let mut k = inner_start;
          while k < n && bytes[k].is_ascii_whitespace() {
            k += 1;
          }
          if k < n && bytes[k] == b'1' {
            let after_one = k + 1;
            let mut m = after_one;
            while m < n && bytes[m].is_ascii_whitespace() {
              m += 1;
            }
            if m < n && bytes[m] == b')' {
              let abs_start = start + i;
              let abs_end = start + m + 1;
              out.push(Diagnostic {
                code: "sql084",
                severity: Severity::Hint,
                message: "COUNT(1) is equivalent to COUNT(*) -- use COUNT(*) for clarity".into(),
                range: text_size::TextRange::new((abs_start as u32).into(), (abs_end as u32).into()),
              });
              return;
            }
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
