//! sql096: `INSERT INTO t VALUES (1, 2, );` -- trailing comma before
//! the closing paren in a VALUES tuple. PG rejects this at parse time.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql096"
  }
  fn default_severity(&self) -> Severity {
    Severity::Error
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let start: usize = u32::from(stmt.range.start()) as usize;
    let end: usize = (u32::from(stmt.range.end()) as usize).min(source.len());
    // Important: do NOT strip string literals here; that turns
    // `('a', 'b')` into `(   ,    )` which then has a "trailing"
    // comma right before `)`. Keep the raw body for this rule.
    let body = &source[start..end];
    let upper = body.to_ascii_uppercase();
    // Only inspect statements that mention VALUES (INSERT, INSERT
    // ... ON CONFLICT, multi-row VALUES expressions).
    if !upper.contains("INSERT") && !upper.contains("VALUES") {
      return;
    }
    let Some(values_at) = upper.find("VALUES") else { return };
    let bytes = body.as_bytes();
    let n = bytes.len();
    let mut k = values_at + 6;
    while k < n && bytes[k].is_ascii_whitespace() {
      k += 1;
    }
    if k >= n || bytes[k] != b'(' {
      return;
    }
    // Scan the VALUES tuple chain. For each `(...)`, check whether
    // the last non-whitespace char before `)` is `,`.
    let mut i = k;
    while i < n {
      if bytes[i] != b'(' {
        i += 1;
        continue;
      }
      let mut depth = 0i32;
      let open = i;
      while i < n {
        match bytes[i] {
          b'(' => depth += 1,
          b')' => {
            depth -= 1;
            if depth == 0 {
              break;
            }
          },
          b'\'' => {
            i += 1;
            while i < n && bytes[i] != b'\'' {
              i += 1;
            }
          },
          _ => {},
        }
        i += 1;
      }
      if i >= n {
        break;
      }
      let close = i;
      // Walk backwards from close-1, skipping whitespace.
      let mut j = close;
      while j > open + 1 {
        j -= 1;
        if !bytes[j].is_ascii_whitespace() {
          break;
        }
      }
      if bytes[j] == b',' {
        let abs_start = start + j;
        let abs_end = start + j + 1;
        out.push(Diagnostic {
          code: "sql096",
          severity: Severity::Error,
          message: "trailing `,` before `)` in VALUES tuple -- PG rejects this at parse time".into(),
          range: text_size::TextRange::new((abs_start as u32).into(), (abs_end as u32).into()),
        });
        return;
      }
      i += 1;
    }
  }
}
