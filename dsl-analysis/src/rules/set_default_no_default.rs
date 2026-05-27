//! sql496: `UPDATE t SET col = DEFAULT` where `col` has no DEFAULT
//! definition. PG resets `col` to its default expression:
//!   * column is NOT NULL with no default -> runtime error
//!     ("null value in column violates not-null constraint")
//!   * column is nullable with no default -> silently becomes NULL,
//!     usually not what the author intended.
//!
//! pg_query exposes the `DEFAULT` keyword as `Expr::Other("")`, so
//! we text-scan the SET clause for `<col> = DEFAULT` patterns.

use crate::clause_scan::is_word;
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::{Statement, StatementKind};
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql496"
  }
  fn default_severity(&self) -> Severity {
    Severity::Warning
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let StatementKind::Update(upd) = &stmt.kind else { return };
    let Some(t) = catalog.find_table(upd.table.schema.as_deref(), &upd.table.name) else { return };

    let start: usize = u32::from(stmt.range.start()) as usize;
    let end: usize = (u32::from(stmt.range.end()) as usize).min(source.len());
    let body = &source[start..end];
    let upper = body.to_ascii_uppercase();
    let ub = upper.as_bytes();
    let n = ub.len();
    // Find SET clause start (whole-word).
    let Some(set_at) = find_word(ub, b"SET", 0, n) else { return };
    let set_after = set_at + 3;
    // Find WHERE / RETURNING / FROM clause end (anything that
    // terminates SET).
    let mut end_at = n;
    for kw in ["WHERE", "RETURNING", "FROM"] {
      if let Some(p) = find_word(ub, kw.as_bytes(), set_after, n) {
        end_at = end_at.min(p);
      }
    }
    // Scan for `DEFAULT` keywords inside SET clause. Each occurrence
    // must be preceded by `=` (with optional ws) and have a column
    // identifier before the `=`.
    let mut i = set_after;
    while let Some(rel) = find_word(ub, b"DEFAULT", i, end_at) {
      // Walk back over whitespace to find `=`.
      let mut p = rel;
      while p > set_after && ub[p - 1].is_ascii_whitespace() {
        p -= 1;
      }
      if p == set_after || ub[p - 1] != b'=' {
        i = rel + 7;
        continue;
      }
      // Walk back past `=` and whitespace, then read the column name.
      let mut q = p - 1;
      while q > set_after && ub[q - 1].is_ascii_whitespace() {
        q -= 1;
      }
      let id_end = q;
      while q > set_after && is_word(ub[q - 1] as char) {
        q -= 1;
      }
      if q == id_end {
        i = rel + 7;
        continue;
      }
      let col_name = &body[q..id_end];
      let Some(col) = t.columns.iter().find(|c| c.name.eq_ignore_ascii_case(col_name)) else {
        i = rel + 7;
        continue;
      };
      if col.default.is_some() || col.generated.is_some() {
        i = rel + 7;
        continue;
      }
      let abs_s = start + q;
      let abs_e = start + rel + 7;
      let (severity, msg) = if col.nullable {
        (
          Severity::Hint,
          format!(
            "`SET {col_name} = DEFAULT` -- `{}` has no DEFAULT defined, so it silently becomes NULL. Write `SET {col_name} = NULL` to make the intent explicit, or add a column DEFAULT.",
            col.name
          ),
        )
      } else {
        (
          Severity::Error,
          format!(
            "`SET {col_name} = DEFAULT` -- `{}` is NOT NULL with no DEFAULT defined; PG raises `null value in column violates not-null constraint` at runtime.",
            col.name
          ),
        )
      };
      out.push(Diagnostic {
        code: "sql496",
        severity,
        message: msg,
        range: text_size::TextRange::new((abs_s as u32).into(), (abs_e as u32).into()),
      });
      i = rel + 7;
    }
  }
}

fn find_word(ub: &[u8], w: &[u8], from: usize, to: usize) -> Option<usize> {
  let m = w.len();
  let mut i = from;
  while i + m <= to {
    if &ub[i..i + m] == w {
      let prev_ok = i == 0 || !is_word(ub[i - 1] as char);
      let next_ok = i + m == ub.len() || !is_word(ub[i + m] as char);
      if prev_ok && next_ok {
        return Some(i);
      }
    }
    i += 1;
  }
  None
}
