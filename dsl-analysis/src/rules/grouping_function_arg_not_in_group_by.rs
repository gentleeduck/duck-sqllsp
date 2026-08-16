//! sql785: `GROUPING(x)` where `x` does not appear anywhere in the
//! statement's GROUP BY clause -- PostgreSQL raises 42803 ("column ...
//! must appear in GROUP BY clause or be used in an aggregate
//! function"). Checks only that the argument identifier appears
//! somewhere in the GROUP BY clause text (handles GROUPING SETS/
//! ROLLUP/CUBE without needing to parse their nested structure) --
//! conservative by construction, never false-positives on a column
//! that's actually grouped.

use crate::clause_scan::{find_clause, find_clause_end, is_word, parse_simple_ident};
use crate::textutil::contains_word;
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql785"
  }
  fn default_severity(&self) -> Severity {
    Severity::Error
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, body, upper) = crate::stmt_body_upper(stmt, source);
    let ub = upper.as_bytes();
    let Some(gb) = find_clause(ub, b"GROUP BY") else { return };
    let gb_body_start = gb + "GROUP BY".len();
    let gb_end =
      find_clause_end(ub, gb_body_start, &["HAVING", "ORDER", "LIMIT", "OFFSET", "WINDOW", "UNION", "INTERSECT", "EXCEPT"]);
    let gb_text = &upper[gb_body_start..gb_end];

    let mut i = 0usize;
    while i < ub.len() {
      if !word_at(ub, i, b"GROUPING") {
        i += 1;
        continue;
      }
      let p = skip_ws(ub, i + "GROUPING".len());
      if ub.get(p) != Some(&b'(') {
        i += "GROUPING".len();
        continue;
      }
      let Some(close) = match_paren(ub, p) else { break };
      let arg = body[p + 1..close].trim();
      if let Some((_, name)) = parse_simple_ident(arg)
        && !contains_word(gb_text, &name.to_ascii_uppercase())
      {
        out.push(Diagnostic {
          code: "sql785",
          severity: Severity::Error,
          message: format!("GROUPING({name}) -- `{name}` does not appear in the GROUP BY clause"),
          range: crate::range_at(start + i, start + close + 1),
        });
      }
      i = close + 1;
    }
  }
}

fn word_at(ub: &[u8], i: usize, w: &[u8]) -> bool {
  i + w.len() <= ub.len()
    && &ub[i..i + w.len()] == w
    && (i == 0 || !is_word(ub[i - 1] as char))
    && (i + w.len() == ub.len() || !is_word(ub[i + w.len()] as char))
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
