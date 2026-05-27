//! sql290: `percentile_cont(0.5)` / `percentile_disc(0.5)` /
//! `mode()` without the required `WITHIN GROUP (ORDER BY ...)`
//! clause. These are ordered-set aggregates; PG raises 42883 at
//! parse without WITHIN GROUP.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

const FNS: &[&str] = &["percentile_cont(", "percentile_disc(", "mode("];

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql290"
  }
  fn default_severity(&self) -> Severity {
    Severity::Error
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let start: usize = u32::from(stmt.range.start()) as usize;
    let end: usize = (u32::from(stmt.range.end()) as usize).min(source.len());
    let body = &source[start..end];
    let lower = body.to_ascii_lowercase();
    for &fname in FNS {
      let mut from = 0usize;
      while let Some(rel) = lower[from..].find(fname) {
        let at = from + rel;
        if at > 0 {
          let prev = body.as_bytes()[at - 1] as char;
          if prev.is_ascii_alphanumeric() || prev == '_' {
            from = at + fname.len();
            continue;
          }
        }
        let open = at + fname.len() - 1;
        let Some(close) = find_matching_paren(body, open) else { break };
        let after = body[close + 1..].trim_start();
        let after_upper = after.to_ascii_uppercase();
        if !after_upper.starts_with("WITHIN GROUP") {
          out.push(Diagnostic {
            code: "sql290",
            severity: Severity::Error,
            message: format!(
              "`{}` is an ordered-set aggregate -- needs `WITHIN GROUP (ORDER BY <col>)`",
              fname.trim_end_matches('('),
            ),
            range: text_size::TextRange::new(((start + at) as u32).into(), ((start + close + 1) as u32).into()),
          });
        }
        from = close + 1;
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
