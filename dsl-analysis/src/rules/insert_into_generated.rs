//! sql178: Writing to a `GENERATED ALWAYS` column. PG rejects writes
//! to identity/stored generated columns:
//!   * `INSERT INTO t (id, ...) VALUES (...)` where `id` is GENERATED
//!     ALWAYS AS IDENTITY -- requires `OVERRIDING SYSTEM VALUE`.
//!   * `INSERT INTO t (full_name, ...) VALUES (...)` where `full_name`
//!     is GENERATED ALWAYS AS (expr) STORED -- *cannot* be overridden;
//!     the column must be omitted entirely.
//!   * `UPDATE t SET id = ...` (identity) or `SET full_name = ...`
//!     (stored) -- both are runtime errors.
//!
//! Detection uses the catalog: `col.default` carries the
//! `GENERATED ALWAYS AS IDENTITY` text for identity columns, and
//! `col.generated` is set to the expression for STORED generated
//! columns.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::{Catalog, Column};
use dsl_parse::{Statement, StatementKind};
use dsl_resolve::Scope;

pub struct Rule;

#[derive(Clone, Copy)]
enum GenKind {
  Identity,
  Stored,
}

fn classify(col: &Column) -> Option<GenKind> {
  if col.generated.is_some() {
    return Some(GenKind::Stored);
  }
  let default = col.default.as_deref().unwrap_or("");
  if default.to_ascii_uppercase().contains("GENERATED ALWAYS") {
    return Some(GenKind::Identity);
  }
  None
}

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql178"
  }
  fn default_severity(&self) -> Severity {
    Severity::Error
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    match &stmt.kind {
      StatementKind::Insert(ins) => {
        if ins.columns.is_empty() {
          return;
        }
        let Some(t) = catalog.find_table(ins.table.schema.as_deref(), &ins.table.name) else { return };

        let (start, body) = crate::stmt_body(stmt, source);
        let has_override = body.to_ascii_uppercase().contains("OVERRIDING SYSTEM VALUE");

        for col_name in &ins.columns {
          let Some(col) = t.columns.iter().find(|c| c.name.eq_ignore_ascii_case(col_name)) else { continue };
          let Some(kind) = classify(col) else { continue };
          // Identity columns are writable with OVERRIDING SYSTEM
          // VALUE. STORED generated columns are never writable.
          if matches!(kind, GenKind::Identity) && has_override {
            continue;
          }
          let col_at = body.to_ascii_lowercase().find(&col_name.to_ascii_lowercase()).unwrap_or(0);
          let abs_s = start + col_at;
          let abs_e = abs_s + col_name.len();
          let message = match kind {
            GenKind::Identity => format!(
              "`{}` is GENERATED ALWAYS AS IDENTITY -- omit it from the INSERT or add `OVERRIDING SYSTEM VALUE`",
              col_name
            ),
            GenKind::Stored => format!(
              "`{}` is a STORED GENERATED column -- it cannot be written; omit it from the INSERT (no override exists)",
              col_name
            ),
          };
          out.push(Diagnostic {
            code: "sql178",
            severity: Severity::Error,
            message,
            range: crate::range_at(abs_s, abs_e),
          });
        }
      },
      StatementKind::Update(upd) => {
        if upd.assignments.is_empty() {
          return;
        }
        let Some(t) = catalog.find_table(upd.table.schema.as_deref(), &upd.table.name) else { return };

        let (start, body) = crate::stmt_body(stmt, source);

        for (col_name, _) in &upd.assignments {
          let Some(col) = t.columns.iter().find(|c| c.name.eq_ignore_ascii_case(col_name)) else { continue };
          let Some(kind) = classify(col) else { continue };
          let col_at = body.to_ascii_lowercase().find(&col_name.to_ascii_lowercase()).unwrap_or(0);
          let abs_s = start + col_at;
          let abs_e = abs_s + col_name.len();
          let message = match kind {
            GenKind::Identity => format!(
              "`{}` is GENERATED ALWAYS AS IDENTITY -- PG rejects UPDATEs to identity columns",
              col_name
            ),
            GenKind::Stored => format!(
              "`{}` is a STORED GENERATED column -- PG rejects UPDATEs (the value is derived from its expression)",
              col_name
            ),
          };
          out.push(Diagnostic {
            code: "sql178",
            severity: Severity::Error,
            message,
            range: crate::range_at(abs_s, abs_e),
          });
        }
      },
      _ => {},
    }
  }
}
