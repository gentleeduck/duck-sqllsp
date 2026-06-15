//! sql560: `FOREIGN KEY (a, b) REFERENCES t (c)` -- the referencing and
//! referenced column lists have different lengths. Postgres rejects this with
//! 42830 ("number of referencing and referenced columns for foreign key
//! disagree"). The two lists must line up one-to-one.

use crate::clause_scan::is_word;
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql560"
  }
  fn default_severity(&self) -> Severity {
    Severity::Error
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, _body, upper) = crate::stmt_body_upper(stmt, source);
    let ub = upper.as_bytes();
    let n = ub.len();
    let kw = b"FOREIGN KEY";

    let mut i = 0usize;
    while i + kw.len() <= n {
      if ub[i..i + kw.len()] != *kw || (i > 0 && is_word(ub[i - 1] as char)) {
        i += 1;
        continue;
      }
      // Local column list.
      let mut p = skip_ws(ub, i + kw.len());
      if ub.get(p) != Some(&b'(') {
        i += kw.len();
        continue;
      }
      let Some(local_close) = match_paren(ub, p) else { break };
      let local = count_cols(ub, p + 1, local_close);
      // REFERENCES <table> [ ( ref cols ) ]
      p = skip_ws(ub, local_close + 1);
      if !word_at(ub, p, b"REFERENCES") {
        i = local_close + 1;
        continue;
      }
      p = skip_ws(ub, p + 10);
      // Skip the (possibly schema-qualified, possibly quoted) table name.
      while p < n && (is_word(ub[p] as char) || ub[p] == b'.' || ub[p] == b'"') {
        p += 1;
      }
      p = skip_ws(ub, p);
      if ub.get(p) == Some(&b'(')
        && let Some(ref_close) = match_paren(ub, p)
      {
        let referenced = count_cols(ub, p + 1, ref_close);
        if local != referenced {
          out.push(Diagnostic {
            code: "sql560",
            severity: Severity::Error,
            message: format!(
              "FOREIGN KEY lists {local} column(s) but references {referenced} -- the counts must match (PG error 42830)"
            ),
            range: crate::range_at(start + i, start + ref_close + 1),
          });
        }
        i = ref_close + 1;
      } else {
        i = local_close + 1;
      }
    }
  }
}

fn count_cols(ub: &[u8], from: usize, to: usize) -> usize {
  if from >= to || ub[from..to].iter().all(|b| b.is_ascii_whitespace()) {
    return 0;
  }
  let mut count = 1usize;
  let mut depth = 0i32;
  let mut i = from;
  while i < to {
    match ub[i] {
      b'(' | b'[' => depth += 1,
      b')' | b']' => depth -= 1,
      b',' if depth == 0 => count += 1,
      _ => {},
    }
    i += 1;
  }
  count
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

fn match_paren(bytes: &[u8], open: usize) -> Option<usize> {
  let mut depth = 0i32;
  let mut i = open;
  while i < bytes.len() {
    match bytes[i] {
      b'(' => depth += 1,
      b')' => {
        depth -= 1;
        if depth == 0 {
          return Some(i);
        }
      },
      b'\'' => {
        i += 1;
        while i < bytes.len() && bytes[i] != b'\'' {
          i += 1
        }
      },
      _ => {},
    }
    i += 1;
  }
  None
}
