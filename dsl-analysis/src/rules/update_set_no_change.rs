//! sql149: `UPDATE t SET x = x` -- assigning a column to itself. The
//! row gets an unnecessary write (and a trigger fires) for no
//! semantic change.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql149"
  }
  fn default_severity(&self) -> Severity {
    Severity::Hint
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let start: usize = u32::from(stmt.range.start()) as usize;
    let end: usize = (u32::from(stmt.range.end()) as usize).min(source.len());
    let body = &source[start..end];
    let upper = body.to_ascii_uppercase();
    let trimmed = upper.trim_start();
    if !trimmed.starts_with("UPDATE ") {
      return;
    }
    // Find SET <list> WHERE / ;.
    let Some(set_at) = upper.find(" SET ") else { return };
    let after_set = set_at + 5;
    let end_at = upper[after_set..].find(" WHERE ").map(|p| after_set + p).unwrap_or(end - start);
    let set_text = &body[after_set..end_at];
    // Split on top-level commas, then check each `col = <expr>`.
    let bytes = set_text.as_bytes();
    let n = bytes.len();
    let mut depth = 0i32;
    let mut start_a = 0usize;
    let mut idx = 0usize;
    while idx <= n {
      let at_end = idx == n;
      let c = if at_end { b',' } else { bytes[idx] };
      match c {
        b'(' => {
          depth += 1;
          idx += 1;
          continue;
        },
        b')' => {
          depth -= 1;
          idx += 1;
          continue;
        },
        b'\'' if !at_end => {
          idx += 1;
          while idx < n && bytes[idx] != b'\'' {
            idx += 1;
          }
          if idx < n {
            idx += 1;
          }
          continue;
        },
        _ => {},
      }
      if c == b',' && depth == 0 {
        let assign = &set_text[start_a..idx];
        if let Some(eq_at) = assign.find('=') {
          let lhs = assign[..eq_at].trim();
          let rhs = assign[eq_at + 1..].trim();
          if !lhs.is_empty() && lhs.eq_ignore_ascii_case(rhs) {
            let abs_start = start + after_set + start_a + (assign.len() - assign.trim_start().len());
            let abs_end =
              start + after_set + start_a + assign.trim_end().len() + (assign.len() - assign.trim_start().len());
            out.push(Diagnostic {
              code: "sql149",
              severity: Severity::Hint,
              message: format!(
                "`{lhs} = {rhs}` is a no-op assignment -- the row is rewritten and triggers fire for nothing"
              ),
              range: text_size::TextRange::new((abs_start as u32).into(), (abs_end as u32).into()),
            });
            return;
          }
        }
        start_a = idx + 1;
      }
      if at_end {
        break;
      }
      idx += 1;
    }
  }
}
