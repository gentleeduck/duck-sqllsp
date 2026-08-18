//! sql801: `EXECUTE <dynamic sql> USING a, b` where the highest `$N`
//! placeholder referenced in the dynamic SQL text doesn't match the
//! number of USING arguments. Scans the whole EXECUTE target text for
//! `$N` placeholders regardless of whether the target is a plain
//! string or wrapped in `format(...)` -- `format()`'s own `%s`/`%I`/
//! `%L` substitutions are a separate mechanism, not counted here.

use crate::clause_scan::{is_word, split_top_level};
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql801"
  }
  fn default_severity(&self) -> Severity {
    Severity::Error
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, body, upper) = crate::stmt_body_upper(stmt, source);
    let ub = upper.as_bytes();
    let mut i = 0usize;
    while i < ub.len() {
      if !word_at(ub, i, b"EXECUTE") {
        i += 1;
        continue;
      }
      let expr_start = skip_ws(ub, i + "EXECUTE".len());
      let Some(using_rel) = find_word_from(ub, expr_start, b"USING") else {
        i += "EXECUTE".len();
        continue;
      };
      let expr_text = &upper[expr_start..using_rel];
      let Some(max_n) = highest_dollar_placeholder(expr_text) else {
        i = using_rel + 5;
        continue;
      };
      let args_start = skip_ws(ub, using_rel + 5);
      let args_end = find_word_from(ub, args_start, b"INTO").unwrap_or_else(|| find_stmt_end(ub, args_start));
      let arg_count = split_top_level(&body[args_start..args_end]).iter().filter(|(s, _)| !s.trim().is_empty()).count();
      if max_n != arg_count {
        out.push(Diagnostic {
          code: "sql801",
          severity: Severity::Error,
          message: format!(
            "EXECUTE references ${max_n} but USING provides {arg_count} argument(s) -- {}",
            if max_n > arg_count { "missing argument, raises an error at runtime" } else { "unused argument" }
          ),
          range: crate::range_at(start + using_rel, start + args_end),
        });
      }
      i = args_end;
    }
  }
}

/// Highest `$N` (N >= 1) placeholder referenced in `s`, or None.
fn highest_dollar_placeholder(s: &str) -> Option<usize> {
  let b = s.as_bytes();
  let mut max_n: Option<usize> = None;
  let mut i = 0usize;
  while i < b.len() {
    if b[i] == b'$' {
      let d_start = i + 1;
      let mut j = d_start;
      while j < b.len() && b[j].is_ascii_digit() {
        j += 1;
      }
      if j > d_start {
        if let Ok(n) = s[d_start..j].parse::<usize>() {
          max_n = Some(max_n.map_or(n, |m| m.max(n)));
        }
        i = j;
        continue;
      }
    }
    i += 1;
  }
  max_n
}

fn find_stmt_end(ub: &[u8], from: usize) -> usize {
  let mut depth = 0i32;
  let mut i = from;
  while i < ub.len() {
    match ub[i] {
      b'(' => depth += 1,
      b')' => depth -= 1,
      b';' if depth == 0 => return i,
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
  ub.len()
}

fn find_word_from(ub: &[u8], from: usize, w: &[u8]) -> Option<usize> {
  let mut i = from;
  while i + w.len() <= ub.len() {
    if word_at(ub, i, w) {
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
