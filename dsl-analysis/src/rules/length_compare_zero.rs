//! sql540: `WHERE length(s) = 0` / `length(s) > 0` -- comparing a string's
//! length to zero is an indirect, non-sargable way to ask "is it empty?".
//! `length(s) = 0` is `s = ''` and `length(s) > 0` is `s <> ''` (for non-NULL
//! `s`). The direct form is clearer and can use an index on `s`.

use crate::clause_scan::is_word;
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

const FNS: &[&str] = &["length(", "char_length(", "octet_length(", "character_length("];

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql540"
  }
  fn default_severity(&self) -> Severity {
    Severity::Hint
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, body) = crate::stmt_body(stmt, source);
    let lower = body.to_ascii_lowercase();
    let bytes = body.as_bytes();
    let n = bytes.len();

    for fname in FNS {
      let mut from = 0usize;
      while let Some(rel) = lower[from..].find(fname) {
        let at = from + rel;
        if at > 0 && (bytes[at - 1].is_ascii_alphanumeric() || bytes[at - 1] == b'_') {
          from = at + fname.len();
          continue;
        }
        let open = at + fname.len() - 1;
        let Some(close) = match_paren(bytes, open) else { break };
        let inner = body[open + 1..close].trim();
        // Operator + literal 0 / 1 after the call.
        let mut p = close + 1;
        while p < n && bytes[p].is_ascii_whitespace() {
          p += 1;
        }
        if let Some((suggestion, end)) = empty_check(bytes, p, n, inner) {
          out.push(Diagnostic {
            code: "sql540",
            severity: Severity::Hint,
            message: format!("length comparison -- this is just `{suggestion}`"),
            range: crate::range_at(start + at, start + end),
          });
          from = end;
        } else {
          from = close + 1;
        }
      }
    }
  }
}

/// Recognise `<op> <n>` after the length() call that reduces to an empty /
/// non-empty test. Returns the suggested rewrite and the end offset.
fn empty_check(bytes: &[u8], p: usize, n: usize, inner: &str) -> Option<(String, usize)> {
  let (op, after) = read_op(bytes, p)?;
  let ns = skip_ws(bytes, after);
  let (num, ne) = read_int(bytes, ns, n)?;
  // length >= 0 is always true and length < 0 never -- those are other rules'
  // jobs; here we only map the empty/non-empty cases.
  let suggestion = match (op, num) {
    ("=", 0) => format!("{inner} = ''"),
    (">", 0) | (">=", 1) | ("<>", 0) | ("!=", 0) => format!("{inner} <> ''"),
    _ => return None,
  };
  Some((suggestion, ne))
}

fn read_op(bytes: &[u8], i: usize) -> Option<(&'static str, usize)> {
  match (bytes.get(i), bytes.get(i + 1)) {
    (Some(b'>'), Some(b'=')) => Some((">=", i + 2)),
    (Some(b'<'), Some(b'>')) => Some(("<>", i + 2)),
    (Some(b'!'), Some(b'=')) => Some(("!=", i + 2)),
    (Some(b'='), _) => Some(("=", i + 1)),
    (Some(b'>'), _) => Some((">", i + 1)),
    _ => None,
  }
}

fn read_int(bytes: &[u8], start: usize, to: usize) -> Option<(i64, usize)> {
  let mut i = start;
  while i < to && bytes[i].is_ascii_digit() {
    i += 1;
  }
  if i == start {
    return None;
  }
  // Reject a following word/decimal char so `0.5` / `0x` don't match as `0`.
  if matches!(bytes.get(i), Some(&b) if b == b'.' || is_word(b as char)) {
    return None;
  }
  let v: i64 = std::str::from_utf8(&bytes[start..i]).ok()?.parse().ok()?;
  Some((v, i))
}

fn skip_ws(bytes: &[u8], mut i: usize) -> usize {
  while i < bytes.len() && bytes[i].is_ascii_whitespace() {
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
