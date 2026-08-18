//! sql769: `CYCLE ... USING <col>` names a working column that
//! collides with a column already produced by the recursive CTE's own
//! column list. PostgreSQL rejects this as a duplicate column name.
//! `SEARCH ... SET <col>` collision is a separate, narrower case and
//! is out of scope for this rule.

use crate::clause_scan::{find_recursive_cte, is_word};
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql769"
  }
  fn default_severity(&self) -> Severity {
    Severity::Error
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, body, upper) = crate::stmt_body_upper(stmt, source);
    let Some(cte) = find_recursive_cte(body, &upper) else { return };
    if cte.cols.is_empty() {
      return;
    }
    let ub = upper.as_bytes();
    if let Some((mark_start, mark_name)) = cycle_using_col(ub, body, cte.term_end)
      && cte.cols.iter().any(|c| c.eq_ignore_ascii_case(&mark_name))
    {
      out.push(Diagnostic {
        code: "sql769",
        severity: Severity::Error,
        message: format!("CYCLE working column `{mark_name}` collides with an existing CTE column of the same name"),
        range: crate::range_at(start + mark_start, start + mark_start + mark_name.len()),
      });
    }
  }
}

/// Find `CYCLE ... USING <col>` after `from` and return the `<col>`
/// identifier's position + name.
fn cycle_using_col(ub: &[u8], body: &str, from: usize) -> Option<(usize, String)> {
  let cyc = find_word_from(ub, from, b"CYCLE")?;
  let using = find_word_from(ub, cyc, b"USING")?;
  let i = skip_ws(ub, using + "USING".len());
  let mut e = i;
  while e < ub.len() && is_word(ub[e] as char) {
    e += 1;
  }
  if e == i {
    return None;
  }
  Some((i, body[i..e].to_string()))
}

fn find_word_from(ub: &[u8], from: usize, w: &[u8]) -> Option<usize> {
  let mut i = from;
  while i + w.len() <= ub.len() {
    if &ub[i..i + w.len()] == w
      && (i == 0 || !is_word(ub[i - 1] as char))
      && (i + w.len() == ub.len() || !is_word(ub[i + w.len()] as char))
    {
      return Some(i);
    }
    i += 1;
  }
  None
}

fn skip_ws(ub: &[u8], mut i: usize) -> usize {
  while i < ub.len() && ub[i].is_ascii_whitespace() {
    i += 1;
  }
  i
}
