//! sql631: `last_value(...)` / `nth_value(...)` over a window that has an
//! `ORDER BY` but no explicit frame clause. The default frame is
//! `RANGE BETWEEN UNBOUNDED PRECEDING AND CURRENT ROW`, so `last_value` returns
//! the *current* row's value, not the partition's last -- a classic footgun.
//! Add an explicit frame, e.g. `ROWS BETWEEN UNBOUNDED PRECEDING AND UNBOUNDED
//! FOLLOWING`.
//!
//! `first_value` is intentionally not flagged: the default frame already starts
//! at the partition start, so it returns the correct value.

use crate::clause_scan::is_word;
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

const FNS: &[&str] = &["last_value(", "nth_value("];

/// Index of the `)` matching the `(` at `open`.
fn matching(bytes: &[u8], open: usize) -> Option<usize> {
  let mut depth = 0i32;
  let mut i = open;
  while i < bytes.len() {
    match bytes[i] {
      b'(' => depth += 1,
      b')' => {
        depth -= 1;
        if depth == 0 {
          return Some(i);
        }
      }
      _ => {}
    }
    i += 1;
  }
  None
}

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql631"
  }
  fn default_severity(&self) -> Severity {
    Severity::Warning
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, body) = crate::stmt_body(stmt, source);
    let lower = body.to_ascii_lowercase();
    let lb = lower.as_bytes();
    let n = lb.len();
    for &needle in FNS {
      let nlen = needle.len();
      let mut from = 0usize;
      while let Some(rel) = lower[from..].find(needle) {
        let at = from + rel;
        from = at + nlen;
        if at > 0 && (lb[at - 1].is_ascii_alphanumeric() || lb[at - 1] == b'_') {
          continue;
        }
        // skip the function's own argument list
        let Some(arg_close) = matching(lb, at + nlen - 1) else { continue };
        // expect `over (`
        let mut j = arg_close + 1;
        while j < n && lb[j].is_ascii_whitespace() {
          j += 1;
        }
        if j + 4 > n || &lb[j..j + 4] != b"over" || lb.get(j + 4).is_some_and(|&b| is_word(b as char)) {
          continue;
        }
        j += 4;
        while j < n && lb[j].is_ascii_whitespace() {
          j += 1;
        }
        if j >= n || lb[j] != b'(' {
          continue; // named window (`OVER w`) -- frame not visible here
        }
        let Some(win_close) = matching(lb, j) else { continue };
        let spec = &lower[j + 1..win_close];
        if spec.contains("order by")
          && !spec.contains("rows")
          && !spec.contains("range")
          && !spec.contains("groups")
        {
          let name = needle.trim_end_matches('(');
          out.push(Diagnostic {
            code: "sql631",
            severity: Severity::Warning,
            message: format!("`{name}` with ORDER BY but no frame uses the default frame (ends at CURRENT ROW) -- add an explicit `ROWS BETWEEN UNBOUNDED PRECEDING AND UNBOUNDED FOLLOWING`"),
            range: crate::range_at(start + at, start + at + name.len()),
          });
        }
      }
    }
  }
}
