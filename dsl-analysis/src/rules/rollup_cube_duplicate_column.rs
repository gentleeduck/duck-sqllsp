//! sql786: `ROLLUP (a, a)` / `CUBE (a, a)` -- the same column listed
//! twice. Replaces the spec's original sql786 (empty ROLLUP/CUBE
//! column list), verified unreachable -- pg_query rejects `ROLLUP ()`
//! / `CUBE ()` as a hard parse error before any LintRule sees it,
//! same class of trap as batches 1, 2, and 4's swaps.

use crate::clause_scan::{is_word, parse_simple_ident, split_top_level};
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql786"
  }
  fn default_severity(&self) -> Severity {
    Severity::Warning
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, body, upper) = crate::stmt_body_upper(stmt, source);
    let ub = upper.as_bytes();
    let mut i = 0usize;
    while i < ub.len() {
      let is_rollup = word_at(ub, i, b"ROLLUP");
      let is_cube = !is_rollup && word_at(ub, i, b"CUBE");
      if !is_rollup && !is_cube {
        i += 1;
        continue;
      }
      let kwlen = if is_rollup { 6 } else { 4 };
      let p = skip_ws(ub, i + kwlen);
      if ub.get(p) != Some(&b'(') {
        i += kwlen;
        continue;
      }
      let Some(close) = match_paren(ub, p) else { break };
      let list = &body[p + 1..close];
      let mut seen: Vec<String> = Vec::new();
      for (entry, off) in split_top_level(list) {
        let Some((_, name)) = parse_simple_ident(entry) else { continue };
        if seen.iter().any(|s| s.eq_ignore_ascii_case(&name)) {
          let lead = entry.len() - entry.trim_start().len();
          let abs = p + 1 + off + lead;
          out.push(Diagnostic {
            code: "sql786",
            severity: Severity::Warning,
            message: format!("column `{name}` appears more than once in {}", if is_rollup { "ROLLUP" } else { "CUBE" }),
            range: crate::range_at(start + abs, start + abs + name.len()),
          });
        } else {
          seen.push(name);
        }
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
