//! sql296: `REINDEX` (TABLE / INDEX / SCHEMA / DATABASE) inside an
//! open transaction. PG holds AccessExclusiveLock for the whole
//! tx duration -- a sustained outage on busy tables. Run REINDEX
//! outside BEGIN/COMMIT, or use CONCURRENTLY (PG12+, and outside
//! tx -- see sql214).

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql296"
  }
  fn default_severity(&self) -> Severity {
    Severity::Warning
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, body, upper) = crate::stmt_body_upper(stmt, source);
    if !upper.trim_start().starts_with("REINDEX") {
      return;
    }
    if upper.contains("CONCURRENTLY") {
      return;
    } // separate rule sql214 handles that combo
    let prelude_clean = strip_noise_and_dollar(&source[..start]);
    let prelude = prelude_clean.to_ascii_uppercase();
    let begins = count_kw(&prelude, "BEGIN") + count_phrase(&prelude, "START TRANSACTION");
    let closes = count_kw_prev_next(&prelude, "COMMIT", &["ON"], &["PREPARED"])
      + count_kw_excluding(&prelude, "ROLLBACK", &["TO", "PREPARED"]);
    if begins <= closes {
      return;
    }
    let lead = body.len() - body.trim_start().len();
    let abs_s = start + lead;
    let abs_e = abs_s + "REINDEX".len();
    out.push(Diagnostic {
      code: "sql296",
      severity: Severity::Warning,
      message: "REINDEX inside transaction -- holds AccessExclusiveLock until COMMIT; run outside BEGIN or use REINDEX CONCURRENTLY (PG12+)".into(),
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

fn count_kw_excluding(s: &str, needle: &str, excluded: &[&str]) -> usize {
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
    let after_pos = at + needle.len();
    let after_ok = after_pos >= bytes.len()
      || !{
        let p = bytes[after_pos] as char;
        p.is_ascii_alphanumeric() || p == '_'
      };
    if before_ok && after_ok {
      let mut k = after_pos;
      while k < bytes.len() && bytes[k].is_ascii_whitespace() {
        k += 1
      }
      let after_text = &s[k..];
      let is_excluded = excluded.iter().any(|ex| {
        let elen = ex.len();
        after_text.len() >= elen
          && after_text[..elen].eq_ignore_ascii_case(ex)
          && (after_text.len() == elen
            || !{
              let p = after_text.as_bytes()[elen] as char;
              p.is_ascii_alphanumeric() || p == '_'
            })
      });
      if !is_excluded {
        n += 1
      }
    }
    from = after_pos;
  }
  n
}

fn count_kw_prev_next(s: &str, needle: &str, excluded_prev: &[&str], excluded_next: &[&str]) -> usize {
  let bytes = s.as_bytes();
  let nl = needle.len();
  let n = bytes.len();
  let mut count = 0usize;
  let mut i = 0usize;
  while i + nl <= n {
    if &s[i..i + nl] == needle {
      let prev_ok = i == 0
        || !{
          let p = bytes[i - 1] as char;
          p.is_ascii_alphanumeric() || p == '_'
        };
      let next_ok_kw = i + nl == n
        || !{
          let p = bytes[i + nl] as char;
          p.is_ascii_alphanumeric() || p == '_'
        };
      if prev_ok && next_ok_kw {
        let mut p = i;
        while p > 0 && bytes[p - 1].is_ascii_whitespace() {
          p -= 1
        }
        let word_end = p;
        while p > 0 && (bytes[p - 1].is_ascii_alphanumeric() || bytes[p - 1] == b'_') {
          p -= 1
        }
        let prev_word = &s[p..word_end];
        let prev_excluded = excluded_prev.iter().any(|w| prev_word.eq_ignore_ascii_case(w));
        let mut k = i + nl;
        while k < n && bytes[k].is_ascii_whitespace() {
          k += 1
        }
        let after = &s[k..];
        let next_excluded = excluded_next.iter().any(|ex| {
          let elen = ex.len();
          after.len() >= elen
            && after[..elen].eq_ignore_ascii_case(ex)
            && (after.len() == elen
              || !{
                let p = after.as_bytes()[elen] as char;
                p.is_ascii_alphanumeric() || p == '_'
              })
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
