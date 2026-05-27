//! sql340: `NEW.id := <expr>` inside a `BEFORE INSERT` trigger body.
//!
//! When the target table has a SERIAL / IDENTITY PK, assigning NEW.id
//! before INSERT silently bypasses the sequence default. Usually a
//! bug: either the trigger should use a different column or it should
//! call `nextval()` explicitly so the sequence stays in sync.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql340"
  }
  fn default_severity(&self) -> Severity {
    Severity::Warning
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let start: usize = u32::from(stmt.range.start()) as usize;
    let end: usize = (u32::from(stmt.range.end()) as usize).min(source.len());
    let body = &source[start..end];
    let upper = body.to_ascii_uppercase();
    // Either the statement is itself BEFORE INSERT, or it's a
    // RETURNS trigger function body and some statement in the
    // surrounding buffer declares BEFORE INSERT.
    let stmt_is_before_insert = upper.contains("BEFORE INSERT");
    let is_trigger_fn_body = upper.contains("RETURNS TRIGGER") || upper.contains("RETURNS  TRIGGER");
    let buffer_has_before_insert = source.to_ascii_uppercase().contains("BEFORE INSERT");
    if !(stmt_is_before_insert || (is_trigger_fn_body && buffer_has_before_insert)) {
      return;
    }
    let bytes = body.as_bytes();
    // Walk every `NEW.<ident> :=` assignment.
    let mut i = 0usize;
    while i + 4 < bytes.len() {
      if upper.as_bytes()[i] == b'N' && i + 4 < bytes.len() && &upper[i..i + 4] == "NEW." {
        let prev_ok = i == 0 || !is_word(bytes[i - 1] as char);
        if !prev_ok {
          i += 1;
          continue;
        }
        let mut k = i + 4;
        let col_start = k;
        while k < bytes.len() && (bytes[k].is_ascii_alphanumeric() || bytes[k] == b'_') {
          k += 1
        }
        let col = &body[col_start..k];
        // Skip whitespace then look for ':='.
        let mut m = k;
        while m < bytes.len() && bytes[m].is_ascii_whitespace() {
          m += 1
        }
        if m + 1 < bytes.len() && bytes[m] == b':' && bytes[m + 1] == b'=' {
          // Heuristic: column name `id` or `*_id` is the most common PK shape.
          let lc = col.to_ascii_lowercase();
          if lc == "id" || lc.ends_with("_id") {
            let abs_s = start + i;
            let abs_e = start + m + 2;
            out.push(Diagnostic {
              code: "sql340",
              severity: Severity::Warning,
              message: format!(
                "assigning NEW.{col} in a BEFORE INSERT trigger bypasses the column's DEFAULT (SERIAL/IDENTITY); call nextval() explicitly if you mean to advance the sequence"
              ),
              range: text_size::TextRange::new((abs_s as u32).into(), (abs_e as u32).into()),
            });
            return;
          }
        }
        i = k;
        continue;
      }
      i += 1;
    }
  }
}

fn is_word(c: char) -> bool {
  c.is_alphanumeric() || c == '_'
}
