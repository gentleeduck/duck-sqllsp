//! sql277: `COMMENT ON FUNCTION foo IS '...'` without argument
//! signature. Same hazard as DROP FUNCTION -- fails when multiple
//! overloads exist (PG 42725). Hint: always pass the arg-type list.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql277"
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
    let needle = "COMMENT ON FUNCTION ";
    let Some(at) = upper.find(needle) else { return };
    let after = at + needle.len();
    let rest = body[after..].trim_start();
    let id_end = rest.find(|c: char| !c.is_ascii_alphanumeric() && c != '_' && c != '.' && c != '"').unwrap_or(rest.len());
    let name = rest[..id_end].to_string();
    if name.is_empty() { return }
    let post = rest[id_end..].trim_start();
    if post.starts_with('(') { return }
    let lead = body.len() - body.trim_start().len();
    let abs_s = start + lead;
    let abs_e = start + body.find(';').unwrap_or(body.len());
    out.push(Diagnostic {
      code: "sql277",
      severity: Severity::Hint,
      message: format!(
        "COMMENT ON FUNCTION `{name}` without argument signature -- fails when overloads exist; pass `({{arg types}})`"
      ),
      range: text_size::TextRange::new((abs_s as u32).into(), (abs_e as u32).into()),
    });
  }
}
