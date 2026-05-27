//! sql215: `GROUP BY ROLLUP(a)` / `CUBE(a)` with a single grouping
//! column. ROLLUP(a) ≡ GROUPING SETS ((a), ()), CUBE(a) likewise,
//! which is a one-extra-row trick rarely intended. Suggest GROUPING
//! SETS or remove the wrapper.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql215"
  }
  fn default_severity(&self) -> Severity {
    Severity::Hint
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let start: usize = u32::from(stmt.range.start()) as usize;
    let end: usize = (u32::from(stmt.range.end()) as usize).min(source.len());
    let body = &source[start..end];
    let upper = body.to_ascii_uppercase();
    for kw in ["ROLLUP", "CUBE"] {
      let needle = format!("{kw}(");
      let mut from = 0usize;
      while let Some(rel) = upper[from..].find(&needle) {
        let at = from + rel;
        if at > 0 {
          let prev = body.as_bytes()[at - 1] as char;
          if prev.is_ascii_alphanumeric() || prev == '_' {
            from = at + needle.len();
            continue;
          }
        }
        let open = at + needle.len();
        let Some(close) = find_matching_paren(body, open - 1) else {
          from = open;
          continue;
        };
        let inner = &body[open..close];
        if count_top_level_commas(inner) == 0 {
          out.push(Diagnostic {
            code: "sql215",
            severity: Severity::Hint,
            message: format!("`{kw}({})` with single grouping element -- emits one extra all-null row only; prefer GROUPING SETS or plain GROUP BY", inner.trim()),
            range: text_size::TextRange::new(((start + at) as u32).into(), ((start + close + 1) as u32).into()),
          });
        }
        from = close + 1;
      }
    }
  }
}

fn count_top_level_commas(text: &str) -> usize {
  let bytes = text.as_bytes();
  let mut depth = 0i32;
  let mut commas = 0usize;
  let mut i = 0usize;
  while i < bytes.len() {
    match bytes[i] {
      b'(' => depth += 1,
      b')' => depth -= 1,
      b',' if depth == 0 => commas += 1,
      b'\'' => {
        i += 1;
        while i < bytes.len() && bytes[i] != b'\'' {
          i += 1
        }
      },
      _ => {},
    }
    i += 1;
  }
  commas
}

fn find_matching_paren(s: &str, open: usize) -> Option<usize> {
  let bytes = s.as_bytes();
  let mut depth = 0i32;
  let mut i = open;
  while i < bytes.len() {
    match bytes[i] {
      b'(' => depth += 1,
      b')' => {
        depth -= 1;
        if depth == 0 {
          return Some(i);
        }
      },
      b'\'' => {
        i += 1;
        while i < bytes.len() && bytes[i] != b'\'' {
          i += 1
        }
      },
      _ => {},
    }
    i += 1;
  }
  None
}
