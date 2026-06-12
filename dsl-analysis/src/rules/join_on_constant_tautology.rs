//! sql517: `JOIN ... ON 1 = 1` -- the join condition is a numeric constant
//! tautology, so the join produces a full cartesian product. That's almost
//! always a placeholder someone forgot to fill in, or an accidental cross
//! join. Suggests an explicit `CROSS JOIN` (if intended) or a real predicate.
//!
//! Only the *numeric* `n = n` form is flagged, never `ON TRUE` -- the latter
//! is the idiomatic, intentional condition for `LEFT JOIN LATERAL (...)`.

use crate::clause_scan::{find_clause, find_clause_end};
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

// Keywords that terminate an ON join-condition. LEFT/RIGHT/... precede JOIN,
// so they must end the clause too. Imperfect boundary detection only ever
// yields a longer body (a false negative) -- never a false positive, since
// we flag only when the body is *exactly* a numeric tautology.
const STOPWORDS: &[&str] = &[
  "JOIN", "LEFT", "RIGHT", "INNER", "FULL", "CROSS", "NATURAL", "WHERE", "GROUP", "ORDER", "HAVING", "LIMIT", "OFFSET",
  "WINDOW", "RETURNING", "UNION", "INTERSECT", "EXCEPT", "FETCH",
];

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql517"
  }
  fn default_severity(&self) -> Severity {
    Severity::Warning
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, body, upper) = crate::stmt_body_upper(stmt, source);
    let ub = upper.as_bytes();

    let mut from = 0usize;
    while let Some(rel) = find_clause(&ub[from..], b"ON").map(|p| p + from) {
      let on_kw = rel;
      let after = on_kw + 2;
      let end = find_clause_end(ub, after, STOPWORDS);
      let cond = body[after..end].trim();
      if is_numeric_tautology(cond) {
        out.push(Diagnostic {
          code: "sql517",
          severity: Severity::Warning,
          message: format!(
            "join condition `ON {cond}` is a constant tautology -- this is effectively a CROSS JOIN; \
             write `CROSS JOIN` explicitly or supply a real join predicate"
          ),
          range: crate::range_at(start + on_kw, start + end),
        });
      }
      from = end.max(after);
    }
  }
}

/// True for `<int> = <same int>` after stripping whitespace and any balanced
/// wrapping parens (e.g. `1=1`, `1 = 1`, `(2=2)`). Not `1=2`, not `TRUE`.
fn is_numeric_tautology(cond: &str) -> bool {
  let mut s: String = cond.chars().filter(|c| !c.is_whitespace()).collect();
  while s.len() >= 2 && s.starts_with('(') && s.ends_with(')') {
    s = s[1..s.len() - 1].to_string();
  }
  let Some((l, r)) = s.split_once('=') else { return false };
  !l.is_empty() && l == r && l.bytes().all(|b| b.is_ascii_digit())
}
