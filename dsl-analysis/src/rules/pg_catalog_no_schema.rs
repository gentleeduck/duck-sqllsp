//! sql245: `FROM pg_class` (bare) instead of `FROM pg_catalog.pg_class`.
//! search_path resolution lets attackers shadow pg_class with a
//! user-schema table; explicit `pg_catalog.` prefix is the safe
//! pattern (CVE-2018-1058). Same applies to common pg_catalog
//! relations.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

const SYS_REL: &[&str] = &[
  "pg_class",
  "pg_attribute",
  "pg_index",
  "pg_proc",
  "pg_type",
  "pg_namespace",
  "pg_constraint",
  "pg_database",
  "pg_roles",
  "pg_stat_activity",
  "pg_stat_user_tables",
  "pg_stat_user_indexes",
  "pg_stat_replication",
  "pg_settings",
];

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql245"
  }
  fn default_severity(&self) -> Severity {
    Severity::Hint
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, body) = crate::stmt_body(stmt, source);
    let lower = body.to_ascii_lowercase();
    let bytes = lower.as_bytes();
    for rel in SYS_REL {
      let mut from = 0usize;
      while let Some(found) = lower[from..].find(rel) {
        let at = from + found;
        // word boundary
        let prev_ok = at == 0
          || !{
            let p = bytes[at - 1] as char;
            p.is_ascii_alphanumeric() || p == '_' || p == '.'
          };
        let after = at + rel.len();
        let after_ok = after >= bytes.len()
          || !{
            let p = bytes[after] as char;
            p.is_ascii_alphanumeric() || p == '_'
          };
        if !prev_ok || !after_ok {
          from = at + rel.len();
          continue;
        }
        // Already pg_catalog. prefix?
        if at >= 11 && &lower[at - 11..at] == "pg_catalog." {
          from = at + rel.len();
          continue;
        }
        let abs_s = start + at;
        let abs_e = abs_s + rel.len();
        out.push(Diagnostic {
          code: "sql245",
          severity: Severity::Hint,
          message: format!(
            "`{rel}` referenced without `pg_catalog.` prefix -- search_path can shadow system catalogs (CVE-2018-1058)"
          ),
          range: crate::range_at(abs_s, abs_e),
        });
        from = after;
      }
    }
  }
}
