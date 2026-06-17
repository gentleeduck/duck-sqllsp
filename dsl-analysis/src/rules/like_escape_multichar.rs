//! sql752: `LIKE 'a%' ESCAPE '\\!'` -- the ESCAPE string must be empty or a
//! single character. PostgreSQL raises 22019 ("invalid escape string ... must
//! be empty or one character") at runtime for a longer literal. Usually a typo
//! or a misunderstanding of the clause.

use crate::clause_scan::is_word;
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql752"
  }
  fn default_severity(&self) -> Severity {
    Severity::Warning
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, body, upper) = crate::stmt_body_upper(stmt, source);
    let ub = upper.as_bytes();
    let n = ub.len();

    let mut i = 0usize;
    while i < n {
      if ub[i] == b'\'' {
        i += 1;
        while i < n && ub[i] != b'\'' {
          i += 1;
        }
        i += 1;
        continue;
      }
      if !word_at(ub, i, b"ESCAPE") {
        i += 1;
        continue;
      }
      let p = skip_ws(ub, i + 6);
      if ub.get(p) == Some(&b'\'') {
        let cs = p + 1;
        let mut j = cs;
        while j < n && ub[j] != b'\'' {
          j += 1;
        }
        if j < n && body[cs..j].chars().count() > 1 {
          out.push(Diagnostic {
            code: "sql752",
            severity: Severity::Warning,
            message: "LIKE ESCAPE string must be empty or a single character -- raises an error at runtime (PG 22019)".into(),
            range: crate::range_at(start + p, start + j + 1),
          });
        }
        i = j + 1;
        continue;
      }
      i += 6;
    }
  }
}

fn word_at(ub: &[u8], i: usize, w: &[u8]) -> bool {
  i + w.len() <= ub.len()
    && &ub[i..i + w.len()] == w
    && (i == 0 || !is_word(ub[i - 1] as char))
    && (i + w.len() == ub.len() || !is_word(ub[i + w.len()] as char))
}

fn skip_ws(ub: &[u8], mut i: usize) -> usize {
  while i < ub.len() && ub[i].is_ascii_whitespace() {
    i += 1;
  }
  i
}
