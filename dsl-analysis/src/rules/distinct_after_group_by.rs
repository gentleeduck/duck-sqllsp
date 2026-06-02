//! sql120: `SELECT DISTINCT ... GROUP BY ...` -- GROUP BY already
//! produces unique rows, the DISTINCT is dead weight.

use crate::{Diagnostic, LintRule, Severity};
use crate::textutil::is_word;
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql120"
  }
  fn default_severity(&self) -> Severity {
    Severity::Hint
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, raw) = crate::stmt_body(stmt, source);
    let body_owned = crate::textutil::strip_noise_full(raw);
    let body = body_owned.as_str();
    let upper = body.to_ascii_uppercase();
    let bytes = upper.as_bytes();
    // Look for `SELECT DISTINCT` followed by `GROUP BY` later in
    // the same statement. Skip `DISTINCT ON` which is semantically
    // different.
    let Some(sel) = upper.find("SELECT") else { return };
    let after_sel = sel + 6;
    if after_sel >= upper.len() {
      return;
    }
    let after_trim = upper[after_sel..].trim_start();
    if !after_trim.starts_with("DISTINCT") {
      return;
    }
    // DISTINCT ON is a different beast.
    let after_dist = after_trim[8..].trim_start();
    if after_dist.starts_with("ON") {
      return;
    }
    let dist_at = sel + 6 + (upper[after_sel..].len() - after_trim.len());
    // Look for GROUP BY after.
    if !upper[dist_at + 8..].contains("GROUP BY") {
      return;
    }
    let prev_ok = dist_at == 0 || !is_word(bytes[dist_at - 1] as char);
    let next_ok = dist_at + 8 == bytes.len() || !is_word(bytes[dist_at + 8] as char);
    if !(prev_ok && next_ok) {
      return;
    }
    let abs_start = start + dist_at;
    let abs_end = start + dist_at + 8;
    out.push(Diagnostic {
      code: "sql120",
      severity: Severity::Hint,
      message: "DISTINCT is redundant when GROUP BY is present -- GROUP BY already produces unique rows".into(),
      range: crate::range_at(abs_start, abs_end),
    });
  }
}

