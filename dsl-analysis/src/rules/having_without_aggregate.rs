//! sql460: `SELECT id FROM t HAVING id > 5` -- HAVING without
//! GROUP BY and without any aggregate function in the predicate.
//! PG silently runs the predicate against the whole-table single
//! group, which yields the same set of rows as a plain WHERE but
//! after aggregation has (notionally) happened. Almost always a
//! typo of "WHERE", or a leftover from removing a GROUP BY without
//! relocating the predicate.

use crate::clause_scan::{find_clause, find_clause_end, is_word};
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;
use text_size::TextRange;

pub struct Rule;

const AGGREGATES: &[&[u8]] = &[
  b"COUNT", b"SUM", b"AVG", b"MIN", b"MAX",
  b"ARRAY_AGG", b"STRING_AGG", b"JSON_AGG", b"JSONB_AGG",
  b"JSON_OBJECT_AGG", b"JSONB_OBJECT_AGG",
  b"BOOL_AND", b"BOOL_OR", b"EVERY",
  b"BIT_AND", b"BIT_OR",
  b"STDDEV", b"STDDEV_POP", b"STDDEV_SAMP",
  b"VARIANCE", b"VAR_POP", b"VAR_SAMP",
  b"CORR", b"COVAR_POP", b"COVAR_SAMP",
];

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql460"
  }
  fn default_severity(&self) -> Severity {
    Severity::Warning
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, raw) = crate::stmt_body(stmt, source);
    let cleaned = crate::textutil::strip_noise_full(raw);
    let upper = cleaned.to_ascii_uppercase();
    let ub = upper.as_bytes();
    // Skip if GROUP BY appears anywhere -- legitimate aggregation
    // context.
    if find_clause(ub, b"GROUP BY").is_some() {
      return;
    }
    let Some(rel_having) = find_clause(ub, b"HAVING") else {
      return;
    };
    let pred_start = rel_having + 6;
    let stopwords = ["ORDER BY", "LIMIT", "OFFSET", "FOR", "FETCH", "WINDOW", "RETURNING", "UNION", "INTERSECT", "EXCEPT"];
    let pred_end = find_clause_end(ub, pred_start, &stopwords);
    let pred_upper = &ub[pred_start..pred_end];
    if contains_aggregate_call(pred_upper) {
      return;
    }
    let abs_s = start + rel_having;
    let abs_e = start + pred_end;
    out.push(Diagnostic {
      code: "sql460",
      severity: Severity::Warning,
      message: "HAVING without GROUP BY and without any aggregate function in the predicate -- this filters the implicit whole-table single group, which is equivalent to a plain WHERE. Did you mean `WHERE`?".into(),
      range: TextRange::new((abs_s as u32).into(), (abs_e as u32).into()),
    });
  }
}

fn contains_aggregate_call(upper: &[u8]) -> bool {
  let n = upper.len();
  for agg in AGGREGATES {
    let m = agg.len();
    let mut i = 0usize;
    while i + m <= n {
      if &upper[i..i + m] == *agg
        && (i == 0 || !is_word(upper[i - 1] as char))
      {
        let mut k = i + m;
        while k < n && upper[k].is_ascii_whitespace() {
          k += 1;
        }
        if k < n && upper[k] == b'(' {
          return true;
        }
      }
      i += 1;
    }
  }
  false
}
