//! sql292: `LIMIT 0` returns zero rows. Sometimes used to fetch
//! the column metadata of a query without the rows, but more often
//! a leftover placeholder. Worth a Hint to confirm intent.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql292"
  }
  fn default_severity(&self) -> Severity {
    Severity::Hint
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, raw) = crate::stmt_body(stmt, source);
    let body_owned = crate::textutil::strip_noise_full(raw);
    let body = body_owned.as_str();
    let upper = body.to_ascii_uppercase();
    if let Some((s, e)) = find_limit_zero(body, &upper) {
      out.push(Diagnostic {
        code: "sql292",
        severity: Severity::Hint,
        message: "LIMIT 0 returns zero rows -- placeholder or metadata-only query? Drop the LIMIT or set a real bound".into(),
        range: crate::range_at(start + s, start + e),
      });
    }
    // SQL-standard form: `FETCH FIRST 0 ROWS ONLY` (and FETCH NEXT 0).
    if let Some((s, e)) = find_fetch_first_zero(body, &upper) {
      out.push(Diagnostic {
        code: "sql292",
        severity: Severity::Hint,
        message: "`FETCH FIRST 0 ROWS ONLY` returns zero rows -- placeholder or metadata-only query? Drop the clause or set a real bound".into(),
        range: crate::range_at(start + s, start + e),
      });
    }
  }
}

/// Find a `LIMIT 0` token; returns (start, end) of the `LIMIT 0` slice.
fn find_limit_zero(body: &str, upper: &str) -> Option<(usize, usize)> {
  let at = upper.find("LIMIT ")?;
  let after = at + "LIMIT ".len();
  let rest = body[after..].trim_start();
  let leading = body[after..].len() - rest.len();
  let num_end = rest.find(|c: char| !c.is_ascii_digit()).unwrap_or(rest.len());
  if num_end == 0 {
    return None;
  }
  let n: i64 = rest[..num_end].parse().ok()?;
  if n != 0 {
    return None;
  }
  Some((at, after + leading + num_end))
}

/// Find a `FETCH FIRST 0 ROWS` or `FETCH NEXT 0 ROWS` token; returns
/// (start, end). The optional ONLY / WITH TIES tail isn't included
/// in the underlined span -- caller's range covers the bound.
fn find_fetch_first_zero(body: &str, upper: &str) -> Option<(usize, usize)> {
  for kw in ["FETCH FIRST ", "FETCH NEXT "] {
    if let Some(at) = upper.find(kw) {
      let after = at + kw.len();
      let rest = body[after..].trim_start();
      let leading = body[after..].len() - rest.len();
      let num_end = rest.find(|c: char| !c.is_ascii_digit()).unwrap_or(rest.len());
      if num_end == 0 {
        continue;
      }
      let n: i64 = match rest[..num_end].parse() {
        Ok(v) => v,
        Err(_) => continue,
      };
      if n != 0 {
        continue;
      }
      // Optional ROW/ROWS follows.
      return Some((at, after + leading + num_end));
    }
  }
  None
}
