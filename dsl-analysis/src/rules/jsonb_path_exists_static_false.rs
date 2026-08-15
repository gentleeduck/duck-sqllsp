//! sql780: `jsonb_path_exists(doc, '$.a ? (1 == 2)')` -- the filter's
//! comparison is between two literal numbers, so it evaluates to the
//! same result on every row. Only flags the always-false shapes (`==`
//! with different literals, `!=` with equal literals); an always-true
//! literal filter is a related but separate case, out of scope here.

use crate::clause_scan::is_word;
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql780"
  }
  fn default_severity(&self) -> Severity {
    Severity::Warning
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, body, upper) = crate::stmt_body_upper(stmt, source);
    let ub = upper.as_bytes();
    let mut i = 0usize;
    while i < ub.len() {
      if !word_at(ub, i, b"JSONB_PATH_EXISTS") {
        i += 1;
        continue;
      }
      let p = skip_ws(ub, i + "JSONB_PATH_EXISTS".len());
      if ub.get(p) != Some(&b'(') {
        i += "JSONB_PATH_EXISTS".len();
        continue;
      }
      let Some(close) = match_paren(ub, p) else { break };
      if let Some((lit_s, lit_e)) = first_string_literal(body, p, close)
        && let Some((rel_s, rel_e)) = find_static_false_filter(&body[lit_s + 1..lit_e])
      {
        let abs_s = lit_s + 1 + rel_s;
        let abs_e = lit_s + 1 + rel_e;
        out.push(Diagnostic {
          code: "sql780",
          severity: Severity::Warning,
          message: "jsonpath filter compares two literal numbers -- always false on every row".into(),
          range: crate::range_at(start + abs_s, start + abs_e),
        });
      }
      i = close + 1;
    }
  }
}

/// First single-quoted string literal's `(open_quote, close_quote)`
/// offsets inside `(open+1, close)` -- the path-expression argument
/// (the context-item argument before it is typically a bare column
/// reference, not quoted).
fn first_string_literal(body: &str, open: usize, close: usize) -> Option<(usize, usize)> {
  let b = body.as_bytes();
  let mut i = open + 1;
  while i < close {
    if b[i] == b'\'' {
      let s = i;
      i += 1;
      while i < close && b[i] != b'\'' {
        i += 1;
      }
      if i >= close {
        return None;
      }
      return Some((s, i));
    }
    i += 1;
  }
  None
}

/// Find `( <num> (==|!=) <num> )` inside `path` where the comparison
/// is always false, returning its `(start, end)` byte span within
/// `path`.
fn find_static_false_filter(path: &str) -> Option<(usize, usize)> {
  let b = path.as_bytes();
  let mut i = 0usize;
  while i < b.len() {
    if b[i] != b'(' {
      i += 1;
      continue;
    }
    let mut j = skip_ws(b, i + 1);
    let n1_start = j;
    j = skip_num(b, j);
    if j == n1_start {
      i += 1;
      continue;
    }
    let Ok(n1) = path[n1_start..j].parse::<f64>() else {
      i += 1;
      continue;
    };
    j = skip_ws(b, j);
    let (op, oplen) = if path[j..].starts_with("==") {
      ("==", 2)
    } else if path[j..].starts_with("!=") {
      ("!=", 2)
    } else {
      i += 1;
      continue;
    };
    j += oplen;
    j = skip_ws(b, j);
    let n2_start = j;
    j = skip_num(b, j);
    if j == n2_start {
      i += 1;
      continue;
    }
    let Ok(n2) = path[n2_start..j].parse::<f64>() else {
      i += 1;
      continue;
    };
    j = skip_ws(b, j);
    if b.get(j) != Some(&b')') {
      i += 1;
      continue;
    }
    let equal = (n1 - n2).abs() < f64::EPSILON;
    let always_false = (op == "==" && !equal) || (op == "!=" && equal);
    if always_false {
      return Some((i, j + 1));
    }
    i += 1;
  }
  None
}

fn skip_num(b: &[u8], mut i: usize) -> usize {
  if i < b.len() && b[i] == b'-' {
    i += 1;
  }
  while i < b.len() && (b[i].is_ascii_digit() || b[i] == b'.') {
    i += 1;
  }
  i
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
