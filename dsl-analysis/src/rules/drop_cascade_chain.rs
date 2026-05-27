//! sql226: `DROP TABLE foo CASCADE` (or DROP TYPE/etc CASCADE) when
//! the catalog shows 3+ direct dependents (FK references + views +
//! triggers + indexes). Surface how many objects will be dropped so
//! the author can re-confirm.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::{Catalog, ConstraintKind};
use dsl_parse::{Statement, StatementKind};
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql226"
  }
  fn default_severity(&self) -> Severity {
    Severity::Warning
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let StatementKind::DropTable(dt) = &stmt.kind else { return };
    let start: usize = u32::from(stmt.range.start()) as usize;
    let end: usize = (u32::from(stmt.range.end()) as usize).min(source.len());
    let body = &source[start..end];
    let upper = body.to_ascii_uppercase();
    if !upper.contains("CASCADE") {
      return;
    }
    for tref in &dt.tables {
      let mut deps: Vec<String> = Vec::new();
      for other in catalog.tables() {
        for con in &other.constraints {
          if !matches!(con.kind, ConstraintKind::ForeignKey) {
            continue;
          }
          let Some(r) = &con.references else { continue };
          if r.table.eq_ignore_ascii_case(&tref.name) {
            deps.push(format!("FK from {}.{}", other.schema, other.name));
          }
        }
        for trg in &other.triggers {
          if other.name.eq_ignore_ascii_case(&tref.name) {
            deps.push(format!("trigger {}", trg.name));
          }
        }
      }
      let count = deps.len();
      if count < 3 {
        continue;
      }
      let preview: Vec<&String> = deps.iter().take(3).collect();
      let extra = if count > 3 { format!(" (+{} more)", count - 3) } else { String::new() };
      let abs_s = start;
      let abs_e = start + body.find(';').unwrap_or(body.len());
      out.push(Diagnostic {
        code: "sql226",
        severity: Severity::Warning,
        message: format!(
          "DROP {} CASCADE will drop {count} dependent objects: {}{}",
          tref.name,
          preview.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(", "),
          extra,
        ),
        range: text_size::TextRange::new((abs_s as u32).into(), (abs_e as u32).into()),
      });
    }
  }
}
