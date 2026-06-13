//! sql529: `HAVING COUNT(*) > 0` -- a group only exists if it has at least
//! one row, so `COUNT(*)` is always >= 1 and the predicate is always true. It
//! filters nothing; the GROUP BY already guarantees non-empty groups. Common
//! when someone reaches for HAVING expecting WHERE-style row filtering.
//!
//! Restricted to `COUNT(*)` / `COUNT(1)` (which count rows, not non-NULL
//! values) and to comparisons that hold for every integer >= 1.

use crate::clause_scan::{find_clause, find_clause_end, is_word};
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

const STOPWORDS: &[&str] = &["ORDER", "LIMIT", "OFFSET", "WINDOW", "UNION", "INTERSECT", "EXCEPT", "FETCH", "FOR"];

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql529"
  }
  fn default_severity(&self) -> Severity {
    Severity::Warning
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, body, upper) = crate::stmt_body_upper(stmt, source);
    let ub = upper.as_bytes();
    let bytes = body.as_bytes();

    let Some(h) = find_clause(ub, b"HAVING") else { return };
    let hs = h + 6;
    let he = find_clause_end(ub, hs, STOPWORDS);

    let mut i = hs;
    while i + 5 <= he {
      if ub[i..i + 5] != *b"COUNT" || (i > 0 && is_word(ub[i - 1] as char)) {
        i += 1;
        continue;
      }
      // COUNT ( * | 1 )
      let mut p = skip_ws(bytes, i + 5);
      if bytes.get(p) != Some(&b'(') {
        i += 5;
        continue;
      }
      p = skip_ws(bytes, p + 1);
      let arg_ok = bytes.get(p) == Some(&b'*') || bytes.get(p) == Some(&b'1');
      let mut q = skip_ws(bytes, p + 1);
      if !arg_ok || bytes.get(q) != Some(&b')') {
        i += 5;
        continue;
      }
      q = skip_ws(bytes, q + 1);
      // Operator + numeric literal.
      let Some((op, after_op)) = read_op(bytes, q) else {
        i += 5;
        continue;
      };
      let ns = skip_ws(bytes, after_op);
      let Some((n, ne)) = read_number(bytes, ns, he) else {
        i = q;
        continue;
      };
      if always_true(op, n) {
        out.push(Diagnostic {
          code: "sql529",
          severity: Severity::Warning,
          message: format!(
            "`HAVING COUNT(*) {op} {}` is always true -- every group has at least one row",
            trim_num(&body[ns..ne])
          ),
          range: crate::range_at(start + i, start + ne),
        });
      }
      i = ne;
    }
  }
}

/// Holds for every integer count >= 1.
fn always_true(op: &str, n: f64) -> bool {
  match op {
    ">" => n < 1.0,
    ">=" => n <= 1.0,
    "<>" | "!=" => n < 1.0,
    _ => false,
  }
}

fn read_op(bytes: &[u8], i: usize) -> Option<(&'static str, usize)> {
  match (bytes.get(i), bytes.get(i + 1)) {
    (Some(b'>'), Some(b'=')) => Some((">=", i + 2)),
    (Some(b'<'), Some(b'>')) => Some(("<>", i + 2)),
    (Some(b'!'), Some(b'=')) => Some(("!=", i + 2)),
    (Some(b'>'), _) => Some((">", i + 1)),
    _ => None,
  }
}

/// Read an integer/decimal literal (optional leading `-`). Returns its value
/// and end offset.
fn read_number(bytes: &[u8], start: usize, to: usize) -> Option<(f64, usize)> {
  let mut i = start;
  if bytes.get(i) == Some(&b'-') {
    i += 1;
  }
  let digits_start = i;
  while i < to && (bytes[i].is_ascii_digit() || bytes[i] == b'.') {
    i += 1;
  }
  if i == digits_start {
    return None;
  }
  let s = std::str::from_utf8(&bytes[start..i]).ok()?;
  let n: f64 = s.parse().ok()?;
  Some((n, i))
}

fn trim_num(s: &str) -> &str {
  s.trim()
}

fn skip_ws(bytes: &[u8], mut i: usize) -> usize {
  while i < bytes.len() && bytes[i].is_ascii_whitespace() {
    i += 1;
  }
  i
}
