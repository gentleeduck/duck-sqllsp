//! sql502: `WHERE timestamptz_col <op> TIMESTAMP 'lit'` -- comparing
//! a `timestamptz` column to a plain `TIMESTAMP` literal makes PG
//! coerce the literal to timestamptz using the *session* timezone.
//! The same query then returns different rows depending on session
//! TZ, which is almost never intended. Prefer the explicit form:
//! `TIMESTAMPTZ '<lit>'` (with offset) or `'lit'::timestamptz` so
//! the timezone is unambiguous.

use crate::clause_scan::{find_clause, find_clause_end, is_word};
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::{Statement, StatementKind};
use dsl_resolve::Scope;
use text_size::TextRange;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql502"
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
      StatementKind::Update(u) => Some(&u.table),
      StatementKind::Delete(d) => Some(&d.table),
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
    let raw_bytes = raw.as_bytes();
    let stopwords = ["GROUP BY", "ORDER BY", "HAVING", "LIMIT", "OFFSET", "FOR", "FETCH", "WINDOW", "RETURNING"];
    let Some(rel_where) = find_clause(ub, b"WHERE") else { return };
    let pred_start = rel_where + 5;
    let pred_end = find_clause_end(ub, pred_start, &stopwords).min(ub.len());

    let mut i = pred_start;
    let mut emitted: std::collections::HashSet<usize> = std::collections::HashSet::new();
    while i + 9 <= pred_end {
      // Find `TIMESTAMP` keyword (NOT `TIMESTAMPTZ`).
      if !(&ub[i..i + 9] == b"TIMESTAMP" && (i == 0 || !is_word(ub[i - 1] as char))) {
        i += 1;
        continue;
      }
      // Reject if followed by TZ (TIMESTAMPTZ) or `WITH TIME ZONE`.
      let after_kw = i + 9;
      if after_kw < pred_end && (ub[after_kw] == b'T' || ub[after_kw] == b'Z') {
        // Could be TIMESTAMPTZ; check.
        if after_kw + 2 <= pred_end && &ub[after_kw..after_kw + 2] == b"TZ" && (after_kw + 2 == pred_end || !is_word(ub[after_kw + 2] as char)) {
          i = after_kw + 2;
          continue;
        }
      }
      if after_kw < pred_end && is_word(ub[after_kw] as char) {
        // `TIMESTAMP_FOO` -- not the keyword.
        i += 1;
        continue;
      }
      // Walk forward over ws and optional `WITH TIME ZONE`.
      let mut k = after_kw;
      while k < pred_end && raw_bytes[k].is_ascii_whitespace() {
        k += 1;
      }
      if k + 14 <= pred_end && &ub[k..k + 14] == b"WITH TIME ZONE" {
        // This is TIMESTAMP WITH TIME ZONE -- equivalent to TIMESTAMPTZ; skip.
        i = k + 14;
        continue;
      }
      // Expect a `'...'` literal next.
      if k >= pred_end || raw_bytes[k] != b'\'' {
        i = after_kw;
        continue;
      }
      // Read literal -- we don't need the contents, just the end.
      let mut lit_end = k + 1;
      while lit_end < pred_end {
        if raw_bytes[lit_end] == b'\'' {
          if lit_end + 1 < pred_end && raw_bytes[lit_end + 1] == b'\'' {
            lit_end += 2;
            continue;
          }
          lit_end += 1;
          break;
        }
        lit_end += 1;
      }
      // Walk back from `TIMESTAMP` to find the comparison op and the LHS column.
      // We expect `<col> <op> TIMESTAMP '...'`.
      let mut p = i;
      while p > pred_start && raw_bytes[p - 1].is_ascii_whitespace() {
        p -= 1;
      }
      // Read op: `=`, `<`, `>`, `<=`, `>=`, `<>`, `!=`.
      let op_end = p;
      let mut op_start = p;
      while op_start > pred_start && matches!(raw_bytes[op_start - 1], b'=' | b'<' | b'>' | b'!') {
        op_start -= 1;
      }
      if op_start == op_end {
        i = after_kw;
        continue;
      }
      // Walk back over ws and the LHS column ident.
      let mut q = op_start;
      while q > pred_start && raw_bytes[q - 1].is_ascii_whitespace() {
        q -= 1;
      }
      let id_end = q;
      while q > pred_start {
        let b = raw_bytes[q - 1];
        if b.is_ascii_alphanumeric() || b == b'_' || b == b'.' {
          q -= 1;
        } else {
          break;
        }
      }
      if q == id_end {
        i = after_kw;
        continue;
      }
      let lhs = &raw[q..id_end];
      let lhs_bare = lhs.rsplit('.').next().unwrap_or(lhs);
      let Some(col) = t.columns.iter().find(|c| c.name.eq_ignore_ascii_case(lhs_bare)) else {
        i = lit_end;
        continue;
      };
      // Must be a timestamptz column.
      let lc = col.data_type.to_ascii_lowercase();
      if !(lc == "timestamptz" || lc == "timestamp with time zone") {
        i = lit_end;
        continue;
      }
      if emitted.insert(i) {
        let abs_s = start + q;
        let abs_e = start + lit_end;
        out.push(Diagnostic {
          code: "sql502",
          severity: Severity::Hint,
          message: format!(
            "`{lhs}` is `timestamptz` but the literal is a plain `TIMESTAMP` -- PG coerces it using the *session* timezone at runtime, so the same query returns different rows depending on TZ. Use `TIMESTAMPTZ '...'` (with offset) or `'...'::timestamptz` for an unambiguous comparison."
          ),
          range: TextRange::new((abs_s as u32).into(), (abs_e as u32).into()),
        });
      }
      i = lit_end;
    }
  }
}
