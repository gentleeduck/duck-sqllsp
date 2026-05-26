//! sql224: `SET CONSTRAINTS ALL DEFERRED` (or any SET CONSTRAINTS
//! form) outside an explicit transaction block. The effect is
//! transaction-scoped, so issuing it autocommit means PG resets the
//! constraint mode immediately afterwards -- no-op.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql224"
  }
  fn default_severity(&self) -> Severity {
    Severity::Warning
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let start: usize = u32::from(stmt.range.start()) as usize;
    let end: usize = (u32::from(stmt.range.end()) as usize).min(source.len());
    let body = &source[start..end];
    let upper = body.to_ascii_uppercase();
    if !upper.trim_start().starts_with("SET CONSTRAINTS") { return }
    let prelude_clean = strip_noise(&source[..start]);
    let prelude = prelude_clean.to_ascii_uppercase();
    let begins = count_kw(&prelude, "BEGIN") + count_phrase(&prelude, "START TRANSACTION");
    // `ROLLBACK TO [SAVEPOINT]` / `COMMIT PREPARED` don't end the tx.
    // CREATE TEMP TABLE ... ON COMMIT {DROP|DELETE|PRESERVE} ROWS has
    // `COMMIT` as a clause keyword, not a tx-close. Exclude any
    // COMMIT preceded by ON. count_with_prev_exclude does that.
    let closes = count_with_prev_exclude(&prelude, "COMMIT", &["ON"], &["PREPARED"])
      + count_excluding_followups(&prelude, "ROLLBACK", &["TO", "PREPARED"]);
    if begins > closes { return }
    let abs_s = start;
    let abs_e = start + body.find(';').unwrap_or(body.len());
    out.push(Diagnostic {
      code: "sql224",
      severity: Severity::Warning,
      message: "SET CONSTRAINTS outside transaction block -- effect is tx-scoped; wrap in BEGIN/COMMIT to actually defer".into(),
      range: text_size::TextRange::new((abs_s as u32).into(), (abs_e as u32).into()),
    });
  }
}

fn count_kw(s: &str, needle: &str) -> usize {
  let bytes = s.as_bytes();
  let mut from = 0usize;
  let mut n = 0usize;
  while let Some(rel) = s[from..].find(needle) {
    let at = from + rel;
    let before_ok = at == 0 || !{ let p = bytes[at - 1] as char; p.is_ascii_alphanumeric() || p == '_' };
    let after = at + needle.len();
    let after_ok = after >= bytes.len() || !{ let p = bytes[after] as char; p.is_ascii_alphanumeric() || p == '_' };
    if before_ok && after_ok { n += 1 }
    from = at + needle.len();
  }
  n
}

fn count_phrase(s: &str, needle: &str) -> usize {
  s.matches(needle).count()
}

/// Count word-bounded occurrences of `needle`, excluding ones that are
/// preceded by any word in `excluded_prev` (e.g. `ON COMMIT`) or
/// followed by any word in `excluded_next` (e.g. `COMMIT PREPARED`).
fn count_with_prev_exclude(haystack: &str, needle: &str, excluded_prev: &[&str], excluded_next: &[&str]) -> usize {
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
        // Walk backwards through whitespace to find prev word.
        let mut p = i;
        while p > 0 && bytes[p - 1].is_ascii_whitespace() { p -= 1 }
        let word_end = p;
        while p > 0 && (bytes[p - 1].is_ascii_alphanumeric() || bytes[p - 1] == b'_') { p -= 1 }
        let prev_word = &haystack[p..word_end];
        let prev_excluded = excluded_prev.iter().any(|w| prev_word.eq_ignore_ascii_case(w));
        // Walk forward through whitespace to find next word.
        let mut k = i + nlen;
        while k < n && bytes[k].is_ascii_whitespace() { k += 1 }
        let after = &haystack[k..];
        let next_excluded = excluded_next.iter().any(|ex| {
          let elen = ex.len();
          after.len() >= elen && after[..elen].eq_ignore_ascii_case(ex)
            && (after.len() == elen || !(after.as_bytes()[elen].is_ascii_alphanumeric() || after.as_bytes()[elen] == b'_'))
        });
        if !prev_excluded && !next_excluded { count += 1 }
      }
    }
    i += 1;
  }
  count
}

fn count_excluding_followups(haystack: &str, needle: &str, excluded: &[&str]) -> usize {
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

fn strip_noise(s: &str) -> String {
  let mut out = String::with_capacity(s.len());
  let bytes = s.as_bytes();
  let n = bytes.len();
  let mut i = 0usize;
  while i < n {
    if i + 1 < n && bytes[i] == b'-' && bytes[i + 1] == b'-' {
      while i < n && bytes[i] != b'\n' { out.push(' '); i += 1 }
    } else if i + 1 < n && bytes[i] == b'/' && bytes[i + 1] == b'*' {
      let mut depth = 1u32;
      out.push(' '); out.push(' '); i += 2;
      while i + 1 < n && depth > 0 {
        if bytes[i] == b'/' && bytes[i + 1] == b'*' { depth += 1; out.push(' '); out.push(' '); i += 2; }
        else if bytes[i] == b'*' && bytes[i + 1] == b'/' { depth -= 1; out.push(' '); out.push(' '); i += 2; }
        else { out.push(' '); i += 1; }
      }
    } else if bytes[i] == b'\'' {
      out.push(' '); i += 1;
      while i < n && bytes[i] != b'\'' { out.push(' '); i += 1 }
      if i < n { out.push(' '); i += 1 }
    } else if bytes[i].is_ascii() {
      out.push(bytes[i] as char); i += 1;
    } else {
      out.push(' '); i += 1;
    }
  }
  out
}
