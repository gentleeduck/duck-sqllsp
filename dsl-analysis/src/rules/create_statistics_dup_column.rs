//! sql792: `CREATE STATISTICS ... ON a, a FROM t` -- the same column
//! listed twice.

use crate::clause_scan::{find_clause, parse_simple_ident, split_top_level};
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql792"
  }
  fn default_severity(&self) -> Severity {
    Severity::Warning
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, body, upper) = crate::stmt_body_upper(stmt, source);
    if !upper.trim_start().starts_with("CREATE STATISTICS") {
      return;
    }
    let ub = upper.as_bytes();
    let Some(on_rel) = find_clause(ub, b"ON") else { return };
    let list_start = skip_ws(ub, on_rel + 2);
    let list_end = find_clause(&ub[list_start..], b"FROM").map(|r| list_start + r).unwrap_or(ub.len());
    let mut seen: Vec<String> = Vec::new();
    for (entry, off) in split_top_level(&body[list_start..list_end]) {
      let Some((_, name)) = parse_simple_ident(entry) else { continue };
      if seen.iter().any(|s| s.eq_ignore_ascii_case(&name)) {
        let lead = entry.len() - entry.trim_start().len();
        let abs = list_start + off + lead;
        out.push(Diagnostic {
          code: "sql792",
          severity: Severity::Warning,
          message: format!("column `{name}` appears more than once in CREATE STATISTICS"),
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
