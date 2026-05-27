//! sql112: `generate_series(...)` in a FROM clause without an alias
//! ends up named `generate_series` which makes queries hard to read.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql112"
  }
  fn default_severity(&self) -> Severity {
    Severity::Hint
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let start: usize = u32::from(stmt.range.start()) as usize;
    let end: usize = (u32::from(stmt.range.end()) as usize).min(source.len());
    let raw = &source[start..end];
    // Strip line+block comments so a leading `-- header` comment
    // doesn't mask the real CREATE/ALTER TABLE anchor.
    let raw_clean_owned = strip_dollar_and_noise(raw);
    let raw_upper = raw_clean_owned.to_ascii_uppercase();
    // Skip when this is a CREATE TABLE / ALTER TABLE -- generate_series
    // in a column DEFAULT clause is a value expression, not a FROM
    // source. Even if PG would reject the DDL, that's a different rule.
    let raw_trim = raw_upper.trim_start();
    if raw_trim.starts_with("CREATE TABLE")
      || raw_trim.starts_with("CREATE TEMP TABLE")
      || raw_trim.starts_with("CREATE TEMPORARY TABLE")
      || raw_trim.starts_with("CREATE UNLOGGED TABLE")
      || raw_trim.starts_with("ALTER TABLE")
    {
      return;
    }
    // Strip $$...$$ dollar-quoted blocks + comments + strings so a
    // `generate_series` call inside a CREATE FUNCTION body doesn't
    // false-fire (the function body is opaque to this text scan).
    let body_owned = strip_dollar_and_noise(raw);
    let body = body_owned.as_str();
    let upper = body.to_ascii_uppercase();
    let bytes = upper.as_bytes();
    let n = bytes.len();
    let mut i = 0;
    while i + 15 <= n {
      if &upper[i..i + 15] == "GENERATE_SERIES" && (i == 0 || !is_word(bytes[i - 1] as char)) {
        // Only fire when generate_series is used as a FROM/JOIN source.
        // The preceding token (skipping whitespace) should be FROM /
        // JOIN / `,`. In SELECT lists (`SELECT generate_series(...)`)
        // and in ARRAY(SELECT generate_series(...)) constructors, the
        // function is a value expression -- no alias needed.
        let mut p = i;
        while p > 0 && bytes[p - 1].is_ascii_whitespace() {
          p -= 1
        }
        let word_end = p;
        while p > 0 && (bytes[p - 1].is_ascii_alphanumeric() || bytes[p - 1] == b'_') {
          p -= 1
        }
        let prev_word = &upper[p..word_end];
        let comma_before = word_end > 0 && bytes[word_end - 1] == b',';
        let is_from_source = matches!(prev_word, "FROM" | "JOIN" | "LATERAL") || comma_before;
        if !is_from_source {
          i += 1;
          continue;
        }
        let mut j = i + 15;
        while j < n && bytes[j].is_ascii_whitespace() {
          j += 1;
        }
        if j < n && bytes[j] == b'(' {
          // Find matching close paren accounting for nesting.
          let mut depth = 0i32;
          let mut k = j;
          while k < n {
            match bytes[k] {
              b'(' => depth += 1,
              b')' => {
                depth -= 1;
                if depth == 0 {
                  break;
                }
              },
              _ => {},
            }
            k += 1;
          }
          if k >= n {
            return;
          }
          // After ), skip ws -- if next token is AS or an
          // identifier (not a keyword like WHERE/CROSS/...),
          // there's an alias.
          let mut m = k + 1;
          while m < n && bytes[m].is_ascii_whitespace() {
            m += 1;
          }
          let alias_ok = if m + 2 <= n && &upper[m..m + 2] == "AS" && (m + 2 == n || !is_word(bytes[m + 2] as char)) {
            true
          } else if m < n && is_word(bytes[m] as char) {
            // Identifier follows -- check it's not a clause kw.
            let id_start = m;
            let mut id_end = m;
            while id_end < n && is_word(bytes[id_end] as char) {
              id_end += 1;
            }
            let id = &upper[id_start..id_end];
            !matches!(
              id,
              "WHERE"
                | "CROSS"
                | "JOIN"
                | "INNER"
                | "LEFT"
                | "RIGHT"
                | "FULL"
                | "ON"
                | "USING"
                | "GROUP"
                | "ORDER"
                | "LIMIT"
                | "OFFSET"
                | "HAVING"
                | "WINDOW"
                | "UNION"
                | "INTERSECT"
                | "EXCEPT"
                | "FETCH"
                | "FOR"
                | "RETURNING"
            )
          } else {
            false
          };
          if !alias_ok {
            let abs_start = start + i;
            let abs_end = start + k + 1;
            out.push(Diagnostic {
              code: "sql112",
              severity: Severity::Hint,
              message: "generate_series in FROM without alias -- queries are clearer with `AS series(n)`".into(),
              range: text_size::TextRange::new((abs_start as u32).into(), (abs_end as u32).into()),
            });
            return;
          }
          i = k + 1;
          continue;
        }
      }
      i += 1;
    }
  }
}

fn is_word(c: char) -> bool {
  c.is_alphanumeric() || c == '_'
}

fn strip_dollar_and_noise(s: &str) -> String {
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
