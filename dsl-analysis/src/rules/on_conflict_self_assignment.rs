//! sql536: `INSERT ... ON CONFLICT ... DO UPDATE SET col = col` -- the upsert
//! assigns a column to its own (pre-conflict) value, a no-op. The intent was
//! almost certainly `SET col = EXCLUDED.col` (take the incoming value). The
//! INSERT path of sql516 (which only sees plain UPDATE) misses this, so it
//! gets its own check.

use crate::clause_scan::{find_clause, find_clause_end, parse_simple_ident, split_top_level};
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql536"
  }
  fn default_severity(&self) -> Severity {
    Severity::Warning
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, body, upper) = crate::stmt_body_upper(stmt, source);
    let ub = upper.as_bytes();

    // Scope to the DO UPDATE arm of an ON CONFLICT clause.
    let Some(conflict) = find_clause(ub, b"CONFLICT") else { return };
    let Some(set_rel) = find_clause(&ub[conflict..], b"SET") else { return };
    let set_at = conflict + set_rel;
    let set_after = set_at + 3;
    let set_end = find_clause_end(ub, set_after, &["WHERE", "RETURNING"]);
    if set_end <= set_after {
      return;
    }

    for (assign, off) in split_top_level(&body[set_after..set_end]) {
      let Some(eq) = first_assignment_eq(assign) else { continue };
      let (lhs, rhs) = (assign[..eq].trim(), assign[eq + 1..].trim());
      let (Some(l), Some(r)) = (parse_simple_ident(lhs), parse_simple_ident(rhs)) else { continue };
      if !idents_equal(&l, &r) {
        continue;
      }
      let lead = assign.len() - assign.trim_start().len();
      let abs_start = start + set_after + off + lead;
      let abs_end = start + set_after + off + assign.trim_end().len();
      out.push(Diagnostic {
        code: "sql536",
        severity: Severity::Warning,
        message: format!("`{}` is a no-op in ON CONFLICT DO UPDATE -- did you mean `EXCLUDED.{}`?", assign.trim(), r.1),
        range: crate::range_at(abs_start, abs_end),
      });
    }
  }
}

type Ident = (Option<String>, String);

fn idents_equal(a: &Ident, b: &Ident) -> bool {
  let quals_match = match (&a.0, &b.0) {
    (None, None) => true,
    (Some(x), Some(y)) => x.eq_ignore_ascii_case(y),
    _ => false,
  };
  quals_match && a.1.eq_ignore_ascii_case(&b.1)
}

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
