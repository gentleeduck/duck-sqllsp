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
    let start: usize = u32::from(stmt.range.start()) as usize;
    let end: usize = (u32::from(stmt.range.end()) as usize).min(source.len());
    let raw = &source[start..end];
    let body_owned = crate::textutil::strip_noise_full(raw);
    let body = body_owned.as_str();
    let upper = body.to_ascii_uppercase();
    let Some(at) = upper.find("LIMIT ") else { return };
    let after = at + "LIMIT ".len();
    let rest = body[after..].trim_start();
    let num_end = rest.find(|c: char| !c.is_ascii_digit()).unwrap_or(rest.len());
    if num_end == 0 { return }
    let n: i64 = match rest[..num_end].parse() { Ok(v) => v, Err(_) => return };
    if n != 0 { return }
    let abs_s = start + at;
    let abs_e = start + after + (body[after..].len() - rest.len()) + num_end;
    out.push(Diagnostic {
      code: "sql292",
      severity: Severity::Hint,
      message: "LIMIT 0 returns zero rows -- placeholder or metadata-only query? Drop the LIMIT or set a real bound".into(),
      range: text_size::TextRange::new((abs_s as u32).into(), (abs_e as u32).into()),
    });
  }
}
