//! sql350: `INSERT/UPDATE/DELETE ... RETURNING <list>` lists a column
//! not on the target table. Mirrors sql349 + sql002 coverage gaps.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::{Statement, StatementKind, TableRef};
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql350"
  }
  fn default_severity(&self) -> Severity {
    Severity::Error
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let table_ref: &TableRef = match &stmt.kind {
      StatementKind::Insert(ins) => &ins.table,
      StatementKind::Update(u) => &u.table,
      StatementKind::Delete(d) => &d.table,
      _ => return,
    };
    let Some(t) = catalog.find_table(table_ref.schema.as_deref(), &table_ref.name) else { return };
    let start: usize = u32::from(stmt.range.start()) as usize;
    let end: usize = (u32::from(stmt.range.end()) as usize).min(source.len());
    let body = &source[start..end];
    let upper = body.to_ascii_uppercase();
    let Some(ret_at) = upper.find("RETURNING ") else { return };
    let after = ret_at + 10;
    let rest = &body[after..];
    let stop = rest.find(';').unwrap_or(rest.len());
    let list = &rest[..stop];
    for raw in list.split(',') {
      let token = raw.trim().trim_matches('"');
      // Skip * and qualified or expression forms.
      if token == "*" || token.contains(' ') || token.contains('(') || token.contains('.') { continue }
      if token.is_empty() { continue }
      if t.columns.iter().any(|c| c.name.eq_ignore_ascii_case(token)) { continue }
      let local = list.find(token).unwrap_or(0);
      let abs_s = start + after + local;
      let abs_e = abs_s + token.len();
      out.push(Diagnostic {
        code: "sql350",
        severity: Severity::Error,
        message: format!("RETURNING references unknown column `{token}` on `{}`", table_ref.name),
        range: text_size::TextRange::new((abs_s as u32).into(), (abs_e as u32).into()),
      });
    }
  }
}
