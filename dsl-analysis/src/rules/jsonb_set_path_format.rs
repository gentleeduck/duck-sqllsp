//! sql223: `jsonb_set(col, 'key', '"val"')` -- path must be an
//! array literal `{key}` or `{a,b,c}` not a bare string. PG raises
//! 22P02 "malformed array literal" at runtime.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql223"
  }
  fn default_severity(&self) -> Severity {
    Severity::Error
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let start: usize = u32::from(stmt.range.start()) as usize;
    let end: usize = (u32::from(stmt.range.end()) as usize).min(source.len());
    let body = &source[start..end];
    let lower = body.to_ascii_lowercase();
    let mut from = 0usize;
    while let Some(rel) = lower[from..].find("jsonb_set(") {
      let at = from + rel;
      if at > 0 {
        let prev = body.as_bytes()[at - 1] as char;
        if prev.is_ascii_alphanumeric() || prev == '_' {
          from = at + 10;
          continue;
        }
      }
      let open = at + "jsonb_set(".len();
      let Some(close) = find_matching_paren(body, open - 1) else {
        from = open;
        continue;
      };
      let inner = &body[open..close];
      let args = split_top_level(inner);
      if args.len() < 2 {
        from = close + 1;
        continue;
      }
      let path = args[1].trim();
      if let Some(stripped) = path.strip_prefix('\'').and_then(|s| s.strip_suffix('\'')) {
        let p = stripped.trim();
        if !p.starts_with('{') || !p.ends_with('}') {
          let off = inner.find(path).unwrap_or(0);
          out.push(Diagnostic {
            code: "sql223",
            severity: Severity::Error,
            message: format!(
              "jsonb_set path `{p}` must be a `text[]` literal -- use `'{{{}}}'` or `ARRAY['{}']` (PG 22P02)",
              p, p,
            ),
            range: text_size::TextRange::new(
              ((start + open + off) as u32).into(),
              ((start + open + off + path.len()) as u32).into(),
            ),
          });
        }
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
