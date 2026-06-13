//! sql535: `WHERE a <> 1 AND a <> 2 AND a <> 3` -- a chain of not-equal tests
//! on the same column, AND-ed together. Equivalent to `a NOT IN (1, 2, 3)`,
//! which is shorter and clearer. The mirror of sql519 (`= ... OR` -> `IN`).
//! Fires at 3+ values on one column to stay quiet on trivial two-way chains.

use crate::clause_scan::{find_clause, find_clause_end, is_word, parse_simple_ident};
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;
use std::collections::HashMap;

const STOPWORDS: &[&str] =
  &["GROUP", "ORDER", "HAVING", "LIMIT", "OFFSET", "WINDOW", "RETURNING", "UNION", "INTERSECT", "EXCEPT", "FETCH", "FOR"];

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql535"
  }
  fn default_severity(&self) -> Severity {
    Severity::Hint
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, body, upper) = crate::stmt_body_upper(stmt, source);
    let ub = upper.as_bytes();

    for needle in [&b"WHERE"[..], &b"ON"[..], &b"HAVING"[..]] {
      let mut from = 0usize;
      while let Some(rel) = find_clause(&ub[from..], needle).map(|p| p + from) {
        let ps = rel + needle.len();
        let pe = find_clause_end(ub, ps, STOPWORDS);
        scan(body, start, ps, pe, out);
        from = pe.max(ps);
      }
    }
  }
}

fn scan(body: &str, abs: usize, from: usize, to: usize, out: &mut Vec<Diagnostic>) {
  let mut groups: HashMap<String, (usize, usize, usize)> = HashMap::new();
  for (seg_start, seg_end) in split_on_and(body, from, to) {
    let term = &body[seg_start..seg_end];
    let Some(key) = as_col_neq(term) else { continue };
    let lead = term.len() - term.trim_start().len();
    let s = seg_start + lead;
    let e = seg_start + term.trim_end().len();
    let entry = groups.entry(key).or_insert((0, s, e));
    entry.0 += 1;
    entry.1 = entry.1.min(s);
    entry.2 = entry.2.max(e);
  }
  for (key, (count, lo, hi)) in groups {
    if count >= 3 {
      out.push(Diagnostic {
        code: "sql535",
        severity: Severity::Hint,
        message: format!("`{key}` is not-equal-compared to {count} values with AND -- use `{key} NOT IN (...)` instead"),
        range: crate::range_at(abs + lo, abs + hi),
      });
    }
  }
}

/// If `term` is `<simple_ident> {<> | !=} <value>` (value free of top-level
/// OR), return the lowercased `qual.name` key.
fn as_col_neq(term: &str) -> Option<String> {
  let bytes = term.as_bytes();
  let mut depth = 0i32;
  let mut i = 0usize;
  let mut op = None;
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
      b'<' if depth == 0 && bytes.get(i + 1) == Some(&b'>') => {
        op = Some((i, 2));
        break;
      },
      b'!' if depth == 0 && bytes.get(i + 1) == Some(&b'=') => {
        op = Some((i, 2));
        break;
      },
      _ => {},
    }
    i += 1;
  }
  let (pos, len) = op?;
  let (lhs, rhs) = (term[..pos].trim(), term[pos + len..].trim());
  let (qual, name) = parse_simple_ident(lhs)?;
  if rhs.is_empty() || rhs.to_ascii_uppercase().contains(" OR ") {
    return None;
  }
  Some(match qual {
    Some(q) => format!("{}.{}", q.to_ascii_lowercase(), name.to_ascii_lowercase()),
    None => name.to_ascii_lowercase(),
  })
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
