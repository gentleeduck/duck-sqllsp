//! sql754: `to_tsquery('quick brown fox')` -- to_tsquery expects tsquery
//! syntax (lexemes joined by `&`, `|`, `!`, `<->`), so a plain phrase with
//! spaces and no operator raises a syntax error at runtime. For free text use
//! `plainto_tsquery(...)` (whitespace -> AND), `phraseto_tsquery(...)`, or
//! `websearch_to_tsquery(...)`. (Companion to sql499 tsvector_text_literal.)

use crate::clause_scan::is_word;
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql754"
  }
  fn default_severity(&self) -> Severity {
    Severity::Warning
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, body, upper) = crate::stmt_body_upper(stmt, source);
    let ub = upper.as_bytes();
    let n = ub.len();

    let mut i = 0usize;
    while i < n {
      // Word-bounded TO_TSQUERY: the left boundary check excludes
      // plainto_/phraseto_/websearch_to_tsquery (those accept free text).
      if !word_at(ub, i, b"TO_TSQUERY") {
        i += 1;
        continue;
      }
      let p = skip_ws(ub, i + 10);
      if ub.get(p) != Some(&b'(') {
        i += 10;
        continue;
      }
      let Some(close) = match_paren(ub, p) else { break };
      // The query is the last argument (an optional regconfig may precede it).
      if let Some(&(as_, ae)) = top_level_args(ub, p, close).last()
        && let Some((s, e)) = trim_range(ub, as_, ae)
        && e >= s + 2
        && ub[s] == b'\''
        && ub[e - 1] == b'\''
      {
        let q = &body[s + 1..e - 1];
        if q.contains(' ') && !q.bytes().any(|c| matches!(c, b'&' | b'|' | b'!' | b'<' | b'(' | b')' | b':')) {
          out.push(Diagnostic {
            code: "sql754",
            severity: Severity::Warning,
            message: "to_tsquery() needs tsquery operators -- a plain phrase with spaces is a syntax error; use plainto_tsquery/websearch_to_tsquery".into(),
            range: crate::range_at(start + s, start + e),
          });
        }
      }
      i = close + 1;
    }
  }
}

fn trim_range(ub: &[u8], mut s: usize, mut e: usize) -> Option<(usize, usize)> {
  while s < e && ub[s].is_ascii_whitespace() {
    s += 1;
  }
  while e > s && ub[e - 1].is_ascii_whitespace() {
    e -= 1;
  }
  (s < e).then_some((s, e))
}

fn top_level_args(ub: &[u8], open: usize, close: usize) -> Vec<(usize, usize)> {
  let mut args = Vec::new();
  let mut depth = 0i32;
  let mut argstart = open + 1;
  let mut i = open + 1;
  while i < close {
    match ub[i] {
      b'(' | b'[' => depth += 1,
      b')' | b']' => depth -= 1,
      b'\'' => {
        i += 1;
        while i < close && ub[i] != b'\'' {
          i += 1;
        }
      },
      b',' if depth == 0 => {
        args.push((argstart, i));
        argstart = i + 1;
      },
      _ => {},
    }
    i += 1;
  }
  if argstart < close || !args.is_empty() {
    args.push((argstart, close));
  }
  args
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
          i += 1
        }
      },
      _ => {},
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
