//! sql429: `WHERE col == 1` (C-style) and `WHERE col <=> 1` (MySQL
//! null-safe equal) -- PG accepts neither. PG's `==` raises
//! "operator does not exist", and `<=>` raises a similar error
//! (the spaceship operator is MySQL-specific).
//!
//! Real fix: `=` for C-style typos, and `IS NOT DISTINCT FROM` for
//! NULL-safe equality.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;
use text_size::TextRange;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql429"
  }
  fn default_severity(&self) -> Severity {
    Severity::Error
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let start: usize = u32::from(stmt.range.start()) as usize;
    let end: usize = (u32::from(stmt.range.end()) as usize).min(source.len());
    let raw = &source[start..end];
    let cleaned = crate::textutil::strip_noise_full(raw);
    let bytes = cleaned.as_bytes();
    let n = bytes.len();
    let mut i = 0usize;
    while i + 1 < n {
      // `==` -- two consecutive `=`
      if bytes[i] == b'=' && bytes[i + 1] == b'=' {
        let abs_s = start + i;
        let abs_e = start + i + 2;
        out.push(Diagnostic {
          code: "sql429",
          severity: Severity::Error,
          message: "`==` is not a Postgres operator -- use `=` for equality (or `IS NOT DISTINCT FROM` for NULL-safe equality)".into(),
          range: TextRange::new((abs_s as u32).into(), (abs_e as u32).into()),
        });
        i += 2;
        continue;
      }
      // `<=>` -- three consecutive chars: <, =, >
      if i + 2 < n && bytes[i] == b'<' && bytes[i + 1] == b'=' && bytes[i + 2] == b'>' {
        let abs_s = start + i;
        let abs_e = start + i + 3;
        out.push(Diagnostic {
          code: "sql429",
          severity: Severity::Error,
          message: "`<=>` is the MySQL NULL-safe equal operator -- Postgres uses `IS NOT DISTINCT FROM` instead".into(),
          range: TextRange::new((abs_s as u32).into(), (abs_e as u32).into()),
        });
        i += 3;
        continue;
      }
      i += 1;
    }
  }
}
