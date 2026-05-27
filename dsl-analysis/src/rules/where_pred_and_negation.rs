//! sql422: `WHERE X AND NOT X` -- the same predicate AND its
//! negation is always false; the query returns zero rows. Almost
//! certainly a typo or unfinished refactor.

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
    "sql422"
  }
  fn default_severity(&self) -> Severity {
    Severity::Warning
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
    let conjuncts = split_top_level_and(pred);
    if conjuncts.len() < 2 {
      return;
    }
    // Normalize each conjunct to (negated?, canonical-body).
    let mut positive: HashSet<String> = HashSet::new();
    let mut negative: HashSet<String> = HashSet::new();
    for c in &conjuncts {
      let (neg, body) = strip_not_prefix(c.trim());
      let key = canon(body);
      if key.is_empty() {
        continue;
      }
      if neg {
        if positive.contains(&key) {
          let abs_s = start + rel_where;
          let abs_e = start + pred_end;
          out.push(Diagnostic {
            code: "sql422",
            severity: Severity::Warning,
            message: format!("`{body}` and `NOT {body}` both appear in WHERE -- the conjunction is always false, query returns zero rows"),
            range: TextRange::new((abs_s as u32).into(), (abs_e as u32).into()),
          });
          return;
        }
        negative.insert(key);
      } else {
        if negative.contains(&key) {
          let abs_s = start + rel_where;
          let abs_e = start + pred_end;
          out.push(Diagnostic {
            code: "sql422",
            severity: Severity::Warning,
            message: format!("`{body}` and `NOT {body}` both appear in WHERE -- the conjunction is always false, query returns zero rows"),
            range: TextRange::new((abs_s as u32).into(), (abs_e as u32).into()),
          });
          return;
        }
        positive.insert(key);
      }
    }
  }
}

/// Pull a leading `NOT ` off a predicate text. Returns (was_negated,
/// remainder).
fn strip_not_prefix(s: &str) -> (bool, &str) {
  let t = s.trim();
  if t.len() >= 4 && t[..4].eq_ignore_ascii_case("NOT ")
    && (t.as_bytes().get(3).map(|b| b.is_ascii_whitespace()).unwrap_or(false))
  {
    return (true, t[4..].trim_start());
  }
  if t.len() >= 3 && t[..3].eq_ignore_ascii_case("NOT") {
    // `NOT(` form -- check if next is `(`.
    let after = &t[3..];
    if after.starts_with('(') {
      return (true, after);
    }
  }
  (false, t)
}

fn canon(s: &str) -> String {
  s.trim().trim_matches(|c: char| c == '(' || c == ')' || c.is_whitespace())
    .split_whitespace().collect::<Vec<_>>().join(" ").to_ascii_lowercase()
}

fn split_top_level_and(s: &str) -> Vec<String> {
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
    if depth == 0
      && i + 3 <= n
      && &ub[i..i + 3] == b"AND"
      && (i == 0 || !is_word(ub[i - 1] as char))
      && (i + 3 == n || !is_word(ub[i + 3] as char))
    {
      out.push(s[last..i].to_string());
      last = i + 3;
      i += 3;
      continue;
    }
    i += 1;
  }
  out.push(s[last..].to_string());
  out
}
