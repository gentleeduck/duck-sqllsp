//! sql274: `SELECT ... INTO TEMP foo FROM bar` (or INTO TEMPORARY)
//! where `foo` is also a real catalog table. PG allows it -- the
//! temp shadows the base for the session -- but it almost always
//! breaks subsequent queries that thought they were hitting the
//! base table.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::{Statement, StatementKind};
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql274"
  }
  fn default_severity(&self) -> Severity {
    Severity::Warning
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    if !matches!(&stmt.kind, StatementKind::Select(_) | StatementKind::Unknown { .. }) {
      return;
    }
    let start: usize = u32::from(stmt.range.start()) as usize;
    let end: usize = (u32::from(stmt.range.end()) as usize).min(source.len());
    let body = &source[start..end];
    let upper = body.to_ascii_uppercase();
    let Some(into_at) = upper.find(" INTO ") else { return };
    let after = into_at + " INTO ".len();
    let rest = body[after..].trim_start();
    let rest_upper = rest.to_ascii_uppercase();
    let prefix = if rest_upper.starts_with("TEMP ") {
      "TEMP "
    } else if rest_upper.starts_with("TEMPORARY ") {
      "TEMPORARY "
    } else {
      return;
    };
    let body_after_prefix = &rest[prefix.len()..].trim_start();
    let id_end = body_after_prefix
      .find(|c: char| !c.is_ascii_alphanumeric() && c != '_' && c != '.' && c != '"')
      .unwrap_or(body_after_prefix.len());
    let name = body_after_prefix[..id_end].trim_matches('"');
    if name.is_empty() {
      return;
    }
    let bare = name.rsplit('.').next().unwrap_or(name);
    if catalog.find_table(None, bare).is_none() {
      return;
    }
    let abs_s = start + after;
    let abs_e = abs_s + (rest.len() - body_after_prefix.len() + id_end);
    out.push(Diagnostic {
      code: "sql274",
      severity: Severity::Warning,
      message: format!(
        "SELECT INTO TEMP `{bare}` shadows the real catalog table for this session -- subsequent queries on `{bare}` will hit the temp; rename the temp"
      ),
      range: text_size::TextRange::new((abs_s as u32).into(), (abs_e as u32).into()),
    });
  }
}
