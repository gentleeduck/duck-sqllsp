//! sql122: `LIKE` inside a query without explicit `COLLATE` -- the
//! collation comes from the column or the session, which has burned
//! teams on multi-locale deployments. Hint to add `COLLATE "C"` or
//! `COLLATE "und-x-icu"` for predictable behaviour.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql122"
  }
  fn default_severity(&self) -> Severity {
    Severity::Hint
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let start: usize = u32::from(stmt.range.start()) as usize;
    let end: usize = (u32::from(stmt.range.end()) as usize).min(source.len());
    let body = &source[start..end];
    let upper = body.to_ascii_uppercase();
    // Only fire inside CREATE INDEX / CREATE VIEW / CREATE MATERIALIZED
    // VIEW where the collation actually gets baked in. Ad-hoc SELECTs
    // are fine.
    if !upper.contains("CREATE INDEX")
      && !upper.contains("CREATE UNIQUE INDEX")
      && !upper.contains("CREATE VIEW")
      && !upper.contains("CREATE MATERIALIZED VIEW")
    {
      return;
    }
    // Skip if COLLATE already present.
    if upper.contains("COLLATE") {
      return;
    }
    let bytes = upper.as_bytes();
    let n = bytes.len();
    let mut i = 0;
    while i + 4 <= n {
      if &upper[i..i + 4] == "LIKE"
        && (i == 0 || !is_word(bytes[i - 1] as char))
        && (i + 4 == n || !is_word(bytes[i + 4] as char))
      {
        let abs_start = start + i;
        let abs_end = start + i + 4;
        out.push(Diagnostic {
                    code: "sql122",
                    severity: Severity::Hint,
                    message: "LIKE inside CREATE INDEX/VIEW without COLLATE -- add `COLLATE \"C\"` for predictable, locale-independent matching".into(),
                    range: text_size::TextRange::new(
                        (abs_start as u32).into(),
                        (abs_end as u32).into(),
                    ),
                });
        return;
      }
      i += 1;
    }
  }
}

fn is_word(c: char) -> bool {
  c.is_alphanumeric() || c == '_'
}
