//! sql157: `RAISE EXCEPTION ... USING ERRCODE = my_var` -- an
//! unquoted identifier as the errcode value is almost always a typo
//! for a SQLSTATE string literal like `'P0001'` or `'23505'`.

use crate::{Diagnostic, LintRule, Severity};
use crate::textutil::is_word;
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql157"
  }
  fn default_severity(&self) -> Severity {
    Severity::Warning
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, body, upper) = crate::stmt_body_upper(stmt, source);
    let bytes = body.as_bytes();
    let n = bytes.len();
    // Find every `ERRCODE =` (or `ERRCODE  =`) inside USING.
    let mut i = 0;
    while i + 7 <= n {
      if upper.as_bytes()[i..i + 7].eq_ignore_ascii_case(b"ERRCODE")
        && (i == 0 || !is_word(bytes[i - 1] as char))
        && (i + 7 == n || !is_word(bytes[i + 7] as char))
      {
        let mut j = i + 7;
        while j < n && bytes[j].is_ascii_whitespace() {
          j += 1;
        }
        if j >= n || bytes[j] != b'=' {
          i += 1;
          continue;
        }
        j += 1;
        while j < n && bytes[j].is_ascii_whitespace() {
          j += 1;
        }
        if j >= n {
          return;
        }
        // Quoted string => OK.
        if bytes[j] == b'\'' {
          i = j + 1;
          continue;
        }
        // SQLSTATE func / identifier ref / numeric literal?
        // Numeric literal (e.g. 23505) without quotes -- still a
        // typed mismatch. Flag.
        let ident_start = j;
        while j < n && (is_word(bytes[j] as char) || bytes[j].is_ascii_digit()) {
          j += 1;
        }
        if j > ident_start {
          let abs_start = start + ident_start;
          let abs_end = start + j;
          out.push(Diagnostic {
                        code: "sql157",
                        severity: Severity::Warning,
                        message: "RAISE USING ERRCODE = `<unquoted>` -- expects a SQLSTATE string literal (e.g. `'P0001'`) or a named condition; unquoted identifiers are treated as variable references".into(),
                        range: crate::range_at(abs_start, abs_end),
                    });
          return;
        }
        i = j;
        continue;
      }
      i += 1;
    }
  }
}

