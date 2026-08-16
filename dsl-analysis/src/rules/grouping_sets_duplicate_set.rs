//! sql784: `GROUPING SETS ((a,b), (a,b))` -- the same set of grouping
//! columns appears twice (regardless of column order within the set).
//! PostgreSQL doesn't reject this, but it's virtually always a
//! copy-paste mistake.

use crate::clause_scan::{find_clause, parse_simple_ident, split_top_level};
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql784"
  }
  fn default_severity(&self) -> Severity {
    Severity::Hint
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, body, upper) = crate::stmt_body_upper(stmt, source);
    let ub = upper.as_bytes();
    let Some(gs) = find_clause(ub, b"GROUPING SETS") else { return };
    let p = skip_ws(ub, gs + "GROUPING SETS".len());
    if ub.get(p) != Some(&b'(') {
      return;
    }
    let Some(close) = match_paren(ub, p) else { return };
    let list = &body[p + 1..close];
    let mut seen: Vec<Vec<String>> = Vec::new();
    for (entry, off) in split_top_level(list) {
      let trimmed = entry.trim();
      let cols: Vec<String> = if trimmed.starts_with('(') && trimmed.ends_with(')') {
        split_top_level(&trimmed[1..trimmed.len() - 1])
          .into_iter()
          .filter_map(|(c, _)| parse_simple_ident(c).map(|(_, n)| n.to_ascii_uppercase()))
          .collect()
      } else if trimmed.is_empty() {
        Vec::new()
      } else if let Some((_, n)) = parse_simple_ident(trimmed) {
        vec![n.to_ascii_uppercase()]
      } else {
        continue;
      };
      let mut sorted = cols;
      sorted.sort();
      let lead = entry.len() - entry.trim_start().len();
      let abs_start = p + 1 + off + lead;
      if seen.contains(&sorted) {
        out.push(Diagnostic {
          code: "sql784",
          severity: Severity::Hint,
          message: "this grouping set duplicates an earlier one in the same GROUPING SETS list".into(),
          range: crate::range_at(start + abs_start, start + abs_start + trimmed.len()),
        });
      } else {
        seen.push(sorted);
      }
    }
  }
}

fn match_paren(ub: &[u8], open: usize) -> Option<usize> {
  let mut depth = 0i32;
  let mut i = open;
  while i < ub.len() {
    match ub[i] {
      b'(' => depth += 1,
      b')' => {
        depth -= 1;
        if depth == 0 {
          return Some(i);
        }
      },
      b'\'' => {
        i += 1;
        while i < ub.len() && ub[i] != b'\'' {
          i += 1;
        }
      },
      _ => {},
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
