//! sql298: CREATE TABLE / FUNCTION / TYPE / INDEX / TRIGGER /
//! CONSTRAINT name longer than 63 bytes. PG silently truncates to
//! NAMEDATALEN-1 (default 63) and emits a NOTICE, so distinct
//! "long_name_abc" / "long_name_xyz" can collide.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

const LIMIT: usize = 63;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql298"
  }
  fn default_severity(&self) -> Severity {
    Severity::Warning
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, raw) = crate::stmt_body(stmt, source);
    let body_owned = crate::textutil::strip_noise_full(raw);
    let body = body_owned.as_str();
    let upper = body.to_ascii_uppercase();
    for kw in [
      "CREATE TABLE ",
      "CREATE TABLE IF NOT EXISTS ",
      "CREATE INDEX ",
      "CREATE UNIQUE INDEX ",
      "CREATE FUNCTION ",
      "CREATE OR REPLACE FUNCTION ",
      "CREATE TYPE ",
      "CREATE DOMAIN ",
      "CREATE SEQUENCE ",
      "CREATE TRIGGER ",
      "CREATE OR REPLACE TRIGGER ",
      "CREATE VIEW ",
      "CREATE OR REPLACE VIEW ",
      "CREATE MATERIALIZED VIEW ",
      "CREATE OR REPLACE MATERIALIZED VIEW ",
      "CONSTRAINT ",
    ] {
      let Some(at) = upper.find(kw) else { continue };
      let after = at + kw.len();
      let rest = &body[after..];
      let lead = rest.len() - rest.trim_start().len();
      let raw = &rest[lead..];
      // Skip CONCURRENTLY / IF NOT EXISTS / leading modifiers for INDEX.
      let id_end = raw.find(|c: char| !c.is_ascii_alphanumeric() && c != '_' && c != '"').unwrap_or(raw.len());
      let name = raw[..id_end].trim_matches('"');
      if name.len() <= LIMIT {
        continue;
      }
      let abs_s = start + after + lead;
      let abs_e = abs_s + id_end;
      out.push(Diagnostic {
        code: "sql298",
        severity: Severity::Warning,
        message: format!(
          "Identifier `{name}` is {} bytes -- PG silently truncates to {LIMIT} (NAMEDATALEN-1); risk of collision",
          name.len(),
        ),
        range: crate::range_at(abs_s, abs_e),
      });
      return;
    }
  }
}
