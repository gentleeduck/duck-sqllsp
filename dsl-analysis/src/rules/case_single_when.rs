//! sql058: `CASE WHEN ... THEN ... ELSE ... END` with exactly one WHEN
//! arm. PL/pgSQL `IF`, or PG's `coalesce`/`nullif`/`iif`-like helpers
//! read better. Hint.

use crate::{Diagnostic, LintRule, Severity};
use crate::textutil::is_word;
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql058"
  }
  fn default_severity(&self) -> Severity {
    Severity::Hint
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, body) = crate::stmt_body(stmt, source);
    let stripped = strip_quoted_and_comments(body);
    let upper = stripped.to_ascii_uppercase();
    let bytes = upper.as_bytes();
    let n = bytes.len();

    // Walk each CASE...END block. Count the WHEN keywords inside.
    let mut i = 0;
    while i + 4 <= n {
      if &upper[i..i + 4] == "CASE"
        && (i == 0 || !is_word(bytes[i - 1] as char))
        && (i + 4 == n || !is_word(bytes[i + 4] as char))
        && let Some(end_pos) = find_matching_end(&upper, i + 4)
      {
        let block = &upper[i + 4..end_pos];
        let count = count_whens(block);
        // Only fire on single-WHEN with NO ELSE -- the truly trivial
        // form that silently NULL-fills unmatched rows. CASE WHEN x
        // THEN a ELSE b END is the canonical if-else expression
        // and is fine.
        let has_else = block_has_else(block);
        if count == 1 && !has_else {
          let abs_start = start + i;
          let abs_end = start + i + 4;
          out.push(Diagnostic {
            code: "sql058",
            severity: Severity::Hint,
            message: "CASE with a single WHEN -- consider `coalesce`/`nullif` or an IF block for readability".into(),
            range: crate::range_at(abs_start, abs_end),
          });
          return;
        }
        i = end_pos;
        continue;
      }
      i += 1;
    }
  }
}

fn find_matching_end(upper: &str, from: usize) -> Option<usize> {
  let bytes = upper.as_bytes();
  let n = bytes.len();
  let mut depth = 1i32;
  let mut i = from;
  while i + 3 <= n {
    if i + 4 <= n
      && &upper[i..i + 4] == "CASE"
      && (i == 0 || !is_word(bytes[i - 1] as char))
      && !is_word(bytes[i + 4] as char)
    {
      depth += 1;
      i += 4;
      continue;
    }
    if &upper[i..i + 3] == "END"
      && (i == 0 || !is_word(bytes[i - 1] as char))
      && (i + 3 == n || !is_word(bytes[i + 3] as char))
    {
      depth -= 1;
      if depth == 0 {
        return Some(i);
      }
      i += 3;
      continue;
    }
    i += 1;
  }
  None
}

/// Whole-word ELSE inside the CASE body (already uppercase).
fn block_has_else(block: &str) -> bool {
  let bytes = block.as_bytes();
  let n = bytes.len();
  let mut i = 0;
  while i + 4 <= n {
    if &block[i..i + 4] == "ELSE"
      && (i == 0 || !is_word(bytes[i - 1] as char))
      && (i + 4 == n || !is_word(bytes[i + 4] as char))
    {
      return true;
    }
    i += 1;
  }
  false
}

fn count_whens(block: &str) -> usize {
  let bytes = block.as_bytes();
  let n = bytes.len();
  let mut count = 0usize;
  let mut i = 0;
  while i + 4 <= n {
    if &block[i..i + 4] == "WHEN" && (i == 0 || !is_word(bytes[i - 1] as char)) && !is_word(bytes[i + 4] as char) {
      count += 1;
      i += 4;
      continue;
    }
    i += 1;
  }
  count
}

/// Space-preserving variant -- output indices map 1:1 to input.
fn strip_quoted_and_comments(s: &str) -> String {
  let mut out = String::with_capacity(s.len());
  let bytes = s.as_bytes();
  let n = bytes.len();
  let mut i = 0;
  while i < n {
    if i + 1 < n && bytes[i] == b'-' && bytes[i + 1] == b'-' {
      while i < n && bytes[i] != b'\n' {
        out.push(' ');
        i += 1;
      }
    } else if i + 1 < n && bytes[i] == b'/' && bytes[i + 1] == b'*' {
      out.push(' ');
      out.push(' ');
      i += 2;
      while i + 1 < n && !(bytes[i] == b'*' && bytes[i + 1] == b'/') {
        out.push(' ');
        i += 1;
      }
      if i + 1 < n {
        out.push(' ');
        out.push(' ');
        i += 2;
      } else {
        while i < n {
          out.push(' ');
          i += 1;
        }
      }
    } else if bytes[i] == b'\'' {
      out.push(' ');
      i += 1;
      while i < n && bytes[i] != b'\'' {
        out.push(' ');
        i += 1;
      }
      if i < n {
        out.push(' ');
        i += 1;
      }
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

