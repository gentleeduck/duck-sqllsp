//! sql334: `SELECT setseed(...)` without a nearby deterministic guard.
//!
//! `setseed()` is a hidden source of plan instability: subsequent calls
//! to `random()` become deterministic, which is great for tests but
//! dangerous in shared-session contexts because the seed leaks across
//! queries. Hint when a buffer calls setseed without an obvious test
//! marker (`BEGIN;` / `SET LOCAL` / comment-pragma).

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql334"
  }
  fn default_severity(&self) -> Severity {
    Severity::Hint
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let start: usize = u32::from(stmt.range.start()) as usize;
    let end: usize = (u32::from(stmt.range.end()) as usize).min(source.len());
    let raw = &source[start..end];
    let body_owned = crate::textutil::strip_noise_full(raw);
    let body = body_owned.as_str();
    let upper = body.to_ascii_uppercase();
    let Some(at) = upper.find("SETSEED(") else { return };
    let prefix_upper = source[..start].to_ascii_uppercase();
    if prefix_upper.contains("BEGIN") || prefix_upper.contains("SET LOCAL") {
      return;
    }
    let abs_s = start + at;
    let abs_e = abs_s + "SETSEED".len();
    out.push(Diagnostic {
      code: "sql334",
      severity: Severity::Hint,
      message: "setseed() leaks the RNG seed across queries in the same session -- wrap in BEGIN or `SET LOCAL` for test reproducibility".into(),
      range: text_size::TextRange::new((abs_s as u32).into(), (abs_e as u32).into()),
    });
  }
}
