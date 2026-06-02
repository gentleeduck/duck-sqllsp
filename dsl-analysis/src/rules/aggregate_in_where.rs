//! sql424: `WHERE count(*) > 1` -- aggregate function in WHERE.
//! PG raises 42803 "aggregate functions are not allowed in WHERE";
//! the user almost certainly wanted HAVING (after a GROUP BY) or to
//! move the aggregate into a subquery.

use crate::clause_scan::{find_clause, find_clause_end, is_word};
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;
use text_size::TextRange;

pub struct Rule;

/// Known SQL aggregate function names (lowercase). Conservative list
/// -- false negatives are fine; false positives are not.
const AGGREGATES: &[&str] = &[
  "count",
  "sum",
  "avg",
  "min",
  "max",
  "array_agg",
  "string_agg",
  "json_agg",
  "jsonb_agg",
  "json_object_agg",
  "jsonb_object_agg",
  "bool_and",
  "bool_or",
  "every",
  "bit_and",
  "bit_or",
  "stddev",
  "stddev_pop",
  "stddev_samp",
  "variance",
  "var_pop",
  "var_samp",
  "regr_count",
  "regr_avgx",
  "regr_avgy",
  "regr_intercept",
  "regr_r2",
  "regr_slope",
  "regr_sxx",
  "regr_sxy",
  "regr_syy",
  "corr",
  "covar_pop",
  "covar_samp",
];

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql424"
  }
  fn default_severity(&self) -> Severity {
    Severity::Error
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, raw) = crate::stmt_body(stmt, source);
    let cleaned = crate::textutil::strip_noise_full(raw);
    let upper = cleaned.to_ascii_uppercase();
    let bytes_u = upper.as_bytes();
    let bytes = cleaned.as_bytes();
    let stopwords = ["GROUP BY", "ORDER BY", "LIMIT", "OFFSET", "HAVING", "FOR", "FETCH", "WINDOW", "RETURNING", "UNION", "INTERSECT", "EXCEPT"];
    // Walk WHERE, JOIN-ON, and GROUP BY clause bodies. HAVING is the
    // *correct* place for aggregates, so we don't scan it. Each body
    // uses the same paren-stack trick: aggregates inside a
    // (SELECT ...) / (WITH ...) subquery are PG-legal.
    let mut emitted_at: std::collections::HashSet<usize> = std::collections::HashSet::new();
    for (needle, label) in [(&b"WHERE"[..], "WHERE"), (&b"ON"[..], "JOIN ON"), (&b"GROUP BY"[..], "GROUP BY")] {
      let mut from = 0usize;
      while let Some(rel) = find_clause(&bytes_u[from..], needle).map(|p| p + from) {
        let pred_start = rel + needle.len();
        let pred_end = find_clause_end(bytes_u, pred_start, &stopwords);
        scan_pred(bytes, bytes_u, pred_start, pred_end, start, label, &mut emitted_at, out);
        from = pred_end.max(rel + needle.len());
      }
    }
  }
}

#[allow(clippy::too_many_arguments)]
fn scan_pred(
  bytes: &[u8],
  bytes_u: &[u8],
  pred_start: usize,
  pred_end: usize,
  start: usize,
  clause_label: &str,
  emitted_at: &mut std::collections::HashSet<usize>,
  out: &mut Vec<Diagnostic>,
) {
  let mut stack: Vec<bool> = Vec::new();
  let mut i = pred_start;
  while i < pred_end {
      let c = bytes[i];
      if c == b'\'' {
        i += 1;
        while i < pred_end && bytes[i] != b'\'' {
          i += 1;
        }
        i = (i + 1).min(pred_end);
        continue;
      }
      if c == b'(' {
        // Peek the first word inside.
        let mut j = i + 1;
        while j < pred_end && bytes[j].is_ascii_whitespace() {
          j += 1;
        }
        let is_subquery = (j + 6 <= pred_end && bytes_u[j..j + 6] == *b"SELECT" && (j + 6 == pred_end || !is_word(bytes_u[j + 6] as char)))
          || (j + 4 <= pred_end && bytes_u[j..j + 4] == *b"WITH" && (j + 4 == pred_end || !is_word(bytes_u[j + 4] as char)));
        stack.push(is_subquery);
        i += 1;
        continue;
      }
      if c == b')' {
        stack.pop();
        i += 1;
        continue;
      }
      // Check for `<aggregate>(`.
      if c.is_ascii_alphabetic() {
        let word_start = i;
        let mut k = i;
        while k < pred_end && (bytes[k].is_ascii_alphanumeric() || bytes[k] == b'_') {
          k += 1;
        }
        // Must be followed by `(` (allow no whitespace).
        let lookahead_paren = k < pred_end && bytes[k] == b'(';
        if lookahead_paren {
          let name = std::str::from_utf8(&bytes[word_start..k]).unwrap_or("").to_ascii_lowercase();
          if AGGREGATES.contains(&name.as_str())
            && !stack.iter().any(|&is_sub| is_sub)
            && emitted_at.insert(word_start)
          {
            // Find matching `)` for the call to underline.
            let mut depth = 1i32;
            let mut p = k + 1;
            while p < pred_end && depth > 0 {
              match bytes[p] {
                b'(' => depth += 1,
                b')' => depth -= 1,
                b'\'' => {
                  p += 1;
                  while p < pred_end && bytes[p] != b'\'' {
                    p += 1;
                  }
                },
                _ => {},
              }
              p += 1;
            }
            let abs_s = start + word_start;
            let abs_e = start + p;
            out.push(Diagnostic {
              code: "sql424",
              severity: Severity::Error,
              message: format!(
                "aggregate `{name}(...)` cannot appear in {clause_label} -- PG raises 42803 at parse time; use HAVING (with GROUP BY) or move the aggregate into a subquery"
              ),
              range: TextRange::new((abs_s as u32).into(), (abs_e as u32).into()),
            });
          }
        }
        i = k.max(i + 1);
        continue;
      }
      i += 1;
    }
}
