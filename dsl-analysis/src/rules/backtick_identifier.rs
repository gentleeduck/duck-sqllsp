//! sql600: `` `col` `` -- backtick-quoted identifiers. Backticks are MySQL's
//! identifier quoting; PostgreSQL quotes identifiers with double quotes
//! (`"col"`) and rejects backticks as a syntax error. Backticks that appear
//! inside a single-quoted string literal are skipped (there they're ordinary
//! characters, not identifier delimiters).

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql600"
  }
  fn default_severity(&self) -> Severity {
    Severity::Error
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, body) = crate::stmt_body(stmt, source);
    let bytes = body.as_bytes();
    let n = bytes.len();
    let mut i = 0usize;
    let mut in_str = false;
    while i < n {
      let c = bytes[i];
      if in_str {
        if c == b'\'' {
          // doubled '' is an escaped quote, still inside the string
          if i + 1 < n && bytes[i + 1] == b'\'' {
            i += 2;
            continue;
          }
          in_str = false;
        }
        i += 1;
        continue;
      }
      match c {
        b'\'' => {
          in_str = true;
          i += 1;
        }
        b'`' => {
          // span to the matching backtick (or end of statement)
          let open = i;
          let mut j = i + 1;
          while j < n && bytes[j] != b'`' {
            j += 1;
          }
          let end = if j < n { j + 1 } else { n };
          out.push(Diagnostic {
            code: "sql600",
            severity: Severity::Error,
            message: "backtick-quoted identifier -- backticks are MySQL; PostgreSQL quotes identifiers with double quotes (e.g. \"col\")".into(),
            range: crate::range_at(start + open, start + end),
          });
          i = end;
        }
        _ => i += 1,
      }
    }
  }
}
