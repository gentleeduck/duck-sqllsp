//! sql614: a MySQL-style inline `KEY ...` / `INDEX ...` definition inside
//! `CREATE TABLE`. PostgreSQL doesn't allow secondary indexes in the table
//! body -- only PRIMARY KEY / UNIQUE / FOREIGN KEY constraints -- and rejects
//! the statement. Create the index separately with `CREATE INDEX ... ON t (...)`
//! after the table.

use crate::clause_scan::{is_word, split_top_level};
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

/// Does `seg` (a trimmed column-list element, uppercased) begin with the bare
/// keyword `kw` followed by a non-word char?
fn starts_with_kw(seg: &str, kw: &str) -> bool {
  let sb = seg.as_bytes();
  let kb = kw.as_bytes();
  sb.len() > kb.len() && &sb[..kb.len()] == kb && !is_word(sb[kb.len()] as char)
}

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql614"
  }
  fn default_severity(&self) -> Severity {
    Severity::Error
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, _body, upper) = crate::stmt_body_upper(stmt, source);
    if !upper.trim_start().starts_with("CREATE") || !upper.contains("TABLE") {
      return;
    }
    let ub = upper.as_bytes();
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
      let t = seg.trim_start();
      let lead = seg.len() - t.len();
      // `KEY ...` / `INDEX ...` as a top-level element is a MySQL inline index.
      // `PRIMARY KEY` / `FOREIGN KEY` / `UNIQUE` elements start with a different
      // keyword and are untouched.
      if starts_with_kw(t, "KEY") || starts_with_kw(t, "INDEX") {
        let at = open + 1 + off + lead;
        let kwlen = if starts_with_kw(t, "KEY") { 3 } else { 5 };
        out.push(Diagnostic {
          code: "sql614",
          severity: Severity::Error,
          message: "inline KEY/INDEX in CREATE TABLE is MySQL syntax -- PostgreSQL has no table-body indexes; use a separate `CREATE INDEX`".into(),
          range: crate::range_at(start + at, start + at + kwlen),
        });
      }
    }
  }
}
