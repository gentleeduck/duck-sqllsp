//! sql158: `PERFORM <select>` inside PL/pgSQL where the SELECT calls
//! no function with side effects -- the result is silently discarded.
//! Suggest dropping PERFORM (cheap NO-OP) or using the result.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql158"
  }
  fn default_severity(&self) -> Severity {
    Severity::Hint
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let start: usize = u32::from(stmt.range.start()) as usize;
    let end: usize = (u32::from(stmt.range.end()) as usize).min(source.len());
    let body = &source[start..end];
    let upper = body.to_ascii_uppercase();
    if !upper.contains("LANGUAGE PLPGSQL") {
      return;
    }
    let bytes = body.as_bytes();
    let n = bytes.len();
    let mut i = 0;
    while i + 7 <= n {
      if upper.as_bytes()[i..i + 7].eq_ignore_ascii_case(b"PERFORM")
        && (i == 0 || !is_word(bytes[i - 1] as char))
        && (i + 7 == n || !is_word(bytes[i + 7] as char))
      {
        // Find end of statement (`;` at top level).
        let mut j = i + 7;
        while j < n && bytes[j] != b';' {
          if bytes[j] == b'\'' {
            j += 1;
            while j < n && bytes[j] != b'\'' {
              j += 1;
            }
            if j < n {
              j += 1;
            }
            continue;
          }
          j += 1;
        }
        let stmt_text = &upper[i + 7..j];
        // Side-effecting calls? If present, PERFORM is fine.
        let side_effect = stmt_text.contains("PG_ADVISORY_")
          || stmt_text.contains("PG_NOTIFY")
          || stmt_text.contains("PG_SLEEP")
          || stmt_text.contains("NEXTVAL(")
          || stmt_text.contains("SETVAL(")
          || stmt_text.contains("RANDOM(")
          || stmt_text.contains("LASTVAL(");
        // Conservative: if the body has no FROM (so it's a pure
        // expression / single function call), and no side-effect
        // call, the PERFORM does nothing.
        if !side_effect && !stmt_text.contains(" FROM ") {
          let abs_start = start + i;
          let abs_end = start + i + 7;
          out.push(Diagnostic {
            code: "sql158",
            severity: Severity::Hint,
            message: "PERFORM <pure expression> -- the result is discarded and the call has no side effects".into(),
            range: text_size::TextRange::new((abs_start as u32).into(), (abs_end as u32).into()),
          });
          return;
        }
        i = j.saturating_add(1);
        continue;
      }
      i += 1;
    }
  }
}

fn is_word(c: char) -> bool {
  c.is_alphanumeric() || c == '_'
}
