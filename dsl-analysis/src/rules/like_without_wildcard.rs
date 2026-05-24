//! sql052: `LIKE 'plain string'` -- no wildcard means LIKE behaves
//! exactly like `=`, and `=` is faster.
//!
//! Example: `WHERE name LIKE 'alice'` -> use `WHERE name = 'alice'`.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::{Statement, StatementKind};
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql052"
  }
  fn default_severity(&self) -> Severity {
    Severity::Hint
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    if !matches!(stmt.kind, StatementKind::Select(_) | StatementKind::Update(_) | StatementKind::Delete(_)) {
      return;
    }
    let start: usize = u32::from(stmt.range.start()) as usize;
    let end: usize = (u32::from(stmt.range.end()) as usize).min(source.len());
    let body = &source[start..end];
    let upper = body.to_ascii_uppercase();
    let bytes = body.as_bytes();
    let upper_bytes = upper.as_bytes();
    let n = bytes.len();

    // Walk for ` LIKE '...'` patterns. Once found, check the string
    // contents for any wildcard (`%` or `_`).
    let mut i = 0;
    while i + 5 <= n {
      if &upper[i..i + 4] == "LIKE"
        && (i == 0 || !is_word(upper_bytes[i - 1] as char))
        && (i + 4 == n || !is_word(upper_bytes[i + 4] as char))
      {
        let mut j = i + 4;
        while j < n && bytes[j].is_ascii_whitespace() {
          j += 1;
        }
        if j < n && bytes[j] == b'\'' {
          let str_start = j + 1;
          let mut k = str_start;
          while k < n && bytes[k] != b'\'' {
            k += 1;
          }
          if k < n {
            let pat = &body[str_start..k];
            if !pat.contains('%') && !pat.contains('_') {
              let abs_start = start + i;
              let abs_end = start + k + 1;
              out.push(Diagnostic {
                code: "sql052",
                severity: Severity::Hint,
                message: format!("`LIKE '{pat}'` has no wildcards -- use `=` for a literal match"),
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
