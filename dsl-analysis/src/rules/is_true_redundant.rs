//! sql555: `WHERE active IS TRUE` / `WHERE active IS FALSE` -- in a boolean
//! predicate the `IS TRUE` is redundant (`WHERE active`) and `IS FALSE` is
//! just `NOT active`. Scoped to WHERE / ON / HAVING so a SELECT-list
//! `x IS TRUE` (which legitimately produces a boolean value) is left alone.
//! `IS NOT TRUE` / `IS NOT FALSE` are NOT flagged -- their NULL handling does
//! not reduce to a plain expression. (Parallels sql054 for `= true`.)

use crate::clause_scan::{find_clause, find_clause_end, is_word};
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

const STOPWORDS: &[&str] =
  &["GROUP", "ORDER", "LIMIT", "OFFSET", "WINDOW", "RETURNING", "UNION", "INTERSECT", "EXCEPT", "FETCH", "FOR"];

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql555"
  }
  fn default_severity(&self) -> Severity {
    Severity::Hint
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, _body, upper) = crate::stmt_body_upper(stmt, source);
    let ub = upper.as_bytes();

    for needle in [&b"WHERE"[..], &b"ON"[..], &b"HAVING"[..]] {
      let mut from = 0usize;
      while let Some(rel) = find_clause(&ub[from..], needle).map(|p| p + from) {
        let ps = rel + needle.len();
        let pe = find_clause_end(ub, ps, STOPWORDS);
        scan(ub, start, ps, pe, out);
        from = pe.max(ps);
      }
    }
  }
}

fn scan(ub: &[u8], abs: usize, from: usize, to: usize, out: &mut Vec<Diagnostic>) {
  let mut i = from;
  while i + 2 <= to {
    if &ub[i..i + 2] == b"IS" && (i == from || !is_word(ub[i - 1] as char)) && !is_word(*ub.get(i + 2).unwrap_or(&b' ') as char) {
      let mut p = i + 2;
      while p < to && ub[p].is_ascii_whitespace() {
        p += 1;
      }
      // Skip `IS NOT TRUE/FALSE` -- different NULL semantics.
      let (msg, end) = if word(ub, p, b"TRUE", to) {
        ("redundant `IS TRUE` -- use the expression directly", p + 4)
      } else if word(ub, p, b"FALSE", to) {
        ("redundant `IS FALSE` -- use `NOT <expr>` instead", p + 5)
      } else {
        i += 1;
        continue;
      };
      out.push(Diagnostic {
        code: "sql555",
        severity: Severity::Hint,
        message: msg.into(),
        range: crate::range_at(abs + i, abs + end),
      });
      i = end;
      continue;
    }
    i += 1;
  }
}

fn word(ub: &[u8], i: usize, kw: &[u8], to: usize) -> bool {
  i + kw.len() <= to && ub[i..i + kw.len()] == *kw && (i + kw.len() == to || !is_word(ub[i + kw.len()] as char))
}
