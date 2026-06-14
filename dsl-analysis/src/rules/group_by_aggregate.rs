//! sql543: `GROUP BY count(*)` / `GROUP BY sum(x)` -- an aggregate function in
//! the GROUP BY list. Postgres rejects this at execution with 42803
//! ("aggregate functions are not allowed in GROUP BY"). Usually a confusion
//! with HAVING, or a column that was meant to be a plain expression.

use crate::clause_scan::{find_clause, find_clause_end, is_word, split_top_level};
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

const AGG_FNS: &[&str] = &[
  "count", "sum", "avg", "min", "max", "array_agg", "string_agg", "json_agg", "jsonb_agg", "bool_or", "bool_and",
  "every", "stddev", "stddev_pop", "stddev_samp", "variance", "var_pop", "var_samp",
];

const STOPWORDS: &[&str] = &["HAVING", "ORDER", "LIMIT", "OFFSET", "WINDOW", "UNION", "INTERSECT", "EXCEPT", "FETCH", "FOR"];

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql543"
  }
  fn default_severity(&self) -> Severity {
    Severity::Error
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, body, upper) = crate::stmt_body_upper(stmt, source);
    let ub = upper.as_bytes();

    let Some(g) = find_clause(ub, b"GROUP") else { return };
    // Require the BY of GROUP BY.
    let mut p = g + 5;
    while p < ub.len() && ub[p].is_ascii_whitespace() {
      p += 1;
    }
    if !(p + 2 <= ub.len() && &ub[p..p + 2] == b"BY") {
      return;
    }
    let gs = p + 2;
    let ge = find_clause_end(ub, gs, STOPWORDS);

    for (item, off) in split_top_level(&body[gs..ge]) {
      if first_aggregate(item) {
        let lead = item.len() - item.trim_start().len();
        out.push(Diagnostic {
          code: "sql543",
          severity: Severity::Error,
          message: "aggregate functions are not allowed in GROUP BY (PG error 42803) -- did you mean HAVING?".into(),
          range: crate::range_at(start + gs + off + lead, start + gs + off + item.trim_end().len()),
        });
      }
    }
  }
}

/// True if `item` contains an aggregate-function call (`<agg>(`).
fn first_aggregate(item: &str) -> bool {
  let lower = item.to_ascii_lowercase();
  let bytes = lower.as_bytes();
  for fname in AGG_FNS {
    let needle = format!("{fname}(");
    let mut from = 0usize;
    while let Some(rel) = lower[from..].find(&needle) {
      let at = from + rel;
      if at == 0 || !is_word(bytes[at - 1] as char) {
        return true;
      }
      from = at + needle.len();
    }
  }
  false
}
