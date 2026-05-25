//! sql237: A shell command (pg_dump, psql, pg_restore, createdb,
//! dropdb) appears as the first token of a statement. PG raises a
//! syntax error -- the author probably pasted a terminal command
//! into the SQL buffer by mistake.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

const SHELL_CMDS: &[&str] = &[
  "pg_dump", "pg_restore", "psql", "createdb", "dropdb",
  "createuser", "dropuser", "pg_basebackup", "pgbench", "vacuumdb",
  "reindexdb",
];

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql237"
  }
  fn default_severity(&self) -> Severity {
    Severity::Error
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let start: usize = u32::from(stmt.range.start()) as usize;
    let end: usize = (u32::from(stmt.range.end()) as usize).min(source.len());
    let body = &source[start..end];
    let trimmed = body.trim_start();
    let first_token: String = trimmed.chars().take_while(|c| c.is_ascii_alphanumeric() || *c == '_').collect();
    if first_token.is_empty() { return }
    let lc = first_token.to_ascii_lowercase();
    if !SHELL_CMDS.contains(&lc.as_str()) { return }
    let lead = body.len() - trimmed.len();
    let abs_s = start + lead;
    let abs_e = abs_s + first_token.len();
    out.push(Diagnostic {
      code: "sql237",
      severity: Severity::Error,
      message: format!(
        "`{first_token}` is a shell command, not SQL -- run it from the terminal, not inside psql/the LSP buffer"
      ),
      range: text_size::TextRange::new((abs_s as u32).into(), (abs_e as u32).into()),
    });
  }
}
