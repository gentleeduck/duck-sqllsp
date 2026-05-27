//! sql510: `WHERE col SIMILAR TO col` / `NOT SIMILAR TO col` --
//! companion to sql508 for the SIMILAR TO operator. A column
//! compared against itself is always TRUE (for non-NULL rows) for
//! the positive form and always FALSE for the negated form,
//! regardless of what's in the column. Almost always a copy-paste
//! typo for two distinct columns.

use crate::clause_scan::{find_clause, find_clause_end, is_word};
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;
use text_size::TextRange;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql510"
  }
  fn default_severity(&self) -> Severity {
    Severity::Warning
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let start: usize = u32::from(stmt.range.start()) as usize;
    let end: usize = (u32::from(stmt.range.end()) as usize).min(source.len());
    let raw = &source[start..end];
    let cleaned = crate::textutil::strip_noise_full(raw);
    let upper = cleaned.to_ascii_uppercase();
    let ub = upper.as_bytes();
    let bytes = cleaned.as_bytes();
    let stopwords = ["GROUP BY", "ORDER BY", "HAVING", "LIMIT", "OFFSET", "FOR", "FETCH", "WINDOW", "RETURNING", "UNION", "INTERSECT", "EXCEPT"];
    let Some(rel_where) = find_clause(ub, b"WHERE") else { return };
    let pred_start = rel_where + 5;
    let pred_end = find_clause_end(ub, pred_start, &stopwords).min(ub.len());

    let mut emitted: std::collections::HashSet<usize> = std::collections::HashSet::new();
    let mut i = pred_start;
    while i + 7 <= pred_end {
      // Match `SIMILAR` at word boundary.
      if !(&ub[i..i + 7] == b"SIMILAR" && (i == 0 || !is_word(ub[i - 1] as char)) && (i + 7 == ub.len() || !is_word(ub[i + 7] as char))) {
        i += 1;
        continue;
      }
      // Then `TO`.
      let mut k = i + 7;
      while k < pred_end && bytes[k].is_ascii_whitespace() {
        k += 1;
      }
      if k + 2 > pred_end || &ub[k..k + 2] != b"TO" || (k + 2 < ub.len() && is_word(ub[k + 2] as char)) {
        i += 7;
        continue;
      }
      let op_end = k + 2;
      // Detect preceding NOT.
      let mut prev = i;
      while prev > pred_start && ub[prev - 1].is_ascii_whitespace() {
        prev -= 1;
      }
      let (op_text, op_start) = if prev >= 3 && &ub[prev - 3..prev] == b"NOT" && (prev == 3 || !is_word(ub[prev - 4] as char)) {
        ("NOT SIMILAR TO".to_string(), prev - 3)
      } else {
        ("SIMILAR TO".to_string(), i)
      };
      // LHS: walk back from op_start over ws + ident.
      let mut l = op_start;
      while l > pred_start && bytes[l - 1].is_ascii_whitespace() {
        l -= 1;
      }
      let lhs_end = l;
      while l > pred_start {
        let b = bytes[l - 1];
        if b.is_ascii_alphanumeric() || b == b'_' || b == b'.' {
          l -= 1;
        } else {
          break;
        }
      }
      if l == lhs_end {
        i = op_end;
        continue;
      }
      let lhs = &raw[l..lhs_end];
      // RHS: skip ws + ident.
      let mut r = op_end;
      while r < pred_end && bytes[r].is_ascii_whitespace() {
        r += 1;
      }
      let rhs_start = r;
      while r < pred_end {
        let b = bytes[r];
        if b.is_ascii_alphanumeric() || b == b'_' || b == b'.' {
          r += 1;
        } else {
          break;
        }
      }
      if r == rhs_start {
        i = op_end;
        continue;
      }
      let rhs = &raw[rhs_start..r];
      // Compare idents (bare names; same qualifier if both qualified).
      let a_bare = lhs.rsplit('.').next().unwrap_or(lhs);
      let b_bare = rhs.rsplit('.').next().unwrap_or(rhs);
      if !a_bare.eq_ignore_ascii_case(b_bare) {
        i = op_end;
        continue;
      }
      let a_q = if lhs.contains('.') { lhs.rsplit_once('.').map(|x| x.0) } else { None };
      let b_q = if rhs.contains('.') { rhs.rsplit_once('.').map(|x| x.0) } else { None };
      if let (Some(qa), Some(qb)) = (a_q, b_q)
        && !qa.eq_ignore_ascii_case(qb)
      {
        i = op_end;
        continue;
      }
      if emitted.insert(op_start) {
        let abs_s = start + l;
        let abs_e = start + r;
        let negated = op_text.starts_with("NOT ");
        let msg = if negated {
          format!(
            "`{lhs} {op_text} {lhs}` -- a column compared against itself with `SIMILAR TO` is always FALSE for non-NULL rows; almost always a copy-paste typo for two distinct columns."
          )
        } else {
          format!(
            "`{lhs} {op_text} {lhs}` -- a column compared against itself with `SIMILAR TO` is always TRUE for non-NULL rows; the predicate has no filter effect. Almost always a copy-paste typo for two distinct columns."
          )
        };
        out.push(Diagnostic {
          code: "sql510",
          severity: Severity::Warning,
          message: msg,
          range: TextRange::new((abs_s as u32).into(), (abs_e as u32).into()),
        });
      }
      i = op_end;
    }
  }
}
