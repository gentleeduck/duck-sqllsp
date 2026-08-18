//! sql774: `EXCLUDE USING <am> (col WITH op, col WITH op)` -- the same
//! column (or expression) listed twice. Always a copy-paste mistake.
//! Replaces the spec's original sql774 (missing operator after
//! `WITH`), verified unreachable -- pg_query rejects that as a hard
//! parse error before any LintRule sees it.

use crate::clause_scan::{is_word, split_top_level};
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql774"
  }
  fn default_severity(&self) -> Severity {
    Severity::Warning
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, body, upper) = crate::stmt_body_upper(stmt, source);
    let ub = upper.as_bytes();
    let mut i = 0usize;
    while let Some((open, close)) = next_exclude_list(ub, i) {
      let list = &body[open + 1..close];
      let mut seen: Vec<String> = Vec::new();
      for (entry, off) in split_top_level(list) {
        let Some(col) = entry_target(entry) else { continue };
        if seen.iter().any(|s| s.eq_ignore_ascii_case(&col)) {
          let lead = entry.len() - entry.trim_start().len();
          let abs = open + 1 + off + lead;
          out.push(Diagnostic {
            code: "sql774",
            severity: Severity::Warning,
            message: format!("`{col}` appears more than once in the EXCLUDE constraint"),
            range: crate::range_at(start + abs, start + abs + col.len()),
          });
        } else {
          seen.push(col);
        }
      }
      i = close + 1;
    }
  }
}

/// The `col`/`expr` portion of a `col WITH op` exclusion entry -- text
/// before the ` WITH ` keyword, trimmed.
fn entry_target(entry: &str) -> Option<String> {
  let upper = entry.to_ascii_uppercase();
  let at = upper.find(" WITH ")?;
  let t = entry[..at].trim();
  if t.is_empty() { None } else { Some(t.to_string()) }
}

fn next_exclude_list(ub: &[u8], from: usize) -> Option<(usize, usize)> {
  let mut i = from;
  loop {
    let excl = find_word_from(ub, i, b"EXCLUDE")?;
    let mut j = skip_ws(ub, excl + "EXCLUDE".len());
    if !word_at(ub, j, b"USING") {
      i = excl + "EXCLUDE".len();
      continue;
    }
    j = skip_ws(ub, j + "USING".len());
    while j < ub.len() && is_word(ub[j] as char) {
      j += 1;
    }
    j = skip_ws(ub, j);
    if ub.get(j) != Some(&b'(') {
      i = excl + "EXCLUDE".len();
      continue;
    }
    let close = match_paren(ub, j)?;
    return Some((j, close));
  }
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
