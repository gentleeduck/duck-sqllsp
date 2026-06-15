//! sql561: `SELECT ... LIMIT ALL` -- `LIMIT ALL` is the explicit spelling of
//! "no limit", exactly the same as omitting the clause. It's harmless but
//! pure noise; drop it.

use crate::clause_scan::is_word;
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql561"
  }
  fn default_severity(&self) -> Severity {
    Severity::Hint
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, _body, upper) = crate::stmt_body_upper(stmt, source);
    let ub = upper.as_bytes();
    let n = ub.len();
    let mut i = 0usize;
    while i + 5 <= n {
      if &ub[i..i + 5] == b"LIMIT" && (i == 0 || !is_word(ub[i - 1] as char)) && !is_word(*ub.get(i + 5).unwrap_or(&b' ') as char) {
        let mut p = i + 5;
        while p < n && ub[p].is_ascii_whitespace() {
          p += 1;
        }
        if p + 3 <= n && &ub[p..p + 3] == b"ALL" && (p + 3 == n || !is_word(ub[p + 3] as char)) {
          out.push(Diagnostic {
            code: "sql561",
            severity: Severity::Hint,
            message: "`LIMIT ALL` means no limit -- it's redundant, just omit it".into(),
            range: crate::range_at(start + i, start + p + 3),
          });
          i = p + 3;
          continue;
        }
      }
      i += 1;
    }
  }
}
