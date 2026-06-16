//! sql627: the MySQL infix operators `XOR` (logical exclusive-or) and `DIV`
//! (integer division). Neither is a PostgreSQL operator, so both raise a syntax
//! error. Replace `a XOR b` with `a <> b` (booleans) or `a # b` (bitwise), and
//! `a DIV b` with `a / b` (integer operands) or `div(a, b)`.

use crate::clause_scan::is_word;
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

const OPS: &[(&str, &str)] = &[
  ("XOR", "PostgreSQL has no XOR -- use `<>` (booleans) or `#` (bitwise)"),
  ("DIV", "PostgreSQL has no DIV operator -- use `/` (integer operands) or `div(a, b)`"),
];

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql627"
  }
  fn default_severity(&self) -> Severity {
    Severity::Error
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, _body, upper) = crate::stmt_body_upper(stmt, source);
    let ub = upper.as_bytes();
    let n = ub.len();
    for &(op, msg) in OPS {
      let len = op.len();
      let mut i = 0usize;
      while i + len <= n {
        // require a word boundary on both sides AND surrounding whitespace, so
        // this only matches the infix-operator usage, not a stray identifier.
        if &ub[i..i + len] == op.as_bytes()
          && i > 0
          && ub[i - 1].is_ascii_whitespace()
          && i + len < n
          && ub[i + len].is_ascii_whitespace()
          && !is_word(ub[i - 1] as char)
        {
          out.push(Diagnostic {
            code: "sql627",
            severity: Severity::Error,
            message: format!("`{op}` is a MySQL operator -- {msg}"),
            range: crate::range_at(start + i, start + i + len),
          });
          i += len;
          continue;
        }
        i += 1;
      }
    }
  }
}
