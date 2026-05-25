//! sql217: `SELECT ... LEFT JOIN ... FOR UPDATE` -- the FOR UPDATE
//! locks rows in every joined table even when LEFT JOIN matched no
//! row on the right side. PG returns NULL on the right but still
//! tries to lock; with `OF <alias>` you can restrict but the default
//! form is rarely what the author meant. Suggest FOR UPDATE OF <left>
//! to scope the lock or switch to INNER JOIN.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::{Statement, StatementKind};
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql217"
  }
  fn default_severity(&self) -> Severity {
    Severity::Warning
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let StatementKind::Select(_) = &stmt.kind else { return };
    let start: usize = u32::from(stmt.range.start()) as usize;
    let end: usize = (u32::from(stmt.range.end()) as usize).min(source.len());
    let body = &source[start..end];
    let upper = body.to_ascii_uppercase();
    let has_left = upper.contains("LEFT JOIN") || upper.contains("LEFT OUTER JOIN");
    let has_for_lock = upper.contains("FOR UPDATE") || upper.contains("FOR SHARE")
      || upper.contains("FOR NO KEY UPDATE") || upper.contains("FOR KEY SHARE");
    if !has_left || !has_for_lock { return }
    // If author already scoped via OF <alias>, accept it.
    if upper.contains(" OF ") { return }
    let Some(at) = upper.find("FOR UPDATE")
      .or_else(|| upper.find("FOR SHARE"))
      .or_else(|| upper.find("FOR NO KEY UPDATE"))
      .or_else(|| upper.find("FOR KEY SHARE"))
    else { return };
    let abs_s = start + at;
    let abs_e = abs_s + upper[at..].find(|c: char| c == ';' || c == '\n').unwrap_or(upper.len() - at);
    out.push(Diagnostic {
      code: "sql217",
      severity: Severity::Warning,
      message: "FOR UPDATE with LEFT JOIN locks rows from optional side even when the join didn't match -- scope with FOR UPDATE OF <alias> or switch to INNER JOIN".into(),
      range: text_size::TextRange::new((abs_s as u32).into(), (abs_e as u32).into()),
    });
  }
}
