//! sql346: `CREATE INDEX ... USING BRIN` on a table the live catalog
//! says has fewer than 10k rows. BRIN is built for large append-only
//! tables (logs, time series). On a small table the index returns
//! whole heap pages and the planner picks seq-scan anyway.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

const SMALL_TABLE_THRESHOLD: f64 = 10_000.0;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql346"
  }
  fn default_severity(&self) -> Severity {
    Severity::Hint
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let start: usize = u32::from(stmt.range.start()) as usize;
    let end: usize = (u32::from(stmt.range.end()) as usize).min(source.len());
    let body = &source[start..end];
    let upper = body.to_ascii_uppercase();
    if !upper.contains("CREATE INDEX") && !upper.contains("CREATE UNIQUE INDEX") {
      return;
    }
    let Some(using_at) = upper.find("USING BRIN") else { return };
    let Some(on_at) = upper.find(" ON ") else { return };
    let after_on = on_at + 4;
    let rest = &body[after_on..];
    let lead = rest.len() - rest.trim_start().len();
    let raw = &rest[lead..];
    let tbl_end =
      raw.find(|c: char| !c.is_ascii_alphanumeric() && c != '_' && c != '.' && c != '"').unwrap_or(raw.len());
    let table = raw[..tbl_end].rsplit('.').next().unwrap_or(&raw[..tbl_end]).trim_matches('"').to_string();
    if table.is_empty() {
      return;
    }
    let Some(t) = catalog.find_table(None, &table) else { return };
    let Some(est) = t.row_estimate else { return };
    if est >= SMALL_TABLE_THRESHOLD {
      return;
    }
    let abs_s = start + using_at;
    let abs_e = abs_s + "USING BRIN".len();
    out.push(Diagnostic {
      code: "sql346",
      severity: Severity::Hint,
      message: format!(
        "BRIN index on `{}` (~{:.0} rows) -- BRIN is for >100k row append-only tables; planner will seq-scan anyway",
        table, est
      ),
      range: text_size::TextRange::new((abs_s as u32).into(), (abs_e as u32).into()),
    });
  }
}
