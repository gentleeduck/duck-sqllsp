//! sql344: `ORDER BY <col> USING <op>` where the column's type family
//! is one of the families that lacks a meaningful total order
//! (json/jsonb/bytea/uuid). PG accepts the syntax but the comparison
//! is lexicographic on the wire representation -- almost never the
//! intent.

use crate::typing::{TypeFamily, column_family};
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql344"
  }
  fn default_severity(&self) -> Severity {
    Severity::Hint
  }

  fn check(&self, source: &str, stmt: &Statement, scope: &Scope, catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, body, upper) = crate::stmt_body_upper(stmt, source);
    let Some(ob_at) = upper.find("ORDER BY ") else { return };
    let after = ob_at + 9;
    let rest = &body[after..];
    let stop = rest.find([';', ')']).unwrap_or(rest.len());
    let clause = &rest[..stop];
    let clause_upper = clause.to_ascii_uppercase();
    let Some(using_at) = clause_upper.find(" USING ") else { return };
    let col_raw = clause[..using_at].trim().trim_end_matches(',').trim();
    let col_first = col_raw.split_whitespace().next().unwrap_or("");
    if col_first.is_empty() {
      return;
    }
    let (qual, col) = split_dotted(col_first);
    let Some(fam) = column_family(scope, catalog, qual.as_deref(), &col) else { return };
    let problematic = matches!(fam, TypeFamily::Json | TypeFamily::Bytea | TypeFamily::Uuid | TypeFamily::Array);
    if !problematic {
      return;
    }
    let abs_s = start + after;
    let abs_e = start + after + using_at + 7;
    out.push(Diagnostic {
      code: "sql344",
      severity: Severity::Hint,
      message: format!(
        "ORDER BY ... USING on `{}` (family `{}`) -- lexicographic byte order is rarely the intended ordering",
        col,
        fam.name()
      ),
      range: crate::range_at(abs_s, abs_e),
    });
  }
}

fn split_dotted(s: &str) -> (Option<String>, String) {
  let s = s.trim().trim_matches('"');
  if let Some(dot) = s.find('.') {
    let q = s[..dot].trim_matches('"').to_string();
    let c = s[dot + 1..].trim_matches('"').to_string();
    (Some(q), c)
  } else {
    (None, s.to_string())
  }
}
