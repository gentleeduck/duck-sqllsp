//! sql280: `ALTER TABLE t ADD CONSTRAINT c CHECK (...)` without
//! `NOT VALID`. PG scans every existing row to validate, holding
//! AccessExclusiveLock the whole time. On big tables that's a
//! sustained outage. Pattern: ADD CONSTRAINT ... NOT VALID + later
//! `VALIDATE CONSTRAINT` (only ShareUpdateExclusiveLock).

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql280"
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
    if !upper.trim_start().starts_with("ALTER TABLE") {
      return;
    }
    let has_add_constraint_check = upper.contains("ADD CONSTRAINT") && upper.contains("CHECK");
    let has_add_constraint_fk = upper.contains("ADD CONSTRAINT") && upper.contains("FOREIGN KEY");
    // Also catch the unnamed form `ALTER TABLE t ADD CHECK (...)` /
    // `ADD FOREIGN KEY (...)` -- the validation cost is identical.
    let has_add_check_unnamed = !has_add_constraint_check && upper.contains("ADD CHECK");
    let has_add_fk_unnamed = !has_add_constraint_fk && upper.contains("ADD FOREIGN KEY");
    if !(has_add_constraint_check || has_add_constraint_fk || has_add_check_unnamed || has_add_fk_unnamed) {
      return;
    }
    if upper.contains("NOT VALID") {
      return;
    }
    let at = upper.find("ADD CONSTRAINT").or_else(|| upper.find("ADD CHECK")).or_else(|| upper.find("ADD FOREIGN KEY"));
    let Some(at) = at else { return };
    let abs_s = start + at;
    let abs_e = start + body.find(';').unwrap_or(body.len());
    out.push(Diagnostic {
      code: "sql280",
      severity: Severity::Hint,
      message: "ADD CONSTRAINT CHECK / FOREIGN KEY without NOT VALID -- scans every row under AccessExclusiveLock; use `... NOT VALID` then `ALTER TABLE t VALIDATE CONSTRAINT c` to avoid the outage".into(),
      range: text_size::TextRange::new((abs_s as u32).into(), (abs_e as u32).into()),
    });
  }
}
