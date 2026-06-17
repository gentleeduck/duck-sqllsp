//! sql633: server-side filesystem functions `pg_read_file`,
//! `pg_read_binary_file`, `pg_ls_dir`, and `pg_stat_file`. They expose the
//! database server's filesystem (reading arbitrary files / listing directories)
//! with the postgres OS user's privileges and are restricted to superusers and
//! the `pg_read_server_files` role. Exposing them through application SQL is a
//! data-exfiltration / privilege-escalation vector. Complements sql632
//! (lo_import / lo_export).

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

const FNS: &[&str] = &["pg_read_file(", "pg_read_binary_file(", "pg_ls_dir(", "pg_stat_file("];

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql633"
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
          code: "sql633",
          severity: Severity::Warning,
          message: format!("`{name}` exposes the database server's filesystem (superuser / pg_read_server_files only) -- avoid calling it from application SQL"),
          range: crate::range_at(start + at, start + at + name.len()),
        });
      }
    }
  }
}
