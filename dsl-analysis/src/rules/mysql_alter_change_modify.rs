//! sql672: MySQL's `ALTER TABLE ... CHANGE [COLUMN]` / `MODIFY [COLUMN]`
//! sub-commands. PostgreSQL spells column changes differently:
//! `MODIFY col <type>` -> `ALTER COLUMN col TYPE <type>` (plus separate
//! `SET/DROP DEFAULT`, `SET/DROP NOT NULL`); `CHANGE old new <type>` ->
//! `RENAME COLUMN old TO new` *and* `ALTER COLUMN new TYPE <type>`. The MySQL
//! keywords are syntax errors in PG.

use crate::clause_scan::is_word;
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

fn kw(b: &[u8], i: usize, w: &[u8]) -> bool {
  i + w.len() <= b.len()
    && &b[i..i + w.len()] == w
    && (i == 0 || !is_word(b[i - 1] as char))
    && b.get(i + w.len()).is_none_or(|&c| !is_word(c as char))
}

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql672"
  }
  fn default_severity(&self) -> Severity {
    Severity::Error
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, _body, upper) = crate::stmt_body_upper(stmt, source);
    let u = upper.trim_start();
    if !u.starts_with("ALTER") || !upper.contains("TABLE") {
      return;
    }
    let b = upper.as_bytes();
    let n = b.len();
    let mut depth = 0i32;
    let mut i = 0usize;
    while i < n {
      match b[i] {
        b'(' | b'[' => depth += 1,
        b')' | b']' => depth -= 1,
        _ if depth == 0 => {
          let hit = if kw(b, i, b"CHANGE") {
            Some(("CHANGE", 6, "RENAME COLUMN old TO new (and ALTER COLUMN new TYPE ...)"))
          } else if kw(b, i, b"MODIFY") {
            Some(("MODIFY", 6, "ALTER COLUMN col TYPE ... (plus SET/DROP DEFAULT / NOT NULL)"))
          } else {
            None
          };
          if let Some((name, len, pg)) = hit {
            out.push(Diagnostic {
              code: "sql672",
              severity: Severity::Error,
              message: format!("`{name}` is MySQL ALTER syntax -- PostgreSQL uses `{pg}`"),
              range: crate::range_at(start + i, start + i + len),
            });
            return;
          }
        }
        _ => {}
      }
      i += 1;
    }
  }
}
