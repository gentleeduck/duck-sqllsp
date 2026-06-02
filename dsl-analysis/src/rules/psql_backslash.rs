//! sql310: line starts with `\<letter>` -- psql meta-command
//! (`\d`, `\dt`, `\l`, `\timing`, `\copy`, etc). Only psql parses
//! these; the SQL server raises 42601. Common when copy-pasting
//! from a psql session into a file or app.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql310"
  }
  fn default_severity(&self) -> Severity {
    Severity::Error
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, body) = crate::stmt_body(stmt, source);
    let trimmed = body.trim_start();
    if !trimmed.starts_with('\\') {
      return;
    }
    let bytes = trimmed.as_bytes();
    if bytes.len() < 2 || !bytes[1].is_ascii_alphabetic() {
      return;
    }
    let lead = body.len() - trimmed.len();
    let abs_s = start + lead;
    let line_end = trimmed.find('\n').unwrap_or(trimmed.len());
    let abs_e = abs_s + line_end;
    let cmd_end = trimmed.find(|c: char| c.is_whitespace()).unwrap_or(trimmed.len());
    let cmd = &trimmed[..cmd_end];
    out.push(Diagnostic {
      code: "sql310",
      severity: Severity::Error,
      message: format!(
        "`{cmd}` is a psql meta-command, not SQL -- only the psql client interprets it; remove from the buffer or run in psql"
      ),
      range: crate::range_at(abs_s, abs_e),
    });
  }
}
