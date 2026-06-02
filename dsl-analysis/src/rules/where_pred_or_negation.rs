//! sql455: `WHERE X OR NOT X` -- a predicate ORed with its own
//! negation is a tautology (always TRUE for non-NULL X; NULL when X
//! is NULL, which WHERE then drops). Either the user meant a
//! different second branch, or the whole `OR` clause should be
//! removed. Mirror of sql422 (the AND-version that yields always-
//! FALSE).

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
    "sql455"
  }
  fn default_severity(&self) -> Severity {
    Severity::Warning
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, raw) = crate::stmt_body(stmt, source);
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
    let or_clauses = split_top_level_or(pred);
    if or_clauses.len() < 2 {
      return;
    }
    let mut positive: HashSet<String> = HashSet::new();
    let mut negative: HashSet<String> = HashSet::new();
    for c in &or_clauses {
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
            code: "sql455",
            severity: Severity::Warning,
            message: format!(
              "`{body}` and `NOT {body}` both appear in WHERE (OR-combined) -- the disjunction is always TRUE (NULL for NULL input); the OR has no filter effect"
            ),
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
            code: "sql455",
            severity: Severity::Warning,
            message: format!(
              "`{body}` and `NOT {body}` both appear in WHERE (OR-combined) -- the disjunction is always TRUE (NULL for NULL input); the OR has no filter effect"
            ),
            range: TextRange::new((abs_s as u32).into(), (abs_e as u32).into()),
          });
          return;
        }
        positive.insert(key);
      }
    }
  }
}

fn strip_not_prefix(s: &str) -> (bool, &str) {
  let t = s.trim();
  if t.len() >= 4 && t[..4].eq_ignore_ascii_case("NOT ") {
    return (true, t[4..].trim_start());
  }
  if t.len() >= 3 && t[..3].eq_ignore_ascii_case("NOT") {
    let after = &t[3..];
    if after.starts_with('(') {
      return (true, after);
    }
  }
  (false, t)
}

fn canon(s: &str) -> String {
  s.trim().trim_matches(|c: char| c == '(' || c == ')' || c.is_whitespace()).split_whitespace().collect::<Vec<_>>().join(" ").to_ascii_lowercase()
}

fn split_top_level_or(s: &str) -> Vec<String> {
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
      && i + 2 <= n
      && &ub[i..i + 2] == b"OR"
      && (i == 0 || !is_word(ub[i - 1] as char))
      && (i + 2 == n || !is_word(ub[i + 2] as char))
    {
      out.push(s[last..i].to_string());
      last = i + 2;
      i += 2;
      continue;
    }
    i += 1;
  }
  out.push(s[last..].to_string());
  out
}
