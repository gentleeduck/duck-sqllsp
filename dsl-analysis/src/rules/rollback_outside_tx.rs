//! sql211: bare `ROLLBACK;` / `COMMIT;` with no preceding BEGIN /
//! START TRANSACTION in the source. PG emits a WARNING ("there is no
//! transaction in progress") and the statement is a no-op.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql211"
  }
  fn default_severity(&self) -> Severity {
    Severity::Warning
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, _body, upper) = crate::stmt_body_upper(stmt, source);
    let trimmed = upper.trim_start();
    let (kw, kw_len) = if trimmed.starts_with("ROLLBACK") {
      ("ROLLBACK", "ROLLBACK".len())
    } else if trimmed.starts_with("COMMIT") {
      ("COMMIT", "COMMIT".len())
    } else {
      return;
    };
    // Skip ROLLBACK TO SAVEPOINT (always valid where SAVEPOINT was set).
    if trimmed.starts_with("ROLLBACK TO") {
      return;
    }
    // Walk source up to this stmt start, look for unclosed BEGIN/START
    // TRANSACTION. Strip comments / strings so commented-out
    // `-- BEGIN` doesn't count.
    let prelude_clean = crate::textutil::strip_comments_only(&source[..start]);
    let prelude = prelude_clean.to_ascii_uppercase();
    let begins = count_occurrences(&prelude, "BEGIN") + count_occurrences(&prelude, "START TRANSACTION");
    // `ROLLBACK TO [SAVEPOINT]` / `COMMIT PREPARED` don't end the active tx.
    // `ON COMMIT {DROP|DELETE|PRESERVE}` in temp-table DDL is a clause
    // keyword, not a tx-close.
    let closes = count_with_prev_exclude(&prelude, "COMMIT", &["ON"], &["PREPARED"])
      + count_excluding_followups(&prelude, "ROLLBACK", &["TO", "PREPARED"]);
    if begins > closes {
      return;
    }
    let lead = upper.len() - trimmed.len();
    let abs_s = start + lead;
    let abs_e = abs_s + kw_len;
    out.push(Diagnostic {
      code: "sql211",
      severity: Severity::Warning,
      message: format!("`{kw}` with no open transaction -- PG emits warning + no-op"),
      range: crate::range_at(abs_s, abs_e),
    });
  }
}

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
        let mut p = i;
        while p > 0 && bytes[p - 1].is_ascii_whitespace() {
          p -= 1
        }
        let word_end = p;
        while p > 0 && (bytes[p - 1].is_ascii_alphanumeric() || bytes[p - 1] == b'_') {
          p -= 1
        }
        let prev_word = &haystack[p..word_end];
        let prev_excluded = excluded_prev.iter().any(|w| prev_word.eq_ignore_ascii_case(w));
        let mut k = i + nlen;
        while k < n && bytes[k].is_ascii_whitespace() {
          k += 1
        }
        let after = &haystack[k..];
        let next_excluded = excluded_next.iter().any(|ex| {
          let elen = ex.len();
          after.len() >= elen
            && after[..elen].eq_ignore_ascii_case(ex)
            && (after.len() == elen
              || !(after.as_bytes()[elen].is_ascii_alphanumeric() || after.as_bytes()[elen] == b'_'))
        });
        if !prev_excluded && !next_excluded {
          count += 1
        }
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
        while k < n && bytes[k].is_ascii_whitespace() {
          k += 1
        }
        let after = &haystack[k..];
        let is_excluded = excluded.iter().any(|ex| {
          let elen = ex.len();
          after.len() >= elen
            && after[..elen].eq_ignore_ascii_case(ex)
            && (after.len() == elen
              || !(after.as_bytes()[elen].is_ascii_alphanumeric() || after.as_bytes()[elen] == b'_'))
        });
        if !is_excluded {
          count += 1
        }
      }
    }
    i += 1;
  }
  count
}
fn count_occurrences(s: &str, needle: &str) -> usize {
  let bytes = s.as_bytes();
  let mut from = 0usize;
  let mut n = 0usize;
  while let Some(rel) = s[from..].find(needle) {
    let at = from + rel;
    let before_ok = at == 0
      || !{
        let p = bytes[at - 1] as char;
        p.is_ascii_alphanumeric() || p == '_'
      };
    let after = at + needle.len();
    let after_ok = after >= bytes.len()
      || !{
        let p = bytes[after] as char;
        p.is_ascii_alphanumeric() || p == '_'
      };
    if before_ok && after_ok {
      n += 1
    }
    from = at + needle.len();
  }
  n
}
