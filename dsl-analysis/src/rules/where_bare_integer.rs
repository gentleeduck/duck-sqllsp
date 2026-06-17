//! sql592: `WHERE 1` / `WHERE 0` -- a bare integer where a boolean is
//! required. This is a MySQL idiom (`WHERE 1` = always true); PostgreSQL has a
//! real boolean type and rejects it with 42804 ("argument of WHERE must be
//! type boolean, not type integer"). Use `WHERE true` / `WHERE false`, or a
//! real predicate.

use crate::clause_scan::{find_clause, find_clause_end};
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

const STOPWORDS: &[&str] =
  &["GROUP", "ORDER", "HAVING", "LIMIT", "OFFSET", "WINDOW", "RETURNING", "UNION", "INTERSECT", "EXCEPT", "FETCH", "FOR"];

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql592"
  }
  fn default_severity(&self) -> Severity {
    Severity::Error
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, body, upper) = crate::stmt_body_upper(stmt, source);
    let ub = upper.as_bytes();
    let Some(at) = find_clause(ub, b"WHERE") else { return };
    let ps = at + 5;
    let pe = find_clause_end(ub, ps, STOPWORDS);
    let pred = body[ps..pe].trim();
    // The entire predicate is a bare integer literal.
    if !pred.is_empty() && pred.bytes().all(|b| b.is_ascii_digit()) {
      let lead = body[ps..pe].len() - body[ps..pe].trim_start().len();
      out.push(Diagnostic {
        code: "sql592",
        severity: Severity::Error,
        message: format!("`WHERE {pred}` is not boolean -- PG requires a boolean predicate (42804); use `WHERE true`/`false` or a real condition"),
        range: crate::range_at(start + ps + lead, start + ps + body[ps..pe].trim_end().len()),
      });
    }
  }
}
