//! sql751: `X'1G'` -- a hexadecimal string literal containing a non-hex
//! character. PostgreSQL raises 22P03 ("... is not a valid hexadecimal digit")
//! at parse. Usually a typo. (Companion to sql750 bit_string_invalid_digit.)

use crate::clause_scan::is_word;
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql751"
  }
  fn default_severity(&self) -> Severity {
    Severity::Error
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, _body, upper) = crate::stmt_body_upper(stmt, source);
    let ub = upper.as_bytes();
    let n = ub.len();

    let mut i = 0usize;
    while i < n {
      if ub[i] == b'\'' {
        i += 1;
        while i < n && ub[i] != b'\'' {
          i += 1;
        }
        i += 1;
        continue;
      }
      // `X'...'` hex-string literal (X not part of an identifier).
      if ub[i] == b'X' && ub.get(i + 1) == Some(&b'\'') && (i == 0 || !is_word(ub[i - 1] as char)) {
        let cs = i + 2;
        let mut j = cs;
        while j < n && ub[j] != b'\'' {
          j += 1;
        }
        if j < n && ub[cs..j].iter().any(|&c| !c.is_ascii_hexdigit()) {
          out.push(Diagnostic {
            code: "sql751",
            severity: Severity::Error,
            message: "hex-string literal contains a non-hexadecimal digit (only 0-9, A-F are allowed)".into(),
            range: crate::range_at(start + i, start + j + 1),
          });
        }
        i = j + 1;
        continue;
      }
      i += 1;
    }
  }
}
