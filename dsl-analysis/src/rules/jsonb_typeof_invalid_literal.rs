//! sql709: `jsonb_typeof(x) = 'int'` -- comparing json(b)_typeof to a string
//! that is not one of the values it can return. jsonb_typeof / json_typeof
//! only ever yield 'object', 'array', 'string', 'number', 'boolean' or 'null',
//! so a comparison to anything else is a constant (always false for `=`,
//! always true for `<>`). Usually a wrong type name (`'int'`, `'text'`,
//! `'bool'`).

use crate::clause_scan::is_word;
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

const VALID: &[&str] = &["object", "array", "string", "number", "boolean", "null"];

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql709"
  }
  fn default_severity(&self) -> Severity {
    Severity::Warning
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, body, upper) = crate::stmt_body_upper(stmt, source);
    let ub = upper.as_bytes();
    let bb = body.as_bytes();
    let n = ub.len();

    let mut i = 0usize;
    while i < n {
      let flen = if word_at(ub, i, b"JSONB_TYPEOF") {
        12
      } else if word_at(ub, i, b"JSON_TYPEOF") {
        11
      } else {
        i += 1;
        continue;
      };
      let p = skip_ws(ub, i + flen);
      if ub.get(p) != Some(&b'(') {
        i += flen;
        continue;
      }
      let Some(close) = match_paren(ub, p) else { break };
      // `... ) =|<>|!= '<literal>'`
      let q = skip_ws(ub, close + 1);
      let (oplen, neq) = match (ub.get(q), ub.get(q + 1)) {
        (Some(b'='), _) => (1, false),
        (Some(b'<'), Some(b'>')) => (2, true),
        (Some(b'!'), Some(b'=')) => (2, true),
        _ => {
          i = close + 1;
          continue;
        },
      };
      let m = skip_ws(ub, q + oplen);
      if bb.get(m) == Some(&b'\'') {
        let mut k = m + 1;
        while k < n && bb[k] != b'\'' {
          k += 1;
        }
        let content = &body[m + 1..k];
        if !VALID.contains(&content.to_ascii_lowercase().as_str()) {
          let verb = if neq { "always true" } else { "always false" };
          out.push(Diagnostic {
            code: "sql709",
            severity: Severity::Warning,
            message: format!("json_typeof never returns '{content}' -- this comparison is {verb}"),
            range: crate::range_at(start + m, start + (k + 1).min(n)),
          });
        }
        i = (k + 1).min(n);
        continue;
      }
      i = close + 1;
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
