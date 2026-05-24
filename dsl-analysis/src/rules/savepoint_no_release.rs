//! sql062: `SAVEPOINT x` declared but never `RELEASE`d (or rolled back
//! to). Long-lived savepoints leak resources and confuse readers.
//!
//! v1 scope: in the same buffer, every `SAVEPOINT name` should have a
//! matching `RELEASE [SAVEPOINT] name` or `ROLLBACK TO [SAVEPOINT]
//! name`. Inter-file flows are out of scope.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql062"
  }
  fn default_severity(&self) -> Severity {
    Severity::Hint
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let start: usize = u32::from(stmt.range.start()) as usize;
    let end: usize = (u32::from(stmt.range.end()) as usize).min(source.len());
    let body = &source[start..end];
    let upper = body.to_ascii_uppercase();
    if !contains_word(&upper, "SAVEPOINT") {
      return;
    }

    // Read the savepoint name after the keyword. Then scan the
    // whole source (not just this statement) for a matching
    // RELEASE / ROLLBACK TO.
    let Some(name) = savepoint_name(&upper, body) else { return };
    let full_upper = source.to_ascii_uppercase();
    let released = matches_release(&full_upper, &name) || matches_rollback_to(&full_upper, &name);
    if !released {
      // Locate the SAVEPOINT keyword + name within the statement
      // so the diagnostic highlights only that identifier.
      let bytes = body.as_bytes();
      let n = bytes.len();
      let upper_bytes = upper.as_bytes();
      let mut sp_pos = 0usize;
      let mut i = 0;
      while i + 9 <= n {
        if upper_bytes[i..i + 9].eq_ignore_ascii_case(b"SAVEPOINT")
          && (i == 0 || !is_word(upper_bytes[i - 1] as char))
          && (i + 9 == n || !is_word(upper_bytes[i + 9] as char))
        {
          let mut j = i + 9;
          while j < n && bytes[j].is_ascii_whitespace() {
            j += 1;
          }
          let s = j;
          while j < n && (bytes[j].is_ascii_alphanumeric() || bytes[j] == b'_') {
            j += 1;
          }
          if body[s..j].eq_ignore_ascii_case(&name) {
            sp_pos = s;
            let abs_start = start + s;
            let abs_end = start + j;
            out.push(Diagnostic {
              code: "sql062",
              severity: Severity::Hint,
              message: format!("SAVEPOINT `{name}` is never released or rolled back to in this buffer"),
              range: text_size::TextRange::new((abs_start as u32).into(), (abs_end as u32).into()),
            });
            return;
          }
        }
        i += 1;
      }
      let _ = sp_pos;
    }
  }
}

fn is_word(c: char) -> bool {
  c.is_alphanumeric() || c == '_'
}

fn savepoint_name(upper: &str, original: &str) -> Option<String> {
  let idx = upper.find("SAVEPOINT")?;
  let after = idx + "SAVEPOINT".len();
  let bytes = original.as_bytes();
  let n = bytes.len();
  let mut i = after;
  while i < n && bytes[i].is_ascii_whitespace() {
    i += 1;
  }
  let start = i;
  while i < n && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_') {
    i += 1;
  }
  if i == start {
    return None;
  }
  Some(original[start..i].to_string())
}

fn matches_release(upper: &str, name: &str) -> bool {
  let needle1 = format!("RELEASE SAVEPOINT {}", name.to_ascii_uppercase());
  let needle2 = format!("RELEASE {}", name.to_ascii_uppercase());
  upper.contains(&needle1) || upper.contains(&needle2)
}

fn matches_rollback_to(upper: &str, name: &str) -> bool {
  let needle1 = format!("ROLLBACK TO SAVEPOINT {}", name.to_ascii_uppercase());
  let needle2 = format!("ROLLBACK TO {}", name.to_ascii_uppercase());
  upper.contains(&needle1) || upper.contains(&needle2)
}

fn contains_word(haystack: &str, needle: &str) -> bool {
  let bytes = haystack.as_bytes();
  let n_bytes = needle.as_bytes();
  let mut i = 0;
  while i + n_bytes.len() <= bytes.len() {
    if &bytes[i..i + n_bytes.len()] == n_bytes {
      let prev_ok = i == 0 || !is_word(bytes[i - 1] as char);
      let next_ok = i + n_bytes.len() == bytes.len() || !is_word(bytes[i + n_bytes.len()] as char);
      if prev_ok && next_ok {
        return true;
      }
    }
    i += 1;
  }
  false
}
