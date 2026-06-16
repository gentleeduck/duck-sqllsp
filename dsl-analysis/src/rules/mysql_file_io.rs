//! sql642: MySQL file-I/O syntax -- `SELECT ... INTO OUTFILE 'path'`,
//! `... INTO DUMPFILE 'path'`, and `LOAD DATA [LOCAL] INFILE 'path' INTO
//! TABLE ...`. PostgreSQL has none of these; bulk-load and dump with `COPY`
//! (server-side, superuser) or `\copy` (client-side, in psql). They also read
//! and write files on the database server, so they're a file-access vector as
//! well as a syntax error.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

/// (phrase words, advice). Each phrase matches whitespace-separated, ASCII-cased.
const PHRASES: &[(&[&str], &str)] = &[
  (&["INTO", "OUTFILE"], "use `COPY ... TO` (server) or `\\copy` (client)"),
  (&["INTO", "DUMPFILE"], "use `COPY ... TO` (server) or `\\copy` (client)"),
  (&["LOAD", "DATA"], "use `COPY ... FROM` (server) or `\\copy` (client)"),
];

/// Index of the start of the first whitespace-separated occurrence of `words`.
fn find_phrase(upper: &str, words: &[&str]) -> Option<usize> {
  let b = upper.as_bytes();
  let n = b.len();
  let first = words[0].as_bytes();
  let is_word = |c: u8| c.is_ascii_alphanumeric() || c == b'_';
  let mut i = 0usize;
  'outer: while i + first.len() <= n {
    if &b[i..i + first.len()] == first
      && (i == 0 || !is_word(b[i - 1]))
    {
      let mut j = i + first.len();
      for w in &words[1..] {
        if j >= n || !b[j].is_ascii_whitespace() {
          i += 1;
          continue 'outer;
        }
        while j < n && b[j].is_ascii_whitespace() {
          j += 1;
        }
        let wb = w.as_bytes();
        if j + wb.len() > n || &b[j..j + wb.len()] != wb {
          i += 1;
          continue 'outer;
        }
        j += wb.len();
      }
      if j == n || !is_word(b[j]) {
        return Some(i);
      }
    }
    i += 1;
  }
  None
}

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql642"
  }
  fn default_severity(&self) -> Severity {
    Severity::Error
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, _body, upper) = crate::stmt_body_upper(stmt, source);
    for &(words, advice) in PHRASES {
      if let Some(at) = find_phrase(&upper, words) {
        let label = words.join(" ");
        let end = at + label.len();
        out.push(Diagnostic {
          code: "sql642",
          severity: Severity::Error,
          message: format!("`{label}` is MySQL file I/O -- PostgreSQL has no equivalent; {advice}"),
          range: crate::range_at(start + at, start + end),
        });
      }
    }
  }
}
