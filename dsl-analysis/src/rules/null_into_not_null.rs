//! sql177: `INSERT INTO t (a, ...) VALUES (NULL, ...)` where `a` is
//! NOT NULL and has no default. PG errors at runtime with `null value
//! in column "a" violates not-null constraint`. Catch at edit time.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::{Statement, StatementKind};
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql177"
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
      let lit = raw_val.trim();
      if !lit.eq_ignore_ascii_case("NULL") {
        continue;
      }
      let Some(col) = t.columns.iter().find(|c| c.name.eq_ignore_ascii_case(col_name)) else { continue };
      if col.nullable {
        continue;
      }
      if col.default.is_some() {
        continue;
      }
      // Range = the NULL literal itself.
      let rel = raw_val.as_ptr() as usize - body.as_ptr() as usize;
      let lead = raw_val.len() - raw_val.trim_start().len();
      let abs_s = start + rel + lead;
      let abs_e = abs_s + lit.len();
      out.push(Diagnostic {
        code: "sql177",
        severity: Severity::Error,
        message: format!(
          "NULL inserted into `{}` which is NOT NULL and has no default -- PG will reject at exec",
          col.name
        ),
        range: text_size::TextRange::new((abs_s as u32).into(), (abs_e as u32).into()),
      });
    }
  }
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
      }
      b'\'' => {
        i += 1;
        while i < n && bytes[i] != b'\'' {
          i += 1;
        }
      }
      _ => {}
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
      }
      b'(' => depth += 1,
      b')' => depth -= 1,
      b',' if depth == 0 => {
        out.push(&s[start..i]);
        start = i + 1;
      }
      _ => {}
    }
    i += 1;
  }
  if start < n {
    out.push(&s[start..]);
  }
  out
}
