//! sql201: `CREATE FUNCTION ... SECURITY DEFINER ...` without an
//! explicit `SET search_path = ...` clause. Tracks CVE-2018-1058
//! escalation: a SECURITY DEFINER function inherits the caller's
//! search_path, letting a hostile schema shadow `public.fn(...)`.
//! PG docs recommend pinning search_path to `pg_catalog, pg_temp`.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql201"
  }
  fn default_severity(&self) -> Severity {
    Severity::Warning
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let start: usize = u32::from(stmt.range.start()) as usize;
    let end: usize = (u32::from(stmt.range.end()) as usize).min(source.len());
    let raw = &source[start..end];
    let body_owned = crate::textutil::strip_noise_full(raw);
    let body = body_owned.as_str();
    let upper = body.to_ascii_uppercase();
    if !upper.contains("CREATE") || !upper.contains("FUNCTION") {
      return;
    }
    if !upper.contains("SECURITY DEFINER") {
      return;
    }
    // If author already pinned search_path, accept either at function-level
    // SET search_path = ... or via `ALTER FUNCTION ... SET search_path` (which
    // can't appear inside CREATE so we only check the CREATE body here).
    let has_set = upper.contains("SET SEARCH_PATH");
    if has_set {
      return;
    }
    let Some(at) = upper.find("SECURITY DEFINER") else { return };
    let abs_s = start + at;
    let abs_e = abs_s + "SECURITY DEFINER".len();
    out.push(Diagnostic {
      code: "sql201",
      severity: Severity::Warning,
      message: "SECURITY DEFINER without `SET search_path = pg_catalog, pg_temp` -- vulnerable to search_path hijack (CVE-2018-1058)".into(),
      range: text_size::TextRange::new((abs_s as u32).into(), (abs_e as u32).into()),
    });
  }
}
