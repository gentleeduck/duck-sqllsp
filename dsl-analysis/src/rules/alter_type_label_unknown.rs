//! sql286: `ALTER TYPE x ADD VALUE 'new' BEFORE 'bogus'` where
//! `bogus` is not one of `x`'s enum labels. PG raises 22023 at
//! parse. Catches typos in the anchor label.
//!
//! Source-text scan: harvests every `CREATE TYPE x AS ENUM (...)`
//! to know each enum's labels, then validates the BEFORE/AFTER
//! anchor.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;
use std::collections::HashMap;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql286"
  }
  fn default_severity(&self) -> Severity {
    Severity::Error
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let start: usize = u32::from(stmt.range.start()) as usize;
    let end: usize = (u32::from(stmt.range.end()) as usize).min(source.len());
    let raw = &source[start..end];
    let body_owned = crate::textutil::strip_noise_full(raw);
    let body = body_owned.as_str();
    let upper = body.to_ascii_uppercase();
    if !upper.contains("ALTER TYPE") {
      return;
    }
    let Some(add_at) = upper.find("ADD VALUE") else { return };
    let anchor_kw = if let Some(b) = upper[add_at..].find("BEFORE") {
      Some(("BEFORE", add_at + b))
    } else if let Some(a) = upper[add_at..].find("AFTER") {
      Some(("AFTER", add_at + a))
    } else {
      return;
    };
    let Some((kw, kw_at)) = anchor_kw else { return };
    let after = kw_at + kw.len();
    let rest = body[after..].trim_start();
    if !rest.starts_with('\'') {
      return;
    }
    let Some(close_rel) = rest[1..].find('\'') else { return };
    let anchor = &rest[1..1 + close_rel];
    // Find the type name (between ALTER TYPE and ADD VALUE).
    let at_at = upper.find("ALTER TYPE").unwrap();
    let after_at = at_at + "ALTER TYPE".len();
    let name_rest = body[after_at..].trim_start();
    let id_end = name_rest
      .find(|c: char| !c.is_ascii_alphanumeric() && c != '_' && c != '.' && c != '"')
      .unwrap_or(name_rest.len());
    let type_name = name_rest[..id_end].rsplit('.').next().unwrap_or("").trim_matches('"').to_string();
    if type_name.is_empty() {
      return;
    }
    let labels = enum_labels(source, &type_name);
    if labels.is_empty() {
      return;
    }
    if labels.iter().any(|l| l.eq_ignore_ascii_case(anchor)) {
      return;
    }
    let abs_s = start + after + (body[after..].len() - rest.len()) + 1;
    let abs_e = abs_s + close_rel;
    out.push(Diagnostic {
      code: "sql286",
      severity: Severity::Error,
      message: format!("{kw} `{anchor}` is not a label of enum `{type_name}` -- valid labels: {}", labels.join(", "),),
      range: text_size::TextRange::new((abs_s as u32).into(), (abs_e as u32).into()),
    });
    let _ = HashMap::<(), ()>::new();
  }
}

fn enum_labels(source: &str, name: &str) -> Vec<String> {
  let upper = source.to_ascii_uppercase();
  let needle = "CREATE TYPE ";
  let mut from = 0usize;
  while let Some(rel) = upper[from..].find(needle) {
    let at = from + rel;
    let after = at + needle.len();
    let rest = &source[after..];
    let id_end =
      rest.find(|c: char| !c.is_ascii_alphanumeric() && c != '_' && c != '.' && c != '"').unwrap_or(rest.len());
    let n = rest[..id_end].rsplit('.').next().unwrap_or("").trim_matches('"');
    if n.eq_ignore_ascii_case(name) {
      let body_upper = &upper[after + id_end..];
      if let Some(enum_at) = body_upper.find("AS ENUM") {
        let after_enum = after + id_end + enum_at + "AS ENUM".len();
        let post = source[after_enum..].trim_start();
        if !post.starts_with('(') {
          return Vec::new();
        }
        let open = after_enum + (source[after_enum..].len() - post.len());
        let Some(close) = find_matching_paren(source, open) else { return Vec::new() };
        let inner = &source[open + 1..close];
        let mut out = Vec::new();
        for raw in inner.split(',') {
          let t = raw.trim();
          if let Some(s) = t.strip_prefix('\'').and_then(|s| s.strip_suffix('\'')) {
            out.push(s.to_string());
          }
        }
        return out;
      }
    }
    from = after;
  }
  Vec::new()
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
