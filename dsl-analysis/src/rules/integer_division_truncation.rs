//! sql504: `<int_col> / <int_literal>` -- integer/integer division
//! in PG truncates toward zero (e.g. `5 / 2` is `2`, not `2.5`). If
//! the author meant float division, cast one side: `col::float / 2`
//! or `col / 2.0`. Catches the common case where the LHS is a
//! known integer column and the RHS is a bare integer literal.

use crate::clause_scan::{find_clause, find_clause_end};
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::{Statement, StatementKind};
use dsl_resolve::Scope;
use text_size::TextRange;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql504"
  }
  fn default_severity(&self) -> Severity {
    Severity::Hint
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let Some(target) = (match &stmt.kind {
      StatementKind::Select(s) => {
        if s.from.len() != 1 {
          return;
        }
        s.from.first()
      },
      _ => return,
    }) else {
      return;
    };
    let Some(t) = catalog.find_table(target.schema.as_deref(), &target.name) else { return };

    let start: usize = u32::from(stmt.range.start()) as usize;
    let end: usize = (u32::from(stmt.range.end()) as usize).min(source.len());
    let raw = &source[start..end];
    let cleaned = crate::textutil::strip_noise_full(raw);
    let upper = cleaned.to_ascii_uppercase();
    let ub = upper.as_bytes();
    let bytes = cleaned.as_bytes();
    let n = bytes.len();
    // Restrict to the SELECT-list region (between SELECT and FROM).
    let Some(select_at) = find_word(ub, b"SELECT", 0, n) else { return };
    let list_start = select_at + 6;
    let list_end = find_clause(ub, b"FROM").unwrap_or(n);
    // Also accept WHERE region for sql504 since it's the same semantic mistake.
    let where_stop = find_clause_end(ub, list_start, &["GROUP BY", "ORDER BY", "LIMIT", "OFFSET", "HAVING", "FOR", "FETCH", "WINDOW", "RETURNING"]);
    let _ = where_stop;

    let mut emitted: std::collections::HashSet<usize> = std::collections::HashSet::new();
    let mut i = list_start;
    let stop = list_end;
    while i < stop {
      if bytes[i] != b'/' {
        i += 1;
        continue;
      }
      let op_at = i;
      // Skip line comments / division forms we don't want: `//`.
      if (op_at + 1 < n && bytes[op_at + 1] == b'/') || (op_at > 0 && bytes[op_at - 1] == b'/') {
        i += 1;
        continue;
      }
      // LHS: walk back over ws, then read ident (no cast involved).
      let mut l = op_at;
      while l > 0 && bytes[l - 1].is_ascii_whitespace() {
        l -= 1;
      }
      // Reject if the char just before is `)` or `'` or a digit -- those
      // mean it's a cast / literal / numeric expression and we'd need
      // deeper type inference to be safe.
      if l > 0 {
        let b = bytes[l - 1];
        if b == b')' || b == b'\'' || b == b'.' {
          i += 1;
          continue;
        }
      }
      let id_end = l;
      while l > 0 {
        let b = bytes[l - 1];
        if b.is_ascii_alphanumeric() || b == b'_' || b == b'.' {
          l -= 1;
        } else {
          break;
        }
      }
      if l == id_end {
        i += 1;
        continue;
      }
      let lhs = &raw[l..id_end];
      // Reject if followed by `::` (column has explicit cast).
      // Actually `::` would be AFTER the ident on the right, before /.
      // We need to detect `col::int / 2` -- already handled because
      // walking back from `/` would stop at `int` (after the colons)
      // and `int` isn't a column name. So that case naturally drops.
      let lhs_bare = lhs.rsplit('.').next().unwrap_or(lhs);
      let Some(col) = t.columns.iter().find(|c| c.name.eq_ignore_ascii_case(lhs_bare)) else {
        i += 1;
        continue;
      };
      let dtype = col.data_type.to_ascii_lowercase();
      let is_int = matches!(dtype.as_str(), "int" | "integer" | "int2" | "int4" | "int8" | "smallint" | "bigint");
      if !is_int {
        i += 1;
        continue;
      }
      // RHS: skip ws, read digits, ensure NO `.` follows (would make
      // it a float literal).
      let mut r = op_at + 1;
      while r < n && bytes[r].is_ascii_whitespace() {
        r += 1;
      }
      let num_start = r;
      while r < n && bytes[r].is_ascii_digit() {
        r += 1;
      }
      if r == num_start {
        i += 1;
        continue;
      }
      // If followed by `.` (float literal), skip.
      if r < n && bytes[r] == b'.' {
        i += 1;
        continue;
      }
      let rhs_literal = &raw[num_start..r];
      // Skip RHS = 0 -- that's sql278's territory (runtime div-by-0)
      // and our truncation message ("write i::float / 0") is nonsense.
      if rhs_literal.chars().all(|c| c == '0') {
        i = r;
        continue;
      }
      if emitted.insert(op_at) {
        let abs_s = start + l;
        let abs_e = start + r;
        out.push(Diagnostic {
          code: "sql504",
          severity: Severity::Hint,
          message: format!(
            "`{lhs} / {rhs_literal}` -- integer ÷ integer truncates toward zero (e.g. `5 / 2 = 2`). If you wanted a fractional result, write `{lhs}::float / {rhs_literal}` or `{lhs} / {rhs_literal}.0`."
          ),
          range: TextRange::new((abs_s as u32).into(), (abs_e as u32).into()),
        });
      }
      i = r;
    }
  }
}

fn find_word(ub: &[u8], w: &[u8], from: usize, to: usize) -> Option<usize> {
  let m = w.len();
  let mut i = from;
  while i + m <= to {
    if &ub[i..i + m] == w {
      let prev_ok = i == 0 || !(ub[i - 1] as char).is_alphanumeric() && ub[i - 1] != b'_';
      let next_ok = i + m == ub.len() || !(ub[i + m] as char).is_alphanumeric() && ub[i + m] != b'_';
      if prev_ok && next_ok {
        return Some(i);
      }
    }
    i += 1;
  }
  None
}
