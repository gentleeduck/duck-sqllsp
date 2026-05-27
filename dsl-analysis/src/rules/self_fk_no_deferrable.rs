//! sql304: CREATE TABLE foo (..., parent_id REFERENCES foo(id)) --
//! self-referential FK without DEFERRABLE. INSERT into a chain
//! requires inserting parents before children; DEFERRABLE INITIALLY
//! DEFERRED lets you insert in any order inside a tx.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::{Statement, StatementKind};
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql304"
  }
  fn default_severity(&self) -> Severity {
    Severity::Hint
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let StatementKind::CreateTable(ct) = &stmt.kind else { return };
    let start: usize = u32::from(stmt.range.start()) as usize;
    let end: usize = (u32::from(stmt.range.end()) as usize).min(source.len());
    let body = &source[start..end];
    let upper = body.to_ascii_uppercase();
    let needle = format!("REFERENCES {}", ct.table.name.to_ascii_uppercase());
    let alt = format!("REFERENCES \"{}\"", ct.table.name);
    let mut from = 0usize;
    while let Some(rel) = upper[from..].find(&needle).or_else(|| upper[from..].find(&alt.to_ascii_uppercase())) {
      let at = from + rel;
      if upper[at..].contains("DEFERRABLE") {
        return;
      }
      let abs_s = start + at;
      let abs_e = abs_s + needle.len().min(body.len() - at);
      out.push(Diagnostic {
        code: "sql304",
        severity: Severity::Hint,
        message: format!(
          "Self-referential FK to `{}` without DEFERRABLE -- parent rows must exist before children; add `DEFERRABLE INITIALLY DEFERRED` to insert chains in any order",
          ct.table.name,
        ),
        range: text_size::TextRange::new((abs_s as u32).into(), (abs_e as u32).into()),
      });
      from = at + needle.len();
    }
  }
}
