//! sql299: `PRIMARY KEY (a, a)` / `UNIQUE (a, a)` -- duplicate
//! column in the key. PG raises 42P16 at parse time.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql299"
  }
  fn default_severity(&self) -> Severity {
    Severity::Error
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let start: usize = u32::from(stmt.range.start()) as usize;
    let end: usize = (u32::from(stmt.range.end()) as usize).min(source.len());
    let body = &source[start..end];
    let upper = body.to_ascii_uppercase();
    for kw in ["PRIMARY KEY", "UNIQUE"] {
      let mut from = 0usize;
      while let Some(rel) = upper[from..].find(kw) {
        let at = from + rel;
        if at > 0 {
          let prev = body.as_bytes()[at - 1] as char;
          if prev.is_ascii_alphanumeric() || prev == '_' {
            from = at + kw.len();
            continue;
          }
        }
        let after = at + kw.len();
        let rest = body[after..].trim_start();
        if !rest.starts_with('(') {
          from = after;
          continue;
        }
        let open = after + (body[after..].len() - rest.len());
        let Some(close) = find_matching_paren(body, open) else { break };
        let inner = &body[open + 1..close];
        let mut cols: Vec<String> =
          inner.split(',').map(|c| c.trim().trim_matches('"').to_ascii_lowercase()).filter(|c| !c.is_empty()).collect();
        let original_len = cols.len();
        cols.sort();
        cols.dedup();
        if cols.len() != original_len {
          out.push(Diagnostic {
            code: "sql299",
            severity: Severity::Error,
            message: format!("`{kw} ({})` has duplicate column(s) -- PG raises 42P16", inner.trim()),
            range: text_size::TextRange::new(((start + at) as u32).into(), ((start + close + 1) as u32).into()),
          });
        }
        from = close + 1;
      }
    }
  }
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
