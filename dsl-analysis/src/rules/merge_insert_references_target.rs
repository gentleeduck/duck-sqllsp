//! sql794: `WHEN NOT MATCHED THEN INSERT ... VALUES (target.col, ...)`
//! -- referencing the MERGE target's alias inside the INSERT branch,
//! which only runs when NO target row matched. PostgreSQL rejects this
//! ("invalid reference to FROM-clause entry").

use crate::clause_scan::is_word;
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql794"
  }
  fn default_severity(&self) -> Severity {
    Severity::Error
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, body, upper) = crate::stmt_body_upper(stmt, source);
    if !upper.trim_start().starts_with("MERGE") {
      return;
    }
    let ub = upper.as_bytes();
    let Some(target_alias) = merge_into_alias(ub, body) else { return };
    let target_alias_upper = target_alias.to_ascii_uppercase();
    let mut i = 0usize;
    while i < ub.len() {
      if !word_at(ub, i, b"INSERT") {
        i += 1;
        continue;
      }
      if !preceded_by_not_matched(ub, i) {
        i += "INSERT".len();
        continue;
      }
      let Some(values_rel) = find_word_from(ub, i, b"VALUES") else {
        i += "INSERT".len();
        continue;
      };
      let vp = skip_ws(ub, values_rel + 6);
      if ub.get(vp) != Some(&b'(') {
        i = values_rel + 6;
        continue;
      }
      let Some(close) = match_paren(ub, vp) else { break };
      if let Some(rel) = find_word_from(ub, vp, target_alias_upper.as_bytes())
        && rel < close
        && ub.get(rel + target_alias_upper.len()) == Some(&b'.')
      {
        out.push(Diagnostic {
          code: "sql794",
          severity: Severity::Error,
          message: format!(
            "INSERT VALUES references `{target_alias}` (the MERGE target) -- no target row exists in the NOT MATCHED branch"
          ),
          range: crate::range_at(start + rel, start + rel + target_alias_upper.len()),
        });
      }
      i = close + 1;
    }
  }
}

/// The MERGE target's alias, or its unqualified table name if no
/// alias was given.
fn merge_into_alias(ub: &[u8], body: &str) -> Option<String> {
  let into_rel = find_word_from(ub, 0, b"INTO")?;
  let mut i = skip_ws(ub, into_rel + 4);
  let name_start = i;
  while i < ub.len() && (is_word(ub[i] as char) || ub[i] == b'.') {
    i += 1;
  }
  if i == name_start {
    return None;
  }
  let name = &body[name_start..i];
  let mut j = skip_ws(ub, i);
  if ub[j..].starts_with(b"AS") && (j + 2 >= ub.len() || !is_word(ub[j + 2] as char)) {
    j = skip_ws(ub, j + 2);
  }
  let alias_start = j;
  while j < ub.len() && is_word(ub[j] as char) {
    j += 1;
  }
  if j > alias_start {
    let word_up = std::str::from_utf8(&ub[alias_start..j]).unwrap_or("");
    const STOP: &[&str] = &["USING", "ON"];
    if !STOP.contains(&word_up) {
      return Some(body[alias_start..j].to_string());
    }
  }
  Some(name.rsplit('.').next().unwrap_or(name).to_string())
}

/// True when the nearest preceding `WHEN` (scanning back from `at`)
/// has `NOT` and `MATCHED` between it and `at`.
fn preceded_by_not_matched(ub: &[u8], at: usize) -> bool {
  let mut i = at;
  while i > 0 {
    i -= 1;
    if word_at(ub, i, b"WHEN") {
      let seg = &ub[i..at];
      return contains_word_bytes(seg, b"NOT") && contains_word_bytes(seg, b"MATCHED");
    }
  }
  false
}

fn contains_word_bytes(hay: &[u8], w: &[u8]) -> bool {
  let mut i = 0usize;
  while i + w.len() <= hay.len() {
    if word_at(hay, i, w) {
      return true;
    }
    i += 1;
  }
  false
}

fn find_word_from(ub: &[u8], from: usize, w: &[u8]) -> Option<usize> {
  let mut i = from;
  while i + w.len() <= ub.len() {
    if word_at(ub, i, w) {
      return Some(i);
    }
    i += 1;
  }
  None
}

fn word_at(ub: &[u8], i: usize, w: &[u8]) -> bool {
  i + w.len() <= ub.len()
    && &ub[i..i + w.len()] == w
    && (i == 0 || !is_word(ub[i - 1] as char))
    && (i + w.len() == ub.len() || !is_word(ub[i + w.len()] as char))
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
