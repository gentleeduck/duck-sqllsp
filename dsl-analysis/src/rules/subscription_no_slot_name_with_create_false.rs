//! sql796: `CREATE SUBSCRIPTION ... WITH (create_slot = false)` with
//! no `slot_name` -- PostgreSQL can't infer which replication slot to
//! use when it isn't asked to create one, and raises an error.

use crate::clause_scan::is_word;
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql796"
  }
  fn default_severity(&self) -> Severity {
    Severity::Error
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, _body, upper) = crate::stmt_body_upper(stmt, source);
    if !upper.trim_start().starts_with("CREATE SUBSCRIPTION") {
      return;
    }
    let ub = upper.as_bytes();
    let Some(with_rel) = word_pos(ub, b"WITH") else { return };
    let p = skip_ws(ub, with_rel + 4);
    if ub.get(p) != Some(&b'(') {
      return;
    }
    let Some(close) = match_paren(ub, p) else { return };
    let opts = &upper[p + 1..close];
    let Some(cs_rel) = word_pos(opts.as_bytes(), b"CREATE_SLOT") else { return };
    let after = opts[cs_rel + "CREATE_SLOT".len()..].trim_start();
    if !after.starts_with('=') {
      return;
    }
    let val = after[1..].trim_start();
    if !val.starts_with("FALSE") {
      return;
    }
    if !contains_word(opts, "SLOT_NAME") {
      out.push(Diagnostic {
        code: "sql796",
        severity: Severity::Error,
        message: "create_slot = false with no slot_name -- PostgreSQL can't infer which replication slot to use".into(),
        range: crate::range_at(start + p, start + close + 1),
      });
    }
  }
}

fn contains_word(s: &str, w: &str) -> bool {
  crate::textutil::contains_word(s, w)
}

fn word_pos(ub: &[u8], w: &[u8]) -> Option<usize> {
  let mut i = 0usize;
  while i + w.len() <= ub.len() {
    if &ub[i..i + w.len()] == w
      && (i == 0 || !is_word(ub[i - 1] as char))
      && (i + w.len() == ub.len() || !is_word(ub[i + w.len()] as char))
    {
      return Some(i);
    }
    i += 1;
  }
  None
}

fn match_paren(ub: &[u8], open: usize) -> Option<usize> {
  let mut depth = 0i32;
  let mut i = open;
  while i < ub.len() {
    match ub[i] {
      b'(' => depth += 1,
      b')' => {
        depth -= 1;
        if depth == 0 {
          return Some(i);
        }
      },
      b'\'' => {
        i += 1;
        while i < ub.len() && ub[i] != b'\'' {
          i += 1;
        }
      },
      _ => {},
    }
    i += 1;
  }
  None
}

fn skip_ws(ub: &[u8], mut i: usize) -> usize {
  while i < ub.len() && ub[i].is_ascii_whitespace() {
    i += 1;
  }
  i
}
