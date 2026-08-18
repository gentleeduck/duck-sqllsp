//! sql776: `EXCLUDE USING gist (col WITH =)` on a single column with
//! only the `=` operator -- functionally a weaker, slower UNIQUE
//! constraint (GIST equality lookups don't get a btree's lookup speed,
//! so this loses index efficiency for no behavioral gain over UNIQUE).

use crate::clause_scan::{is_word, split_top_level};
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql776"
  }
  fn default_severity(&self) -> Severity {
    Severity::Hint
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, body, upper) = crate::stmt_body_upper(stmt, source);
    let ub = upper.as_bytes();
    let mut i = 0usize;
    while let Some(excl) = find_word_from(ub, i, b"EXCLUDE") {
      let mut j = skip_ws(ub, excl + "EXCLUDE".len());
      if !word_at(ub, j, b"USING") {
        i = excl + "EXCLUDE".len();
        continue;
      }
      j = skip_ws(ub, j + "USING".len());
      while j < ub.len() && is_word(ub[j] as char) {
        j += 1;
      }
      j = skip_ws(ub, j);
      if ub.get(j) != Some(&b'(') {
        i = excl + "EXCLUDE".len();
        continue;
      }
      let open = j;
      let Some(close) = match_paren(ub, open) else { break };
      let entries: Vec<(&str, usize)> = split_top_level(&body[open + 1..close]);
      if entries.len() == 1 {
        let (entry, _) = entries[0];
        let upper_entry = entry.to_ascii_uppercase();
        if let Some(at) = upper_entry.find(" WITH ") {
          let op = upper_entry[at + " WITH ".len()..].trim();
          if op == "=" {
            out.push(Diagnostic {
              code: "sql776",
              severity: Severity::Hint,
              message: "single-column EXCLUDE USING ... WITH = is functionally a slower UNIQUE constraint -- consider UNIQUE instead".into(),
              range: crate::range_at(start + open, start + close + 1),
            });
          }
        }
      }
      i = close + 1;
    }
  }
}

fn find_word_from(ub: &[u8], from: usize, w: &[u8]) -> Option<usize> {
  let mut i = from;
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

fn word_at(ub: &[u8], i: usize, w: &[u8]) -> bool {
  i + w.len() <= ub.len()
    && &ub[i..i + w.len()] == w
    && (i == 0 || !is_word(ub[i - 1] as char))
    && (i + w.len() == ub.len() || !is_word(ub[i + w.len()] as char))
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
