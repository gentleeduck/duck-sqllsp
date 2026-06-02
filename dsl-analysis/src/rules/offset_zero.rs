//! sql466: `... OFFSET 0` -- skipping zero rows is a no-op. Almost
//! always a leftover from a parameterized template (`OFFSET $offset`
//! where offset=0) or a placeholder. Drop the clause for clarity.
//! Note: PG occasionally uses `OFFSET 0` as an optimization fence
//! to prevent subquery unnesting (a deliberate trick); we still emit
//! a Hint since the common case is unintentional.
//!
//! Also covers SQL-standard `OFFSET 0 ROWS` / `OFFSET 0 ROW`.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;
use text_size::TextRange;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql466"
  }
  fn default_severity(&self) -> Severity {
    Severity::Hint
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, raw) = crate::stmt_body(stmt, source);
    let body_owned = crate::textutil::strip_noise_full(raw);
    let body = body_owned.as_str();
    let upper = body.to_ascii_uppercase();
    let Some(at) = upper.find("OFFSET ") else { return };
    let after = at + "OFFSET ".len();
    let rest = body[after..].trim_start();
    let leading = body[after..].len() - rest.len();
    let num_end = rest.find(|c: char| !c.is_ascii_digit()).unwrap_or(rest.len());
    if num_end == 0 {
      return;
    }
    let n: i64 = match rest[..num_end].parse() {
      Ok(v) => v,
      Err(_) => return,
    };
    if n != 0 {
      return;
    }
    let abs_s = start + at;
    let abs_e = start + after + leading + num_end;
    out.push(Diagnostic {
      code: "sql466",
      severity: Severity::Hint,
      message: "OFFSET 0 is a no-op -- skipping zero rows has no effect; drop the clause for clarity (note: PG sometimes uses OFFSET 0 as a deliberate planner fence -- if so, document it)".into(),
      range: TextRange::new((abs_s as u32).into(), (abs_e as u32).into()),
    });
  }
}
