//! sql258: `SET LOCAL <foo> = <val>` outside an explicit
//! transaction block. SET LOCAL scopes to the tx, so issued in
//! autocommit it's a no-op + immediate reset. Catches the migration
//! file that calls SET LOCAL search_path then forgets to wrap in
//! BEGIN/COMMIT.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql258"
  }
  fn default_severity(&self) -> Severity {
    Severity::Warning
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, body, upper) = crate::stmt_body_upper(stmt, source);
    if !upper.trim_start().starts_with("SET LOCAL") {
      return;
    }
    let prelude_clean = strip_noise_and_dollar(&source[..start]);
    let prelude = prelude_clean.to_ascii_uppercase();
    let begins = count_kw(&prelude, "BEGIN") + count_phrase(&prelude, "START TRANSACTION");
    // Exclude `ON COMMIT {DROP|DELETE|PRESERVE}` (temp-table DDL),
    // `ROLLBACK TO [SAVEPOINT]`, `COMMIT PREPARED` from tx-close count.
    let closes = count_with_prev_exclude(&prelude, "COMMIT", &["ON"], &["PREPARED"])
      + count_with_prev_exclude(&prelude, "ROLLBACK", &[], &["TO", "PREPARED"]);
    if begins > closes {
      return;
    }
    let abs_s = start;
    let abs_e = start + body.find(';').unwrap_or(body.len());
    out.push(Diagnostic {
      code: "sql258",
      severity: Severity::Warning,
      message: "SET LOCAL outside transaction -- scope is tx-bound, autocommit issues an immediate reset; wrap in BEGIN/COMMIT or drop LOCAL".into(),
      range: crate::range_at(abs_s, abs_e),
    });
  }
}

fn count_kw(s: &str, needle: &str) -> usize {
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

fn count_phrase(s: &str, needle: &str) -> usize {
  s.matches(needle).count()
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

fn strip_noise_and_dollar(s: &str) -> String {
  let mut out: Vec<u8> = s.as_bytes().to_vec();
  let n = out.len();
  let mut i = 0usize;
  while i < n {
    if i + 1 < n && out[i] == b'-' && out[i + 1] == b'-' {
      while i < n && out[i] != b'\n' {
        out[i] = b' ';
        i += 1
      }
      continue;
    }
    if i + 1 < n && out[i] == b'/' && out[i + 1] == b'*' {
      let mut depth = 1u32;
      out[i] = b' ';
      out[i + 1] = b' ';
      i += 2;
      while i + 1 < n && depth > 0 {
        if out[i] == b'/' && out[i + 1] == b'*' {
          depth += 1;
          out[i] = b' ';
          out[i + 1] = b' ';
          i += 2;
        } else if out[i] == b'*' && out[i + 1] == b'/' {
          depth -= 1;
          out[i] = b' ';
          out[i + 1] = b' ';
          i += 2;
        } else {
          out[i] = b' ';
          i += 1;
        }
      }
      continue;
    }
    if out[i] == b'\'' {
      out[i] = b' ';
      i += 1;
      while i < n && out[i] != b'\'' {
        out[i] = b' ';
        i += 1
      }
      if i < n {
        out[i] = b' ';
        i += 1
      }
      continue;
    }
    if out[i] == b'$' {
      let mut k = i + 1;
      while k < n && (out[k].is_ascii_alphanumeric() || out[k] == b'_') {
        k += 1
      }
      if k < n && out[k] == b'$' {
        let tag_bytes = &out[i + 1..k];
        let closer: Vec<u8> =
          std::iter::once(b'$').chain(tag_bytes.iter().copied()).chain(std::iter::once(b'$')).collect();
        let closer_len = closer.len();
        out[i..k + 1].fill(b' ');
        i = k + 1;
        while i + closer_len <= n {
          if out[i..i + closer_len] == *closer {
            break;
          }
          out[i] = b' ';
          i += 1;
        }
        if i + closer_len <= n {
          out[i..i + closer_len].fill(b' ');
          i += closer_len;
        }
        continue;
      }
    }
    i += 1;
  }
  String::from_utf8(out).unwrap_or_else(|_| s.to_string())
}
