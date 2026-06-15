//! sql563: `col = ANY(ARRAY[1, 2, 1])` -- a duplicate element in an ANY / ALL
//! array literal. The planner dedups it, but it bloats the query and is
//! usually a copy-paste typo for a different value. (The `IN (...)` spelling
//! is covered by sql306.)

use crate::clause_scan::is_word;
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;
use std::collections::HashSet;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql563"
  }
  fn default_severity(&self) -> Severity {
    Severity::Hint
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, body, upper) = crate::stmt_body_upper(stmt, source);
    let ub = upper.as_bytes();
    let n = ub.len();

    for kw in [&b"ANY"[..], &b"ALL"[..]] {
      let mut i = 0usize;
      while i + 3 <= n {
        if ub[i..i + 3] != *kw || (i > 0 && is_word(ub[i - 1] as char)) {
          i += 1;
          continue;
        }
        let mut p = skip_ws(ub, i + 3);
        if ub.get(p) != Some(&b'(') {
          i += 3;
          continue;
        }
        let Some(call_close) = match_pair(ub, p, b'(', b')') else { break };
        p = skip_ws(ub, p + 1);
        if !ub[p..].starts_with(b"ARRAY") {
          i = call_close + 1;
          continue;
        }
        p = skip_ws(ub, p + 5);
        if ub.get(p) == Some(&b'[')
          && let Some(rb) = match_pair(ub, p, b'[', b']')
        {
          flag_dups(body, start, p + 1, rb, out);
        }
        i = call_close + 1;
      }
    }
  }
}

fn flag_dups(body: &str, abs: usize, from: usize, to: usize, out: &mut Vec<Diagnostic>) {
  let mut seen: HashSet<String> = HashSet::new();
  for (s, e) in split_top_level(from, to, body.as_bytes()) {
    let elem = body[s..e].trim();
    if elem.is_empty() {
      continue;
    }
    let key = elem.to_ascii_lowercase();
    if !seen.insert(key) {
      let lead = (e - s) - body[s..e].trim_start().len();
      out.push(Diagnostic {
        code: "sql563",
        severity: Severity::Hint,
        message: format!("`{elem}` appears more than once in the array -- the duplicate has no effect"),
        range: crate::range_at(abs + s + lead, abs + s + body[s..e].trim_end().len()),
      });
    }
  }
}

fn split_top_level(from: usize, to: usize, bytes: &[u8]) -> Vec<(usize, usize)> {
  let mut out = Vec::new();
  let mut depth = 0i32;
  let mut last = from;
  let mut i = from;
  while i < to {
    match bytes[i] {
      b'(' | b'[' => depth += 1,
      b')' | b']' => depth -= 1,
      b'\'' => {
        i += 1;
        while i < to && bytes[i] != b'\'' {
          i += 1;
        }
      },
      b',' if depth == 0 => {
        out.push((last, i));
        last = i + 1;
      },
      _ => {},
    }
    i += 1;
  }
  out.push((last, to));
  out
}

fn skip_ws(bytes: &[u8], mut i: usize) -> usize {
  while i < bytes.len() && bytes[i].is_ascii_whitespace() {
    i += 1;
  }
  i
}

fn match_pair(bytes: &[u8], from: usize, open: u8, close: u8) -> Option<usize> {
  let mut depth = 0i32;
  let mut i = from;
  while i < bytes.len() {
    let b = bytes[i];
    if b == open {
      depth += 1;
    } else if b == close {
      depth -= 1;
      if depth == 0 {
        return Some(i);
      }
    } else if b == b'\'' {
      i += 1;
      while i < bytes.len() && bytes[i] != b'\'' {
        i += 1;
      }
    }
    i += 1;
  }
  None
}
