//! sql289: `CREATE TABLE ... INHERITS (parent)` -- table inheritance
//! predates partitioning and has surprising semantics (UNIQUE/PK
//! aren't enforced across children, FK only references parent rows).
//! For partitioning use cases, declarative partitioning (PG10+) is
//! the recommended path: `CREATE TABLE child PARTITION OF parent ...`.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql289"
  }
  fn default_severity(&self) -> Severity {
    Severity::Hint
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let start: usize = u32::from(stmt.range.start()) as usize;
    let end: usize = (u32::from(stmt.range.end()) as usize).min(source.len());
    let body = &source[start..end];
    let upper = body.to_ascii_uppercase();
    if !upper.contains("CREATE TABLE") { return }
    let Some(at) = upper.find("INHERITS") else { return };
    if at > 0 {
      let prev = body.as_bytes()[at - 1] as char;
      if prev.is_ascii_alphanumeric() || prev == '_' { return }
    }
    let after = at + "INHERITS".len();
    let rest = body[after..].trim_start();
    if !rest.starts_with('(') { return }
    let abs_s = start + at;
    let abs_e = abs_s + "INHERITS".len();
    out.push(Diagnostic {
      code: "sql289",
      severity: Severity::Hint,
      message: "Table inheritance via INHERITS -- UNIQUE/PK/FK aren't enforced across children; for partitioning prefer declarative partitions (`PARTITION OF parent`)".into(),
      range: text_size::TextRange::new((abs_s as u32).into(), (abs_e as u32).into()),
    });
  }
}
