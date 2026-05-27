//! sql500: `date_col1 - date_col2` -- PG returns `integer` (days),
//! not `interval`. Confusing for authors expecting an interval; the
//! result also doesn't compose with interval-arithmetic. If you
//! want an interval, use `age(d1, d2)`; if you want days, alias the
//! result so the units are explicit (e.g. `AS days`).
//!
//! Note: `timestamp - timestamp` returns an interval (no warning);
//! `date - interval` returns a timestamp (no warning).

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::{Statement, StatementKind};
use dsl_resolve::Scope;
use text_size::TextRange;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql500"
  }
  fn default_severity(&self) -> Severity {
    Severity::Hint
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let target = match &stmt.kind {
      StatementKind::Select(s) => {
        if s.from.len() != 1 {
          return;
        }
        s.from.first()
      },
      _ => return,
    };
    let Some(target) = target else { return };
    let Some(t) = catalog.find_table(target.schema.as_deref(), &target.name) else { return };

    let start: usize = u32::from(stmt.range.start()) as usize;
    let end: usize = (u32::from(stmt.range.end()) as usize).min(source.len());
    let raw = &source[start..end];
    let bytes = raw.as_bytes();
    let n = bytes.len();
    let mut emitted: std::collections::HashSet<usize> = std::collections::HashSet::new();
    let mut i = 0usize;
    while i < n {
      // Skip string literals.
      if bytes[i] == b'\'' {
        i += 1;
        while i < n && bytes[i] != b'\'' {
          i += 1;
        }
        i = (i + 1).min(n);
        continue;
      }
      if bytes[i] != b'-' {
        i += 1;
        continue;
      }
      let op_at = i;
      // Skip arithmetic operators like `--` (comment) or `->`
      // (json), although strip_noise should handle comments; we use
      // raw bytes here, so guard explicitly.
      if (op_at + 1 < n && bytes[op_at + 1] == b'-')
        || (op_at > 0 && bytes[op_at - 1] == b'-')
        || (op_at + 1 < n && bytes[op_at + 1] == b'>')
      {
        i += 1;
        continue;
      }
      // Read LHS ident (walking back).
      let mut l = op_at;
      while l > 0 && bytes[l - 1].is_ascii_whitespace() {
        l -= 1;
      }
      let lhs_end = l;
      while l > 0 {
        let b = bytes[l - 1];
        if b.is_ascii_alphanumeric() || b == b'_' || b == b'.' {
          l -= 1;
        } else {
          break;
        }
      }
      if l == lhs_end {
        i += 1;
        continue;
      }
      // Ensure LHS not preceded by `-` or alphanumeric (would mean
      // it's part of a longer expression).
      if l > 0 {
        let b = bytes[l - 1];
        if b.is_ascii_alphanumeric() || b == b'_' || b == b'.' {
          i += 1;
          continue;
        }
      }
      let lhs = &raw[l..lhs_end];
      // Read RHS ident (walking forward).
      let mut r = op_at + 1;
      while r < n && bytes[r].is_ascii_whitespace() {
        r += 1;
      }
      // Reject `INTERVAL '...'` form on the RHS.
      let rhs_id_start = r;
      while r < n {
        let b = bytes[r];
        if b.is_ascii_alphanumeric() || b == b'_' || b == b'.' {
          r += 1;
        } else {
          break;
        }
      }
      if rhs_id_start == r {
        i = op_at + 1;
        continue;
      }
      let rhs = &raw[rhs_id_start..r];
      if rhs.eq_ignore_ascii_case("INTERVAL") || rhs.eq_ignore_ascii_case("CURRENT_DATE") {
        // date - INTERVAL '1 day' returns timestamp; skip.
        // date - CURRENT_DATE may also be date; we'd need type to know.
        i = r.max(op_at + 1);
        continue;
      }
      // Both sides must be bare column names (or alias.col) that
      // exist in the target table as `date`.
      let lhs_bare = lhs.rsplit('.').next().unwrap_or(lhs);
      let rhs_bare = rhs.rsplit('.').next().unwrap_or(rhs);
      let l_col = t.columns.iter().find(|c| c.name.eq_ignore_ascii_case(lhs_bare));
      let r_col = t.columns.iter().find(|c| c.name.eq_ignore_ascii_case(rhs_bare));
      let both_date = matches!((l_col, r_col), (Some(a), Some(b)) if a.data_type.eq_ignore_ascii_case("date") && b.data_type.eq_ignore_ascii_case("date"));
      if both_date && emitted.insert(op_at) {
        let abs_s = start + l;
        let abs_e = start + r;
        out.push(Diagnostic {
          code: "sql500",
          severity: Severity::Hint,
          message: format!(
            "`{lhs} - {rhs}` -- `date - date` returns `integer` (days), not `interval`. Use `age({lhs}, {rhs})` for an interval, or alias the result (e.g. `AS days`) to make units explicit."
          ),
          range: TextRange::new((abs_s as u32).into(), (abs_e as u32).into()),
        });
      }
      i = r.max(op_at + 1);
    }
  }
}
