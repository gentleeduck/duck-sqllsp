//! sql250: `SELECT count(*) FROM t FOR UPDATE` -- PG raises 0A000
//! "FOR UPDATE is not allowed with aggregate functions" / "with
//! GROUP BY clause" at parse time. Catches the pattern where lock
//! intent is bolted onto an aggregate query.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::{Statement, StatementKind};
use dsl_resolve::Scope;

pub struct Rule;

const AGG: &[&str] = &[
  "count(", "sum(", "avg(", "min(", "max(", "string_agg(", "array_agg(",
  "json_agg(", "jsonb_agg(", "bool_and(", "bool_or(", "every(",
];

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql250"
  }
  fn default_severity(&self) -> Severity {
    Severity::Error
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let StatementKind::Select(_) = &stmt.kind else { return };
    let start: usize = u32::from(stmt.range.start()) as usize;
    let end: usize = (u32::from(stmt.range.end()) as usize).min(source.len());
    let body = &source[start..end];
    let upper = body.to_ascii_uppercase();
    if !upper.contains("FOR UPDATE") && !upper.contains("FOR SHARE")
      && !upper.contains("FOR NO KEY UPDATE") && !upper.contains("FOR KEY SHARE")
    { return }
    let lower = body.to_ascii_lowercase();
    let has_agg = AGG.iter().any(|agg| lower.contains(agg));
    let has_group_by = upper.contains("GROUP BY") || upper.contains("HAVING");
    if !has_agg && !has_group_by { return }
    let Some(at) = upper.find("FOR UPDATE")
      .or_else(|| upper.find("FOR SHARE"))
      .or_else(|| upper.find("FOR NO KEY UPDATE"))
      .or_else(|| upper.find("FOR KEY SHARE"))
    else { return };
    let abs_s = start + at;
    let abs_e = abs_s + upper[at..].find(|c: char| c == ';' || c == '\n').unwrap_or(upper.len() - at);
    out.push(Diagnostic {
      code: "sql250",
      severity: Severity::Error,
      message: "FOR UPDATE/SHARE on aggregate or GROUP BY query -- PG raises 0A000; locking targets must be plain row sources".into(),
      range: text_size::TextRange::new((abs_s as u32).into(), (abs_e as u32).into()),
    });
  }
}
