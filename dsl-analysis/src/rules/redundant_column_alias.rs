//! sql531: `SELECT name AS name` -- aliasing a column to its own name. The
//! `AS name` is dead: the output column is already called `name`. Also covers
//! `SELECT u.name AS name` (the unqualified output name already matches).
//! Pure noise; drop the alias.
//!
//! Only a bare column reference whose alias equals the column's base name is
//! flagged -- `lower(name) AS name` (a real rename) and `name AS full_name`
//! are left alone.

use crate::clause_scan::{find_clause, find_clause_end, is_word, parse_simple_ident, split_top_level};
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

const STOPWORDS: &[&str] = &["FROM", "WHERE", "GROUP", "ORDER", "HAVING", "LIMIT", "OFFSET", "WINDOW", "UNION", "INTERSECT", "EXCEPT", "FETCH"];

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql531"
  }
  fn default_severity(&self) -> Severity {
    Severity::Hint
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, body, upper) = crate::stmt_body_upper(stmt, source);
    let ub = upper.as_bytes();

    let mut from = 0usize;
    while let Some(rel) = find_clause(&ub[from..], b"SELECT").map(|p| p + from) {
      let ls = rel + 6;
      let le = find_clause_end(ub, ls, STOPWORDS);
      for (item, off) in split_top_level(&body[ls..le]) {
        if let Some((span_s, span_e)) = redundant_alias(item) {
          out.push(Diagnostic {
            code: "sql531",
            severity: Severity::Hint,
            message: format!("redundant alias `{}` -- the column is already named that", item.trim()),
            range: crate::range_at(start + ls + off + span_s, start + ls + off + span_e),
          });
        }
      }
      from = le.max(ls);
    }
  }
}

/// If `item` is `<col> AS <alias>` where the bare column's base name equals
/// the bare alias, return the trimmed span (relative to `item`).
fn redundant_alias(item: &str) -> Option<(usize, usize)> {
  let bytes = item.as_bytes();
  // Find a top-level, word-bounded ` AS `.
  let mut depth = 0i32;
  let mut i = 0usize;
  let mut as_at = None;
  while i < bytes.len() {
    match bytes[i] {
      b'(' | b'[' => depth += 1,
      b')' | b']' => depth -= 1,
      b'\'' | b'"' => {
        let q = bytes[i];
        i += 1;
        while i < bytes.len() && bytes[i] != q {
          i += 1;
        }
      },
      b'a' | b'A' if depth == 0 => {
        let is_as = bytes.get(i + 1).is_some_and(|b| b.eq_ignore_ascii_case(&b's'));
        let prev_ok = i == 0 || !is_word(bytes[i - 1] as char);
        let next_ok = bytes.get(i + 2).is_none_or(|&b| !is_word(b as char));
        if is_as && prev_ok && next_ok {
          as_at = Some((i, i + 2));
          break;
        }
      },
      _ => {},
    }
    i += 1;
  }
  let (as_s, as_e) = as_at?;
  let lhs = item[..as_s].trim();
  let alias = item[as_e..].trim();
  // Alias must be a bare identifier (quoted aliases carry case meaning).
  if alias.is_empty() || !alias.bytes().all(|b| is_word(b as char)) {
    return None;
  }
  let (_, base) = parse_simple_ident(lhs)?;
  if base.eq_ignore_ascii_case(alias) {
    let lead = item.len() - item.trim_start().len();
    Some((lead, item.trim_end().len()))
  } else {
    None
  }
}
