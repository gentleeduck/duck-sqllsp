//! sql317: `[identifier]` (square-bracket quoting) -- MSSQL/T-SQL
//! syntax. PG uses double quotes. Avoids false positives on array
//! subscripts by requiring the bracket content to look like an
//! identifier (no operators, single token, no digits-only).

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql317"
  }
  fn default_severity(&self) -> Severity {
    Severity::Error
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let start: usize = u32::from(stmt.range.start()) as usize;
    let end: usize = (u32::from(stmt.range.end()) as usize).min(source.len());
    let body = &source[start..end];
    let bytes = body.as_bytes();
    let mut i = 0usize;
    while i < bytes.len() {
      if bytes[i] == b'\'' {
        i += 1;
        while i < bytes.len() && bytes[i] != b'\'' { i += 1 }
        if i < bytes.len() { i += 1 }
        continue;
      }
      if bytes[i] != b'[' { i += 1; continue }
      let open = i;
      let mut k = open + 1;
      while k < bytes.len() && bytes[k] != b']' && bytes[k] != b'\n' { k += 1 }
      if k >= bytes.len() || bytes[k] != b']' { i = open + 1; continue }
      let inside = &body[open + 1..k];
      let identlike = !inside.is_empty()
        && inside.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == ' ' || c == '.')
        && inside.chars().any(|c| c.is_ascii_alphabetic())
        && !inside.contains(':');
      if !identlike { i = k + 1; continue }
      // Ignore array index patterns like `col[0]` -- preceded by identifier char.
      if open > 0 {
        let prev = bytes[open - 1] as char;
        if prev.is_ascii_alphanumeric() || prev == '_' || prev == ')' || prev == ']' || prev == '"' { i = k + 1; continue }
      }
      out.push(Diagnostic {
        code: "sql317",
        severity: Severity::Error,
        message: format!(
          "`[{inside}]` is MSSQL/T-SQL identifier quoting -- PG uses `\"{inside}\"`"
        ),
        range: text_size::TextRange::new(((start + open) as u32).into(), ((start + k + 1) as u32).into()),
      });
      i = k + 1;
    }
  }
}
