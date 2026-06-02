//! sql181: `INSERT INTO t (name) VALUES ('long-string')` where
//! `name` is declared `VARCHAR(n)` and the literal exceeds n.
//! PG truncates silently in some modes, errors with 22001 in
//! strict mode. Flag at edit time so the user knows.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::{Statement, StatementKind};
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql181"
  }
  fn default_severity(&self) -> Severity {
    Severity::Warning
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let StatementKind::Insert(ins) = &stmt.kind else { return };
    if ins.columns.is_empty() {
      return;
    }
    let Some(t) = catalog.find_table(ins.table.schema.as_deref(), &ins.table.name) else { return };

    let (start, body, upper) = crate::stmt_body_upper(stmt, source);
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
      let lit_text = &trimmed[1..trimmed.len() - 1];
      let Some(col) = t.columns.iter().find(|c| c.name.eq_ignore_ascii_case(col_name)) else { continue };
      let Some(max) = parse_varchar_length(&col.data_type) else { continue };
      if lit_text.chars().count() <= max {
        continue;
      }
      let rel = raw_val.as_ptr() as usize - body.as_ptr() as usize;
      let lead = raw_val.len() - raw_val.trim_start().len();
      let abs_s = start + rel + lead;
      let abs_e = abs_s + trimmed.len();
      out.push(Diagnostic {
        code: "sql181",
        severity: Severity::Warning,
        message: format!(
          "string literal exceeds `{}` declared length {} (PG raises 22001 in strict mode)",
          col.name, max
        ),
        range: crate::range_at(abs_s, abs_e),
      });
    }
  }
}

fn parse_varchar_length(decl: &str) -> Option<usize> {
  let upper = decl.to_ascii_uppercase();
  let upper = upper.rsplit('.').next().unwrap_or(&upper).trim();
  for prefix in ["VARCHAR", "CHARACTER VARYING", "CHARACTER", "CHAR"] {
    if let Some(rest) = upper.strip_prefix(prefix) {
      let rest = rest.trim();
      if rest.starts_with('(') {
        let inner = &rest[1..rest.find(')').unwrap_or(rest.len())];
        if let Ok(n) = inner.trim().parse::<usize>() {
          return Some(n);
        }
      }
    }
  }
  None
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
