//! sql772: the recursive term contains a top-level ORDER BY, LIMIT, or
//! DISTINCT -- disallowed in a recursive query's recursive term. Only
//! the term's own top level is checked (via `unwrap_parens` +
//! depth-0 `find_clause`) -- a nested subquery's ORDER BY/LIMIT is
//! legal and left alone.

use crate::clause_scan::{find_clause, find_recursive_cte, unwrap_parens};
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

const FORBIDDEN: &[&[u8]] = &[b"ORDER BY", b"LIMIT", b"DISTINCT"];

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql772"
  }
  fn default_severity(&self) -> Severity {
    Severity::Error
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, body, upper) = crate::stmt_body_upper(stmt, source);
    let Some(cte) = find_recursive_cte(body, &upper) else { return };
    let ub = upper.as_bytes();
    let (s, e) = unwrap_parens(ub, cte.term_start, cte.term_end);
    let slice = &upper[s..e];
    for &kw in FORBIDDEN {
      if let Some(rel) = find_clause(slice.as_bytes(), kw) {
        let abs = s + rel;
        let kw_str = std::str::from_utf8(kw).unwrap_or("");
        out.push(Diagnostic {
          code: "sql772",
          severity: Severity::Error,
          message: format!("{kw_str} is not allowed in a recursive query's recursive term"),
          range: crate::range_at(start + abs, start + abs + kw.len()),
        });
      }
    }
  }
}
