//! sql233: `CREATE MATERIALIZED VIEW mv ... WITH NO DATA;` followed
//! by `SELECT ... FROM mv` somewhere later in the buffer. PG raises
//! 55000 "materialized view is not populated" when queried before a
//! REFRESH MATERIALIZED VIEW. Catches the omission.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql233"
  }
  fn default_severity(&self) -> Severity {
    Severity::Warning
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let start: usize = u32::from(stmt.range.start()) as usize;
    let end: usize = (u32::from(stmt.range.end()) as usize).min(source.len());
    let body = &source[start..end];
    let upper = body.to_ascii_uppercase();
    if !upper.contains("CREATE MATERIALIZED VIEW") { return }
    if !upper.contains("WITH NO DATA") { return }
    // Extract MV name.
    let needle = "CREATE MATERIALIZED VIEW";
    let after = upper.find(needle).unwrap() + needle.len();
    let rest = &body[after..];
    let lead = rest.len() - rest.trim_start().len();
    let raw = &rest[lead..];
    let mut name_start = 0usize;
    let after_if = if raw.to_ascii_uppercase().starts_with("IF NOT EXISTS") {
      name_start = "IF NOT EXISTS".len();
      &raw[name_start..].trim_start()
    } else { raw };
    let id_end = after_if.find(|c: char| !c.is_ascii_alphanumeric() && c != '_' && c != '.' && c != '"').unwrap_or(after_if.len());
    let name = after_if[..id_end].rsplit('.').next().unwrap_or(&after_if[..id_end]).trim_matches('"').to_string();
    if name.is_empty() { return }
    let tail = &source[start + end - start..];
    let _ = name_start;
    if tail.to_ascii_uppercase().contains(&format!("REFRESH MATERIALIZED VIEW {}", name.to_ascii_uppercase())) { return }
    if tail.to_ascii_uppercase().contains(&format!("FROM {}", name.to_ascii_uppercase())) {
      let abs_s = start;
      let abs_e = start + body.find(';').unwrap_or(body.len());
      out.push(Diagnostic {
        code: "sql233",
        severity: Severity::Warning,
        message: format!(
          "MATERIALIZED VIEW `{name}` created WITH NO DATA but queried later -- needs REFRESH MATERIALIZED VIEW first (PG 55000)"
        ),
        range: text_size::TextRange::new((abs_s as u32).into(), (abs_e as u32).into()),
      });
    }
  }
}
