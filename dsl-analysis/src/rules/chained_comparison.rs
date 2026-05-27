//! sql267: `a = b = c` chained comparison. SQL doesn't have Python-
//! style chaining; this parses as `(a = b) = c` which compares a
//! boolean to c. Almost always a logic bug. Hint: `a = b AND b = c`.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql267"
  }
  fn default_severity(&self) -> Severity {
    Severity::Warning
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let start: usize = u32::from(stmt.range.start()) as usize;
    let end: usize = (u32::from(stmt.range.end()) as usize).min(source.len());
    let body = &source[start..end];
    let bytes = body.as_bytes();
    // Look for "tok1 = tok2 = tok3" pattern at top paren depth.
    let mut i = 0usize;
    let mut depth = 0i32;
    while i < bytes.len() {
      match bytes[i] {
        b'(' => depth += 1,
        b')' => depth -= 1,
        b'\'' => {
          i += 1;
          while i < bytes.len() && bytes[i] != b'\'' {
            i += 1
          }
        },
        b'='
          if depth == 0
            && i + 1 < bytes.len()
            && bytes[i + 1] != b'='
            && bytes[i + 1] != b'>'
            && (i == 0 || (bytes[i - 1] != b'!' && bytes[i - 1] != b'<' && bytes[i - 1] != b'>')) =>
        {
          // Find next non-space identifier, then skip past it and check for another `=`.
          let mut k = i + 1;
          while k < bytes.len() && bytes[k].is_ascii_whitespace() {
            k += 1
          }
          while k < bytes.len()
            && (bytes[k].is_ascii_alphanumeric() || bytes[k] == b'_' || bytes[k] == b'.' || bytes[k] == b'"')
          {
            k += 1
          }
          while k < bytes.len() && bytes[k].is_ascii_whitespace() {
            k += 1
          }
          if k < bytes.len() && bytes[k] == b'=' && k + 1 < bytes.len() && bytes[k + 1] != b'=' && bytes[k + 1] != b'>'
          {
            // Walk back to find the lhs start of the first `=`.
            let mut s = i;
            while s > 0 && (bytes[s - 1].is_ascii_whitespace()) {
              s -= 1
            }
            while s > 0
              && (bytes[s - 1].is_ascii_alphanumeric()
                || bytes[s - 1] == b'_'
                || bytes[s - 1] == b'.'
                || bytes[s - 1] == b'"')
            {
              s -= 1
            }
            let mut e = k + 1;
            while e < bytes.len() && bytes[e].is_ascii_whitespace() {
              e += 1
            }
            while e < bytes.len()
              && (bytes[e].is_ascii_alphanumeric() || bytes[e] == b'_' || bytes[e] == b'.' || bytes[e] == b'"')
            {
              e += 1
            }
            out.push(Diagnostic {
              code: "sql267",
              severity: Severity::Warning,
              message:
                "Chained `=` -- SQL doesn't chain comparisons; this parses as `(a=b)=c` -- rewrite as `a = b AND b = c`"
                  .into(),
              range: text_size::TextRange::new(((start + s) as u32).into(), ((start + e) as u32).into()),
            });
            i = e;
            continue;
          }
        },
        _ => {},
      }
      i += 1;
    }
  }
}
