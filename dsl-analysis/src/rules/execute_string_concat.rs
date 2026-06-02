//! sql507: `EXECUTE '<sql>' || <var>` -- building dynamic SQL by
//! string-concatenating a parameter is a SQL-injection vector. Use
//! `EXECUTE ... USING <var>` for value parameters, or
//! `format('%I' / '%L', ...)` for identifier / literal interpolation
//! that survives malicious input.

use crate::clause_scan::is_word;
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;
use text_size::TextRange;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql507"
  }
  fn default_severity(&self) -> Severity {
    Severity::Warning
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, raw) = crate::stmt_body(stmt, source);
    let bytes = raw.as_bytes();
    let n = bytes.len();
    let mut i = 0usize;
    let mut emitted: std::collections::HashSet<usize> = std::collections::HashSet::new();
    while i + 7 <= n {
      // Find `EXECUTE` keyword (case-insensitive, word-bounded).
      let kw_match = bytes[i..i + 7].eq_ignore_ascii_case(b"EXECUTE")
        && (i == 0 || !is_word(bytes[i - 1] as char))
        && (i + 7 == n || !is_word(bytes[i + 7] as char));
      if !kw_match {
        i += 1;
        continue;
      }
      let exec_at = i;
      // Determine the end of the EXECUTE expression: the next `;` at
      // depth 0 outside any string.
      let mut p = exec_at + 7;
      let mut depth: i32 = 0;
      let arg_start = p;
      while p < n {
        match bytes[p] {
          b'\'' => {
            p += 1;
            while p < n {
              if bytes[p] == b'\'' {
                if p + 1 < n && bytes[p + 1] == b'\'' {
                  p += 2;
                  continue;
                }
                p += 1;
                break;
              }
              p += 1;
            }
            continue;
          },
          b'(' => depth += 1,
          b')' => depth -= 1,
          b';' if depth == 0 => break,
          _ => {},
        }
        p += 1;
      }
      let arg_end = p;
      let arg = &raw[arg_start..arg_end];
      // Skip if the argument is wrapped in `format(...)` (the safe
      // idiomatic pattern).
      let arg_trimmed = arg.trim_start();
      if arg_trimmed.to_ascii_lowercase().starts_with("format(") {
        i = arg_end.max(exec_at + 7);
        continue;
      }
      // Search the argument for a top-level `||` operator. We need
      // to skip over string literals (anything `||` inside a quoted
      // literal is just two characters).
      let abytes = arg.as_bytes();
      let an = abytes.len();
      let mut j = 0usize;
      let mut found_concat = false;
      while j < an {
        match abytes[j] {
          b'\'' => {
            j += 1;
            while j < an {
              if abytes[j] == b'\'' {
                if j + 1 < an && abytes[j + 1] == b'\'' {
                  j += 2;
                  continue;
                }
                j += 1;
                break;
              }
              j += 1;
            }
            continue;
          },
          b'|' if j + 1 < an && abytes[j + 1] == b'|' => {
            found_concat = true;
            break;
          },
          _ => {},
        }
        j += 1;
      }
      if found_concat && emitted.insert(exec_at) {
        let abs_s = start + exec_at;
        let abs_e = start + arg_end;
        out.push(Diagnostic {
          code: "sql507",
          severity: Severity::Warning,
          message: "`EXECUTE` with string concatenation (`||`) is a SQL-injection vector -- the appended value is spliced in unescaped. Use `EXECUTE ... USING <value>` for value parameters, or `format('... %I ... %L ...', ident, literal)` for identifier/literal interpolation that escapes properly.".into(),
          range: TextRange::new((abs_s as u32).into(), (abs_e as u32).into()),
        });
      }
      i = arg_end.max(exec_at + 7);
    }
  }
}
