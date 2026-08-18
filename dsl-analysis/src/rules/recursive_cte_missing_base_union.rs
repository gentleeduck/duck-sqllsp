//! sql770: the recursive term references the CTE itself more than
//! once. PostgreSQL allows exactly one self-reference in a recursive
//! term ("recursive reference to query ... must not appear more than
//! once").

use crate::clause_scan::{find_recursive_cte, is_word};
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql770"
  }
  fn default_severity(&self) -> Severity {
    Severity::Error
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, body, upper) = crate::stmt_body_upper(stmt, source);
    let Some(cte) = find_recursive_cte(body, &upper) else { return };
    let name = &upper[cte.name_start..cte.name_end];
    let ub = upper.as_bytes();
    let mut count = 0usize;
    let mut last_at = 0usize;
    let mut i = cte.term_start;
    while i < cte.term_end {
      if word_at(ub, i, name.as_bytes()) {
        count += 1;
        last_at = i;
        i += name.len();
      } else {
        i += 1;
      }
    }
    if count >= 2 {
      out.push(Diagnostic {
        code: "sql770",
        severity: Severity::Error,
        message: format!(
          "recursive term references `{}` {count} times -- PostgreSQL allows exactly one self-reference",
          &body[cte.name_start..cte.name_end]
        ),
        range: crate::range_at(start + last_at, start + last_at + name.len()),
      });
    }
  }
}

fn word_at(ub: &[u8], i: usize, w: &[u8]) -> bool {
  i + w.len() <= ub.len()
    && &ub[i..i + w.len()] == w
    && (i == 0 || !is_word(ub[i - 1] as char))
    && (i + w.len() == ub.len() || !is_word(ub[i + w.len()] as char))
}
