//! sql760: `PARTITION BY RANGE/LIST/HASH (a, a)` -- the same column
//! listed twice in the partition key. Always a copy-paste mistake; a
//! repeated column contributes nothing to the partitioning strategy.

use crate::clause_scan::{find_clause, parse_simple_ident, split_top_level};
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql760"
  }
  fn default_severity(&self) -> Severity {
    Severity::Warning
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, body, upper) = crate::stmt_body_upper(stmt, source);
    let ub = upper.as_bytes();
    let Some(kw) = find_clause(ub, b"PARTITION BY") else { return };
    let mut i = skip_ws(ub, kw + "PARTITION BY".len());
    for strategy in ["RANGE", "LIST", "HASH"] {
      if upper[i..].starts_with(strategy) {
        i += strategy.len();
        break;
      }
    }
    i = skip_ws(ub, i);
    if ub.get(i) != Some(&b'(') {
      return;
    }
    let Some(close) = match_paren(ub, i) else { return };
    let list = &body[i + 1..close];
    let mut seen: Vec<String> = Vec::new();
    for (entry, off) in split_top_level(list) {
      let Some((_, name)) = parse_simple_ident(entry) else { continue };
      if seen.iter().any(|s| s.eq_ignore_ascii_case(&name)) {
        let lead = entry.len() - entry.trim_start().len();
        let abs = i + 1 + off + lead;
        out.push(Diagnostic {
          code: "sql760",
          severity: Severity::Warning,
          message: format!("column `{name}` appears more than once in the partition key"),
          range: crate::range_at(start + abs, start + abs + name.len()),
        });
      } else {
        seen.push(name);
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
