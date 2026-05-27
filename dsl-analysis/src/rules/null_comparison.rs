//! sql015: comparison with NULL using `=` or `<>` (or `!=`). Always
//! yields NULL; the user almost always meant `IS NULL` / `IS NOT NULL`.
//!
//! Detection is text-level on the statement source slice -- our Expr
//! type stringifies binary ops, so a structural walk doesn't help.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;
use text_size::TextRange;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql015"
  }
  fn default_severity(&self) -> Severity {
    Severity::Warning
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let range: TextRange = stmt.range;
    let start: u32 = range.start().into();
    let end: u32 = range.end().into();
    let slice = &source[start as usize..end.min(source.len() as u32) as usize];

    // Skip when the only `= NULL` occurrence is part of an assignment
    // (UPDATE ... SET col = NULL / INSERT ... col = NULL): that's
    // setting a value, not comparing.
    let upper_slice = slice.to_ascii_uppercase();
    let in_set_assignment = upper_slice.contains("UPDATE ") && upper_slice.contains(" SET ");
    // Patterns sorted longest-op-first so `!= NULL` matches as
    // `!= NULL`, not as the trailing `= NULL` substring -- otherwise
    // the message misreports the operator. Left-NULL patterns
    // (`NULL = ...`) require a word boundary before NULL.
    let right_patterns: &[&str] = &[
      "!= NULL", "!=NULL", "<> NULL", "<>NULL", "= NULL", "=NULL",
      // Long-form `<op> CAST(NULL AS ...)` -- same semantics.
      "!= CAST(NULL", "!=CAST(NULL", "<> CAST(NULL", "<>CAST(NULL", "= CAST(NULL", "=CAST(NULL",
    ];
    let left_patterns: &[&str] = &[
      "NULL !=", "NULL!=", "NULL <>", "NULL<>", "NULL =", "NULL=",
    ];
    for pat in right_patterns {
      if in_set_assignment && pat.starts_with("=") {
        let Some(set_at) = upper_slice.find(" SET ") else { continue };
        let where_at = upper_slice[set_at..].find(" WHERE ").map(|p| set_at + p);
        let in_predicate =
          if let Some(wh) = where_at { find_outside_strings(&slice[wh..], pat).is_some() } else { false };
        if !in_predicate {
          continue;
        }
      }
      if find_outside_strings(slice, pat).is_some() {
        out.push(Diagnostic {
          code: "sql015",
          severity: Severity::Warning,
          message: format!("comparison `{pat}` always yields NULL; use `IS NULL` or `IS NOT NULL`"),
          range,
        });
        return;
      }
    }
    for pat in left_patterns {
      if let Some(at) = find_outside_strings(slice, pat)
        && !preceded_by_word_char(slice.as_bytes(), at)
      {
        out.push(Diagnostic {
          code: "sql015",
          severity: Severity::Warning,
          message: format!("comparison `{pat}` always yields NULL; use `IS NULL` or `IS NOT NULL`"),
          range,
        });
        return;
      }
    }
  }
}

fn preceded_by_word_char(bytes: &[u8], at: usize) -> bool {
  if at == 0 {
    return false;
  }
  let b = bytes[at - 1];
  b.is_ascii_alphanumeric() || b == b'_'
}

fn find_outside_strings(s: &str, needle: &str) -> Option<usize> {
  let needle_upper = needle.to_ascii_uppercase();
  let needle_bytes = needle_upper.as_bytes();
  let bytes = s.as_bytes();
  let mut in_single = false;
  let mut i = 0;
  while i + needle_bytes.len() <= bytes.len() {
    let b = bytes[i];
    if b == b'\'' && (i == 0 || bytes[i - 1] != b'\\') {
      in_single = !in_single;
    }
    if !in_single {
      // Case-insensitive ASCII byte compare -- avoids slicing the
      // uppercased string at byte offsets that may land inside a
      // multi-byte UTF-8 codepoint (panic on `μ`/`α`/etc).
      let mut matches = true;
      for k in 0..needle_bytes.len() {
        let cand = bytes[i + k].to_ascii_uppercase();
        if cand != needle_bytes[k] {
          matches = false;
          break;
        }
      }
      if matches {
        return Some(i);
      }
    }
    i += 1;
  }
  None
}
