//! sql312: column declared `SERIAL` / `BIGSERIAL` / `SMALLSERIAL`.
//! These are the legacy pre-PG10 form. On PG10+ the preferred form
//! is `GENERATED ALWAYS AS IDENTITY` (SQL standard, no leaked
//! sequence permissions, no ownership coupling, can be UPDATEd
//! via OVERRIDING SYSTEM VALUE).

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::{Statement, StatementKind};
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql312"
  }
  fn default_severity(&self) -> Severity {
    Severity::Hint
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let StatementKind::CreateTable(ct) = &stmt.kind else { return };
    let start: usize = u32::from(stmt.range.start()) as usize;
    let _ = source;
    for col in &ct.columns {
      let ty = col.type_name.to_ascii_uppercase();
      let is_serial = matches!(ty.as_str(), "SERIAL" | "BIGSERIAL" | "SMALLSERIAL" | "SERIAL2" | "SERIAL4" | "SERIAL8");
      if !is_serial {
        continue;
      }
      let abs_s = u32::from(col.range.start()) as usize + start;
      let abs_e = u32::from(col.range.end()) as usize + start;
      out.push(Diagnostic {
        code: "sql312",
        severity: Severity::Hint,
        message: format!(
          "`{}` is legacy syntax (pre-PG10); prefer `{} GENERATED ALWAYS AS IDENTITY` (SQL standard, cleaner permissions)",
          ty,
          match ty.as_str() {
            "SMALLSERIAL" | "SERIAL2" => "SMALLINT",
            "BIGSERIAL" | "SERIAL8" => "BIGINT",
            _ => "INTEGER",
          },
        ),
        range: text_size::TextRange::new((abs_s as u32).into(), (abs_e as u32).into()),
      });
    }
  }
}
