//! sql441: `WHERE EXISTS (SELECT 1 FROM other_table)` -- the inner
//! subquery does not reference any column from the OUTER statement,
//! so the EXISTS is uncorrelated and degenerates to "does
//! `other_table` have any rows" (a single boolean check repeated for
//! every outer row). Almost always a typo (forgot the join
//! predicate), an unfinished refactor, or a misuse of EXISTS where
//! `LIMIT 1` would do.
//!
//! Heuristic: walk each top-level WHERE `EXISTS (...)` /
//! `NOT EXISTS (...)`. If the subquery body contains no token
//! matching `<outer_alias>.` (case-insensitive) for any outer
//! binding's alias OR table name, flag.

use crate::clause_scan::{find_clause, find_clause_end, is_word};
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;
use text_size::TextRange;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql441"
  }
  fn default_severity(&self) -> Severity {
    Severity::Warning
  }

  fn check(&self, source: &str, stmt: &Statement, scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    if scope.is_empty() {
      return;
    }
    let start: usize = u32::from(stmt.range.start()) as usize;
    let end: usize = (u32::from(stmt.range.end()) as usize).min(source.len());
    let raw = &source[start..end];
    let cleaned = crate::textutil::strip_noise_full(raw);
    let upper = cleaned.to_ascii_uppercase();
    let ub = upper.as_bytes();
    let bytes = cleaned.as_bytes();
    let stopwords =
      ["GROUP BY", "ORDER BY", "LIMIT", "OFFSET", "HAVING", "FOR", "FETCH", "WINDOW", "RETURNING", "UNION", "INTERSECT", "EXCEPT"];
    let Some(rel_where) = find_clause(ub, b"WHERE") else {
      return;
    };
    let pred_start = rel_where + 5;
    let pred_end = find_clause_end(ub, pred_start, &stopwords);
    // Collect outer alias / table-name keys, lower-cased + a trailing
    // `.` (the qualifier shape we'll look for).
    let outer_keys: Vec<String> = scope.bindings.keys().map(|k| format!("{}.", k.to_ascii_lowercase())).collect();
    if outer_keys.is_empty() {
      return;
    }

    let mut i = pred_start;
    while i + 6 <= pred_end {
      // Word-bounded EXISTS, optionally preceded by NOT.
      if &ub[i..i + 6] == b"EXISTS"
        && (i == 0 || !is_word(ub[i - 1] as char))
        && (i + 6 == pred_end || !is_word(ub[i + 6] as char))
      {
        let mut k = i + 6;
        while k < pred_end && bytes[k].is_ascii_whitespace() {
          k += 1;
        }
        if k >= pred_end || bytes[k] != b'(' {
          i += 6;
          continue;
        }
        let Some(close_rel) = match_paren(bytes, k, pred_end) else {
          i += 6;
          continue;
        };
        let sub = &cleaned[k + 1..close_rel];
        let sub_lower = sub.to_ascii_lowercase();
        let is_correlated = outer_keys.iter().any(|key| token_contains(&sub_lower, key));
        if !is_correlated {
          let abs_s = start + i;
          let abs_e = start + close_rel + 1;
          out.push(Diagnostic {
            code: "sql441",
            severity: Severity::Warning,
            message:
              "uncorrelated EXISTS -- the subquery does not reference any column from the outer query, so it degenerates to \"does this table have any rows\"; either add the join predicate (e.g. `WHERE inner.fk = outer.id`) or rewrite as a single boolean: `(SELECT count(*) FROM ... LIMIT 1) > 0`".into(),
            range: TextRange::new((abs_s as u32).into(), (abs_e as u32).into()),
          });
        }
        i = close_rel + 1;
        continue;
      }
      i += 1;
    }
  }
}

/// True when `key` appears in `s` at a position where the preceding
/// character is NOT word-shaped (so we don't false-fire on
/// `some_users.col` matching key `users.`).
fn token_contains(s: &str, key: &str) -> bool {
  let bs = s.as_bytes();
  let bk = key.as_bytes();
  let n = bs.len();
  let m = bk.len();
  if m == 0 || m > n {
    return false;
  }
  let mut i = 0;
  while i + m <= n {
    if &bs[i..i + m] == bk && (i == 0 || !is_word(bs[i - 1] as char)) {
      return true;
    }
    i += 1;
  }
  false
}

fn match_paren(bytes: &[u8], open: usize, end: usize) -> Option<usize> {
  let mut depth: i32 = 0;
  let mut i = open;
  while i < end {
    let c = bytes[i];
    if c == b'\'' {
      i += 1;
      while i < end && bytes[i] != b'\'' {
        i += 1;
      }
      i = (i + 1).min(end);
      continue;
    }
    if c == b'(' {
      depth += 1;
    } else if c == b')' {
      depth -= 1;
      if depth == 0 {
        return Some(i);
      }
    }
    i += 1;
  }
  None
}
