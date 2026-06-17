//! sql662: `SELECT DISTINCT ON <expr>` without parentheses around the
//! expression list. The syntax is `DISTINCT ON (expr [, ...])`; the parentheses
//! are required, so `DISTINCT ON col` is a syntax error (42601). Wrap the
//! expression(s): `DISTINCT ON (col)`.

use crate::clause_scan::is_word;
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

fn kw(b: &[u8], i: usize, w: &[u8]) -> bool {
  i + w.len() <= b.len()
    && &b[i..i + w.len()] == w
    && (i == 0 || !is_word(b[i - 1] as char))
    && b.get(i + w.len()).is_none_or(|&c| !is_word(c as char))
}

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql662"
  }
  fn default_severity(&self) -> Severity {
    Severity::Error
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, raw) = crate::stmt_body(stmt, source);
    let cleaned = crate::textutil::strip_noise_full(raw);
    let upper = cleaned.to_ascii_uppercase();
    let b = upper.as_bytes();
    let n = b.len();
    let mut i = 0usize;
    while i + 8 <= n {
      if kw(b, i, b"DISTINCT") {
        let mut j = i + 8;
        while j < n && b[j].is_ascii_whitespace() {
          j += 1;
        }
        if kw(b, j, b"ON") {
          let mut k = j + 2;
          while k < n && b[k].is_ascii_whitespace() {
            k += 1;
          }
          if k >= n || b[k] != b'(' {
            out.push(Diagnostic {
              code: "sql662",
              severity: Severity::Error,
              message: "DISTINCT ON requires parentheses around the expression list -- write `DISTINCT ON (expr)` (PG 42601)".into(),
              range: crate::range_at(start + j, start + j + 2),
            });
            return;
          }
          i = k;
          continue;
        }
      }
      i += 1;
    }
  }
}
