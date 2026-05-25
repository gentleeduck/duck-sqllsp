//! sql316: MySQL-only types (TINYINT, MEDIUMINT, LONGTEXT, etc).
//! PG accepts INTEGER/SMALLINT/TEXT instead.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

const MYSQL_TYPES: &[(&str, &str)] = &[
  ("TINYINT", "SMALLINT (or BOOLEAN for 0/1)"),
  ("MEDIUMINT", "INTEGER"),
  ("LONGTEXT", "TEXT"),
  ("MEDIUMTEXT", "TEXT"),
  ("TINYTEXT", "TEXT"),
  ("DATETIME", "TIMESTAMP (or TIMESTAMPTZ)"),
  ("TINYBLOB", "BYTEA"),
  ("MEDIUMBLOB", "BYTEA"),
  ("LONGBLOB", "BYTEA"),
  ("BLOB", "BYTEA"),
  ("YEAR", "SMALLINT (or DATE if you need a full date)"),
  ("DOUBLE", "DOUBLE PRECISION"),
];

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql316"
  }
  fn default_severity(&self) -> Severity {
    Severity::Error
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let start: usize = u32::from(stmt.range.start()) as usize;
    let end: usize = (u32::from(stmt.range.end()) as usize).min(source.len());
    let body = &source[start..end];
    let upper = body.to_ascii_uppercase();
    let bytes = upper.as_bytes();
    for (ty, suggest) in MYSQL_TYPES {
      let mut from = 0usize;
      while let Some(rel) = upper[from..].find(ty) {
        let at = from + rel;
        let prev_ok = at == 0 || !{ let p = bytes[at - 1] as char; p.is_ascii_alphanumeric() || p == '_' };
        let after = at + ty.len();
        let after_ok = after >= bytes.len() || !{ let p = bytes[after] as char; p.is_ascii_alphanumeric() || p == '_' };
        if !prev_ok || !after_ok { from = at + ty.len(); continue }
        // DOUBLE special-case: skip when followed by " PRECISION".
        if *ty == "DOUBLE" {
          let post = upper[after..].trim_start();
          if post.starts_with("PRECISION") { from = after; continue }
        }
        let abs_s = start + at;
        let abs_e = abs_s + ty.len();
        out.push(Diagnostic {
          code: "sql316",
          severity: Severity::Error,
          message: format!("`{ty}` is a MySQL type -- PG equivalent: {suggest}"),
          range: text_size::TextRange::new((abs_s as u32).into(), (abs_e as u32).into()),
        });
        from = after;
      }
    }
  }
}
