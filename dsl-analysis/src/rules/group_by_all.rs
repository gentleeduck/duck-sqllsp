//! sql090: PG 17 added `GROUP BY ALL` shorthand. Flag it as a Hint so
//! callers know about the portability cost (works only on PG 17+).

use crate::{Diagnostic, LintRule, Severity};
use crate::textutil::is_word;
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql090"
  }
  fn default_severity(&self) -> Severity {
    Severity::Hint
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, _body, upper) = crate::stmt_body_upper(stmt, source);
    let bytes = upper.as_bytes();
    let n = bytes.len();
    let mut i = 0;
    while i + 8 <= n {
      if &upper[i..i + 8] == "GROUP BY" && (i == 0 || !is_word(bytes[i - 1] as char)) {
        let mut j = i + 8;
        while j < n && bytes[j].is_ascii_whitespace() {
          j += 1;
        }
        if j + 3 <= n && &upper[j..j + 3] == "ALL" {
          let next_ok = j + 3 == n || !is_word(bytes[j + 3] as char);
          if next_ok {
            let abs_start = start + i;
            let abs_end = start + j + 3;
            out.push(Diagnostic {
              code: "sql090",
              severity: Severity::Hint,
              message: "GROUP BY ALL requires PostgreSQL 17+ -- consider listing columns explicitly for portability"
                .into(),
              range: crate::range_at(abs_start, abs_end),
            });
            return;
          }
        }
      }
      i += 1;
    }
  }
}

