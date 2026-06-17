//! sql680: `substring(s FROM n FOR 0)` / `substr(s, n, 0)` -- a length of 0
//! always returns the empty string, so the call is a constant `''`. Usually a
//! typo for a real length, or a sign the length was computed to 0 by mistake.
//! (Companion to sql443 substring_negative_length, sql479 substring_zero_start
//! and sql679 left_right_zero.)

use crate::clause_scan::is_word;
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql680"
  }
  fn default_severity(&self) -> Severity {
    Severity::Warning
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, _body, upper) = crate::stmt_body_upper(stmt, source);
    let ub = upper.as_bytes();
    let n = ub.len();

    let mut i = 0usize;
    while i < n {
      let fname = if word_at(ub, i, b"SUBSTRING") {
        9
      } else if word_at(ub, i, b"SUBSTR") {
        6
      } else {
        i += 1;
        continue;
      };
      let p = skip_ws(ub, i + fname);
      if ub.get(p) != Some(&b'(') {
        i += fname;
        continue;
      }
      let Some(close) = match_paren(ub, p) else { break };

      // SQL keyword form: `... FOR <len>`.
      let len_range = if let Some(forpos) = top_level_for(ub, p + 1, close) {
        trim_range(ub, forpos + 3, close)
      } else {
        // Function form: third positional argument.
        let args = top_level_args(ub, p, close);
        (args.len() == 3).then(|| trim_range(ub, args[2].0, args[2].1)).flatten()
      };

      if let Some((s, e)) = len_range
        && &upper[s..e] == "0"
      {
        out.push(Diagnostic {
          code: "sql680",
          severity: Severity::Warning,
          message: "substring length of 0 is always the empty string -- check the length".into(),
          range: crate::range_at(start + s, start + e),
        });
      }
      i = close + 1;
    }
  }
}

/// Position of a top-level `FOR` keyword between `from` and `to`.
fn top_level_for(ub: &[u8], from: usize, to: usize) -> Option<usize> {
  let mut depth = 0i32;
  let mut i = from;
  while i < to {
    match ub[i] {
      b'(' | b'[' => depth += 1,
      b')' | b']' => depth -= 1,
      b'\'' => {
        i += 1;
        while i < to && ub[i] != b'\'' {
          i += 1;
        }
      },
      b'F' if depth == 0 && word_at(ub, i, b"FOR") => return Some(i),
      _ => {},
    }
    i += 1;
  }
  None
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
