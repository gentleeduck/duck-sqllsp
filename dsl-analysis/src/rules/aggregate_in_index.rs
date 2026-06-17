//! sql654: an aggregate function in a `CREATE INDEX` expression, e.g.
//! `CREATE INDEX ON t (count(x))` or a partial-index predicate
//! `... WHERE sum(x) > 0`. An index expression is evaluated per row, so
//! PostgreSQL forbids aggregates and raises 42803 ("aggregate functions are not
//! allowed in index expressions" / predicates). Index a plain column or an
//! immutable scalar expression instead.

use crate::clause_scan::is_word;
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

const AGGREGATES: &[&str] = &[
  "count", "sum", "avg", "min", "max", "array_agg", "string_agg", "json_agg", "jsonb_agg", "bool_and",
  "bool_or", "every", "stddev", "stddev_pop", "stddev_samp", "variance", "var_pop", "var_samp", "bit_and",
  "bit_or", "corr",
];

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql654"
  }
  fn default_severity(&self) -> Severity {
    Severity::Error
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, raw) = crate::stmt_body(stmt, source);
    let cleaned = crate::textutil::strip_noise_full(raw);
    let upper = cleaned.to_ascii_uppercase();
    let trimmed = upper.trim_start();
    if !trimmed.starts_with("CREATE") || !upper.contains("INDEX") {
      return;
    }
    let lower = cleaned.to_ascii_lowercase();
    let lb = lower.as_bytes();
    let n = lb.len();
    // start after the `INDEX` keyword so an index *named* like an aggregate
    // (improbable, but cheap to exclude) can't trip the scan.
    let scan_from = lower.find("index").map(|p| p + 5).unwrap_or(0);
    let mut i = scan_from;
    while i < n {
      if (i == scan_from || !is_word(lb[i - 1] as char)) && is_word(lb[i] as char) {
        for &agg in AGGREGATES {
          let l = agg.len();
          if i + l <= n && &lb[i..i + l] == agg.as_bytes() && lb.get(i + l).is_none_or(|&c| !is_word(c as char)) {
            let mut p = i + l;
            while p < n && lb[p].is_ascii_whitespace() {
              p += 1;
            }
            if p < n && lb[p] == b'(' {
              out.push(Diagnostic {
                code: "sql654",
                severity: Severity::Error,
                message: format!("aggregate `{agg}` in a CREATE INDEX expression -- PG forbids this (42803); index a plain column or immutable expression"),
                range: crate::range_at(start + i, start + i + l),
              });
              return;
            }
          }
        }
      }
      i += 1;
    }
  }
}
