//! sql072: `SELECT ... FOR UPDATE` without a WHERE clause locks every
//! row of the target table -- almost always a footgun.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::{Statement, StatementKind};
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql072"
  }
  fn default_severity(&self) -> Severity {
    Severity::Warning
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    if !matches!(stmt.kind, StatementKind::Select(_)) {
      return;
    }
    let start: usize = u32::from(stmt.range.start()) as usize;
    let end: usize = (u32::from(stmt.range.end()) as usize).min(source.len());
    let raw = &source[start..end];
    let body_owned = crate::textutil::strip_noise_full(raw);
    let body = body_owned.as_str();
    let upper = body.to_ascii_uppercase();
    let (clause, idx) = if let Some(p) = upper.find("FOR UPDATE") {
      ("FOR UPDATE", p)
    } else if let Some(p) = upper.find("FOR SHARE") {
      ("FOR SHARE", p)
    } else {
      return;
    };
    if contains_word(&upper, "WHERE") {
      return;
    }
    let abs_start = start + idx;
    let abs_end = abs_start + clause.len();
    out.push(Diagnostic {
      code: "sql072",
      severity: Severity::Warning,
      message: "SELECT FOR UPDATE / FOR SHARE without WHERE -- locks every row in the table".into(),
      range: text_size::TextRange::new((abs_start as u32).into(), (abs_end as u32).into()),
    });
  }
}

fn contains_word(haystack: &str, needle: &str) -> bool {
  let bytes = haystack.as_bytes();
  let n_bytes = needle.as_bytes();
  let mut i = 0;
  while i + n_bytes.len() <= bytes.len() {
    if &bytes[i..i + n_bytes.len()] == n_bytes {
      let prev_ok = i == 0 || !is_word(bytes[i - 1] as char);
      let next_ok = i + n_bytes.len() == bytes.len() || !is_word(bytes[i + n_bytes.len()] as char);
      if prev_ok && next_ok {
        return true;
      }
    }
    i += 1;
  }
  false
}
fn is_word(c: char) -> bool {
  c.is_alphanumeric() || c == '_'
}
