//! sql141: `ALTER TYPE x ADD VALUE 'y'` cannot run inside an explicit
//! transaction block -- PG aborts the statement.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql141"
  }
  fn default_severity(&self) -> Severity {
    Severity::Error
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let start: usize = u32::from(stmt.range.start()) as usize;
    let end: usize = (u32::from(stmt.range.end()) as usize).min(source.len());
    let body = &source[start..end];
    let upper = body.to_ascii_uppercase();
    let trimmed = upper.trim_start();
    if !trimmed.starts_with("ALTER TYPE") {
      return;
    }
    if !trimmed.contains("ADD VALUE") {
      return;
    }
    // Inside a BEGIN..COMMIT window? Strip comments + strings +
    // $$...$$ blocks first so a `BEGIN` inside a PL/pgSQL function
    // body (or `BEGIN ATOMIC` in a SQL-language function) doesn't
    // count as an open transaction.
    let before_clean = strip_noise_and_dollar(&source[..start]);
    let before_upper = before_clean.to_ascii_uppercase();
    let begins = count_word(&before_upper, "BEGIN") + count_word(&before_upper, "START TRANSACTION");
    let commits = count_with_prev_exclude(&before_upper, "COMMIT", &["ON"], &["PREPARED"])
      + count_word_excluding(&before_upper, "ROLLBACK", &["TO", "PREPARED"]);
    if begins <= commits {
      return;
    }
    let leading = upper.len() - trimmed.len();
    let abs_start = start + leading;
    let abs_end = abs_start + 10;
    out.push(Diagnostic {
      code: "sql141",
      severity: Severity::Error,
      message: "ALTER TYPE ... ADD VALUE cannot run inside an explicit transaction -- run it outside BEGIN..COMMIT"
        .into(),
      range: text_size::TextRange::new((abs_start as u32).into(), (abs_end as u32).into()),
    });
  }
}

fn count_word(haystack: &str, needle: &str) -> usize {
  let h = haystack.as_bytes();
  let n = h.len();
  let w = needle.len();
  let mut c = 0;
  let mut i = 0;
  while i + w <= n {
    if &haystack[i..i + w] == needle {
      let prev_ok = i == 0 || !is_word(h[i - 1] as char);
      let next_ok = i + w == n || !is_word(h[i + w] as char);
      if prev_ok && next_ok {
        c += 1;
        i += w;
        continue;
      }
    }
    i += 1;
  }
  c
}

fn is_word(c: char) -> bool {
  c.is_alphanumeric() || c == '_'
}

fn count_with_prev_exclude(haystack: &str, needle: &str, excluded_prev: &[&str], excluded_next: &[&str]) -> usize {
  let h = haystack.as_bytes();
  let n = h.len();
  let w = needle.len();
  let mut c = 0;
  let mut i = 0;
  while i + w <= n {
    if &haystack[i..i + w] == needle {
      let prev_ok = i == 0 || !is_word(h[i - 1] as char);
      let next_ok = i + w == n || !is_word(h[i + w] as char);
      if prev_ok && next_ok {
        let mut p = i;
        while p > 0 && h[p - 1].is_ascii_whitespace() { p -= 1 }
        let word_end = p;
        while p > 0 && is_word(h[p - 1] as char) { p -= 1 }
        let prev_word = &haystack[p..word_end];
        let prev_excluded = excluded_prev.iter().any(|wd| prev_word.eq_ignore_ascii_case(wd));
        let mut k = i + w;
        while k < n && h[k].is_ascii_whitespace() { k += 1 }
        let after = &haystack[k..];
        let next_excluded = excluded_next.iter().any(|ex| {
          let elen = ex.len();
          after.len() >= elen && after[..elen].eq_ignore_ascii_case(ex)
            && (after.len() == elen || !is_word(after.as_bytes()[elen] as char))
        });
        if !prev_excluded && !next_excluded { c += 1 }
      }
    }
    i += 1;
  }
  c
}

fn count_word_excluding(haystack: &str, needle: &str, excluded: &[&str]) -> usize {
  let h = haystack.as_bytes();
  let n = h.len();
  let w = needle.len();
  let mut c = 0;
  let mut i = 0;
  while i + w <= n {
    if &haystack[i..i + w] == needle {
      let prev_ok = i == 0 || !is_word(h[i - 1] as char);
      let next_ok = i + w == n || !is_word(h[i + w] as char);
      if prev_ok && next_ok {
        let mut k = i + w;
        while k < n && h[k].is_ascii_whitespace() { k += 1 }
        let after = &haystack[k..];
        let is_excluded = excluded.iter().any(|ex| {
          let elen = ex.len();
          after.len() >= elen && after[..elen].eq_ignore_ascii_case(ex)
            && (after.len() == elen || !is_word(after.as_bytes()[elen] as char))
        });
        if !is_excluded { c += 1 }
        i += w;
        continue;
      }
    }
    i += 1;
  }
  c
}

fn strip_noise_and_dollar(s: &str) -> String {
  let mut out: Vec<u8> = s.as_bytes().to_vec();
  let n = out.len();
  let mut i = 0usize;
  while i < n {
    if i + 1 < n && out[i] == b'-' && out[i + 1] == b'-' {
      while i < n && out[i] != b'\n' { out[i] = b' '; i += 1 }
      continue;
    }
    if i + 1 < n && out[i] == b'/' && out[i + 1] == b'*' {
      let mut depth = 1u32;
      out[i] = b' '; out[i + 1] = b' '; i += 2;
      while i + 1 < n && depth > 0 {
        if out[i] == b'/' && out[i + 1] == b'*' { depth += 1; out[i] = b' '; out[i + 1] = b' '; i += 2; }
        else if out[i] == b'*' && out[i + 1] == b'/' { depth -= 1; out[i] = b' '; out[i + 1] = b' '; i += 2; }
        else { out[i] = b' '; i += 1; }
      }
      continue;
    }
    if out[i] == b'\'' {
      out[i] = b' '; i += 1;
      while i < n && out[i] != b'\'' { out[i] = b' '; i += 1 }
      if i < n { out[i] = b' '; i += 1 }
      continue;
    }
    if out[i] == b'$' {
      let mut k = i + 1;
      while k < n && (out[k].is_ascii_alphanumeric() || out[k] == b'_') { k += 1 }
      if k < n && out[k] == b'$' {
        let tag_bytes = &out[i + 1..k];
        let closer: Vec<u8> = std::iter::once(b'$').chain(tag_bytes.iter().copied()).chain(std::iter::once(b'$')).collect();
        let closer_len = closer.len();
        for j in i..k + 1 { out[j] = b' '; }
        i = k + 1;
        while i + closer_len <= n {
          if out[i..i + closer_len] == *closer { break }
          out[i] = b' ';
          i += 1;
        }
        if i + closer_len <= n {
          for j in i..i + closer_len { out[j] = b' '; }
          i += closer_len;
        }
        continue;
      }
    }
    i += 1;
  }
  String::from_utf8(out).unwrap_or_else(|_| s.to_string())
}
