//! sql524: `col LIKE '%'` -- a pattern that is nothing but `%` matches every
//! non-NULL value, so the predicate does no filtering (it's at most an
//! `IS NOT NULL`). `col NOT LIKE '%'` is the opposite: it matches no non-NULL
//! row, so the query returns nothing. Both are almost always a placeholder
//! that was never filled in (e.g. a search box defaulting to `%`).

use crate::clause_scan::is_word;
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql524"
  }
  fn default_severity(&self) -> Severity {
    Severity::Warning
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, body, upper) = crate::stmt_body_upper(stmt, source);
    let ub = upper.as_bytes();
    let bytes = body.as_bytes();
    let n = ub.len();

    for &(kw, len) in &[(&b"ILIKE"[..], 5usize), (&b"LIKE"[..], 4usize)] {
      let mut i = 0usize;
      while i + len <= n {
        if ub[i..i + len] != *kw
          || (i > 0 && is_word(ub[i - 1] as char))
          || (i + len < n && is_word(ub[i + len] as char))
        {
          i += 1;
          continue;
        }
        // The operand must be a string literal that is one-or-more `%` and
        // nothing else (no `_`, no backslash that could escape a `%`).
        let mut p = i + len;
        while p < n && bytes[p].is_ascii_whitespace() {
          p += 1;
        }
        if bytes.get(p) != Some(&b'\'') {
          i += len;
          continue;
        }
        let Some((content, lit_end)) = read_string(bytes, p) else {
          i += len;
          continue;
        };
        if !content.is_empty() && content.bytes().all(|b| b == b'%') {
          let is_not = preceded_by_not(ub, i);
          let (verb, effect) = if is_not {
            ("NOT LIKE", "matches no non-NULL row, so this returns nothing")
          } else {
            ("LIKE", "matches every non-NULL row, so it filters nothing")
          };
          let span_start = if is_not { not_start(ub, i) } else { i };
          out.push(Diagnostic {
            code: "sql524",
            severity: Severity::Warning,
            message: format!("`{verb} '{content}'` {effect}"),
            range: crate::range_at(start + span_start, start + lit_end),
          });
        }
        i = lit_end;
      }
    }
  }
}

fn preceded_by_not(ub: &[u8], at: usize) -> bool {
  not_word_before(ub, at).is_some()
}

fn not_start(ub: &[u8], at: usize) -> usize {
  not_word_before(ub, at).unwrap_or(at)
}

/// If the word immediately before `at` (skipping whitespace) is `NOT`, return
/// its start offset.
fn not_word_before(ub: &[u8], at: usize) -> Option<usize> {
  let mut j = at;
  while j > 0 && ub[j - 1].is_ascii_whitespace() {
    j -= 1;
  }
  let end = j;
  while j > 0 && is_word(ub[j - 1] as char) {
    j -= 1;
  }
  if end - j == 3 && ub[j..end].eq_ignore_ascii_case(b"NOT") { Some(j) } else { None }
}

/// Read a single-quoted string at `open`, collapsing `''` to `'`.
fn read_string(bytes: &[u8], open: usize) -> Option<(String, usize)> {
  let mut content = String::new();
  let mut i = open + 1;
  while i < bytes.len() {
    if bytes[i] == b'\'' {
      if bytes.get(i + 1) == Some(&b'\'') {
        content.push('\'');
        i += 2;
        continue;
      }
      return Some((content, i + 1));
    }
    content.push(bytes[i] as char);
    i += 1;
  }
  None
}
