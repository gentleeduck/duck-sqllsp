//! sql522: `a LEFT JOIN b ON ... WHERE b.col = 'x'` -- a positive WHERE
//! predicate on the *nullable* (right) side of a LEFT JOIN silently turns it
//! into an INNER JOIN: the NULL-extended rows from unmatched left rows fail
//! the filter and disappear. Almost always a bug -- either the condition
//! belongs in the ON clause (to keep it an outer join) or the join should be
//! an explicit INNER JOIN.
//!
//! Conservative: only a conjunct that *begins* with `alias.col <predicate>`
//! is flagged, and any conjunct mentioning NULL (the legitimate
//! `b.col IS NULL` anti-join / `... OR b.col IS NULL` guard) or containing a
//! top-level OR is skipped, so the idiomatic outer-join-preserving forms
//! never fire.

use crate::clause_scan::{find_clause, find_clause_end, is_word};
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

const STOPWORDS: &[&str] =
  &["GROUP", "ORDER", "HAVING", "LIMIT", "OFFSET", "WINDOW", "RETURNING", "UNION", "INTERSECT", "EXCEPT", "FETCH", "FOR"];

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql522"
  }
  fn default_severity(&self) -> Severity {
    Severity::Warning
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, body, upper) = crate::stmt_body_upper(stmt, source);
    let ub = upper.as_bytes();

    let aliases = left_join_aliases(ub, body);
    if aliases.is_empty() {
      return;
    }
    let Some(where_at) = find_clause(ub, b"WHERE") else { return };
    let pred_start = where_at + 5;
    let pred_end = find_clause_end(ub, pred_start, STOPWORDS);

    for (s, e) in split_on_and(body, pred_start, pred_end) {
      let raw = &body[s..e];
      let conj = raw.trim();
      let conj_u = upper[s..e].trim();
      // Skip null-guarded conjuncts and disjunctions outright.
      if conj_u.contains("NULL") || has_top_level_or(conj) {
        continue;
      }
      let Some(alias) = leading_alias_predicate(conj, &aliases) else { continue };
      let span_s = s + (raw.len() - raw.trim_start().len());
      let span_e = s + raw.trim_end().len();
      out.push(Diagnostic {
        code: "sql522",
        severity: Severity::Warning,
        message: format!(
          "`{alias}` is the nullable side of a LEFT JOIN, but this WHERE filter on it forces \
           an INNER JOIN -- move the condition into the ON clause or use INNER JOIN"
        ),
        range: crate::range_at(start + span_s, start + span_e),
      });
    }
  }
}

/// Aliases (lowercased) of the right-hand table of every `LEFT [OUTER] JOIN`:
/// the identifier immediately preceding that join's `ON`.
fn left_join_aliases(ub: &[u8], body: &str) -> Vec<String> {
  let n = ub.len();
  let mut out = Vec::new();
  let mut i = 0usize;
  while i < n {
    if !word_at(ub, i, b"LEFT") {
      i += 1;
      continue;
    }
    let mut p = skip_ws(ub, i + 4);
    if word_at(ub, p, b"OUTER") {
      p = skip_ws(ub, p + 5);
    }
    if !word_at(ub, p, b"JOIN") {
      i += 4;
      continue;
    }
    let after_join = p + 4;
    // First depth-0 ON belonging to this join.
    if let Some(on_rel) = find_clause(&ub[after_join..], b"ON") {
      let on_at = after_join + on_rel;
      if let Some(alias) = ident_before(body, on_at) {
        out.push(alias.to_ascii_lowercase());
      }
      i = on_at + 2;
    } else {
      i = after_join;
    }
  }
  out
}

/// Read the identifier ending just before `at` (skipping whitespace).
fn ident_before(body: &str, at: usize) -> Option<&str> {
  let bytes = body.as_bytes();
  let mut end = at;
  while end > 0 && bytes[end - 1].is_ascii_whitespace() {
    end -= 1;
  }
  let mut start = end;
  while start > 0 && is_word(bytes[start - 1] as char) {
    start -= 1;
  }
  if start == end {
    return None;
  }
  Some(&body[start..end])
}

