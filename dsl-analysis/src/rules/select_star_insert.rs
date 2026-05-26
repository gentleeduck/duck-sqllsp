//! sql016: `INSERT INTO t SELECT *` is arity-fragile. A schema change to
//! the source table silently corrupts the destination. Always project
//! columns explicitly.
//!
//! Detection runs on the statement source slice because our Insert AST
//! does not carry the inner SELECT today.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::{Statement, StatementKind};
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql016"
  }
  fn default_severity(&self) -> Severity {
    Severity::Warning
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    if !matches!(stmt.kind, StatementKind::Insert(_)) {
      return;
    }

    let start: u32 = stmt.range.start().into();
    let end: u32 = stmt.range.end().into();
    let raw_slice = &source[start as usize..end.min(source.len() as u32) as usize];
    // Strip comments + strings so a header comment like
    // `-- INSERT ... SELECT * FROM ...` doesn't fire.
    let slice_owned = strip_noise(raw_slice);
    let slice = slice_owned.as_str();
    let upper = slice.to_ascii_uppercase();

    // Quick text scan: must contain "SELECT" + "*" with no other
    // identifiers between them (cheap signal for `SELECT * FROM ...`).
    if let Some(sel) = upper.find("SELECT") {
      let after = &upper[sel + 6..];
      // Skip whitespace then check for `*` immediately.
      let trimmed = after.trim_start();
      if trimmed.starts_with('*') {
        let leading_ws = after.len() - trimmed.len();
        let star_rel = sel + 6 + leading_ws;
        let abs_start = start as usize + star_rel;
        let abs_end = abs_start + 1;
        out.push(Diagnostic {
          code: "sql016",
          severity: Severity::Warning,
          message: "INSERT ... SELECT * is fragile -- a column added to the source silently \
                         misaligns the destination. List the source columns explicitly."
            .into(),
          range: text_size::TextRange::new((abs_start as u32).into(), (abs_end as u32).into()),
        });
      }
    }
  }
}

fn strip_noise(s: &str) -> String {
  let mut out: Vec<u8> = s.as_bytes().to_vec();
  let n = out.len();
  let mut i = 0usize;
  while i < n {
    if i + 1 < n && out[i] == b'-' && out[i + 1] == b'-' {
      while i < n && out[i] != b'\n' { out[i] = b' '; i += 1 }
      continue;
    }
    if i + 1 < n && out[i] == b'/' && out[i + 1] == b'*' {
      let mut depth = 1u32;
      out[i] = b' '; out[i + 1] = b' '; i += 2;
      while i + 1 < n && depth > 0 {
        if out[i] == b'/' && out[i + 1] == b'*' { depth += 1; out[i] = b' '; out[i + 1] = b' '; i += 2; }
        else if out[i] == b'*' && out[i + 1] == b'/' { depth -= 1; out[i] = b' '; out[i + 1] = b' '; i += 2; }
        else { out[i] = b' '; i += 1; }
      }
      continue;
    }
    if out[i] == b'\'' {
      out[i] = b' '; i += 1;
      while i < n && out[i] != b'\'' { out[i] = b' '; i += 1 }
      if i < n { out[i] = b' '; i += 1 }
      continue;
    }
    i += 1;
  }
  String::from_utf8(out).unwrap_or_else(|_| s.to_string())
}
