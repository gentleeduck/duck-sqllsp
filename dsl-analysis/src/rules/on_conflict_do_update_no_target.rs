//! sql649: `INSERT ... ON CONFLICT DO UPDATE ...` with no conflict target.
//! `DO UPDATE` needs to know *which* unique index or constraint the conflict is
//! on, so PostgreSQL requires an inference clause -- `ON CONFLICT (col) DO
//! UPDATE` or `ON CONFLICT ON CONSTRAINT name DO UPDATE` -- and otherwise raises
//! 42601 ("ON CONFLICT DO UPDATE requires inference specification or constraint
//! name"). (`ON CONFLICT DO NOTHING` may omit the target and is left alone.)

use crate::clause_scan::is_word;
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql649"
  }
  fn default_severity(&self) -> Severity {
    Severity::Error
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, _body, upper) = crate::stmt_body_upper(stmt, source);
    let ub = upper.as_bytes();
    let n = ub.len();
    let mut i = 0usize;
    while i + 8 <= n {
      if &ub[i..i + 8] == b"CONFLICT"
        && (i == 0 || !is_word(ub[i - 1] as char))
        && ub.get(i + 8).is_none_or(|&b| !is_word(b as char))
      {
        // skip whitespace; a target would be `(` or `ON CONSTRAINT`
        let mut j = i + 8;
        while j < n && ub[j].is_ascii_whitespace() {
          j += 1;
        }
        // targetless: next token is `DO`
        if j + 2 <= n && &ub[j..j + 2] == b"DO" && ub.get(j + 2).is_some_and(|&b| b.is_ascii_whitespace()) {
          let mut k = j + 2;
          while k < n && ub[k].is_ascii_whitespace() {
            k += 1;
          }
          if k + 6 <= n && &ub[k..k + 6] == b"UPDATE" && ub.get(k + 6).is_none_or(|&b| !is_word(b as char)) {
            out.push(Diagnostic {
              code: "sql649",
              severity: Severity::Error,
              message: "ON CONFLICT DO UPDATE needs a conflict target -- add `(column)` or `ON CONSTRAINT name` (PG 42601)".into(),
              range: crate::range_at(start + k, start + k + 6),
            });
          }
        }
        i += 8;
        continue;
      }
      i += 1;
    }
  }
}
