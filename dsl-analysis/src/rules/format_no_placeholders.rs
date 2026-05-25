//! sql270: `format('hello world')` -- call to format() with no `%`
//! placeholders. Result is identical to the input string and the
//! function call overhead is wasted. Hint: pass the string literally.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql270"
  }
  fn default_severity(&self) -> Severity {
    Severity::Hint
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let start: usize = u32::from(stmt.range.start()) as usize;
    let end: usize = (u32::from(stmt.range.end()) as usize).min(source.len());
    let body = &source[start..end];
    let lower = body.to_ascii_lowercase();
    let mut from = 0usize;
    while let Some(rel) = lower[from..].find("format(") {
      let at = from + rel;
      if at > 0 {
        let prev = body.as_bytes()[at - 1] as char;
        if prev.is_ascii_alphanumeric() || prev == '_' { from = at + 7; continue }
      }
      let open = at + "format(".len() - 1;
      let Some(close) = find_matching_paren(body, open) else { from = open; break };
      let inner = body[open + 1..close].trim();
      // Single string literal arg, no extras.
      if let Some(lit) = inner.strip_prefix('\'').and_then(|s| s.strip_suffix('\'')) {
        if !lit.contains('%') {
          out.push(Diagnostic {
            code: "sql270",
            severity: Severity::Hint,
            message: format!(
              "format('{lit}') has no `%` placeholders -- result equals the input, drop the format() call"
            ),
            range: text_size::TextRange::new(((start + at) as u32).into(), ((start + close + 1) as u32).into()),
          });
        }
      }
      from = close + 1;
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
      b')' => { depth -= 1; if depth == 0 { return Some(i); } }
      b'\'' => {
        i += 1;
        while i < bytes.len() && bytes[i] != b'\'' { i += 1 }
      }
      _ => {}
    }
    i += 1;
  }
  None
}
