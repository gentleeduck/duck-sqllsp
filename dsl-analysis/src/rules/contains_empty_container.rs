//! sql477: `col @> '{}'::jsonb` / `col @> '[]'::jsonb` / `col @>
//! ARRAY[]::int[]` -- containment against an empty container is
//! vacuously TRUE for every non-NULL value (the empty container is
//! a subset of everything). The predicate has no filter effect and
//! is almost always a leftover placeholder.

use crate::clause_scan::is_word;
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;
use text_size::TextRange;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql477"
  }
  fn default_severity(&self) -> Severity {
    Severity::Warning
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let start: usize = u32::from(stmt.range.start()) as usize;
    let end: usize = (u32::from(stmt.range.end()) as usize).min(source.len());
    let raw = &source[start..end];
    let raw_bytes = raw.as_bytes();
    let n = raw_bytes.len();
    let mut i = 0usize;
    let mut emitted: std::collections::HashSet<usize> = std::collections::HashSet::new();
    while i + 2 <= n {
      // Only look at `@>` two-char operator.
      if !(raw_bytes[i] == b'@' && raw_bytes[i + 1] == b'>') {
        i += 1;
        continue;
      }
      let op_at = i;
      let mut k = i + 2;
      while k < n && raw_bytes[k].is_ascii_whitespace() {
        k += 1;
      }
      // Possible empty-container literals:
      //   '{}'::jsonb  or  '{}'::json  or  '{}' (bare)
      //   '[]'::jsonb  or  '[]'::json
      //   ARRAY[]::<type>[]
      //   '{}'::<type>[]
      let rest = &raw[k..];
      let is_empty = rest.starts_with("'{}'")
        || rest.starts_with("'[]'")
        || rest.to_ascii_uppercase().starts_with("ARRAY[]");
      if is_empty && emitted.insert(op_at) {
        // Find end of the literal (until next ' ', ')' or end).
        let mut end_at = k;
        // For the ARRAY[] case, skip the bracket then any cast.
        // For the quoted '{}'/'[]' case, skip the closing quote.
        if rest.starts_with('\'') {
          // skip the literal (2 chars + close quote = 4)
          end_at += 4;
          // optional `::type[]` cast follows
          if end_at + 2 <= n && &raw[end_at..(end_at + 2).min(n)] == "::" {
            end_at += 2;
            while end_at < n && (is_word(raw_bytes[end_at] as char) || raw_bytes[end_at] == b'[' || raw_bytes[end_at] == b']') {
              end_at += 1;
            }
          }
        } else {
          // ARRAY[] case
          end_at += 7; // ARRAY[]
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
          code: "sql477",
          severity: Severity::Warning,
          message: "`@> <empty>` (empty jsonb / array container on the RHS) is vacuously TRUE for every non-NULL value -- the predicate has no filter effect and is almost always a leftover placeholder".into(),
          range: TextRange::new((abs_s as u32).into(), (abs_e as u32).into()),
        });
      }
      i += 2;
    }
  }
}
