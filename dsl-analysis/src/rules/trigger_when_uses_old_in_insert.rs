//! sql140: `CREATE TRIGGER ... AFTER INSERT ... WHEN (OLD.x ...)` --
//! INSERT triggers have no OLD row. PG raises an error at runtime.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql140"
  }
  fn default_severity(&self) -> Severity {
    Severity::Error
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, body, upper) = crate::stmt_body_upper(stmt, source);
    if !upper.contains("CREATE TRIGGER") {
      return;
    }
    // Only AFTER/BEFORE/INSTEAD-OF INSERT triggers (no UPDATE/DELETE).
    let on_insert = upper.contains("INSERT") && !upper.contains("UPDATE") && !upper.contains("DELETE");
    if !on_insert {
      return;
    }
    // Find WHEN ( ... ) clause body.
    let Some(when_at) = upper.find("WHEN") else { return };
    let after_when = &body[when_at + 4..];
    let after_trim = after_when.trim_start();
    if !after_trim.starts_with('(') {
      return;
    }
    let off = after_when.len() - after_trim.len();
    let when_body_start = when_at + 4 + off + 1;
    let when_bytes = body.as_bytes();
    let mut depth = 1i32;
    let mut j = when_body_start;
    while j < when_bytes.len() && depth > 0 {
      match when_bytes[j] {
        b'(' => depth += 1,
        b')' => depth -= 1,
        _ => {},
      }
      if depth == 0 {
        break;
      }
      j += 1;
    }
    let when_body = &body[when_body_start..j];
    let when_up = when_body.to_ascii_uppercase();
    // Look for `OLD.` or bare `OLD` as a token.
    if !when_up.contains("OLD.") && !word_in(&when_up, "OLD") {
      return;
    }
    // Find position of OLD inside body for diagnostic range.
    let old_rel = when_up.find("OLD.").or_else(|| when_up.find("OLD")).unwrap();
    let abs_start = start + when_body_start + old_rel;
    let abs_end = abs_start + 3;
    out.push(Diagnostic {
      code: "sql140",
      severity: Severity::Error,
      message:
        "AFTER/BEFORE INSERT trigger references OLD -- there is no OLD row on INSERT, the trigger raises at runtime"
          .into(),
      range: crate::range_at(abs_start, abs_end),
    });
  }
}

fn word_in(haystack: &str, needle: &str) -> bool {
  let h = haystack.as_bytes();
  let n = h.len();
  let w = needle.len();
  let mut i = 0;
  while i + w <= n {
    if &haystack[i..i + w] == needle {
      let prev_ok = i == 0 || !(h[i - 1].is_ascii_alphanumeric() || h[i - 1] == b'_');
      let next_ok = i + w == n || !(h[i + w].is_ascii_alphanumeric() || h[i + w] == b'_');
      if prev_ok && next_ok {
        return true;
      }
    }
    i += 1;
  }
  false
}
