//! sql194: `TRUNCATE foo` (no CASCADE) when another table has an FK
//! referencing `foo`. PG raises 0A000 "cannot truncate a table
//! referenced in a foreign key constraint" at runtime.
//!
//! Uses the merged catalog to find inbound FK references to the
//! truncated table. Skips when the statement already includes CASCADE.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::{Catalog, ConstraintKind};
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql194"
  }
  fn default_severity(&self) -> Severity {
    Severity::Error
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let start: usize = u32::from(stmt.range.start()) as usize;
    let end: usize = (u32::from(stmt.range.end()) as usize).min(source.len());
    let body = &source[start..end];
    let upper = body.to_ascii_uppercase();
    if !upper.trim_start().starts_with("TRUNCATE") {
      return;
    }
    if upper.contains(" CASCADE") {
      return;
    }
    let after_kw = if let Some(p) = upper.find("TRUNCATE TABLE ") {
      p + "TRUNCATE TABLE ".len()
    } else if let Some(p) = upper.find("TRUNCATE ") {
      p + "TRUNCATE ".len()
    } else {
      return;
    };
    let list_end = body[after_kw..].find([';', '\n']).unwrap_or(body.len() - after_kw);
    let list = &body[after_kw..after_kw + list_end];
    let stop_words = ["RESTART", "CONTINUE", "RESTRICT", "CASCADE"];
    for raw in list.split(',') {
      let trimmed = raw.trim();
      let no_kw =
        trimmed.split_whitespace().find(|w| !stop_words.iter().any(|s| w.eq_ignore_ascii_case(s))).unwrap_or("");
      let name = no_kw.trim_matches('"');
      if name.is_empty() {
        continue;
      }
      let bare = name.rsplit('.').next().unwrap_or(name);
      let inbound: Vec<&dsl_catalog::Table> = catalog
        .tables()
        .filter(|tbl| {
          tbl.constraints.iter().any(|c| {
            matches!(c.kind, ConstraintKind::ForeignKey)
              && c.references.as_ref().map(|r| r.table.eq_ignore_ascii_case(bare)).unwrap_or(false)
          })
        })
        .collect();
      if inbound.is_empty() {
        continue;
      }
      let ref_names: Vec<String> = inbound.iter().take(3).map(|t| t.name.clone()).collect();
      let extra = if inbound.len() > 3 { format!(" (+{} more)", inbound.len() - 3) } else { String::new() };
      let off = body[after_kw..].find(name).unwrap_or(0);
      let abs_s = start + after_kw + off;
      let abs_e = abs_s + name.len();
      out.push(Diagnostic {
        code: "sql194",
        severity: Severity::Error,
        message: format!(
          "TRUNCATE `{bare}` without CASCADE -- referenced by FK from {}{} -- PG raises 0A000",
          ref_names.join(", "),
          extra,
        ),
        range: text_size::TextRange::new((abs_s as u32).into(), (abs_e as u32).into()),
      });
    }
  }
}
