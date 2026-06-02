//! sql428: `MAX(*) / SUM(*) / AVG(*)` etc. -- only `count(*)` and
//! `count_if(*)`-style aggregates accept `*`. PG rejects e.g.
//! `function max(*) does not exist`; we surface this at parse time.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;
use text_size::TextRange;

pub struct Rule;

/// Aggregates that DO accept a column expression (no `*` form).
/// `count` is the only built-in that takes `*` legally.
const NON_COUNT_AGGREGATES: &[&str] = &[
  "sum",
  "avg",
  "min",
  "max",
  "array_agg",
  "string_agg",
  "json_agg",
  "jsonb_agg",
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
];

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql428"
  }
  fn default_severity(&self) -> Severity {
    Severity::Error
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, body) = crate::stmt_body(stmt, source);
    let lower = body.to_ascii_lowercase();
    let bytes = body.as_bytes();
    for &fname in NON_COUNT_AGGREGATES {
      let needle = format!("{fname}(");
      let mut from = 0usize;
      while let Some(rel) = lower[from..].find(&needle) {
        let at = from + rel;
        if at > 0 {
          let prev = bytes[at - 1] as char;
          if prev.is_ascii_alphanumeric() || prev == '_' {
            from = at + needle.len();
            continue;
          }
        }
        // Body should be exactly `*` (with optional whitespace).
        let open = at + needle.len();
        let mut j = open;
        while j < bytes.len() && bytes[j].is_ascii_whitespace() {
          j += 1;
        }
        if j < bytes.len() && bytes[j] == b'*' {
          let mut k = j + 1;
          while k < bytes.len() && bytes[k].is_ascii_whitespace() {
            k += 1;
          }
          if k < bytes.len() && bytes[k] == b')' {
            out.push(Diagnostic {
              code: "sql428",
              severity: Severity::Error,
              message: format!(
                "`{fname}(*)` is invalid -- only `count(*)` accepts `*`; PG raises `function {fname}(*) does not exist`"
              ),
              range: TextRange::new(((start + at) as u32).into(), ((start + k + 1) as u32).into()),
            });
            from = k + 1;
            continue;
          }
        }
        from = open;
      }
    }
  }
}
