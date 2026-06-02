//! sql210: `REINDEX [CONCURRENTLY] (TABLE|INDEX) pg_<x>` -- system
//! catalog reindex. PG rejects CONCURRENTLY on system catalogs (only
//! superuser can do plain REINDEX SYSTEM). Catches accidental targets
//! against pg_catalog / pg_toast / information_schema.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql210"
  }
  fn default_severity(&self) -> Severity {
    Severity::Error
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, raw) = crate::stmt_body(stmt, source);
    let body_owned = crate::textutil::strip_noise_full(raw);
    let body = body_owned.as_str();
    let upper = body.to_ascii_uppercase();
    if !upper.trim_start().starts_with("REINDEX") {
      return;
    }
    let concurrently = upper.contains("CONCURRENTLY");
    let after_kw_keywords = ["SYSTEM ", "INDEX ", "TABLE ", "SCHEMA ", "DATABASE "];
    let mut target = String::new();
    for kw in after_kw_keywords {
      if let Some(p) = upper.find(kw) {
        let after = p + kw.len();
        let rest = &body[after..];
        let lead = rest.len() - rest.trim_start().len();
        let raw = &rest[lead..];
        let tok_end =
          raw.find(|c: char| !c.is_ascii_alphanumeric() && c != '_' && c != '.' && c != '"').unwrap_or(raw.len());
        target = raw[..tok_end].to_string();
        break;
      }
    }
    let lc = target.to_ascii_lowercase();
    let is_system = lc.starts_with("pg_") || lc.starts_with("information_schema.") || lc == "information_schema";
    if !is_system {
      return;
    }
    if !concurrently && !upper.contains("SYSTEM ") {
      return;
    }
    let abs_s = start;
    let abs_e = start + body.find(';').unwrap_or(body.len());
    let msg = if concurrently {
      format!(
        "REINDEX CONCURRENTLY on `{target}` -- system catalogs cannot be reindexed concurrently; drop CONCURRENTLY"
      )
    } else {
      format!("REINDEX SYSTEM on `{target}` -- requires superuser; ensure operator permissions")
    };
    out.push(Diagnostic {
      code: "sql210",
      severity: Severity::Error,
      message: msg,
      range: crate::range_at(abs_s, abs_e),
    });
  }
}
