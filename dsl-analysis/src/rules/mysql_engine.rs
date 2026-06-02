//! sql315: `ENGINE=InnoDB` / `ENGINE=MyISAM` / similar -- MySQL
//! storage-engine attribute. PG rejects with 42601.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql315"
  }
  fn default_severity(&self) -> Severity {
    Severity::Error
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, raw) = crate::stmt_body(stmt, source);
    let body_owned = crate::textutil::strip_noise_full(raw);
    let body = body_owned.as_str();
    let upper = body.to_ascii_uppercase();
    let Some(at) = upper.find("ENGINE=") else {
      let Some(at2) = upper.find("ENGINE =") else { return };
      let abs_s = start + at2;
      let abs_e = abs_s + "ENGINE =".len();
      out.push(Diagnostic {
        code: "sql315",
        severity: Severity::Error,
        message: "`ENGINE = ...` is MySQL syntax -- PG has no storage-engine clause; remove this".into(),
        range: crate::range_at(abs_s, abs_e),
      });
      return;
    };
    if at > 0 {
      let prev = body.as_bytes()[at - 1] as char;
      if prev.is_ascii_alphanumeric() || prev == '_' {
        return;
      }
    }
    let abs_s = start + at;
    let abs_e = abs_s + "ENGINE=".len();
    out.push(Diagnostic {
      code: "sql315",
      severity: Severity::Error,
      message: "`ENGINE=...` is MySQL syntax -- PG has no storage-engine clause; remove this".into(),
      range: crate::range_at(abs_s, abs_e),
    });
  }
}
