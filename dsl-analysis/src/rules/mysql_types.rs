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
    let raw = &source[start..end];
    // Strip string literals (e.g. INTERVAL '1 year') + line comments
    // so YEAR/MONTH/DAY tokens inside SQL interval strings or doc
    // comments don't get flagged as MySQL types.
    let body_owned = strip_strings_and_comments(raw);
    let body = body_owned.as_str();
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

fn strip_strings_and_comments(s: &str) -> String {
  let mut out = String::with_capacity(s.len());
  let bytes = s.as_bytes();
  let n = bytes.len();
  let mut i = 0usize;
  while i < n {
    if i + 1 < n && bytes[i] == b'-' && bytes[i + 1] == b'-' {
      while i < n && bytes[i] != b'\n' { out.push(' '); i += 1 }
    } else if bytes[i] == b'\'' {
      out.push(' ');
      i += 1;
      while i < n && bytes[i] != b'\'' { out.push(' '); i += 1 }
      if i < n { out.push(' '); i += 1 }
    } else if bytes[i].is_ascii() {
      out.push(bytes[i] as char);
      i += 1;
    } else {
      out.push(' ');
      i += 1;
    }
  }
  out
}
