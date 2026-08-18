//! sql797: `CREATE PUBLICATION ... FOR TABLE a, a` -- the same table
//! listed twice.

use crate::clause_scan::{find_clause, find_clause_end, split_top_level};
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql797"
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
    let Some(rel) = find_clause(ub, b"TABLE") else { return };
    // Exclude "TABLES IN SCHEMA" and "ALL TABLES" -- only the plain
    // `FOR TABLE` form (word boundary already excludes matching
    // "TABLE" inside "TABLES" via find_clause's whole-word check).
    let list_start = skip_ws(ub, rel + "TABLE".len());
    let list_end = find_clause_end(ub, list_start, &["WITH"]);
    let mut seen: Vec<String> = Vec::new();
    for (entry, off) in split_top_level(&body[list_start..list_end]) {
      let Some(name) = leading_table_ref(entry) else { continue };
      if seen.iter().any(|s| s.eq_ignore_ascii_case(&name)) {
        let lead = entry.len() - entry.trim_start().len();
        let abs = list_start + off + lead;
        out.push(Diagnostic {
          code: "sql797",
          severity: Severity::Error,
          message: format!("table `{name}` appears more than once in FOR TABLE"),
          range: crate::range_at(start + abs, start + abs + name.len()),
        });
      } else {
        seen.push(name);
      }
    }
  }
}

/// A (possibly schema-qualified) table reference at the start of a
/// `FOR TABLE` list entry, ignoring any trailing column list or WHERE
/// clause (`t (col1, col2)`, `t WHERE cond`).
fn leading_table_ref(s: &str) -> Option<String> {
  let t = s.trim_start();
  let end = t.find(|c: char| !(c.is_alphanumeric() || c == '_' || c == '.')).unwrap_or(t.len());
  if end == 0 {
    return None;
  }
  Some(t[..end].to_string())
}

fn skip_ws(ub: &[u8], mut i: usize) -> usize {
  while i < ub.len() && ub[i].is_ascii_whitespace() {
    i += 1;
  }
  i
}
