//! sql282: `WHERE 1=1 AND ...` / `WHERE TRUE AND ...` -- the
//! leading tautology is a placeholder common in dynamic-SQL
//! generators where every real condition gets prepended with
//! `AND`. In hand-written static SQL it's just noise.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql282"
  }
  fn default_severity(&self) -> Severity {
    Severity::Hint
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let start: usize = u32::from(stmt.range.start()) as usize;
    let end: usize = (u32::from(stmt.range.end()) as usize).min(source.len());
    let body = &source[start..end];
    let upper = body.to_ascii_uppercase();
    let Some(where_at) = upper.find("WHERE ") else { return };
    let after = where_at + "WHERE ".len();
    let tail = body[after..].trim_start();
    let tail_upper = tail.to_ascii_uppercase();
    let triggers = ["1=1", "1 = 1", "TRUE"];
    let mut hit_len = 0usize;
    for needle in triggers {
      if tail_upper.starts_with(needle) {
        let post = &tail_upper[needle.len()..];
        let post_trim = post.trim_start();
        if post_trim.starts_with("AND ") || post_trim.starts_with("OR ") || post_trim.is_empty() {
          hit_len = needle.len();
          break;
        }
      }
    }
    if hit_len == 0 { return }
    let abs_s = start + after + (body[after..].len() - tail.len());
    let abs_e = abs_s + hit_len;
    out.push(Diagnostic {
      code: "sql282",
      severity: Severity::Hint,
      message: "WHERE clause starts with tautology placeholder (`1=1` / `TRUE`) -- harmless but adds noise in static SQL; drop it".into(),
      range: text_size::TextRange::new((abs_s as u32).into(), (abs_e as u32).into()),
    });
  }
}
