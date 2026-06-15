//! sql605: an inline foreign-key column declared `NOT NULL` but with an
//! `ON DELETE SET NULL` / `ON UPDATE SET NULL` referential action. When the
//! referenced row changes, PostgreSQL tries to write NULL into the column and
//! the NOT NULL constraint rejects it at runtime -- the cascade can never
//! succeed. Drop the NOT NULL, or use `SET DEFAULT` / `RESTRICT` / `CASCADE`.

use crate::clause_scan::split_top_level;
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql605"
  }
  fn default_severity(&self) -> Severity {
    Severity::Error
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, _body, upper) = crate::stmt_body_upper(stmt, source);
    let ub = upper.as_bytes();
    // locate the outermost parenthesised column list
    let Some(open) = ub.iter().position(|&b| b == b'(') else {
      return;
    };
    let mut depth = 0i32;
    let mut close = None;
    for (k, &b) in ub.iter().enumerate().skip(open) {
      match b {
        b'(' => depth += 1,
        b')' => {
          depth -= 1;
          if depth == 0 {
            close = Some(k);
            break;
          }
        }
        _ => {}
      }
    }
    let Some(close) = close else { return };
    let inner = &upper[open + 1..close];
    for (seg, off) in split_top_level(inner) {
      if let Some(rel) = seg.find("SET NULL")
        && seg.contains("NOT NULL")
      {
        let at = open + 1 + off + rel;
        out.push(Diagnostic {
          code: "sql605",
          severity: Severity::Error,
          message: "`SET NULL` referential action on a NOT NULL column -- the cascade will fail at runtime; drop NOT NULL or use SET DEFAULT / CASCADE".into(),
          range: crate::range_at(start + at, start + at + 8),
        });
      }
    }
  }
}
