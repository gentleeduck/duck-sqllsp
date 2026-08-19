//! sql793: an unconditional `WHEN MATCHED THEN` clause appears before
//! another `WHEN MATCHED [AND ...] THEN` clause in the same MERGE --
//! the unconditional branch always wins first, so the later WHEN
//! MATCHED clause can never run.

use crate::clause_scan::is_word;
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql793"
  }
  fn default_severity(&self) -> Severity {
    Severity::Warning
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, _body, upper) = crate::stmt_body_upper(stmt, source);
    if !upper.trim_start().starts_with("MERGE") {
      return;
    }
    let ub = upper.as_bytes();
    let mut clauses: Vec<(usize, bool)> = Vec::new();
    let mut i = 0usize;
    while i < ub.len() {
      if !word_at(ub, i, b"WHEN") {
        i += 1;
        continue;
      }
      let p = skip_ws(ub, i + 4);
      if !word_at(ub, p, b"MATCHED") {
        // Either "WHEN NOT MATCHED" or some unrelated WHEN -- skip.
        i += 4;
        continue;
      }
      let after_matched = skip_ws(ub, p + 7);
      let is_unconditional = !word_at(ub, after_matched, b"AND");
      clauses.push((i, is_unconditional));
      i = after_matched;
    }
    for (j, &(_, unconditional)) in clauses.iter().enumerate() {
      if unconditional {
        if let Some(&(pos, _)) = clauses.get(j + 1) {
          out.push(Diagnostic {
            code: "sql793",
            severity: Severity::Warning,
            message: "unreachable WHEN MATCHED clause -- an earlier unconditional WHEN MATCHED always matches first"
              .into(),
            range: crate::range_at(start + pos, start + pos + 4),
          });
        }
        break;
      }
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
