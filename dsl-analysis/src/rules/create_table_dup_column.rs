//! sql557: `CREATE TABLE t (id int, id text)` -- the same column name appears
//! twice in the column list. Postgres rejects it at DDL time with 42701
//! ("column \"id\" specified more than once"). Almost always a copy-paste slip.

use crate::clause_scan::is_word;
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;
use std::collections::HashMap;

const CONSTRAINT_KW: &[&str] =
  &["PRIMARY", "UNIQUE", "FOREIGN", "CONSTRAINT", "CHECK", "EXCLUDE", "LIKE", "INHERITS"];

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql557"
  }
  fn default_severity(&self) -> Severity {
    Severity::Error
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, body, upper) = crate::stmt_body_upper(stmt, source);
    let Some((bs, be)) = create_table_body(&upper) else { return };
    let ub = upper.as_bytes();

    let mut seen: HashMap<String, ()> = HashMap::new();
    for (s, e) in split_top_level(bs, be, ub) {
      let seg = body[s..e].trim();
      let seg_u = upper[s..e].trim_start();
      let first = seg_u.split(|c: char| c.is_whitespace() || c == '(').next().unwrap_or("");
      if CONSTRAINT_KW.contains(&first) {
        continue;
      }
      let Some(name) = first_ident(seg) else { continue };
      let key = name.to_ascii_lowercase();
      if seen.insert(key, ()).is_some() {
        let lead = (e - s) - body[s..e].trim_start().len();
        out.push(Diagnostic {
          code: "sql557",
          severity: Severity::Error,
          message: format!("column `{name}` is defined more than once (PG error 42701)"),
          range: crate::range_at(start + s + lead, start + s + lead + name.len()),
        });
      }
    }
  }
}

fn first_ident(seg: &str) -> Option<String> {
  let bytes = seg.as_bytes();
  if bytes.is_empty() {
    return None;
  }
  if bytes[0] == b'"' {
    let rel = seg[1..].find('"')?;
    return Some(seg[1..1 + rel].to_string());
  }
  let mut e = 0;
  while e < bytes.len() && (is_word(bytes[e] as char)) {
    e += 1;
  }
  if e == 0 { None } else { Some(seg[..e].to_string()) }
}

/// Byte range of the column list of a `CREATE TABLE name (...)`, or None for
/// CTAS / no parens.
fn create_table_body(upper: &str) -> Option<(usize, usize)> {
  let at = upper.find("CREATE TABLE")?;
  let bytes = upper.as_bytes();
  let open = bytes[at..].iter().position(|&b| b == b'(')? + at;
  // `CREATE TABLE ... AS SELECT` has no column list.
  if upper[at..open].contains(" AS ") {
    return None;
  }
  let close = match_paren(bytes, open)?;
  Some((open + 1, close))
}

fn split_top_level(from: usize, to: usize, ub: &[u8]) -> Vec<(usize, usize)> {
  let mut out = Vec::new();
  let mut depth = 0i32;
  let mut last = from;
  let mut i = from;
  while i < to {
    match ub[i] {
      b'(' | b'[' => depth += 1,
      b')' | b']' => depth -= 1,
      b'\'' => {
        i += 1;
        while i < to && ub[i] != b'\'' {
          i += 1;
        }
      },
      b',' if depth == 0 => {
        out.push((last, i));
        last = i + 1;
      },
      _ => {},
    }
    i += 1;
  }
  out.push((last, to));
  out
}

fn match_paren(bytes: &[u8], open: usize) -> Option<usize> {
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
