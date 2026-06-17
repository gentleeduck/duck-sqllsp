//! sql578: `CREATE RULE ...` -- the PostgreSQL rule system rewrites queries at
//! parse time and has notoriously surprising semantics (multiple evaluation
//! of volatile functions, interactions with RETURNING, etc.). The PG docs
//! steer new code toward triggers (for row-level side effects) or updatable
//! views with INSTEAD OF triggers. Reserve rules for the rare case they're
//! genuinely needed.

use crate::clause_scan::is_word;
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql578"
  }
  fn default_severity(&self) -> Severity {
    Severity::Hint
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, _body, upper) = crate::stmt_body_upper(stmt, source);
    let ub = upper.as_bytes();
    let n = ub.len();
    // Must start with CREATE (optionally OR REPLACE) then RULE.
    let mut p = skip_ws(ub, 0);
    if !word_at(ub, p, b"CREATE") {
      return;
    }
    p = skip_ws(ub, p + 6);
    if word_at(ub, p, b"OR") {
      p = skip_ws(ub, p + 2);
      if word_at(ub, p, b"REPLACE") {
        p = skip_ws(ub, p + 7);
      }
    }
    if p + 4 <= n && word_at(ub, p, b"RULE") {
      out.push(Diagnostic {
        code: "sql578",
        severity: Severity::Hint,
        message: "CREATE RULE is a legacy feature with surprising rewrite semantics -- prefer a trigger or an updatable view".into(),
        range: crate::range_at(start + p, start + p + 4),
      });
    }
  }
}

fn word_at(ub: &[u8], i: usize, kw: &[u8]) -> bool {
  i + kw.len() <= ub.len()
    && ub[i..i + kw.len()] == *kw
    && (i == 0 || !is_word(ub[i - 1] as char))
    && (i + kw.len() == ub.len() || !is_word(ub[i + kw.len()] as char))
}

fn skip_ws(ub: &[u8], mut i: usize) -> usize {
  while i < ub.len() && ub[i].is_ascii_whitespace() {
    i += 1;
  }
  i
}
