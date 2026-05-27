//! sql238: `<arr> = ARRAY[..., NULL, ...]` -- = on arrays treats
//! NULL elements as never-equal (returns NULL not TRUE). Almost
//! always the author wanted `IS NOT DISTINCT FROM` for full equality
//! including NULL elements.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql238"
  }
  fn default_severity(&self) -> Severity {
    Severity::Warning
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let start: usize = u32::from(stmt.range.start()) as usize;
    let end: usize = (u32::from(stmt.range.end()) as usize).min(source.len());
    let body = &source[start..end];
    let upper = body.to_ascii_uppercase();
    let mut from = 0usize;
    while let Some(rel) = upper[from..].find("ARRAY[") {
      let at = from + rel;
      if at > 0 {
        let prev = body.as_bytes()[at - 1] as char;
        if prev.is_ascii_alphanumeric() || prev == '_' {
          from = at + 6;
          continue;
        }
      }
      let open = at + "ARRAY[".len() - 1;
      let Some(close) = find_matching_bracket(body, open) else { break };
      let inner = &body[open + 1..close];
      let has_null = inner.split(',').any(|tok| tok.trim().eq_ignore_ascii_case("NULL"));
      if !has_null {
        from = close + 1;
        continue;
      }
      // Walk left looking for `=` (skipping whitespace).
      let mut k = at;
      while k > 0 {
        let c = body.as_bytes()[k - 1] as char;
        if c.is_ascii_whitespace() { k -= 1 } else { break }
      }
      if k == 0 {
        from = close + 1;
        continue;
      }
      if body.as_bytes()[k - 1] != b'=' {
        from = close + 1;
        continue;
      }
      out.push(Diagnostic {
        code: "sql238",
        severity: Severity::Warning,
        message: "`= ARRAY[..., NULL, ...]` -- `=` returns NULL when any element is NULL; use `IS NOT DISTINCT FROM` for NULL-aware equality".into(),
        range: text_size::TextRange::new(((start + at) as u32).into(), ((start + close + 1) as u32).into()),
      });
      from = close + 1;
    }
  }
}

fn find_matching_bracket(s: &str, open: usize) -> Option<usize> {
  let bytes = s.as_bytes();
  let mut depth = 0i32;
  let mut i = open;
  while i < bytes.len() {
    match bytes[i] {
      b'[' => depth += 1,
      b']' => {
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
