//! sql478: `col <@ '{}'::jsonb` / `col <@ '[]'::jsonb` / `col <@
//! ARRAY[]::int[]` -- "col is contained-by an empty container" is
//! the inverse of sql477's containment case: the predicate is TRUE
//! only when `col` itself is empty (or NULL is filtered by the
//! comparison). It almost never expresses what the author meant --
//! the intent was probably `col = '{}'::jsonb`, `col IS NULL`, or
//! to remove the placeholder filter entirely.

use crate::clause_scan::is_word;
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;
use text_size::TextRange;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql478"
  }
  fn default_severity(&self) -> Severity {
    Severity::Warning
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, raw) = crate::stmt_body(stmt, source);
    let raw_bytes = raw.as_bytes();
    let n = raw_bytes.len();
    let mut i = 0usize;
    let mut emitted: std::collections::HashSet<usize> = std::collections::HashSet::new();
    while i + 2 <= n {
      if !(raw_bytes[i] == b'<' && raw_bytes[i + 1] == b'@') {
        i += 1;
        continue;
      }
      let op_at = i;
      let mut k = i + 2;
      while k < n && raw_bytes[k].is_ascii_whitespace() {
        k += 1;
      }
      let rest = &raw[k..];
      let is_empty = rest.starts_with("'{}'")
        || rest.starts_with("'[]'")
        || rest.to_ascii_uppercase().starts_with("ARRAY[]");
      if is_empty && emitted.insert(op_at) {
        let mut end_at = k;
        if rest.starts_with('\'') {
          end_at += 4;
          if end_at + 2 <= n && &raw[end_at..(end_at + 2).min(n)] == "::" {
            end_at += 2;
            while end_at < n && (is_word(raw_bytes[end_at] as char) || raw_bytes[end_at] == b'[' || raw_bytes[end_at] == b']') {
              end_at += 1;
            }
          }
        } else {
          end_at += 7;
          if end_at + 2 <= n && &raw[end_at..(end_at + 2).min(n)] == "::" {
            end_at += 2;
            while end_at < n && (is_word(raw_bytes[end_at] as char) || raw_bytes[end_at] == b'[' || raw_bytes[end_at] == b']') {
              end_at += 1;
            }
          }
        }
        let abs_s = start + op_at;
        let abs_e = (start + end_at).min(source.len());
        out.push(Diagnostic {
          code: "sql478",
          severity: Severity::Warning,
          message: "`<@ <empty>` (empty jsonb / array container on the RHS) matches only when the LHS is itself empty -- almost certainly not the intended filter. Did you mean `col = '{}'::jsonb`, `col IS NULL`, or to drop the predicate?".into(),
          range: TextRange::new((abs_s as u32).into(), (abs_e as u32).into()),
        });
      }
      i += 2;
    }
  }
}
