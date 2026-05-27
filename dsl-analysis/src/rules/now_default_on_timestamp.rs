//! sql265: `CREATE TABLE t (..., c TIMESTAMP DEFAULT now(), ...)`
//! -- now() returns `timestamptz` so PG silently converts to local
//! timezone for storage in a non-TZ column. Subsequent reads then
//! drift by tz changes. Suggest TIMESTAMPTZ for the column or
//! `(now() AT TIME ZONE 'UTC')` for the default.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::{Statement, StatementKind};
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql265"
  }
  fn default_severity(&self) -> Severity {
    Severity::Hint
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let StatementKind::CreateTable(ct) = &stmt.kind else { return };
    let start: usize = u32::from(stmt.range.start()) as usize;
    let _ = source;
    for col in &ct.columns {
      let ty = col.type_name.to_ascii_lowercase();
      if !(ty == "timestamp" || ty == "timestamp without time zone") {
        continue;
      }
      let Some(def) = &col.default else { continue };
      let dl = def.to_ascii_lowercase();
      let calls_now = dl.contains("now()")
        || dl.contains("current_timestamp")
        || dl.contains("statement_timestamp")
        || dl.contains("transaction_timestamp")
        || dl.contains("clock_timestamp");
      if !calls_now {
        continue;
      }
      let abs_s = u32::from(col.range.start()) as usize + start;
      let abs_e = u32::from(col.range.end()) as usize + start;
      out.push(Diagnostic {
        code: "sql265",
        severity: Severity::Hint,
        message: format!(
          "Column `{}` is TIMESTAMP without TZ but DEFAULT calls `now()` (which returns timestamptz) -- TZ silently dropped; use TIMESTAMPTZ or wrap default in `(now() AT TIME ZONE 'UTC')`",
          col.name,
        ),
        range: text_size::TextRange::new((abs_s as u32).into(), (abs_e as u32).into()),
      });
    }
  }
}
