//! sql188: `COMMENT ON TABLE bogus IS '...'` where bogus isn't a
//! known catalog table. PG raises 42P01 at runtime. Also catches
//! COMMENT ON COLUMN bogus.col / FUNCTION bogus / TYPE bogus.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql188"
  }
  fn default_severity(&self) -> Severity {
    Severity::Error
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, raw) = crate::stmt_body(stmt, source);
    let body_owned = crate::textutil::strip_noise_full(raw);
    let body = body_owned.as_str();
    let upper = body.to_ascii_uppercase();
    for (needle, kind) in [
      ("COMMENT ON TABLE ", "table"),
      ("COMMENT ON COLUMN ", "column"),
      ("COMMENT ON FUNCTION ", "function"),
      ("COMMENT ON TYPE ", "type"),
      ("COMMENT ON DOMAIN ", "domain"),
      ("COMMENT ON SEQUENCE ", "sequence"),
      ("COMMENT ON VIEW ", "view"),
      ("COMMENT ON MATERIALIZED VIEW ", "matview"),
      ("COMMENT ON INDEX ", "index"),
      ("COMMENT ON TRIGGER ", "trigger"),
    ] {
      let Some(rel) = upper.find(needle) else { continue };
      let after = rel + needle.len();
      let bytes = body.as_bytes();
      let mut k = after;
      while k < bytes.len() && bytes[k].is_ascii_whitespace() {
        k += 1;
      }
      let id_start = k;
      while k < bytes.len()
        && (bytes[k].is_ascii_alphanumeric() || bytes[k] == b'_' || bytes[k] == b'.' || bytes[k] == b'"')
      {
        k += 1;
      }
      let id = &body[id_start..k];
      if id.is_empty() {
        return;
      }
      let parts: Vec<&str> = id.split('.').map(|s| s.trim_matches('"')).collect();
      let target_name = parts.last().unwrap_or(&"");
      let exists = match kind {
        "table" | "view" | "matview" => catalog.find_table(None, target_name).is_some(),
        "column" => {
          if parts.len() < 2 {
            false
          } else {
            let tbl = parts[parts.len() - 2];
            let col = parts[parts.len() - 1];
            catalog
              .find_table(None, tbl)
              .map(|t| t.columns.iter().any(|c| c.name.eq_ignore_ascii_case(col)))
              .unwrap_or(false)
          }
        },
        "function" => {
          catalog.functions.iter().any(|f| f.name.eq_ignore_ascii_case(target_name))
            || dsl_knowledge::tables::functions().contains_key(target_name.to_ascii_lowercase().as_str())
        },
        "type" | "domain" => catalog.types().any(|t| t.name.eq_ignore_ascii_case(target_name)),
        "sequence" => catalog.sequences().any(|s| s.name.eq_ignore_ascii_case(target_name)),
        _ => true, // index / trigger less commonly cataloged; skip.
      };
      if exists {
        return;
      }
      let abs_s = start + id_start;
      let abs_e = start + k;
      out.push(Diagnostic {
        code: "sql188",
        severity: Severity::Error,
        message: format!("COMMENT ON {kind}: `{id}` not found in catalog -- PG raises 42P01 at exec"),
        range: crate::range_at(abs_s, abs_e),
      });
      return;
    }
  }
}
