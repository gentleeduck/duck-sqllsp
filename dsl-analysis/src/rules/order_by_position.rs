//! sql099: `ORDER BY 1, 2` -- positional ORDER BY is fragile because
//! changing the SELECT list silently changes the sort.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql099"
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
      if &upper[i..i + 8] == "ORDER BY" && (i == 0 || !is_word(bytes[i - 1] as char)) {
        let mut j = i + 8;
        while j < n && bytes[j].is_ascii_whitespace() {
          j += 1;
        }
        let digit_start = j;
        while j < n && bytes[j].is_ascii_digit() {
          j += 1;
        }
        if j > digit_start {
          let next_ok = j == n || !is_word(bytes[j] as char);
          if next_ok {
            let abs_start = start + i;
            let abs_end = start + j;
            out.push(Diagnostic {
              code: "sql099",
              severity: Severity::Hint,
              message: "positional ORDER BY -- use column names so the sort survives SELECT-list changes".into(),
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
