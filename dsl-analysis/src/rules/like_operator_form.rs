//! sql554: the operator spellings of LIKE -- `~~`, `~~*`, `!~~`, `!~~*` -- in
//! place of `LIKE` / `ILIKE` / `NOT LIKE` / `NOT ILIKE`. They're the internal
//! operators PG uses to implement those keywords; valid, but obscure and
//! easily confused with the regex operators (`~`, `~*`). Prefer the keyword.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql554"
  }
  fn default_severity(&self) -> Severity {
    Severity::Hint
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, body) = crate::stmt_body(stmt, source);
    let bytes = body.as_bytes();
    let n = bytes.len();
    let mut i = 0usize;
    while i < n {
      match bytes[i] {
        b'\'' => {
          i += 1;
          while i < n && bytes[i] != b'\'' {
            i += 1;
          }
        },
        b'~' if bytes.get(i + 1) == Some(&b'~') => {
          let neg = i > 0 && bytes[i - 1] == b'!';
          let ci = bytes.get(i + 2) == Some(&b'*');
          let op_start = if neg { i - 1 } else { i };
          let op_end = if ci { i + 3 } else { i + 2 };
          let keyword = match (neg, ci) {
            (false, false) => "LIKE",
            (false, true) => "ILIKE",
            (true, false) => "NOT LIKE",
            (true, true) => "NOT ILIKE",
          };
          let op = &body[op_start..op_end];
          out.push(Diagnostic {
            code: "sql554",
            severity: Severity::Hint,
            message: format!("`{op}` is the operator form of {keyword} -- prefer the `{keyword}` keyword"),
            range: crate::range_at(start + op_start, start + op_end),
          });
          i = op_end;
          continue;
        },
        _ => {},
      }
      i += 1;
    }
  }
}
