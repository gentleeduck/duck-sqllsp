//! sql146: `VARCHAR` / `CHARACTER VARYING` without an explicit length.
//! Unbounded VARCHAR is effectively TEXT but with the awkward type
//! name -- prefer `TEXT` when no cap is wanted, or spell the cap when
//! it is.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql146"
  }
  fn default_severity(&self) -> Severity {
    Severity::Hint
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let start: usize = u32::from(stmt.range.start()) as usize;
    let end: usize = (u32::from(stmt.range.end()) as usize).min(source.len());
    let body = &source[start..end];
    let upper = body.to_ascii_uppercase();
    if !upper.contains("CREATE TABLE") && !upper.contains("ALTER TABLE") {
      return;
    }
    let bytes = upper.as_bytes();
    let n = bytes.len();
    // Check both `VARCHAR` (single token) and `CHARACTER VARYING`.
    let candidates: &[(&str, usize)] = &[("CHARACTER VARYING", 17), ("VARCHAR", 7)];
    let mut i = 0;
    while i < n {
      for (kw, w) in candidates {
        if i + w <= n
          && &upper[i..i + w] == *kw
          && (i == 0 || !is_word(bytes[i - 1] as char))
          && (i + w == n || !is_word(bytes[i + w] as char))
        {
          let mut j = i + w;
          while j < n && bytes[j].is_ascii_whitespace() {
            j += 1;
          }
          // Followed by `(` ? Has a limit. Skip.
          if j < n && bytes[j] == b'(' {
            i += w;
            continue;
          }
          let abs_start = start + i;
          let abs_end = start + i + w;
          out.push(Diagnostic {
            code: "sql146",
            severity: Severity::Hint,
            message: "unbounded VARCHAR / CHARACTER VARYING -- use TEXT for no cap, or spell the cap as `VARCHAR(n)`"
              .into(),
            range: text_size::TextRange::new((abs_start as u32).into(), (abs_end as u32).into()),
          });
          return;
        }
      }
      i += 1;
    }
  }
}

fn is_word(c: char) -> bool {
  c.is_alphanumeric() || c == '_'
}
