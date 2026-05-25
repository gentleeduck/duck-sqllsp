//! sql206: `INSERT INTO t (a, b) VALUES ((SELECT 1, 2))` -- the
//! scalar-subquery returns 2 columns where one was expected. Or
//! `INSERT INTO t SELECT 1, 2, 3` where t has only 2 columns.
//! PG raises 42601 / 42P10. Heuristic: counts commas at top level in
//! the subquery projection list.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::{Statement, StatementKind};
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql206"
  }
  fn default_severity(&self) -> Severity {
    Severity::Error
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let StatementKind::Insert(ins) = &stmt.kind else { return };
    let start: usize = u32::from(stmt.range.start()) as usize;
    let end: usize = (u32::from(stmt.range.end()) as usize).min(source.len());
    let raw = &source[start..end];
    // Strip line comments + block comments + string literals before
    // scanning. `-- SELECT x;` in a leading comment must not be mistaken
    // for the INSERT's source-rowset SELECT (was firing on
    // `-- SELECT asdfsd from users;\nINSERT INTO users (a,b,c) VALUES(...)`).
    let body_owned = strip_noise(raw);
    let body = body_owned.as_str();
    let upper = body.to_ascii_uppercase();
    // VALUES form (`INSERT ... VALUES (...)`): no SELECT-source involved;
    // tuple-arity mismatches are handled elsewhere. If VALUES occurs
    // before any SELECT, skip.
    match (upper.find("VALUES"), upper.find("SELECT")) {
      (Some(v), Some(s)) if v < s => return,
      (Some(_), None) => return,
      _ => {},
    }
    // Expected column count = explicit col list, else from catalog table.
    let expected = if !ins.columns.is_empty() {
      ins.columns.len()
    } else if let Some(t) = catalog.find_table(ins.table.schema.as_deref(), &ins.table.name) {
      t.columns.len()
    } else { return };
    // Locate the SELECT keyword that starts the source rowset.
    let Some(sel_at) = upper.find("SELECT") else { return };
    // Find the end of the projection list (before FROM or `)`, end).
    let after_sel = sel_at + "SELECT".len();
    let tail = &upper[after_sel..];
    let stop_from = tail.find(" FROM ").map(|p| p + " FROM ".len() - " FROM ".len());
    let stop_close = paren_close_at_depth_zero(body, after_sel);
    let stop_semi = tail.find(';');
    let stop = [stop_from, stop_close.map(|p| p - after_sel), stop_semi]
      .iter().flatten().copied().min();
    let proj_end = after_sel + stop.unwrap_or(tail.len());
    let proj = &body[after_sel..proj_end];
    let count = count_top_level_commas(proj) + 1;
    // Star can mean anything; skip.
    if proj.trim().contains('*') { return }
    if count == expected { return }
    let abs_s = start + sel_at;
    let abs_e = start + proj_end;
    out.push(Diagnostic {
      code: "sql206",
      severity: Severity::Error,
      message: format!(
        "INSERT source SELECT returns {count} columns -- target expects {expected}"
      ),
      range: text_size::TextRange::new((abs_s as u32).into(), (abs_e as u32).into()),
    });
  }
}

fn count_top_level_commas(text: &str) -> usize {
  let bytes = text.as_bytes();
  let mut depth = 0i32;
  let mut commas = 0usize;
  let mut i = 0usize;
  while i < bytes.len() {
    match bytes[i] {
      b'(' => depth += 1,
      b')' => depth -= 1,
      b',' if depth == 0 => commas += 1,
      b'\'' => {
        i += 1;
        while i < bytes.len() && bytes[i] != b'\'' { i += 1 }
      }
      _ => {}
    }
    i += 1;
  }
  commas
}

/// Replace `-- ... \n` lines, `/* ... */` blocks, and `'...'` literals with
/// equal-length space runs so byte offsets are preserved.
fn strip_noise(s: &str) -> String {
  let bytes = s.as_bytes();
  let mut out: Vec<u8> = bytes.to_vec();
  let n = out.len();
  let mut i = 0usize;
  while i < n {
    if i + 1 < n && out[i] == b'-' && out[i + 1] == b'-' {
      while i < n && out[i] != b'\n' {
        out[i] = b' ';
        i += 1;
      }
      continue;
    }
    if i + 1 < n && out[i] == b'/' && out[i + 1] == b'*' {
      while i + 1 < n && !(out[i] == b'*' && out[i + 1] == b'/') {
        out[i] = b' ';
        i += 1;
      }
      if i + 1 < n {
        out[i] = b' ';
        out[i + 1] = b' ';
        i += 2;
      }
      continue;
    }
    if out[i] == b'\'' {
      let q = i;
      i += 1;
      while i < n && out[i] != b'\'' { i += 1 }
      for k in q + 1..i.min(n) { out[k] = b' ' }
      if i < n { i += 1; }
      continue;
    }
    i += 1;
  }
  String::from_utf8(out).unwrap_or_else(|_| s.to_string())
}

fn paren_close_at_depth_zero(text: &str, from: usize) -> Option<usize> {
  let bytes = text.as_bytes();
  let mut depth = 0i32;
  let mut i = from;
  while i < bytes.len() {
    match bytes[i] {
      b'(' => depth += 1,
      b')' => {
        if depth == 0 { return Some(i); }
        depth -= 1;
      }
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
