//! sql183: `INSERT INTO t (id) VALUES ('not-a-uuid')` where `id` is
//! UUID. PG raises 22P02 at runtime. Accept only:
//!   * 8-4-4-4-12 hex (dashed canonical form)
//!   * 32 hex chars (no dashes) -- PG also accepts this
//!   * Surrounding braces `{...}`

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::{Statement, StatementKind};
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql183"
  }
  fn default_severity(&self) -> Severity {
    Severity::Error
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let StatementKind::Insert(ins) = &stmt.kind else { return };
    if ins.columns.is_empty() {
      return;
    }
    let Some(t) = catalog.find_table(ins.table.schema.as_deref(), &ins.table.name) else { return };

    let start: usize = u32::from(stmt.range.start()) as usize;
    let end: usize = (u32::from(stmt.range.end()) as usize).min(source.len());
    let raw = &source[start..end];
    let body_owned = crate::textutil::strip_noise_full(raw);
    let body = body_owned.as_str();
    let upper = body.to_ascii_uppercase();
    let Some(values_at) = upper.find("VALUES") else { return };
    let bytes = body.as_bytes();
    let n = bytes.len();
    let mut k = values_at + 6;
    while k < n && bytes[k].is_ascii_whitespace() {
      k += 1;
    }
    if k >= n || bytes[k] != b'(' {
      return;
    }
    let Some(close) = match_paren(bytes, k) else { return };
    let tuple = &body[k + 1..close];
    let values = split_top_commas(tuple);
    if values.len() != ins.columns.len() {
      return;
    }

    for (col_name, raw_val) in ins.columns.iter().zip(values.iter()) {
      let trimmed = raw_val.trim();
      if !trimmed.starts_with('\'') || !trimmed.ends_with('\'') {
        continue;
      }
      let Some(col) = t.columns.iter().find(|c| c.name.eq_ignore_ascii_case(col_name)) else { continue };
      let ty = col.data_type.to_ascii_uppercase();
      let ty = ty.rsplit('.').next().unwrap_or(&ty).trim();
      if ty != "UUID" {
        continue;
      }
      let lit = &trimmed[1..trimmed.len() - 1];
      if looks_like_uuid(lit) {
        continue;
      }
      let rel = raw_val.as_ptr() as usize - body.as_ptr() as usize;
      let lead = raw_val.len() - raw_val.trim_start().len();
      let abs_s = start + rel + lead;
      let abs_e = abs_s + trimmed.len();
      out.push(Diagnostic {
        code: "sql183",
        severity: Severity::Error,
        message: format!(
          "literal `'{}'` not a valid UUID -- PG raises 22P02 at exec",
          lit.chars().take(60).collect::<String>(),
        ),
        range: text_size::TextRange::new((abs_s as u32).into(), (abs_e as u32).into()),
      });
    }
  }
}

fn looks_like_uuid(s: &str) -> bool {
  // Strip surrounding braces.
  let s = s.trim().trim_start_matches('{').trim_end_matches('}');
  let hex_only: String = s.chars().filter(|c| !c.is_whitespace() && *c != '-').collect();
  if hex_only.len() != 32 {
    return false;
  }
  if !hex_only.chars().all(|c| c.is_ascii_hexdigit()) {
    return false;
  }
  // Optional: enforce 8-4-4-4-12 when dashes are present.
  if s.contains('-') {
    let parts: Vec<&str> = s.split('-').collect();
    if parts.len() != 5 {
      return false;
    }
    let expected = [8, 4, 4, 4, 12];
    for (i, p) in parts.iter().enumerate() {
      if p.len() != expected[i] {
        return false;
      }
    }
  }
  true
}

fn match_paren(bytes: &[u8], open: usize) -> Option<usize> {
  let n = bytes.len();
  let mut depth = 0i32;
  let mut i = open;
  while i < n {
    match bytes[i] {
      b'(' => depth += 1,
      b')' => {
        depth -= 1;
        if depth == 0 {
          return Some(i);
        }
      },
      b'\'' => {
        i += 1;
        while i < n && bytes[i] != b'\'' {
          i += 1;
        }
      },
      _ => {},
    }
    i += 1;
  }
  None
}

fn split_top_commas(s: &str) -> Vec<&str> {
  let bytes = s.as_bytes();
  let n = bytes.len();
  let mut out = Vec::new();
  let mut depth = 0i32;
  let mut start = 0usize;
  let mut i = 0usize;
  while i < n {
    match bytes[i] {
      b'\'' => {
        i += 1;
        while i < n && bytes[i] != b'\'' {
          i += 1;
        }
      },
      b'(' => depth += 1,
      b')' => depth -= 1,
      b',' if depth == 0 => {
        out.push(&s[start..i]);
        start = i + 1;
      },
      _ => {},
    }
    i += 1;
  }
  if start < n {
    out.push(&s[start..]);
  }
  out
}
