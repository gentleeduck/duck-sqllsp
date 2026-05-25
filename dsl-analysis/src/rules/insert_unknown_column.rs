//! sql349: `INSERT INTO t (col_list)` lists a column not in the
//! target table's catalog. Catches typos in INSERT statements.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::{Statement, StatementKind};
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql349"
  }
  fn default_severity(&self) -> Severity {
    Severity::Error
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let StatementKind::Insert(ins) = &stmt.kind else { return };
    let Some(t) = catalog.find_table(ins.table.schema.as_deref(), &ins.table.name) else { return };
    if ins.columns.is_empty() { return }
    let start: usize = u32::from(stmt.range.start()) as usize;
    let end: usize = (u32::from(stmt.range.end()) as usize).min(source.len());
    let body = &source[start..end];
    for col in &ins.columns {
      if t.columns.iter().any(|c| c.name.eq_ignore_ascii_case(col)) { continue }
      let Some(at) = find_word(body, col) else { continue };
      let abs_s = start + at;
      let abs_e = abs_s + col.len();
      out.push(Diagnostic {
        code: "sql349",
        severity: Severity::Error,
        message: format!("column `{col}` is not a column of `{}.{}`", ins.table.schema.as_deref().unwrap_or("public"), ins.table.name),
        range: text_size::TextRange::new((abs_s as u32).into(), (abs_e as u32).into()),
      });
    }
  }
}

fn find_word(haystack: &str, needle: &str) -> Option<usize> {
  let h = haystack.as_bytes();
  let n = needle.as_bytes();
  if n.is_empty() { return None }
  let mut i = 0usize;
  while i + n.len() <= h.len() {
    if h[i..i + n.len()].eq_ignore_ascii_case(n) {
      let prev_ok = i == 0 || !is_word(h[i - 1] as char);
      let next_ok = i + n.len() == h.len() || !is_word(h[i + n.len()] as char);
      if prev_ok && next_ok { return Some(i) }
    }
    i += 1;
  }
  None
}

fn is_word(c: char) -> bool { c.is_alphanumeric() || c == '_' }
