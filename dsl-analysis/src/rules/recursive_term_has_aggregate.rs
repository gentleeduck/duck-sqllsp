//! sql771: the recursive term contains an aggregate function call --
//! disallowed ("aggregate functions are not allowed in a recursive
//! query's recursive term").

use crate::clause_scan::{find_recursive_cte, is_word};
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

const AGGREGATES: &[&str] = &[
  "COUNT",
  "SUM",
  "AVG",
  "MIN",
  "MAX",
  "ARRAY_AGG",
  "STRING_AGG",
  "JSON_AGG",
  "JSONB_AGG",
  "BOOL_AND",
  "BOOL_OR",
  "EVERY",
  "JSON_OBJECT_AGG",
  "JSONB_OBJECT_AGG",
  "XMLAGG",
];

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql771"
  }
  fn default_severity(&self) -> Severity {
    Severity::Error
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, body, upper) = crate::stmt_body_upper(stmt, source);
    let Some(cte) = find_recursive_cte(body, &upper) else { return };
    let ub = upper.as_bytes();
    let mut i = cte.term_start;
    while i < cte.term_end {
      if let Some(name) = AGGREGATES.iter().find(|a| word_at(ub, i, a.as_bytes())) {
        let after = skip_ws(ub, i + name.len());
        if ub.get(after) == Some(&b'(') {
          out.push(Diagnostic {
            code: "sql771",
            severity: Severity::Error,
            message: format!(
              "{name}() is an aggregate -- aggregate functions are not allowed in a recursive query's recursive term"
            ),
            range: crate::range_at(start + i, start + i + name.len()),
          });
          i += name.len();
          continue;
        }
      }
      i += 1;
    }
  }
}

fn word_at(ub: &[u8], i: usize, w: &[u8]) -> bool {
  i + w.len() <= ub.len()
    && &ub[i..i + w.len()] == w
    && (i == 0 || !is_word(ub[i - 1] as char))
    && (i + w.len() == ub.len() || !is_word(ub[i + w.len()] as char))
}

fn skip_ws(ub: &[u8], mut i: usize) -> usize {
  while i < ub.len() && ub[i].is_ascii_whitespace() {
    i += 1;
  }
  i
}
