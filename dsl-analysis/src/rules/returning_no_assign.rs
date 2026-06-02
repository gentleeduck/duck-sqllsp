//! sql143: `INSERT/UPDATE/DELETE ... RETURNING ...` inside a PL/pgSQL
//! block without `INTO <vars>` or `STRICT` -- the returned row is
//! silently discarded. Almost always a bug.

use crate::{Diagnostic, LintRule, Severity};
use crate::textutil::is_word;
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql143"
  }
  fn default_severity(&self) -> Severity {
    Severity::Hint
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, raw) = crate::stmt_body(stmt, source);
    // Strip comments/strings so a `-- RETURNING ...` header comment
    // or `'RETURNING'` literal doesn't trigger this rule.
    let body_owned = crate::textutil::strip_comments_only(raw);
    let body = body_owned.as_str();
    let upper = body.to_ascii_uppercase();
    // Only fire when the statement is wrapped in a PL/pgSQL function
    // body or a DO block -- top-level RETURNING is fine.
    if !upper.contains("LANGUAGE PLPGSQL") && !upper.contains("DO $$") {
      return;
    }
    let bytes = upper.as_bytes();
    let n = bytes.len();
    let mut i = 0;
    while i + 9 <= n {
      if &upper[i..i + 9] == "RETURNING"
        && (i == 0 || !is_word(bytes[i - 1] as char))
        && (i + 9 == n || !is_word(bytes[i + 9] as char))
      {
        // Look forward to `;` or `INTO`. If `INTO` appears
        // before `;`, this RETURNING captures into a var -- OK.
        let mut j = i + 9;
        let mut into_first = false;
        while j < n && bytes[j] != b';' {
          if j + 6 <= n && &upper[j..j + 6] == " INTO " {
            into_first = true;
            break;
          }
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
        if !into_first {
          let abs_start = start + i;
          let abs_end = start + i + 9;
          out.push(Diagnostic {
            code: "sql143",
            severity: Severity::Hint,
            message:
              "RETURNING inside PL/pgSQL discarded -- add `INTO <var>` or use PERFORM if the result is intentional"
                .into(),
            range: crate::range_at(abs_start, abs_end),
          });
          return;
        }
        i = j + 1;
        continue;
      }
      i += 1;
    }
  }
}

