//! sql490: `col || ''` / `'' || col` -- concatenating with the empty
//! string is a no-op (the expression equals `col`). Almost always
//! either a placeholder where a real literal should go or a leftover
//! from refactoring. Drop the empty operand.
//!
//! Note: sql413 catches `col || NULL` (returns NULL). sql490 is
//! the empty-string-literal counterpart, which has different
//! semantics (no-op, not NULL).

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;
use text_size::TextRange;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql490"
  }
  fn default_severity(&self) -> Severity {
    Severity::Hint
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let start: usize = u32::from(stmt.range.start()) as usize;
    let end: usize = (u32::from(stmt.range.end()) as usize).min(source.len());
    let raw = &source[start..end];
    let bytes = raw.as_bytes();
    let n = bytes.len();
    let mut emitted: std::collections::HashSet<usize> = std::collections::HashSet::new();
    let mut i = 0usize;
    while i < n {
      let c = bytes[i];
      // Skip single-quoted strings (track empty `''` separately).
      if c == b'\'' {
        // Detect a bare empty literal `''` (two adjacent quotes).
        if i + 1 < n && bytes[i + 1] == b'\'' {
          i += 2;
          continue;
        }
        // Non-empty literal: skip to close (handling `''` escapes).
        i += 1;
        while i < n {
          if bytes[i] == b'\'' {
            if i + 1 < n && bytes[i + 1] == b'\'' {
              i += 2;
              continue;
            }
            i += 1;
            break;
          }
          i += 1;
        }
        continue;
      }
      // `||` two-byte operator at this position
      if c == b'|' && i + 1 < n && bytes[i + 1] == b'|' {
        let op_at = i;
        // Look LEFT: skip whitespace back, then check if the two
        // chars before form `''`.
        let mut l = op_at;
        while l > 0 && bytes[l - 1].is_ascii_whitespace() {
          l -= 1;
        }
        let left_empty = l >= 2 && bytes[l - 1] == b'\'' && bytes[l - 2] == b'\'';
        // Look RIGHT: skip ws fwd, check if next two are `''`.
        let mut r = op_at + 2;
        while r < n && bytes[r].is_ascii_whitespace() {
          r += 1;
        }
        let right_empty = r + 1 < n && bytes[r] == b'\'' && bytes[r + 1] == b'\''
          // Confirm not the start of a longer literal `'foo'`
          && (r + 2 == n || bytes[r + 2] != b'\'');
        // Also confirm left-empty isn't actually inside `'foo''`...
        // i.e., the char before l-2 isn't a non-empty literal char.
        let left_empty_ok = left_empty && (l < 3 || bytes[l - 3] != b'\'');
        let hit = left_empty_ok || right_empty;
        if hit && emitted.insert(op_at) {
          let (abs_s, abs_e) = if left_empty_ok {
            (start + (l - 2), start + op_at + 2)
          } else {
            (start + op_at, start + r + 2)
          };
          let snippet = if left_empty_ok { "'' || ..." } else { "... || ''" };
          out.push(Diagnostic {
            code: "sql490",
            severity: Severity::Hint,
            message: format!(
              "`{snippet}` -- concatenating with the empty string is a no-op. Drop the `''` operand."
            ),
            range: TextRange::new((abs_s as u32).into(), (abs_e as u32).into()),
          });
        }
        i = op_at + 2;
        continue;
      }
      i += 1;
    }
  }
}
