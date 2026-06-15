//! sql608: `CREATE UNIQUE INDEX ... USING <am>` where `<am>` is a non-B-tree
//! access method (hash, gin, gist, brin, spgist). Only B-tree supports unique
//! indexes; PostgreSQL rejects the others with "access method \"...\" does not
//! support unique indexes". Drop UNIQUE, or drop the USING clause to get the
//! default B-tree.

use crate::clause_scan::is_word;
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

const NON_BTREE: &[&str] = &["HASH", "GIN", "GIST", "BRIN", "SPGIST"];

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql608"
  }
  fn default_severity(&self) -> Severity {
    Severity::Error
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, _body, upper) = crate::stmt_body_upper(stmt, source);
    // only CREATE UNIQUE INDEX statements
    let head = upper.trim_start();
    if !head.starts_with("CREATE") || !head.contains("UNIQUE") || !head.contains("INDEX") {
      return;
    }
    let ub = upper.as_bytes();
    let n = ub.len();
    // find `USING` then the access-method word that follows it
    let mut i = 0usize;
    while i + 5 <= n {
      if &ub[i..i + 5] == b"USING"
        && (i == 0 || !is_word(ub[i - 1] as char))
        && ub.get(i + 5).is_none_or(|&b| !is_word(b as char))
      {
        let mut j = i + 5;
        while j < n && ub[j].is_ascii_whitespace() {
          j += 1;
        }
        for &am in NON_BTREE {
          let l = am.len();
          if j + l <= n
            && &ub[j..j + l] == am.as_bytes()
            && ub.get(j + l).is_none_or(|&b| !is_word(b as char))
          {
            out.push(Diagnostic {
              code: "sql608",
              severity: Severity::Error,
              message: format!("`{am}` indexes cannot be UNIQUE in PostgreSQL -- only B-tree supports unique indexes"),
              range: crate::range_at(start + j, start + j + l),
            });
            return;
          }
        }
      }
      i += 1;
    }
  }
}
