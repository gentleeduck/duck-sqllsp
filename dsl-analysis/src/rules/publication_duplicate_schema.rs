//! sql795: `CREATE PUBLICATION ... FOR TABLES IN SCHEMA s, s` -- the
//! same schema listed twice. Replaces the spec's original sql795
//! (`FOR ALL TABLES, TABLE x`), verified unreachable -- `FOR ALL
//! TABLES` and `FOR TABLE` are mutually exclusive grammar productions;
//! pg_query rejects the comma-combination as a hard parse error.

use crate::clause_scan::{find_clause, find_clause_end, parse_simple_ident, split_top_level};
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql795"
  }
  fn default_severity(&self) -> Severity {
    Severity::Error
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, body, upper) = crate::stmt_body_upper(stmt, source);
    if !upper.trim_start().starts_with("CREATE PUBLICATION") {
      return;
    }
    let ub = upper.as_bytes();
    let Some(rel) = find_clause(ub, b"SCHEMA") else { return };
    // Only the `FOR TABLES IN SCHEMA` form -- require "IN" right
    // before "SCHEMA" to avoid matching an unrelated use of the word.
    if !upper[..rel].trim_end().to_ascii_uppercase().ends_with("IN") {
      return;
    }
    let list_start = skip_ws(ub, rel + "SCHEMA".len());
    let list_end = find_clause_end(ub, list_start, &["WITH"]);
    let mut seen: Vec<String> = Vec::new();
    for (entry, off) in split_top_level(&body[list_start..list_end]) {
      let Some((_, name)) = parse_simple_ident(entry) else { continue };
      if seen.iter().any(|s| s.eq_ignore_ascii_case(&name)) {
        let lead = entry.len() - entry.trim_start().len();
        let abs = list_start + off + lead;
        out.push(Diagnostic {
          code: "sql795",
          severity: Severity::Error,
          message: format!("schema `{name}` appears more than once in FOR TABLES IN SCHEMA"),
          range: crate::range_at(start + abs, start + abs + name.len()),
        });
      } else {
        seen.push(name);
      }
    }
  }
}

fn skip_ws(ub: &[u8], mut i: usize) -> usize {
  while i < ub.len() && ub[i].is_ascii_whitespace() {
    i += 1;
  }
  i
}
