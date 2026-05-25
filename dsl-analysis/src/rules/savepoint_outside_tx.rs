//! sql179: `SAVEPOINT s;` outside a transaction errors with
//! 25P01 ("SAVEPOINT can only be used in transaction blocks").
//! Heuristic: walk back from the SAVEPOINT keyword counting
//! BEGIN / START TRANSACTION vs COMMIT / ROLLBACK. If the
//! balance is zero or negative, no active tx -> flag.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql179"
  }
  fn default_severity(&self) -> Severity {
    Severity::Error
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let start: usize = u32::from(stmt.range.start()) as usize;
    let end: usize = (u32::from(stmt.range.end()) as usize).min(source.len());
    let raw = &source[start..end];
    let body = strip_comments(raw);
    let upper = body.to_ascii_uppercase();
    let Some(rel) = upper.find("SAVEPOINT") else { return };
    let kw_at = start + rel;
    if let Some(prev) = source.as_bytes().get(kw_at.saturating_sub(1)).copied() {
      if (prev as char).is_ascii_alphanumeric() || prev == b'_' { return; }
    }
    let after = kw_at + "SAVEPOINT".len();
    if let Some(next) = source.as_bytes().get(after).copied() {
      if (next as char).is_ascii_alphanumeric() || next == b'_' { return; }
    }
    // Count BEGIN/START TRANSACTION vs COMMIT/ROLLBACK in source up
    // to the SAVEPOINT keyword, with comments stripped to keep counts
    // honest.
    let prior_owned = strip_comments(&source[..kw_at]);
    let prior = prior_owned.to_ascii_uppercase();
    let begins = count_word(&prior, "BEGIN") + count_word(&prior, "START TRANSACTION");
    // `ROLLBACK TO SAVEPOINT` and `ROLLBACK TO` are partial rollbacks that
    // do NOT end the transaction -- exclude them. Same for `COMMIT
    // PREPARED` which acts on a 2PC stmt, not the active tx.
    let commits = count_word_excluding_followups(&prior, "COMMIT", &["PREPARED"])
      + count_word_excluding_followups(&prior, "ROLLBACK", &["TO", "PREPARED"]);
    if begins > commits {
      return;
    }
    out.push(Diagnostic {
      code: "sql179",
      severity: Severity::Error,
      message: "SAVEPOINT outside a transaction -- PG raises 25P01".into(),
      range: text_size::TextRange::new((kw_at as u32).into(), (after as u32).into()),
    });
  }
}

fn count_word_excluding_followups(haystack: &str, needle: &str, excluded: &[&str]) -> usize {
  let bytes = haystack.as_bytes();
  let n = bytes.len();
  let nlen = needle.len();
  let mut count = 0;
  let mut i = 0;
  while i + nlen <= n {
    if &haystack[i..i + nlen] == needle {
      let prev_ok = i == 0 || !(bytes[i - 1].is_ascii_alphanumeric() || bytes[i - 1] == b'_');
      let next_ok = i + nlen == n || !(bytes[i + nlen].is_ascii_alphanumeric() || bytes[i + nlen] == b'_');
      if prev_ok && next_ok {
        let mut k = i + nlen;
        while k < n && bytes[k].is_ascii_whitespace() { k += 1 }
        let after = &haystack[k..];
        let is_excluded = excluded.iter().any(|ex| {
          let elen = ex.len();
          after.len() >= elen && after[..elen].eq_ignore_ascii_case(ex)
            && (after.len() == elen || !(after.as_bytes()[elen].is_ascii_alphanumeric() || after.as_bytes()[elen] == b'_'))
        });
        if !is_excluded { count += 1 }
      }
    }
    i += 1;
  }
  count
}

fn count_word(haystack: &str, needle: &str) -> usize {
  let bytes = haystack.as_bytes();
  let n = bytes.len();
  let nlen = needle.len();
  let mut count = 0;
  let mut i = 0;
  while i + nlen <= n {
    if &haystack[i..i + nlen] == needle {
      let prev_ok = i == 0 || !(bytes[i - 1].is_ascii_alphanumeric() || bytes[i - 1] == b'_');
      let next_ok = i + nlen == n || !(bytes[i + nlen].is_ascii_alphanumeric() || bytes[i + nlen] == b'_');
      if prev_ok && next_ok {
        count += 1;
      }
    }
    i += 1;
  }
  count
}

fn strip_comments(s: &str) -> String {
  let mut out = String::with_capacity(s.len());
  let bytes = s.as_bytes();
  let n = bytes.len();
  let mut i = 0usize;
  while i < n {
    if i + 1 < n && bytes[i] == b'-' && bytes[i + 1] == b'-' {
      while i < n && bytes[i] != b'\n' { out.push(' '); i += 1 }
    } else if bytes[i] == b'\'' {
      out.push(' ');
      i += 1;
      while i < n && bytes[i] != b'\'' { out.push(' '); i += 1 }
      if i < n { out.push(' '); i += 1 }
    } else if bytes[i].is_ascii() {
      out.push(bytes[i] as char);
      i += 1;
    } else {
      out.push(' ');
      i += 1;
    }
  }
  out
}
