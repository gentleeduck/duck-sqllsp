//! sql581: a `json` column type (or `::json` cast). `jsonb` is almost always
//! the better choice: it's stored decomposed (so it supports GIN indexes and
//! the containment / path operators), and it dedups keys. Plain `json` only
//! preserves the exact input text and whitespace -- rarely what you want for a
//! stored value. Word-bounded matching skips `jsonb`, `json_*` functions, and
//! `to_json`.

use crate::clause_scan::is_word;
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql581"
  }
  fn default_severity(&self) -> Severity {
    Severity::Hint
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, _body, upper) = crate::stmt_body_upper(stmt, source);
    let ub = upper.as_bytes();
    let n = ub.len();
    let mut i = 0usize;
    while i < n {
      match ub[i] {
        b'\'' => {
          i += 1;
          while i < n && ub[i] != b'\'' {
            i += 1;
          }
        },
        b'J' if i + 4 <= n && &ub[i..i + 4] == b"JSON" => {
          // Word-bounded JSON: excludes JSONB (trailing B), JSON_* and
          // TO_JSON (the `_` / preceding word char fail the boundary), and a
          // `json(` function-style call.
          let prev_ok = i == 0 || !is_word(ub[i - 1] as char);
          let next_ok = ub.get(i + 4).is_none_or(|&b| !is_word(b as char) && b != b'(');
          if prev_ok && next_ok {
            out.push(Diagnostic {
              code: "sql581",
              severity: Severity::Hint,
              message: "prefer `jsonb` over `json` -- jsonb is binary, indexable (GIN), and dedups keys".into(),
              range: crate::range_at(start + i, start + i + 4),
            });
          }
          i += 4;
          continue;
        },
        _ => {},
      }
      i += 1;
    }
  }
}
