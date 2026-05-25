//! sql208: `EXTRACT(<field> FROM <expr>)` where `<field>` is not in
//! the PG-supported list. PG raises 22023 / 0AP01 at runtime. Common
//! typos like `EXTRACT(yearr FROM ts)` or wrong casing handled by
//! lowercase comparison.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

const FIELDS: &[&str] = &[
  "century", "day", "decade", "dow", "doy", "epoch", "hour", "isodow",
  "isoyear", "julian", "microseconds", "millennium", "milliseconds",
  "minute", "month", "quarter", "second", "timezone", "timezone_hour",
  "timezone_minute", "week", "year",
];

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql208"
  }
  fn default_severity(&self) -> Severity {
    Severity::Error
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let start: usize = u32::from(stmt.range.start()) as usize;
    let end: usize = (u32::from(stmt.range.end()) as usize).min(source.len());
    let body = &source[start..end];
    let lower = body.to_ascii_lowercase();
    let mut from = 0usize;
    while let Some(rel) = lower[from..].find("extract(") {
      let at = from + rel;
      if at > 0 {
        let prev = body.as_bytes()[at - 1] as char;
        if prev.is_ascii_alphanumeric() || prev == '_' { from = at + 8; continue }
      }
      let open = at + "extract(".len();
      let Some(from_at) = lower[open..].find(" from ") else { from = open; continue };
      let abs_from = open + from_at;
      let field_raw = body[open..abs_from].trim();
      let field = field_raw.trim_matches('"').trim_matches('\'').to_ascii_lowercase();
      if field.is_empty() { from = abs_from; continue }
      if FIELDS.contains(&field.as_str()) { from = abs_from; continue }
      let abs_s = start + open;
      let abs_e = start + abs_from;
      out.push(Diagnostic {
        code: "sql208",
        severity: Severity::Error,
        message: format!(
          "EXTRACT(`{field_raw}` FROM ...) -- `{field}` not a recognized field; supported: year, month, day, hour, minute, second, dow, doy, epoch, week, quarter, etc"
        ),
        range: text_size::TextRange::new((abs_s as u32).into(), (abs_e as u32).into()),
      });
      from = abs_from + " from ".len();
    }
  }
}
