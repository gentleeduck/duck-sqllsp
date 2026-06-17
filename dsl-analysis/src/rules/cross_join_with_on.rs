//! sql660: a `CROSS JOIN` with an `ON` or `USING` clause. A cross join is an
//! unconditional Cartesian product and takes no join condition, so
//! `a CROSS JOIN b ON ...` is a syntax error (42601). Either drop the condition
//! (a real cross join) or change `CROSS JOIN` to `[INNER] JOIN ... ON ...`.

use crate::clause_scan::is_word;
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

fn kw(b: &[u8], i: usize, w: &[u8]) -> bool {
  i + w.len() <= b.len()
    && &b[i..i + w.len()] == w
    && (i == 0 || !is_word(b[i - 1] as char))
    && b.get(i + w.len()).is_none_or(|&c| !is_word(c as char))
}

/// Keywords that end the scan window after a CROSS JOIN's table reference.
const STOPS: &[&[u8]] = &[
  b"JOIN", b"WHERE", b"GROUP", b"HAVING", b"ORDER", b"LIMIT", b"OFFSET", b"UNION", b"INTERSECT", b"EXCEPT",
  b"RETURNING", b"FETCH", b"WINDOW", b"FOR",
];

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql660"
  }
  fn default_severity(&self) -> Severity {
    Severity::Error
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, raw) = crate::stmt_body(stmt, source);
    let cleaned = crate::textutil::strip_noise_full(raw);
    let upper = cleaned.to_ascii_uppercase();
    let b = upper.as_bytes();
    let n = b.len();
    let mut i = 0usize;
    while i + 5 <= n {
      // find `CROSS` <ws> `JOIN`
      if kw(b, i, b"CROSS") {
        let mut j = i + 5;
        while j < n && b[j].is_ascii_whitespace() {
          j += 1;
        }
        if kw(b, j, b"JOIN") {
          // scan the window after the join until a stop keyword / comma / `)`
          let mut k = j + 4;
          let mut depth = 0i32;
          while k < n {
            match b[k] {
              b'(' | b'[' => depth += 1,
              b')' | b']' => {
                if depth == 0 {
                  break;
                }
                depth -= 1;
              }
              b',' if depth == 0 => break,
              _ if depth == 0 => {
                if kw(b, k, b"ON") || kw(b, k, b"USING") {
                  let kw_len = if kw(b, k, b"ON") { 2 } else { 5 };
                  out.push(Diagnostic {
                    code: "sql660",
                    severity: Severity::Error,
                    message: "CROSS JOIN takes no ON/USING condition -- drop it, or use `JOIN ... ON ...` for a conditional join (PG 42601)".into(),
                    range: crate::range_at(start + k, start + k + kw_len),
                  });
                  return;
                }
                if STOPS.iter().any(|s| kw(b, k, s)) {
                  break;
                }
              }
              _ => {}
            }
            k += 1;
          }
          i = j + 4;
          continue;
        }
      }
      i += 1;
    }
  }
}
