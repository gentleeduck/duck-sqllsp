//! sql492: `col NOT IN (..., NULL, ...)` -- a NULL anywhere in the
//! list of a NOT IN predicate makes the entire predicate evaluate
//! to NULL for every row (because the desugared form is
//! `col <> v1 AND col <> v2 AND col <> NULL`, and the last conjunct
//! is NULL, so the whole AND is NULL). NULL in WHERE is filtered
//! out, so the query returns ZERO rows -- regardless of `col`'s
//! actual values. Classic gotcha.
//!
//! Also flags `col IN (NULL)` as a sole element (always NULL -> 0 rows).

use crate::clause_scan::{find_clause, find_clause_end, is_word};
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;
use text_size::TextRange;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql492"
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
    let ub = upper.as_bytes();
    let bytes = cleaned.as_bytes();
    let stopwords = ["GROUP BY", "ORDER BY", "HAVING", "LIMIT", "OFFSET", "FOR", "FETCH", "WINDOW", "RETURNING", "UNION", "INTERSECT", "EXCEPT"];
    let Some(rel_where) = find_clause(ub, b"WHERE") else {
      return;
    };
    let pred_start = rel_where + 5;
    let pred_end = find_clause_end(ub, pred_start, &stopwords).min(ub.len());
    let mut emitted: std::collections::HashSet<usize> = std::collections::HashSet::new();
    let mut i = pred_start;
    while i < pred_end {
      // Look for `NOT IN` or `IN` followed by `(`.
      let (is_not, kw_end) = if i + 6 <= pred_end
        && &ub[i..i + 3] == b"NOT"
        && (i == 0 || !is_word(ub[i - 1] as char))
        && ub[i + 3].is_ascii_whitespace()
      {
        // Skip whitespace after NOT, expect IN.
        let mut k = i + 3;
        while k < pred_end && ub[k].is_ascii_whitespace() {
          k += 1;
        }
        if k + 2 <= pred_end && &ub[k..k + 2] == b"IN" && (k + 2 == pred_end || !is_word(ub[k + 2] as char)) {
          (true, k + 2)
        } else {
          i += 1;
          continue;
        }
      } else if i + 2 <= pred_end
        && &ub[i..i + 2] == b"IN"
        && (i == 0 || !is_word(ub[i - 1] as char))
        && (i + 2 == pred_end || !is_word(ub[i + 2] as char))
      {
        (false, i + 2)
      } else {
        i += 1;
        continue;
      };
      let pre_start = i;
      // Skip whitespace then expect `(`.
      let mut k = kw_end;
      while k < pred_end && bytes[k].is_ascii_whitespace() {
        k += 1;
      }
      if k >= pred_end || bytes[k] != b'(' {
        i += 1;
        continue;
      }
      let Some(close) = match_paren(bytes, k, pred_end) else {
        i += 1;
        continue;
      };
      let inner_start = k + 1;
      let inner_end = close;
      // Don't flag IN (SELECT ...) -- only literal lists.
      let inner_upper: String = cleaned[inner_start..inner_end].to_ascii_uppercase();
      if inner_upper.trim_start().starts_with("SELECT")
        || inner_upper.trim_start().starts_with("VALUES")
        || inner_upper.trim_start().starts_with("WITH")
      {
        i = close + 1;
        continue;
      }
      // Split items on top-level commas and check for NULL literal.
      let items: Vec<&str> = split_top_commas(&cleaned[inner_start..inner_end]);
      let has_null = items.iter().any(|s| s.trim().eq_ignore_ascii_case("NULL"));
      if has_null && emitted.insert(pre_start) {
        let abs_s = start + pre_start;
        let abs_e = start + close + 1;
        let msg = if is_not {
          "`NOT IN (..., NULL, ...)` returns ZERO rows for every input -- the desugared form contains `col <> NULL` which is NULL, so the whole AND collapses to NULL (filtered out). Strip NULL from the list and add `col IS NOT NULL` explicitly if needed."
        } else if items.len() == 1 {
          "`IN (NULL)` is `col = NULL`, which is NULL (never TRUE) -- the query returns ZERO rows. Use `col IS NULL` to find NULLs, or `col IS NOT NULL` to exclude them."
        } else {
          // IN (..., NULL, ...) with multiple items is less broken
          // (NULL is dead-code in the OR) but still confusing.
          "`IN (..., NULL, ...)` -- NULL is dead-code in an IN list (it never matches via `=`). Use `col IS NULL OR col IN (...)` if you actually want NULLs to qualify, or drop the NULL."
        };
        out.push(Diagnostic {
          code: "sql492",
          severity: if is_not || items.len() == 1 { Severity::Warning } else { Severity::Hint },
          message: msg.into(),
          range: TextRange::new((abs_s as u32).into(), (abs_e as u32).into()),
        });
      }
      i = close + 1;
    }
  }
}

fn split_top_commas(s: &str) -> Vec<&str> {
  let mut out = Vec::new();
  let bytes = s.as_bytes();
  let n = bytes.len();
  let mut depth: i32 = 0;
  let mut start = 0usize;
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
    if c == b'(' || c == b'[' {
      depth += 1;
    } else if c == b')' || c == b']' {
      depth -= 1;
    } else if c == b',' && depth == 0 {
      out.push(&s[start..i]);
      start = i + 1;
    }
    i += 1;
  }
  out.push(&s[start..n]);
  out
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
