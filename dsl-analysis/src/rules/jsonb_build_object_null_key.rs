//! sql783: `jsonb_build_object(NULL, 1, ...)` -- a literal `NULL` in a
//! key position. PostgreSQL raises "null value not allowed for object
//! key" at runtime. Sibling to the existing
//! jsonb_build_object_duplicate_key.

use crate::clause_scan::{is_word, split_top_level};
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql783"
  }
  fn default_severity(&self) -> Severity {
    Severity::Error
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, _body, upper) = crate::stmt_body_upper(stmt, source);
    let ub = upper.as_bytes();
    let mut i = 0usize;
    while i < ub.len() {
      if !word_at(ub, i, b"JSONB_BUILD_OBJECT") {
        i += 1;
        continue;
      }
      let p = skip_ws(ub, i + "JSONB_BUILD_OBJECT".len());
      if ub.get(p) != Some(&b'(') {
        i += "JSONB_BUILD_OBJECT".len();
        continue;
      }
      let Some(close) = match_paren(ub, p) else { break };
      let args = split_top_level(&upper[p + 1..close]);
      for (idx, (arg, off)) in args.iter().enumerate() {
        if idx % 2 != 0 {
          continue;
        }
        if arg.trim() == "NULL" {
          let lead = arg.len() - arg.trim_start().len();
          let abs = p + 1 + off + lead;
          out.push(Diagnostic {
            code: "sql783",
            severity: Severity::Error,
            message: "jsonb_build_object key is a literal NULL -- rejected at runtime".into(),
            range: crate::range_at(start + abs, start + abs + 4),
          });
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
