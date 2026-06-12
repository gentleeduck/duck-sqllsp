//! sql518: `CASE WHEN cond THEN TRUE ELSE FALSE END` -- a single-branch CASE
//! that just maps a condition to a boolean. It's equivalent to `(cond) IS
//! TRUE` (the `IS TRUE` matters: the CASE returns FALSE, not NULL, when
//! `cond` is NULL). The `THEN FALSE ELSE TRUE` form is `(cond) IS NOT TRUE`.
//! Collapsing it is shorter and reads better.
//!
//! Conservative: only the searched single-WHEN form with boolean literals in
//! both arms is flagged; multi-branch, nested, or simple (`CASE x WHEN`) forms
//! are left alone.

use crate::clause_scan::find_clause;
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql518"
  }
  fn default_severity(&self) -> Severity {
    Severity::Hint
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, body, upper) = crate::stmt_body_upper(stmt, source);
    let ub = upper.as_bytes();

    let mut from = 0usize;
    while let Some(rel) = find_clause(&ub[from..], b"CASE").map(|p| p + from) {
      let case_kw = rel;
      let body_start = case_kw + 4;
      let Some(end_kw) = find_matching_end(ub, body_start) else { break };
      from = end_kw + 3; // advance past this CASE...END regardless

      let inner = &body[body_start..end_kw];
      let ib = inner.as_bytes();

      // Nested CASE -> too complex, skip.
      if find_clause(ib, b"CASE").is_some() {
        continue;
      }
      // Searched form with exactly one WHEN.
      let Some(when) = find_clause(ib, b"WHEN") else { continue };
      if when != inner.len() - inner.trim_start().len() {
        continue; // simple CASE (operand before WHEN)
      }
      if find_clause(&ib[when + 4..], b"WHEN").is_some() {
        continue; // more than one branch
      }
      let Some(then) = find_clause(&ib[when + 4..], b"THEN").map(|p| p + when + 4) else { continue };
      let Some(els) = find_clause(&ib[then + 4..], b"ELSE").map(|p| p + then + 4) else { continue };

      let cond = inner[when + 4..then].trim();
      let v1 = inner[then + 4..els].trim();
      let v2 = inner[els + 4..].trim();

      let suggestion = match (bool_lit(v1), bool_lit(v2)) {
        (Some(true), Some(false)) => format!("({cond}) IS TRUE"),
        (Some(false), Some(true)) => format!("({cond}) IS NOT TRUE"),
        _ => continue,
      };
      out.push(Diagnostic {
        code: "sql518",
        severity: Severity::Hint,
        message: format!("redundant boolean CASE -- equivalent to `{suggestion}`"),
        range: crate::range_at(start + case_kw, start + end_kw + 3),
      });
    }
  }
}

fn bool_lit(s: &str) -> Option<bool> {
  if s.eq_ignore_ascii_case("TRUE") {
    Some(true)
  } else if s.eq_ignore_ascii_case("FALSE") {
    Some(false)
  } else {
    None
  }
}

/// Offset of the `END` that closes the `CASE` whose body starts at `from`,
/// accounting for nested `CASE ... END`. Returns None if unbalanced.
fn find_matching_end(ub: &[u8], from: usize) -> Option<usize> {
  let n = ub.len();
  let mut depth = 0i32;
  let mut i = from;
  while i < n {
    if let Some(rel) = next_word(ub, i, b"CASE") {
      if let Some(erel) = next_word(ub, i, b"END") {
        if erel < rel {
          if depth == 0 {
            return Some(erel);
          }
          depth -= 1;
          i = erel + 3;
          continue;
        }
        depth += 1;
        i = rel + 4;
        continue;
      }
      // No END before next CASE -> unbalanced.
      return None;
    } else if let Some(erel) = next_word(ub, i, b"END") {
      if depth == 0 {
        return Some(erel);
      }
      depth -= 1;
      i = erel + 3;
    } else {
      return None;
    }
  }
  None
}

/// Offset of the next whole-word `kw` at or after `from`, or None.
fn next_word(ub: &[u8], from: usize, kw: &[u8]) -> Option<usize> {
  let n = ub.len();
  let m = kw.len();
  let mut i = from;
  while i + m <= n {
    if ub[i..i + m] == *kw
      && (i == 0 || !crate::clause_scan::is_word(ub[i - 1] as char))
      && (i + m == n || !crate::clause_scan::is_word(ub[i + m] as char))
    {
      return Some(i);
    }
    i += 1;
  }
  None
}
