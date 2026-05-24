//! sql144: `CREATE TRIGGER ... AFTER DELETE ... WHEN (NEW.x ...)` --
//! DELETE triggers have no NEW row. Mirror of sql140.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql144"
  }
  fn default_severity(&self) -> Severity {
    Severity::Error
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let start: usize = u32::from(stmt.range.start()) as usize;
    let end: usize = (u32::from(stmt.range.end()) as usize).min(source.len());
    let body = &source[start..end];
    let upper = body.to_ascii_uppercase();
    if !upper.contains("CREATE TRIGGER") {
      return;
    }
    let on_delete = upper.contains("DELETE") && !upper.contains("INSERT") && !upper.contains("UPDATE");
    if !on_delete {
      return;
    }
    let Some(when_at) = upper.find("WHEN") else { return };
    let after_when = &body[when_at + 4..];
    let after_trim = after_when.trim_start();
    if !after_trim.starts_with('(') {
      return;
    }
    let off = after_when.len() - after_trim.len();
    let when_body_start = when_at + 4 + off + 1;
    let bytes = body.as_bytes();
    let mut depth = 1i32;
    let mut j = when_body_start;
    while j < bytes.len() && depth > 0 {
      match bytes[j] {
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
    if !when_up.contains("NEW.") && !word_in(&when_up, "NEW") {
      return;
    }
    let new_rel = when_up.find("NEW.").or_else(|| when_up.find("NEW")).unwrap();
    let abs_start = start + when_body_start + new_rel;
    let abs_end = abs_start + 3;
    out.push(Diagnostic {
      code: "sql144",
      severity: Severity::Error,
      message:
        "AFTER/BEFORE DELETE trigger references NEW -- there is no NEW row on DELETE, the trigger raises at runtime"
          .into(),
      range: text_size::TextRange::new((abs_start as u32).into(), (abs_end as u32).into()),
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
