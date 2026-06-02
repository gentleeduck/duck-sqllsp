//! sql511: `WHERE col @> col` / `col <@ col` / `col && col` --
//! containment / overlap of a column with itself is always TRUE
//! for non-NULL rows (every value contains and is contained by
//! itself; every non-empty array overlaps itself). Almost always a
//! copy-paste typo for two distinct operands. Companion to sql508
//! and sql510 for the array / jsonb operator family.

use crate::clause_scan::{find_clause, find_clause_end};
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;
use text_size::TextRange;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql511"
  }
  fn default_severity(&self) -> Severity {
    Severity::Warning
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, raw) = crate::stmt_body(stmt, source);
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
    while i + 2 <= pred_end {
      let op_kind: Option<(&str, usize)> = if i + 2 <= pred_end && &bytes[i..i + 2] == b"@>" {
        Some(("@>", 2))
      } else if i + 2 <= pred_end && &bytes[i..i + 2] == b"<@" {
        Some(("<@", 2))
      } else if i + 2 <= pred_end && &bytes[i..i + 2] == b"&&" {
        Some(("&&", 2))
      } else {
        None
      };
      let Some((op_text, op_len)) = op_kind else {
        i += 1;
        continue;
      };
      let op_start = i;
      let op_end = i + op_len;
      // LHS: walk back over ws, then read ident.
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
        let what = match op_text {
          "@>" => "contains",
          "<@" => "is contained by",
          "&&" => "overlaps",
          _ => unreachable!(),
        };
        out.push(Diagnostic {
          code: "sql511",
          severity: Severity::Warning,
          message: format!(
            "`{lhs} {op_text} {lhs}` -- a value {what} itself trivially (always TRUE for non-NULL / non-empty rows); the predicate has no filter effect. Almost always a copy-paste typo for two distinct operands."
          ),
          range: TextRange::new((abs_s as u32).into(), (abs_e as u32).into()),
        });
      }
      i = op_end;
    }
  }
}
