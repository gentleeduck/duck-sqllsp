//! sql745: `date_part('yearr', ts)` -- the field name (first argument) is not
//! a recognised date/time field. PostgreSQL raises 22023 at runtime. This is
//! the function-call form of EXTRACT; sql208 covers the `EXTRACT(field FROM
//! ...)` syntax. Only fires when the field is a string literal.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

const FIELDS: &[&str] = &[
  "century",
  "day",
  "decade",
  "dow",
  "doy",
  "epoch",
  "hour",
  "isodow",
  "isoyear",
  "julian",
  "microsecond",
  "microseconds",
  "millennium",
  "millisecond",
  "milliseconds",
  "minute",
  "month",
  "quarter",
  "second",
  "timezone",
  "timezone_hour",
  "timezone_minute",
  "week",
  "year",
];

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql745"
  }
  fn default_severity(&self) -> Severity {
    Severity::Error
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, body) = crate::stmt_body(stmt, source);
    let lower = body.to_ascii_lowercase();
    let bytes = body.as_bytes();
    let mut from = 0usize;
    while let Some(rel) = lower[from..].find("date_part(") {
      let at = from + rel;
      if at > 0 {
        let prev = bytes[at - 1] as char;
        if prev.is_ascii_alphanumeric() || prev == '_' {
          from = at + 10;
          continue;
        }
      }
      let open = at + "date_part(".len() - 1;
      from = at + 10;
      let Some(close) = match_paren(bytes, open) else { break };
      let Some(comma) = top_level_comma(bytes, open + 1, close) else { continue };
      let field_raw = body[open + 1..comma].trim();
      // Only a quoted string literal is a checkable field name.
      if field_raw.len() < 2 || !field_raw.starts_with('\'') || !field_raw.ends_with('\'') {
        continue;
      }
      let field = field_raw.trim_matches('\'').to_ascii_lowercase();
      if field.is_empty() || FIELDS.contains(&field.as_str()) {
        continue;
      }
      out.push(Diagnostic {
        code: "sql745",
        severity: Severity::Error,
        message: format!("date_part('{field}', ...) -- `{field}` is not a recognized field; use year, month, day, hour, dow, epoch, etc"),
        range: crate::range_at(start + open + 1, start + comma),
      });
    }
  }
}

fn top_level_comma(bytes: &[u8], from: usize, to: usize) -> Option<usize> {
  let mut depth = 0i32;
  let mut i = from;
  while i < to {
    match bytes[i] {
      b'(' | b'[' => depth += 1,
      b')' | b']' => depth -= 1,
      b'\'' => {
        i += 1;
        while i < to && bytes[i] != b'\'' {
          i += 1;
        }
      },
      b',' if depth == 0 => return Some(i),
      _ => {},
    }
    i += 1;
  }
  None
}

fn match_paren(bytes: &[u8], open: usize) -> Option<usize> {
  let mut depth = 0i32;
  let mut i = open;
  while i < bytes.len() {
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
        while i < bytes.len() && bytes[i] != b'\'' {
          i += 1
        }
      },
      _ => {},
    }
    i += 1;
  }
  None
}
