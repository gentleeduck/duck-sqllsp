//! sql632: the server-side large-object file functions `lo_import('path')` and
//! `lo_export(oid, 'path')`. They read/write files on the *database server's*
//! filesystem with the postgres OS user's privileges and require superuser --
//! a privilege-escalation and data-exfiltration vector. Move bytes through the
//! client (`\lo_import` / `\lo_export` in psql, or bytea over the wire) instead.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

const FNS: &[&str] = &["lo_import(", "lo_export("];

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql632"
  }
  fn default_severity(&self) -> Severity {
    Severity::Warning
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, body) = crate::stmt_body(stmt, source);
    let lower = body.to_ascii_lowercase();
    let bytes = body.as_bytes();
    for &needle in FNS {
      let mut from = 0usize;
      while let Some(rel) = lower[from..].find(needle) {
        let at = from + rel;
        from = at + needle.len();
        if at > 0 && (bytes[at - 1].is_ascii_alphanumeric() || bytes[at - 1] == b'_') {
          continue;
        }
        let name = needle.trim_end_matches('(');
        out.push(Diagnostic {
          code: "sql632",
          severity: Severity::Warning,
          message: format!("`{name}` reads/writes files on the database server (superuser-only, exfiltration vector) -- transfer bytes through the client instead"),
          range: crate::range_at(start + at, start + at + name.len()),
        });
      }
    }
  }
}
