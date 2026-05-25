//! sql186: `ALTER TABLE t DROP COLUMN id` where another catalog
//! table has a FK that references `t(id)`. PG refuses without
//! CASCADE. Surface the dependency at edit time.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::{Catalog, ConstraintKind};
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql186"
  }
  fn default_severity(&self) -> Severity {
    Severity::Warning
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let start: usize = u32::from(stmt.range.start()) as usize;
    let end: usize = (u32::from(stmt.range.end()) as usize).min(source.len());
    let body = &source[start..end];
    let upper = body.to_ascii_uppercase();
    let Some(at_alter) = upper.find("ALTER TABLE") else { return };
    let after = at_alter + "ALTER TABLE".len();
    let rest = &body[after..];
    let lead = rest.len() - rest.trim_start().len();
    let raw = &rest[lead..];
    let id_end = raw.find(|c: char| !c.is_ascii_alphanumeric() && c != '_' && c != '.' && c != '"').unwrap_or(raw.len());
    let table = raw[..id_end].rsplit('.').next().unwrap_or(&raw[..id_end]).trim_matches('"').to_string();
    if table.is_empty() { return; }
    if !upper.contains("DROP COLUMN") { return; }
    if upper.contains("CASCADE") { return; }
    // Extract dropped column name.
    let drop_at = upper.find("DROP COLUMN").unwrap();
    let dc_after = drop_at + "DROP COLUMN".len();
    let dc_rest = &body[dc_after..];
    let dc_lead = dc_rest.len() - dc_rest.trim_start().len();
    let dc_raw = &dc_rest[dc_lead..];
    let dc_end = dc_raw.find(|c: char| !c.is_ascii_alphanumeric() && c != '_' && c != '"').unwrap_or(dc_raw.len());
    let col = dc_raw[..dc_end].trim_matches('"').to_string();
    if col.is_empty() { return; }
    // Find inbound FKs.
    let mut sources: Vec<String> = Vec::new();
    for other in catalog.tables() {
      for con in &other.constraints {
        if !matches!(con.kind, ConstraintKind::ForeignKey) { continue; }
        let Some(refs) = &con.references else { continue };
        if !refs.table.eq_ignore_ascii_case(&table) { continue; }
        if refs.columns.iter().any(|c| c.eq_ignore_ascii_case(&col)) {
          sources.push(format!("{}.{}({})", other.schema, other.name, con.columns.join(", ")));
        }
      }
    }
    if sources.is_empty() { return; }
    let abs_s = start + drop_at;
    let abs_e = start + dc_after + dc_lead + dc_end;
    out.push(Diagnostic {
      code: "sql186",
      severity: Severity::Warning,
      message: format!(
        "DROP COLUMN `{col}` would break {} inbound FK(s): {}. Use CASCADE or drop those constraints first.",
        sources.len(),
        sources.join(", ")
      ),
      range: text_size::TextRange::new((abs_s as u32).into(), (abs_e as u32).into()),
    });
  }
}
