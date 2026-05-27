//! sql190: `INSERT INTO t (...) ... ON CONFLICT (col, ...) DO ...`
//! where `(col, ...)` is not the target of any PRIMARY KEY / UNIQUE
//! constraint or unique index on `t`. PG raises 42P10 "there is no
//! unique or exclusion constraint matching the ON CONFLICT spec".
//!
//! Skip when `ON CONFLICT ON CONSTRAINT <name>` or no column list
//! is provided -- those forms target an explicit constraint or are
//! DO NOTHING with no inference.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::{Catalog, ConstraintKind};
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql190"
  }
  fn default_severity(&self) -> Severity {
    Severity::Error
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let start: usize = u32::from(stmt.range.start()) as usize;
    let end: usize = (u32::from(stmt.range.end()) as usize).min(source.len());
    let raw = &source[start..end];
    // Strip line comments + string literals so `-- INSERT ... ON CONFLICT`
    // doesn't trick find('(') into picking the wrong paren.
    let body_owned = strip_comments_and_strings(raw);
    let body = body_owned.as_str();
    let upper = body.to_ascii_uppercase();
    let Some(insert_at) = upper.find("INSERT INTO ") else { return };
    let after_insert = insert_at + "INSERT INTO ".len();
    let rest = &body[after_insert..];
    let raw = rest.trim_start();
    let id_end =
      raw.find(|c: char| !c.is_ascii_alphanumeric() && c != '_' && c != '.' && c != '"').unwrap_or(raw.len());
    let table_raw = &raw[..id_end];
    let table = table_raw.rsplit('.').next().unwrap_or(table_raw).trim_matches('"').to_string();
    if table.is_empty() {
      return;
    }
    let Some(t) = catalog.find_table(None, &table) else { return };

    let Some(oc_at) = upper.find("ON CONFLICT") else { return };
    // Skip ON CONFLICT ON CONSTRAINT form.
    let post = &upper[oc_at + "ON CONFLICT".len()..];
    if post.trim_start().starts_with("ON CONSTRAINT") {
      return;
    }
    // Find the column list paren after ON CONFLICT.
    let paren_off = post.find('(');
    let Some(paren_off) = paren_off else { return };
    let abs_paren = oc_at + "ON CONFLICT".len() + paren_off + 1;
    let close = body[abs_paren..].find(')');
    let Some(close) = close else { return };
    let cols_text = &body[abs_paren..abs_paren + close];
    let mut cols: Vec<String> =
      cols_text.split(',').map(|s| s.trim().trim_matches('"').to_ascii_lowercase()).filter(|s| !s.is_empty()).collect();
    cols.sort();
    if cols.is_empty() {
      return;
    }

    let mut found = false;
    for con in &t.constraints {
      if !matches!(con.kind, ConstraintKind::PrimaryKey | ConstraintKind::Unique) {
        continue;
      }
      let mut c2: Vec<String> = con.columns.iter().map(|s| s.to_ascii_lowercase()).collect();
      c2.sort();
      if c2 == cols {
        found = true;
        break;
      }
    }
    if !found {
      for idx in &t.indexes {
        if !idx.unique {
          continue;
        }
        let mut c2: Vec<String> = idx.columns.iter().map(|s| s.to_ascii_lowercase()).collect();
        c2.sort();
        if c2 == cols {
          found = true;
          break;
        }
      }
    }
    if found {
      return;
    }
    let abs_s = start + oc_at;
    let abs_e = start + abs_paren + close + 1;
    out.push(Diagnostic {
      code: "sql190",
      severity: Severity::Error,
      message: format!(
        "ON CONFLICT ({}) has no matching unique/PK on `{}` -- PG raises 42P10",
        cols.join(", "),
        table,
      ),
      range: text_size::TextRange::new((abs_s as u32).into(), (abs_e as u32).into()),
    });
  }
}

fn strip_comments_and_strings(s: &str) -> String {
  let mut out = String::with_capacity(s.len());
  let bytes = s.as_bytes();
  let n = bytes.len();
  let mut i = 0usize;
  while i < n {
    if i + 1 < n && bytes[i] == b'-' && bytes[i + 1] == b'-' {
      while i < n && bytes[i] != b'\n' {
        out.push(' ');
        i += 1
      }
    } else if bytes[i] == b'\'' {
      out.push(' ');
      i += 1;
      while i < n && bytes[i] != b'\'' {
        out.push(' ');
        i += 1
      }
      if i < n {
        out.push(' ');
        i += 1
      }
    } else if bytes[i].is_ascii() {
      out.push(bytes[i] as char);
      i += 1;
    } else {
      out.push(' ');
      i += 1;
    }
  }
  out
}
