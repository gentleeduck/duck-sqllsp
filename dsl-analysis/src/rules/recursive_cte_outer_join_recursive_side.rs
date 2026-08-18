//! sql773: the recursive term's self-reference sits on the nullable
//! side of an outer join -- disallowed ("recursive reference to query
//! ... must not appear within an outer join").

use crate::clause_scan::{find_recursive_cte, is_word};
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql773"
  }
  fn default_severity(&self) -> Severity {
    Severity::Error
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, body, upper) = crate::stmt_body_upper(stmt, source);
    let Some(cte) = find_recursive_cte(body, &upper) else { return };
    let name = &upper[cte.name_start..cte.name_end];
    let ub = upper.as_bytes();

    // LEFT JOIN <name> -- self-ref is the nullable right-hand side.
    if let Some(pos) = find_after(ub, cte.term_start, cte.term_end, b"LEFT JOIN") {
      let after = skip_ws(ub, pos + "LEFT JOIN".len());
      if word_at(ub, after, name.as_bytes()) {
        out.push(mk_diag(start, after, name.len()));
        return;
      }
    }
    // <name> RIGHT JOIN -- self-ref is the nullable left-hand side.
    let mut i = cte.term_start;
    while let Some(rj) = find_after(ub, i, cte.term_end, b"RIGHT JOIN") {
      let mut left_end = rj;
      while left_end > cte.term_start && ub[left_end - 1].is_ascii_whitespace() {
        left_end -= 1;
      }
      if left_end >= name.len() && word_at(ub, left_end - name.len(), name.as_bytes()) {
        out.push(mk_diag(start, left_end - name.len(), name.len()));
        return;
      }
      i = rj + "RIGHT JOIN".len();
    }
    // FULL [OUTER] JOIN -- either side nullable; the term is already
    // scoped to just the recursive branch, so any self-reference
    // alongside a FULL JOIN in that branch is on a nullable side.
    let has_full = find_after(ub, cte.term_start, cte.term_end, b"FULL JOIN").is_some()
      || find_after(ub, cte.term_start, cte.term_end, b"FULL OUTER JOIN").is_some();
    if has_full && let Some(pos) = find_after(ub, cte.term_start, cte.term_end, name.as_bytes()) {
      out.push(mk_diag(start, pos, name.len()));
    }
  }
}

fn mk_diag(start: usize, at: usize, len: usize) -> Diagnostic {
  Diagnostic {
    code: "sql773",
    severity: Severity::Error,
    message: "recursive self-reference is on the nullable side of an outer join -- PostgreSQL disallows this".into(),
    range: crate::range_at(start + at, start + at + len),
  }
}

fn find_after(ub: &[u8], from: usize, to: usize, w: &[u8]) -> Option<usize> {
  let mut i = from;
  while i + w.len() <= to {
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
