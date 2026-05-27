//! sql293: `NULLIF(1, 'foo')` -- args must be comparable. PG
//! raises 42883 (operator does not exist) at runtime. Same for
//! GREATEST/LEAST with mixed literal types.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql293"
  }
  fn default_severity(&self) -> Severity {
    Severity::Warning
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let start: usize = u32::from(stmt.range.start()) as usize;
    let end: usize = (u32::from(stmt.range.end()) as usize).min(source.len());
    let body = &source[start..end];
    let lower = body.to_ascii_lowercase();
    let mut from = 0usize;
    while let Some(rel) = lower[from..].find("nullif(") {
      let at = from + rel;
      if at > 0 {
        let prev = body.as_bytes()[at - 1] as char;
        if prev.is_ascii_alphanumeric() || prev == '_' {
          from = at + 7;
          continue;
        }
      }
      let open = at + "nullif(".len() - 1;
      let Some(close) = find_matching_paren(body, open) else { break };
      let inner = &body[open + 1..close];
      let args = split_top_level(inner);
      if args.len() != 2 {
        from = close + 1;
        continue;
      }
      let f1 = literal_family(args[0].trim());
      let f2 = literal_family(args[1].trim());
      if let (Some(a), Some(b)) = (f1, f2)
        && a != b
      {
        out.push(Diagnostic {
          code: "sql293",
          severity: Severity::Warning,
          message: format!("NULLIF({a}, {b}) -- arg literals are non-comparable families; PG raises 42883"),
          range: text_size::TextRange::new(((start + at) as u32).into(), ((start + close + 1) as u32).into()),
        });
      }
      from = close + 1;
    }
  }
}

fn split_top_level(text: &str) -> Vec<String> {
  let mut out = Vec::new();
  let bytes = text.as_bytes();
  let mut depth = 0i32;
  let mut start = 0usize;
  let mut i = 0usize;
  while i < bytes.len() {
    match bytes[i] {
      b'(' => depth += 1,
      b')' => depth -= 1,
      b',' if depth == 0 => {
        out.push(text[start..i].to_string());
        start = i + 1
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
  out.push(text[start..].to_string());
  out
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

fn literal_family(s: &str) -> Option<&'static str> {
  let t = s.trim();
  if t.eq_ignore_ascii_case("NULL") {
    return None;
  }
  if t.eq_ignore_ascii_case("TRUE") || t.eq_ignore_ascii_case("FALSE") {
    return Some("boolean");
  }
  if let Some(stripped) = t.strip_prefix('\'')
    && stripped.ends_with('\'')
  {
    return Some("text");
  }
  if t.parse::<i64>().is_ok() {
    return Some("integer");
  }
  if t.parse::<f64>().is_ok() {
    return Some("numeric");
  }
  None
}
