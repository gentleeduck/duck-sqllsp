//! sql311: `string_agg(col, ',')` / `array_agg(col)` /
//! `json_agg(col)` / `jsonb_agg(col)` without an `ORDER BY` clause
//! inside the aggregate -- concatenation order is non-deterministic
//! and depends on the plan.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

const FNS: &[&str] =
  &["string_agg(", "array_agg(", "json_agg(", "jsonb_agg(", "json_object_agg(", "jsonb_object_agg(", "xmlagg("];

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql311"
  }
  fn default_severity(&self) -> Severity {
    Severity::Hint
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, raw) = crate::stmt_body(stmt, source);
    let body_owned = crate::textutil::strip_comments_only(raw);
    let body = body_owned.as_str();
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
        let inner = &body[open + 1..close];
        let inner_upper = inner.to_ascii_uppercase();
        if inner_upper.contains("ORDER BY") {
          from = close + 1;
          continue;
        }
        out.push(Diagnostic {
          code: "sql311",
          severity: Severity::Hint,
          message: format!(
            "`{}` without ORDER BY -- concatenation order is non-deterministic; add `ORDER BY <col>` inside the aggregate",
            fname.trim_end_matches('('),
          ),
          range: crate::range_at(start + at, start + close + 1),
        });
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
