//! sql207: `COALESCE(x)` with a single argument is a no-op -- it
//! returns x unchanged. Almost always a copy-paste bug from a
//! multi-arg COALESCE. Same applies to GREATEST / LEAST / CONCAT.
//! `CONCAT_WS(sep, value)` is also a no-op (the separator is never
//! used when there's only one value to join).

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

/// (function name, expected top-level-comma count for the no-op case).
/// 0 commas = single-arg call; 1 = `CONCAT_WS(sep, value)`.
const FNS: &[(&str, usize)] = &[("coalesce", 0), ("greatest", 0), ("least", 0), ("concat", 0), ("concat_ws", 1)];

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql207"
  }
  fn default_severity(&self) -> Severity {
    Severity::Warning
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, body) = crate::stmt_body(stmt, source);
    let lower = body.to_ascii_lowercase();
    for &(fname, expected_commas) in FNS {
      let needle = format!("{fname}(");
      let mut from = 0usize;
      while let Some(rel) = lower[from..].find(&needle) {
        let at = from + rel;
        if at > 0 {
          let prev = body.as_bytes()[at - 1] as char;
          if prev.is_ascii_alphanumeric() || prev == '_' {
            from = at + needle.len();
            continue;
          }
        }
        let open = at + needle.len();
        let Some(close) = find_matching_paren(body, open - 1) else {
          from = open;
          continue;
        };
        let inner = &body[open..close];
        if count_top_level_commas(inner) == expected_commas {
          // Zero-arg call has empty inner -- different message
          // (for concat() that's "returns empty string", for
          // coalesce/greatest/least it's actually a syntax error in
          // PG; we treat the empty-arg call generically since the
          // parser will reject the latter anyway).
          let is_empty = inner.trim().is_empty();
          let msg = if expected_commas == 0 {
            if is_empty {
              format!(
                "`{fname}()` has no arguments -- {} returns an empty string with no inputs; almost certainly a typo (forgot to add the arguments)",
                fname
              )
            } else {
              format!("`{fname}({})` is a no-op with one argument -- returns the argument unchanged", inner.trim())
            }
          } else {
            format!(
              "`{fname}({})` is a no-op -- the separator never joins anything with only one value; returns the value unchanged",
              inner.trim()
            )
          };
          out.push(Diagnostic {
            code: "sql207",
            severity: Severity::Warning,
            message: msg,
            range: crate::range_at(start + at, start + close + 1),
          });
        }
        from = close + 1;
      }
    }
  }
}

fn count_top_level_commas(text: &str) -> usize {
  let bytes = text.as_bytes();
  let mut depth = 0i32;
  let mut commas = 0usize;
  let mut i = 0usize;
  while i < bytes.len() {
    match bytes[i] {
      b'(' => depth += 1,
      b')' => depth -= 1,
      b',' if depth == 0 => commas += 1,
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
  commas
}

fn find_matching_paren(s: &str, open: usize) -> Option<usize> {
  let bytes = s.as_bytes();
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
