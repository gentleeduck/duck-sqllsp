//! sql509: explicit `pg_temp.<table>` (or `pg_temp_<N>.<table>`)
//! reference. Temporary tables live in a per-backend internal
//! schema whose name (`pg_temp_<backend_id>`) is backend-specific
//! and gets aliased as `pg_temp` in the search_path. Just write the
//! table name unqualified -- PG resolves it via search_path
//! automatically. Explicit qualification leaks an implementation
//! detail into the SQL and can break across sessions or restarts.

use crate::clause_scan::is_word;
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;
use text_size::TextRange;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql509"
  }
  fn default_severity(&self) -> Severity {
    Severity::Hint
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, raw) = crate::stmt_body(stmt, source);
    let cleaned = crate::textutil::strip_noise_full(raw);
    let lower = cleaned.to_ascii_lowercase();
    let lb = lower.as_bytes();
    let bytes = cleaned.as_bytes();
    let n = lb.len();
    let needle = b"pg_temp";
    let m = needle.len();
    let mut i = 0usize;
    let mut emitted: std::collections::HashSet<usize> = std::collections::HashSet::new();
    while i + m <= n {
      if &lb[i..i + m] != needle || (i > 0 && is_word(lb[i - 1] as char)) {
        i += 1;
        continue;
      }
      // After "pg_temp" we accept either:
      //   * nothing word-y (bare schema reference)
      //   * `_<digits>` (pg_temp_3 etc.)
      // Then a `.` to be a qualifier.
      let mut k = i + m;
      // optional `_<digits>` suffix
      if k < n && lb[k] == b'_' {
        let mut p = k + 1;
        if p < n && lb[p].is_ascii_digit() {
          while p < n && lb[p].is_ascii_digit() {
            p += 1;
          }
          k = p;
        } else {
          // pg_temp_<non-digit> -- not the schema we care about
          i += 1;
          continue;
        }
      }
      // Now expect a `.` (qualifier dot).
      if k >= n || bytes[k] != b'.' {
        i += 1;
        continue;
      }
      // Read the table name to include in the message.
      let mut p = k + 1;
      let id_start = p;
      while p < n {
        let b = bytes[p];
        if b.is_ascii_alphanumeric() || b == b'_' {
          p += 1;
        } else {
          break;
        }
      }
      if p == id_start {
        i = k + 1;
        continue;
      }
      let table = &cleaned[id_start..p];
      if emitted.insert(i) {
        let abs_s = start + i;
        let abs_e = start + p;
        out.push(Diagnostic {
          code: "sql509",
          severity: Severity::Hint,
          message: format!(
            "explicit `pg_temp.{table}` reference -- temporary tables live in a per-backend schema (`pg_temp_<backend_id>`) that PG auto-resolves via search_path. Drop the qualifier and just write `{table}`; otherwise the query breaks across sessions or restarts."
          ),
          range: TextRange::new((abs_s as u32).into(), (abs_e as u32).into()),
        });
      }
      i = p;
    }
  }
}
