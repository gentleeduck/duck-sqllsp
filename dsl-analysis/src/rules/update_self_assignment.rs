//! sql516: `UPDATE t SET col = col` -- assigning a column to itself is a
//! no-op. It still dirties the row (fires triggers, bumps xmax, writes a new
//! tuple version), so it's wasteful at best and usually a copy-paste slip
//! where the right-hand side should have referenced a different column or
//! expression. Only the textually-identical `col = col` / `t.col = t.col`
//! form is flagged; `col = col + 1`, `a = b.a`, casts, etc. are left alone.

use crate::clause_scan::{find_clause, find_clause_end, parse_simple_ident, split_top_level};
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::{Statement, StatementKind};
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql516"
  }
  fn default_severity(&self) -> Severity {
    Severity::Warning
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let StatementKind::Update(_) = &stmt.kind else { return };
    let (start, body, upper) = crate::stmt_body_upper(stmt, source);
    let ub = upper.as_bytes();

    let Some(set_at) = find_clause(ub, b"SET") else { return };
    let set_after = set_at + 3;
    let set_end = find_clause_end(ub, set_after, &["WHERE", "FROM", "RETURNING"]);
    if set_end <= set_after {
      return;
    }
    let set_body = &body[set_after..set_end];

    for (assign, off) in split_top_level(set_body) {
      let Some(eq) = first_assignment_eq(assign) else { continue };
      let (lhs, rhs) = (assign[..eq].trim(), assign[eq + 1..].trim());
      let (Some(l), Some(r)) = (parse_simple_ident(lhs), parse_simple_ident(rhs)) else { continue };
      if !idents_equal(&l, &r) {
        continue;
      }
      // Absolute span of the `col = col` assignment.
      let lead = assign.len() - assign.trim_start().len();
      let abs_start = start + set_after + off + lead;
      let abs_end = start + set_after + off + assign.trim_end().len();
      out.push(Diagnostic {
        code: "sql516",
        severity: Severity::Warning,
        message: format!("`{}` assigns a column to itself -- this is a no-op write", assign.trim()),
        range: crate::range_at(abs_start, abs_end),
      });
    }
  }
}

type Ident = (Option<String>, String);

/// Same (optional qualifier, name) pair, case-insensitively. `t.a` matches
/// `t.a` and `a` matches `a`, but `a` does not match `b.a`.
fn idents_equal(a: &Ident, b: &Ident) -> bool {
  let quals_match = match (&a.0, &b.0) {
    (None, None) => true,
    (Some(x), Some(y)) => x.eq_ignore_ascii_case(y),
    _ => false,
  };
  quals_match && a.1.eq_ignore_ascii_case(&b.1)
}

/// Offset of the first top-level `=` that is the assignment operator, i.e.
/// not part of `<=`, `>=`, `<>`, `!=`, or `:=`. Returns None if absent.
fn first_assignment_eq(s: &str) -> Option<usize> {
  let bytes = s.as_bytes();
  let mut depth = 0i32;
  let mut i = 0usize;
  while i < bytes.len() {
    match bytes[i] {
      b'(' | b'[' => depth += 1,
      b')' | b']' => depth -= 1,
      b'\'' => {
        i += 1;
        while i < bytes.len() && bytes[i] != b'\'' {
          i += 1;
        }
      },
      b'=' if depth == 0 => {
        let prev = if i > 0 { bytes[i - 1] } else { b' ' };
        let next = bytes.get(i + 1).copied().unwrap_or(b' ');
        if !matches!(prev, b'<' | b'>' | b'!' | b':') && next != b'=' {
          return Some(i);
        }
      },
      _ => {},
    }
    i += 1;
  }
  None
}
