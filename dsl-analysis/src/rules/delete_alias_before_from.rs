//! sql668: a `DELETE` whose first token isn't `FROM`, e.g.
//! `DELETE t1 FROM t1 JOIN t2 ...` (MySQL / SQL Server multi-table delete).
//! PostgreSQL's grammar is `DELETE FROM target [USING ...]`, so the leading
//! table/alias is a syntax error. Rewrite as
//! `DELETE FROM t1 USING t2 WHERE ...`.

use crate::clause_scan::is_word;
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql668"
  }
  fn default_severity(&self) -> Severity {
    Severity::Error
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, raw) = crate::stmt_body(stmt, source);
    let cleaned = crate::textutil::strip_noise_full(raw);
    let upper = cleaned.to_ascii_uppercase();
    let u = upper.trim_start();
    if !u.starts_with("DELETE") {
      return;
    }
    let b = upper.as_bytes();
    let n = b.len();
    let lead = upper.len() - u.len();
    let mut j = lead + 6;
    while j < n && b[j].is_ascii_whitespace() {
      j += 1;
    }
    // the only valid continuation is `FROM`
    let is_from = j + 4 <= n && &b[j..j + 4] == b"FROM" && b.get(j + 4).is_none_or(|&c| !is_word(c as char));
    if !is_from && j < n {
      out.push(Diagnostic {
        code: "sql668",
        severity: Severity::Error,
        message: "DELETE must be followed by FROM in PostgreSQL -- `DELETE <table> FROM ...` is MySQL/SQL Server; use `DELETE FROM t USING ...`".into(),
        range: crate::range_at(start + j, start + (j + 4).min(n)),
      });
    }
  }
}
