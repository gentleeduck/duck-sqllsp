//! sql617: `NATURAL JOIN` (and `NATURAL LEFT/RIGHT/FULL JOIN`). A natural join
//! implicitly joins on *every* pair of same-named columns, so adding, renaming,
//! or dropping a column on either side silently changes the join condition --
//! a frequent source of surprise breakage. Spell the join out with
//! `JOIN ... ON ...` or `JOIN ... USING (...)`.
//!
//! Complements sql064 (`JOIN` without `ON`), which deliberately skips NATURAL.

use crate::clause_scan::is_word;
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql617"
  }
  fn default_severity(&self) -> Severity {
    Severity::Warning
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, raw) = crate::stmt_body(stmt, source);
    let cleaned = crate::textutil::strip_noise_full(raw);
    let upper = cleaned.to_ascii_uppercase();
    let ub = upper.as_bytes();
    let n = ub.len();
    let mut i = 0usize;
    while i + 7 <= n {
      if &ub[i..i + 7] == b"NATURAL"
        && (i == 0 || !is_word(ub[i - 1] as char))
        && ub.get(i + 7).is_none_or(|&b| !is_word(b as char))
      {
        out.push(Diagnostic {
          code: "sql617",
          severity: Severity::Warning,
          message: "NATURAL JOIN joins on every same-named column -- a later column add/rename silently changes the join; use explicit ON / USING".into(),
          range: crate::range_at(start + i, start + i + 7),
        });
        i += 7;
        continue;
      }
      i += 1;
    }
  }
}
