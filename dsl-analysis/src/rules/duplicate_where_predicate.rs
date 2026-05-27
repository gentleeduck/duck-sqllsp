//! sql421: `WHERE age > 0 AND age > 0` -- duplicate conjunct.
//! Splits the WHERE predicate on top-level AND/OR (paren-aware),
//! normalizes each piece (strip outer parens, collapse whitespace,
//! lowercase), and flags repeats. The dup is wasted parse/plan work
//! and is almost always a copy-paste typo for two distinct
//! predicates.

use crate::clause_scan::{find_clause, find_clause_end, is_word};
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;
use std::collections::HashSet;
use text_size::TextRange;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql421"
  }
  fn default_severity(&self) -> Severity {
    Severity::Hint
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let start: usize = u32::from(stmt.range.start()) as usize;
    let end: usize = (u32::from(stmt.range.end()) as usize).min(source.len());
    let raw = &source[start..end];
    let cleaned = crate::textutil::strip_noise_full(raw);
    let upper = cleaned.to_ascii_uppercase();
    let bytes_u = upper.as_bytes();
    let stopwords = ["GROUP BY", "ORDER BY", "LIMIT", "OFFSET", "HAVING", "FOR", "FETCH", "WINDOW", "RETURNING", "UNION", "INTERSECT", "EXCEPT"];

    let Some(rel_where) = find_clause(bytes_u, b"WHERE") else {
      return;
    };
    let pred_start = rel_where + 5;
    let pred_end = find_clause_end(bytes_u, pred_start, &stopwords);
    let pred = &cleaned[pred_start..pred_end];
    let conjuncts = split_top_level_and_or(pred);
    if conjuncts.len() < 2 {
      return;
    }
    let mut seen: HashSet<String> = HashSet::new();
    let mut emitted: HashSet<String> = HashSet::new();
    for c in &conjuncts {
      let key = canon(c);
      if key.is_empty() {
        continue;
      }
      if !seen.insert(key.clone()) && emitted.insert(key.clone()) {
        let abs_s = start + rel_where;
        let abs_e = start + pred_end;
        out.push(Diagnostic {
          code: "sql421",
          severity: Severity::Hint,
          message: format!("predicate `{}` appears more than once in WHERE -- the repeat is redundant", c.trim()),
          range: TextRange::new((abs_s as u32).into(), (abs_e as u32).into()),
        });
      }
    }
  }
}

fn split_top_level_and_or(s: &str) -> Vec<String> {
  let bytes = s.as_bytes();
  let upper: String = s.to_ascii_uppercase();
  let ub = upper.as_bytes();
  let n = bytes.len();
  let mut out: Vec<String> = Vec::new();
  let mut last = 0usize;
  let mut depth: i32 = 0;
  let mut i = 0;
  while i < n {
    let c = bytes[i];
    if c == b'\'' {
      i += 1;
      while i < n && bytes[i] != b'\'' {
        i += 1;
      }
      i = (i + 1).min(n);
      continue;
    }
    if c == b'(' {
      depth += 1;
      i += 1;
      continue;
    }
    if c == b')' {
      depth -= 1;
      i += 1;
      continue;
    }
    if depth == 0 {
      // AND (3 chars)
      if i + 3 <= n
        && &ub[i..i + 3] == b"AND"
        && (i == 0 || !is_word(ub[i - 1] as char))
        && (i + 3 == n || !is_word(ub[i + 3] as char))
      {
        out.push(s[last..i].to_string());
        last = i + 3;
        i += 3;
        continue;
      }
      // OR (2 chars)
      if i + 2 <= n
        && &ub[i..i + 2] == b"OR"
        && (i == 0 || !is_word(ub[i - 1] as char))
        && (i + 2 == n || !is_word(ub[i + 2] as char))
      {
        out.push(s[last..i].to_string());
        last = i + 2;
        i += 2;
        continue;
      }
    }
    i += 1;
  }
  out.push(s[last..].to_string());
  out
}

fn canon(s: &str) -> String {
  let trimmed = s.trim().trim_matches(|c: char| c == '(' || c == ')' || c.is_whitespace());
  trimmed.split_whitespace().collect::<Vec<_>>().join(" ").to_ascii_lowercase()
}
