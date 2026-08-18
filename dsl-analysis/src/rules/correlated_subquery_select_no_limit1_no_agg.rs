//! sql787: a parenthesized `(SELECT ...)` subquery, correlated to the
//! outer query (references a qualified column whose alias isn't
//! defined in the subquery's own FROM), with no aggregate function and
//! no LIMIT clause -- risks "more than one row returned by a subquery
//! used as an expression" at runtime if more than one row matches.
//! EXISTS/IN/ANY/ALL/SOME-wrapped subqueries are exempt (those are
//! valid multi-row contexts). Conservative: subqueries with their own
//! internal JOIN are skipped entirely -- otherwise the subquery's
//! *own* second joined table would look like an outer reference.

use crate::clause_scan::{find_clause, is_word};
use crate::textutil::contains_word;
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

const AGGREGATES: &[&str] =
  &["COUNT", "SUM", "AVG", "MIN", "MAX", "ARRAY_AGG", "STRING_AGG", "JSON_AGG", "JSONB_AGG", "BOOL_AND", "BOOL_OR"];
const EXEMPT_PRECEDING: &[&str] = &["EXISTS", "IN", "ANY", "ALL", "SOME"];
const KEYWORDS: &[&str] = &["FROM", "WHERE", "AND", "OR", "SELECT", "AS", "ON", "NOT"];

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql787"
  }
  fn default_severity(&self) -> Severity {
    Severity::Warning
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, body, upper) = crate::stmt_body_upper(stmt, source);
    let ub = upper.as_bytes();
    let mut i = 0usize;
    while i < ub.len() {
      if ub[i] != b'(' {
        i += 1;
        continue;
      }
      let p = skip_ws(ub, i + 1);
      if !word_at(ub, p, b"SELECT") {
        i += 1;
        continue;
      }
      let Some(close) = match_paren(ub, i) else { break };
      if let Some(pw) = preceding_word(ub, body, i)
        && EXEMPT_PRECEDING.iter().any(|w| pw.eq_ignore_ascii_case(w))
      {
        i = close + 1;
        continue;
      }
      let sub_upper = &upper[i + 1..close];
      let sub_body = &body[i + 1..close];
      if contains_word(sub_upper, "JOIN") {
        i = close + 1;
        continue;
      }
      if AGGREGATES.iter().any(|a| contains_call(sub_upper, a)) {
        i = close + 1;
        continue;
      }
      if find_clause(sub_upper.as_bytes(), b"LIMIT").is_some() {
        i = close + 1;
        continue;
      }
      let own_alias = from_alias(sub_upper, sub_body);
      if qualified_ref_excluding(sub_body, sub_upper, own_alias.as_deref()).is_some() {
        out.push(Diagnostic {
          code: "sql787",
          severity: Severity::Warning,
          message: "correlated scalar subquery with no aggregate and no LIMIT 1 -- risks \"more than one row returned by a subquery used as an expression\" if more than one row matches".into(),
          range: crate::range_at(start + i, start + close + 1),
        });
      }
      i = close + 1;
    }
  }
}

/// The first FROM-clause table's alias (or name if unaliased).
fn from_alias(sub_upper: &str, sub_body: &str) -> Option<String> {
  let ub = sub_upper.as_bytes();
  let from_rel = find_clause(ub, b"FROM")?;
  let mut i = skip_ws(ub, from_rel + 4);
  let name_start = i;
  while i < ub.len() && is_word(ub[i] as char) {
    i += 1;
  }
  if i == name_start {
    return None;
  }
  let name = &sub_body[name_start..i];
  let mut j = skip_ws(ub, i);
  if sub_upper[j..].starts_with("AS") {
    j = skip_ws(ub, j + 2);
  }
  let alias_start = j;
  while j < ub.len() && is_word(ub[j] as char) {
    j += 1;
  }
  if j > alias_start {
    let word = &sub_upper[alias_start..j];
    const STOP: &[&str] = &["WHERE", "GROUP", "ORDER", "LIMIT", "HAVING", "UNION"];
    if !STOP.contains(&word) {
      return Some(sub_body[alias_start..j].to_ascii_lowercase());
    }
  }
  Some(name.to_ascii_lowercase())
}

/// First `ident.col` qualified reference in `sub_body` whose
/// identifier is NOT `own_alias` (case-insensitive), or None.
fn qualified_ref_excluding(sub_body: &str, sub_upper: &str, own_alias: Option<&str>) -> Option<usize> {
  let ub = sub_upper.as_bytes();
  let mut i = 0usize;
  while i < ub.len() {
    if !is_word(ub[i] as char) || (i > 0 && is_word(ub[i - 1] as char)) {
      i += 1;
      continue;
    }
    let start = i;
    while i < ub.len() && is_word(ub[i] as char) {
      i += 1;
    }
    if ub.get(i) == Some(&b'.') {
      let ident = &sub_body[start..i];
      let ident_upper = &sub_upper[start..i];
      let is_keyword = KEYWORDS.contains(&ident_upper);
      if !is_keyword && own_alias.is_none_or(|a| !ident.eq_ignore_ascii_case(a)) {
        return Some(start);
      }
    }
  }
  None
}

fn preceding_word<'a>(ub: &[u8], body: &'a str, before: usize) -> Option<&'a str> {
  let mut end = before;
  while end > 0 && ub[end - 1].is_ascii_whitespace() {
    end -= 1;
  }
  let word_end = end;
  while end > 0 && is_word(ub[end - 1] as char) {
    end -= 1;
  }
  if end == word_end {
    return None;
  }
  Some(&body[end..word_end])
}

fn contains_call(s: &str, name: &str) -> bool {
  let ub = s.as_bytes();
  let mut i = 0usize;
  while i + name.len() <= ub.len() {
    if word_at(ub, i, name.as_bytes()) {
      let after = skip_ws(ub, i + name.len());
      if ub.get(after) == Some(&b'(') {
        return true;
      }
    }
    i += 1;
  }
  false
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
