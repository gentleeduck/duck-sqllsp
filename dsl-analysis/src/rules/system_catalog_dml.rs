//! sql264: `UPDATE pg_class SET ...` / `DELETE FROM pg_class` and
//! other direct DML against `pg_catalog` system tables. Requires
//! `allow_system_table_mods = on` + superuser and is almost always
//! a footgun (corrupts the catalog). Block it with an Error so the
//! author has to actively dismiss.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql264"
  }
  fn default_severity(&self) -> Severity {
    Severity::Error
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let start: usize = u32::from(stmt.range.start()) as usize;
    let end: usize = (u32::from(stmt.range.end()) as usize).min(source.len());
    let raw = &source[start..end];
    let body_owned = crate::textutil::strip_noise_full(raw);
    let body = body_owned.as_str();
    let upper = body.to_ascii_uppercase();
    let trim = upper.trim_start();
    if !(trim.starts_with("UPDATE") || trim.starts_with("DELETE") || trim.starts_with("INSERT")) { return }
    // Target table heuristic: first identifier after UPDATE / DELETE FROM / INSERT INTO.
    let needle = if trim.starts_with("UPDATE") { "UPDATE " }
      else if trim.starts_with("DELETE") { "DELETE FROM " }
      else { "INSERT INTO " };
    let Some(at) = upper.find(needle) else { return };
    let after = at + needle.len();
    let rest = body[after..].trim_start();
    let id_end = rest.find(|c: char| !c.is_ascii_alphanumeric() && c != '_' && c != '.' && c != '"').unwrap_or(rest.len());
    let tgt_raw = rest[..id_end].trim_matches('"').to_ascii_lowercase();
    let is_sys = tgt_raw.starts_with("pg_") || tgt_raw.starts_with("pg_catalog.")
      || tgt_raw.starts_with("information_schema.");
    if !is_sys { return }
    let abs_s = start + after + (body[after..].len() - rest.len());
    let abs_e = abs_s + id_end;
    out.push(Diagnostic {
      code: "sql264",
      severity: Severity::Error,
      message: format!(
        "Direct DML against system catalog `{tgt_raw}` -- requires allow_system_table_mods + superuser and risks corruption; use proper DDL (ALTER/COMMENT/etc) instead"
      ),
      range: text_size::TextRange::new((abs_s as u32).into(), (abs_e as u32).into()),
    });
  }
}
