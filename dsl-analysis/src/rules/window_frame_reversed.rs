//! sql191: `ROWS BETWEEN <n> FOLLOWING AND <m> PRECEDING` or any
//! frame where the start bound is strictly later than the end bound.
//! PG raises 22023 "frame starting from following row cannot end
//! with current row" (or equivalent) at runtime. Cheap textual scan
//! over BETWEEN ... AND ... pairs.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql191"
  }
  fn default_severity(&self) -> Severity {
    Severity::Error
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, _body, upper) = crate::stmt_body_upper(stmt, source);
    let mut from = 0usize;
    while let Some(rel) = upper[from..].find(" BETWEEN ") {
      let bet_at = from + rel + " BETWEEN ".len();
      let Some(and_rel) = upper[bet_at..].find(" AND ") else { break };
      let and_at = bet_at + and_rel;
      let start_bound = upper[bet_at..and_at].trim();
      let after_and = and_at + " AND ".len();
      let stop = upper[after_and..].find([')', ';', ',']).unwrap_or(upper.len() - after_and);
      let end_bound = upper[after_and..after_and + stop].trim();
      from = after_and + stop;
      let Some((s_rank, _)) = bound_rank(start_bound) else { continue };
      let Some((e_rank, _)) = bound_rank(end_bound) else { continue };
      if s_rank <= e_rank {
        continue;
      }
      let abs_s = start + bet_at - " BETWEEN ".len();
      let abs_e = start + after_and + stop;
      out.push(Diagnostic {
        code: "sql191",
        severity: Severity::Error,
        message: format!(
          "Window frame `BETWEEN {start_bound} AND {end_bound}` is reversed -- start bound is after end bound, PG raises 22023"
        ),
        range: crate::range_at(abs_s, abs_e),
      });
    }
  }
}

/// Map an LSP-friendly frame bound description to a (rank, label) pair.
/// Lower rank = earlier in window. None when the bound doesn't look
/// like a frame bound (lets us coexist with normal SQL `BETWEEN x AND y`
/// predicate forms, which return None here).
fn bound_rank(s: &str) -> Option<(i64, &'static str)> {
  let t = s.trim();
  if t == "UNBOUNDED PRECEDING" {
    return Some((i64::MIN, "UNBOUNDED PRECEDING"));
  }
  if t == "CURRENT ROW" {
    return Some((0, "CURRENT ROW"));
  }
  if t == "UNBOUNDED FOLLOWING" {
    return Some((i64::MAX, "UNBOUNDED FOLLOWING"));
  }
  if let Some(n) = t.strip_suffix(" PRECEDING")
    && let Ok(v) = n.trim().parse::<i64>()
  {
    return Some((-v, "PRECEDING"));
  }
  if let Some(n) = t.strip_suffix(" FOLLOWING")
    && let Ok(v) = n.trim().parse::<i64>()
  {
    return Some((v, "FOLLOWING"));
  }
  None
}
