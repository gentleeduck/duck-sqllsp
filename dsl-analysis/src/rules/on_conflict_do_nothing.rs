//! sql246: `INSERT ... ON CONFLICT DO NOTHING` (without the column
//! list / constraint name to scope it). Without an inference target
//! PG swallows ANY constraint violation: PK clash, UNIQUE, EXCLUDE,
//! even CHECK. Almost always the author wanted to ignore only the
//! specific dup-key case. Suggest naming the conflict target.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql246"
  }
  fn default_severity(&self) -> Severity {
    Severity::Hint
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let start: usize = u32::from(stmt.range.start()) as usize;
    let end: usize = (u32::from(stmt.range.end()) as usize).min(source.len());
    let raw = &source[start..end];
    // Strip comments + strings so a `-- ON CONFLICT with ...` header
    // doesn't hijack the keyword anchor.
    let body_owned = strip_noise(raw);
    let body = body_owned.as_str();
    let upper = body.to_ascii_uppercase();
    let Some(oc_at) = upper.find("ON CONFLICT") else { return };
    let post_oc = &upper[oc_at + "ON CONFLICT".len()..];
    let trimmed = post_oc.trim_start();
    // Scoped via ON CONSTRAINT or (col, ...) -- fine.
    if trimmed.starts_with("ON CONSTRAINT") || trimmed.starts_with('(') { return }
    // Must be followed by DO NOTHING (else DO UPDATE form which still benefits but is intentional).
    if !post_oc.contains("DO NOTHING") { return }
    let abs_s = start + oc_at;
    let abs_e = abs_s + "ON CONFLICT DO NOTHING".len().min(body.len() - oc_at);
    out.push(Diagnostic {
      code: "sql246",
      severity: Severity::Hint,
      message: "ON CONFLICT DO NOTHING (no target) swallows EVERY constraint violation, not just dup-key -- scope with `(col)` or `ON CONSTRAINT <name>`".into(),
      range: text_size::TextRange::new((abs_s as u32).into(), (abs_e as u32).into()),
    });
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
