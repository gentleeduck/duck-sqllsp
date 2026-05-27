//! sql255: `ROW_NUMBER() OVER ()` / `RANK() OVER ()` / `LAG()
//! OVER ()` without an ORDER BY in the window definition. The
//! ranking / position is undefined and changes between executions.
//! PG accepts it, but the result is non-deterministic.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

const ORDER_DEPENDENT: &[&str] = &[
  "row_number(",
  "rank(",
  "dense_rank(",
  "ntile(",
  "lag(",
  "lead(",
  "first_value(",
  "last_value(",
  "nth_value(",
  "percent_rank(",
  "cume_dist(",
];

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql255"
  }
  fn default_severity(&self) -> Severity {
    Severity::Warning
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let start: usize = u32::from(stmt.range.start()) as usize;
    let end: usize = (u32::from(stmt.range.end()) as usize).min(source.len());
    let body = &source[start..end];
    let lower = body.to_ascii_lowercase();
    let upper = body.to_ascii_uppercase();
    for &fn_name in ORDER_DEPENDENT {
      let mut from = 0usize;
      while let Some(rel) = lower[from..].find(fn_name) {
        let at = from + rel;
        if at > 0 {
          let prev = body.as_bytes()[at - 1] as char;
          if prev.is_ascii_alphanumeric() || prev == '_' {
            from = at + fn_name.len();
            continue;
          }
        }
        // Find the matching call paren close.
        let open = at + fn_name.len() - 1;
        let Some(call_close) = find_matching_paren(body, open) else { break };
        // Look for OVER (...) immediately after.
        let after = call_close + 1;
        let post = body[after..].trim_start();
        let post_upper = post.to_ascii_uppercase();
        if !post_upper.starts_with("OVER") {
          from = after;
          continue;
        }
        let over_at = after + (body[after..].len() - post.len());
        let after_over = over_at + "OVER".len();
        let win_post = body[after_over..].trim_start();
        if !win_post.starts_with('(') {
          from = after_over;
          continue;
        }
        let win_open = after_over + (body[after_over..].len() - win_post.len());
        let Some(win_close) = find_matching_paren(body, win_open) else { break };
        let win = &upper[win_open + 1..win_close];
        if win.contains("ORDER BY") {
          from = win_close + 1;
          continue;
        }
        out.push(Diagnostic {
          code: "sql255",
          severity: Severity::Warning,
          message: format!(
            "`{}` OVER () without ORDER BY -- result is non-deterministic, depends on plan choice",
            fn_name.trim_end_matches('('),
          ),
          range: text_size::TextRange::new(((start + at) as u32).into(), ((start + win_close + 1) as u32).into()),
        });
        from = win_close + 1;
      }
    }
  }
}

fn find_matching_paren(s: &str, open: usize) -> Option<usize> {
  let bytes = s.as_bytes();
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
      },
      b'\'' => {
        i += 1;
        while i < bytes.len() && bytes[i] != b'\'' {
          i += 1
        }
      },
      _ => {},
    }
    i += 1;
  }
  None
}
