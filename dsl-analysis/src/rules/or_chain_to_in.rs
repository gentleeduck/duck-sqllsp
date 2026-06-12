//! sql519: `WHERE a = 1 OR a = 2 OR a = 3` -- a chain of equality tests on
//! the same column, OR-ed together. Equivalent to `a IN (1, 2, 3)`, which is
//! shorter, clearer, and lets the planner build a single index probe / hash
//! lookup instead of evaluating each disjunct. Fires at 3+ values on one
//! column to stay quiet on trivial two-way ORs.
//!
//! Scoped to WHERE / ON / HAVING bodies; only clean `col = <value>` disjuncts
//! count, so a term carrying `AND`, an inequality, or a function on the LHS
//! breaks the run rather than producing a wrong suggestion.

use crate::clause_scan::{find_clause, find_clause_end, parse_simple_ident};
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
    "sql519"
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
        let pred_start = rel + needle.len();
        let pred_end = find_clause_end(ub, pred_start, STOPWORDS);
        scan_clause(body, start, pred_start, pred_end, out);
        from = pred_end.max(pred_start);
      }
    }
  }
}

fn scan_clause(body: &str, abs: usize, mut from: usize, mut to: usize, out: &mut Vec<Diagnostic>) {
  // Strip one balanced pair of outer parens so `WHERE (a=1 OR a=2 OR a=3)`
  // is seen as a top-level OR chain.
  let raw = &body[from..to];
  let trimmed = raw.trim();
  if trimmed.starts_with('(') && trimmed.ends_with(')') && wraps_whole(trimmed) {
    from += raw.find('(').unwrap() + 1;
    to = body[..to].rfind(')').unwrap();
  }

  // (col key) -> (count, min_start, max_end) over OR-split equality terms.
  let mut groups: HashMap<String, (usize, usize, usize)> = HashMap::new();
  for (seg_start, seg_end) in split_on_or(body, from, to) {
    let term = &body[seg_start..seg_end];
    let Some((key, _)) = as_col_eq(term) else { continue };
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
        code: "sql519",
        severity: Severity::Hint,
        message: format!("`{key}` is equality-compared to {count} values with OR -- use `{key} IN (...)` instead"),
        range: crate::range_at(abs + lo, abs + hi),
      });
    }
  }
}

/// True when the outermost `(` of `s` closes at the final `)`.
fn wraps_whole(s: &str) -> bool {
  let bytes = s.as_bytes();
  let mut depth = 0i32;
  for (i, &b) in bytes.iter().enumerate() {
    match b {
      b'(' => depth += 1,
      b')' => {
        depth -= 1;
        if depth == 0 {
          return i == bytes.len() - 1;
        }
      },
      _ => {},
    }
  }
  false
}

/// Yield `(start, end)` byte ranges of `body[from..to]` split on whole-word
/// `OR` at paren depth 0, outside single-quoted strings.
fn split_on_or(body: &str, from: usize, to: usize) -> Vec<(usize, usize)> {
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
      b'O' | b'o' if depth == 0 => {
        let is_or = bytes.get(i + 1).is_some_and(|b| b.eq_ignore_ascii_case(&b'R'));
        let prev_ok = i == from || !crate::clause_scan::is_word(bytes[i - 1] as char);
        let next_ok = bytes.get(i + 2).is_none_or(|&b| !crate::clause_scan::is_word(b as char));
        if is_or && prev_ok && next_ok {
          out.push((last, i));
          i += 2;
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

/// If `term` is `<simple_ident> = <value>` (value free of top-level AND),
/// return the lowercased `qual.name` key and the value text.
fn as_col_eq(term: &str) -> Option<(String, String)> {
  let bytes = term.as_bytes();
  let mut depth = 0i32;
  let mut i = 0usize;
  let mut eq = None;
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
      b'=' if depth == 0 => {
        let prev = if i > 0 { bytes[i - 1] } else { b' ' };
        let next = bytes.get(i + 1).copied().unwrap_or(b' ');
        if !matches!(prev, b'<' | b'>' | b'!' | b':') && next != b'=' {
          eq = Some(i);
          break;
        }
      },
      _ => {},
    }
    i += 1;
  }
  let eq = eq?;
  let (lhs, rhs) = (term[..eq].trim(), term[eq + 1..].trim());
  let (qual, name) = parse_simple_ident(lhs)?;
  if rhs.is_empty() || rhs.to_ascii_uppercase().contains(" AND ") {
    return None;
  }
  let key = match qual {
    Some(q) => format!("{}.{}", q.to_ascii_lowercase(), name.to_ascii_lowercase()),
    None => name.to_ascii_lowercase(),
  };
  Some((key, rhs.to_string()))
}
