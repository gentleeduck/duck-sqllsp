//! sql142: `CREATE [OR REPLACE] FUNCTION ... IMMUTABLE` whose body
//! issues DDL (CREATE, ALTER, DROP, TRUNCATE) -- IMMUTABLE promises
//! deterministic output for any given input and is *not* allowed to
//! mutate the database. PG plan caches IMMUTABLE results.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql142"
  }
  fn default_severity(&self) -> Severity {
    Severity::Warning
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, body, upper) = crate::stmt_body_upper(stmt, source);
    if !upper.contains("CREATE") || !upper.contains("FUNCTION") {
      return;
    }
    if !upper.contains("IMMUTABLE") {
      return;
    }
    // Only inspect the body between $$ and matching $$.
    let Some(open) = body.find("$$") else { return };
    let Some(close_rel) = body[open + 2..].find("$$") else { return };
    let body_text = &body[open + 2..open + 2 + close_rel];
    let body_up = body_text.to_ascii_uppercase();
    // Look for any DDL token at the start of a statement-ish chunk.
    for kw in ["CREATE ", "ALTER ", "DROP ", "TRUNCATE ", "GRANT ", "REVOKE "] {
      if let Some(rel) = crate::textutil::find_word(&body_up, kw.trim()) {
        let abs_start = start + open + 2 + rel;
        let abs_end = abs_start + kw.trim().len();
        out.push(Diagnostic {
          code: "sql142",
          severity: Severity::Warning,
          message: format!(
            "IMMUTABLE function body issues DDL (`{}`) -- IMMUTABLE promises pure determinism; PG plan caches results",
            kw.trim()
          ),
          range: crate::range_at(abs_start, abs_end),
        });
        return;
      }
    }
  }
}
