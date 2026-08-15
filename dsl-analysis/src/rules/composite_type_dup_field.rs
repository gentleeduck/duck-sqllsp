//! sql779: `CREATE TYPE ... AS (a int, a text)` -- duplicate field
//! name. Sibling to the existing create_table_dup_column.

use crate::clause_scan::{is_word, split_top_level};
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql779"
  }
  fn default_severity(&self) -> Severity {
    Severity::Error
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, body, upper) = crate::stmt_body_upper(stmt, source);
    if !upper.trim_start().starts_with("CREATE TYPE") {
      return;
    }
    let ub = upper.as_bytes();
    let Some(as_rel) = find_word_from(ub, 0, b"AS") else { return };
    let p = skip_ws(ub, as_rel + 2);
    if ub.get(p) != Some(&b'(') {
      return;
    }
    let Some(close) = match_paren(ub, p) else { return };
    let list = &body[p + 1..close];
    let mut seen: Vec<String> = Vec::new();
    for (entry, off) in split_top_level(list) {
      let Some(name) = leading_ident(entry) else { continue };
      if seen.iter().any(|s| s.eq_ignore_ascii_case(&name)) {
        let lead = entry.len() - entry.trim_start().len();
        let abs = p + 1 + off + lead;
        out.push(Diagnostic {
          code: "sql779",
          severity: Severity::Error,
          message: format!("composite type field `{name}` is defined more than once"),
          range: crate::range_at(start + abs, start + abs + name.len()),
        });
      } else {
        seen.push(name);
      }
    }
  }
}

fn leading_ident(s: &str) -> Option<String> {
  let t = s.trim_start();
  if let Some(rest) = t.strip_prefix('"') {
    let end = rest.find('"')?;
    return Some(rest[..end].to_string());
  }
  let end = t.find(|c: char| !(c.is_alphanumeric() || c == '_')).unwrap_or(t.len());
  if end == 0 {
    return None;
  }
  Some(t[..end].to_string())
}

fn find_word_from(ub: &[u8], from: usize, w: &[u8]) -> Option<usize> {
  let mut i = from;
  while i + w.len() <= ub.len() {
    if &ub[i..i + w.len()] == w
      && (i == 0 || !is_word(ub[i - 1] as char))
      && (i + w.len() == ub.len() || !is_word(ub[i + w.len()] as char))
    {
      return Some(i);
    }
    i += 1;
  }
  None
}

fn match_paren(ub: &[u8], open: usize) -> Option<usize> {
  let mut depth = 0i32;
  let mut i = open;
  while i < ub.len() {
    match ub[i] {
      b'(' => depth += 1,
      b')' => {
        depth -= 1;
        if depth == 0 {
          return Some(i);
        }
      },
      b'\'' => {
        i += 1;
        while i < ub.len() && ub[i] != b'\'' {
          i += 1;
        }
      },
      _ => {},
    }
    i += 1;
  }
  None
}

fn skip_ws(ub: &[u8], mut i: usize) -> usize {
  while i < ub.len() && ub[i].is_ascii_whitespace() {
    i += 1;
  }
  i
}