/// True if `conj` begins with `alias.col` followed by a comparison operator
/// or `IN` / `LIKE` / `BETWEEN` / `IS` -- a positive predicate on the alias.
fn leading_alias_predicate(conj: &str, aliases: &[String]) -> Option<String> {
  let bytes = conj.as_bytes();
  // Read the qualifier identifier.
  let mut i = 0usize;
  while i < bytes.len() && is_word(bytes[i] as char) {
    i += 1;
  }
  if i == 0 || bytes.get(i) != Some(&b'.') {
    return None;
  }
  let qual = conj[..i].to_ascii_lowercase();
  if !aliases.contains(&qual) {
    return None;
  }
  // Skip the column identifier after the dot.
  let mut j = i + 1;
  while j < bytes.len() && (is_word(bytes[j] as char) || bytes[j] == b'"') {
    j += 1;
  }
  if j == i + 1 {
    return None;
  }
  let rest = conj[j..].trim_start();
  let rb = rest.as_bytes();
  let is_predicate = matches!(rb.first(), Some(b'=' | b'<' | b'>' | b'!'))
    || starts_word(rest, "IN")
    || starts_word(rest, "LIKE")
    || starts_word(rest, "ILIKE")
    || starts_word(rest, "BETWEEN")
    || starts_word(rest, "IS")
    || starts_word(rest, "SIMILAR");
  if is_predicate { Some(qual) } else { None }
}

fn starts_word(s: &str, kw: &str) -> bool {
  let u = s.as_bytes();
  let k = kw.as_bytes();
  u.len() >= k.len()
    && u[..k.len()].eq_ignore_ascii_case(k)
    && u.get(k.len()).is_none_or(|&b| !is_word(b as char))
}

fn has_top_level_or(s: &str) -> bool {
  let bytes = s.as_bytes();
  let mut depth = 0i32;
  let mut i = 0usize;
  while i < bytes.len() {
    match bytes[i] {
      b'(' | b'[' => depth += 1,
      b')' | b']' => depth -= 1,
      b'\'' => {
        i += 1;
        while i < bytes.len() && bytes[i] != b'\'' {
          i += 1;
        }
      },
      b'O' | b'o' if depth == 0 => {
        let is_or = bytes.get(i + 1).is_some_and(|b| b.eq_ignore_ascii_case(&b'R'));
        let prev_ok = i == 0 || !is_word(bytes[i - 1] as char);
        let next_ok = bytes.get(i + 2).is_none_or(|&b| !is_word(b as char));
        if is_or && prev_ok && next_ok {
          return true;
        }
      },
      _ => {},
    }
    i += 1;
  }
  false
}

fn split_on_and(body: &str, from: usize, to: usize) -> Vec<(usize, usize)> {
  let bytes = body.as_bytes();
  let mut out = Vec::new();
  let mut depth = 0i32;
  let mut last = from;
  let mut i = from;
  while i < to {
    match bytes[i] {
      b'(' | b'[' => depth += 1,
      b')' | b']' => depth -= 1,
      b'\'' => {
        i += 1;
        while i < to && bytes[i] != b'\'' {
          i += 1;
        }
      },
      b'A' | b'a' if depth == 0 => {
        let is_and = i + 3 <= to && body[i..i + 3].eq_ignore_ascii_case("AND");
        let prev_ok = i == from || !is_word(bytes[i - 1] as char);
        let next_ok = bytes.get(i + 3).is_none_or(|&b| !is_word(b as char));
        if is_and && prev_ok && next_ok {
          out.push((last, i));
          i += 3;
          last = i;
          continue;
        }
      },
      _ => {},
    }
    i += 1;
  }
  out.push((last, to));
  out
}

fn word_at(ub: &[u8], i: usize, kw: &[u8]) -> bool {
  i + kw.len() <= ub.len()
    && ub[i..i + kw.len()] == *kw
    && (i == 0 || !is_word(ub[i - 1] as char))
    && (i + kw.len() == ub.len() || !is_word(ub[i + kw.len()] as char))
}

fn skip_ws(ub: &[u8], mut i: usize) -> usize {
  while i < ub.len() && ub[i].is_ascii_whitespace() {
    i += 1;
  }
  i
}
