//! sql221: `ARRAY[1, 'foo']` -- mixed-type literal constructor. PG
//! raises 42804 "ARRAY types ... and ... cannot be matched" at parse
//! time. Catches the common mistake of bracket-constructed arrays
//! that mix int + text + bool literals.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql221"
  }
  fn default_severity(&self) -> Severity {
    Severity::Error
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
      let Some(close) = find_matching_bracket(body, open) else {
        from = open;
        continue;
      };
      let inner = &body[open + 1..close];
      let mut families: Vec<&'static str> = Vec::new();
      for raw in split_top_level(inner) {
        let lit = raw.trim();
        if lit.is_empty() {
          continue;
        }
        if let Some(f) = literal_family(lit)
          && !families.contains(&f)
        {
          families.push(f);
        }
      }
      if families.len() >= 2 {
        out.push(Diagnostic {
          code: "sql221",
          severity: Severity::Error,
          message: format!(
            "ARRAY[...] elements have divergent literal types: {} -- PG raises 42804",
            families.join(", "),
          ),
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
      b'(' | b'[' => depth += 1,
      b')' | b']' => depth -= 1,
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
