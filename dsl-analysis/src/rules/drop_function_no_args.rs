//! sql260: `DROP FUNCTION foo` without an argument signature.
//! On PG14+ this works when there's only one overload, but it
//! fails if any second overload exists -- the drop becomes ambiguous.
//! Hint: always pass the arg list to make the migration deterministic.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql260"
  }
  fn default_severity(&self) -> Severity {
    Severity::Hint
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, raw) = crate::stmt_body(stmt, source);
    let body_owned = crate::textutil::strip_noise_full(raw);
    let body = body_owned.as_str();
    let upper = body.to_ascii_uppercase();
    let trim = upper.trim_start();
    if !trim.starts_with("DROP FUNCTION") && !trim.starts_with("DROP PROCEDURE") {
      return;
    }
    // Find the name after DROP FUNCTION / DROP PROCEDURE [IF EXISTS] ...
    let needle = if trim.starts_with("DROP FUNCTION") { "DROP FUNCTION" } else { "DROP PROCEDURE" };
    let after = upper.find(needle).unwrap() + needle.len();
    let rest = body[after..].trim_start();
    let rest_upper = rest.to_ascii_uppercase();
    let head_skip = if rest_upper.starts_with("IF EXISTS") { "IF EXISTS".len() } else { 0 };
    let after_head = &rest[head_skip..].trim_start();
    let id_end = after_head
      .find(|c: char| !c.is_ascii_alphanumeric() && c != '_' && c != '.' && c != '"')
      .unwrap_or(after_head.len());
    let name = after_head[..id_end].to_string();
    if name.is_empty() {
      return;
    }
    // After the name, look for an open paren.
    let post = after_head[id_end..].trim_start();
    if post.starts_with('(') {
      return;
    }
    let lead = body.len() - body.trim_start().len();
    let abs_s = start + lead;
    let abs_e = start + body.find(';').unwrap_or(body.len());
    out.push(Diagnostic {
      code: "sql260",
      severity: Severity::Hint,
      message: format!(
        "DROP FUNCTION `{name}` without argument signature -- fails when multiple overloads exist; specify `({{arg types}})`"
      ),
      range: crate::range_at(abs_s, abs_e),
    });
  }
}
