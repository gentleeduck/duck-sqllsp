//! sql037: `SELECT ... INTO var [, var2]` row-shape doesn't match the
//! SELECT projection count.
//!
//! Postgres raises `query has too many/few columns` at runtime. Catch
//! at edit time by counting projection commas vs INTO variable commas.

use crate::{Diagnostic, LintRule, Severity};
use crate::textutil::is_word;
use dsl_catalog::Catalog;
use dsl_parse::{Statement, StatementKind};
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql037"
  }
  fn default_severity(&self) -> Severity {
    Severity::Error
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    if !matches!(stmt.kind, StatementKind::Unknown { .. }) {
      return;
    }
    let (start, body, upper) = crate::stmt_body_upper(stmt, source);
    if !upper.contains("CREATE") || !upper.contains("FUNCTION") {
      return;
    }
    let Some(body_text) = dollar_body(body) else { return };
    let upper_body = body_text.to_ascii_uppercase();
    let stripped = strip_comments(&upper_body);
    let bytes = stripped.as_bytes();
    let n = bytes.len();

    // Find every `SELECT ... INTO ... FROM` (or `... ;`). For each,
    // count projection items + INTO targets.
    let mut i = 0;
    while i + 6 <= n {
      if &stripped[i..i + 6] == "SELECT" {
        let prev_ok = i == 0 || !is_word(bytes[i - 1] as char);
        let next_ok = i + 6 == n || !is_word(bytes[i + 6] as char);
        if prev_ok && next_ok {
          // Find INTO and projection extent.
          let select_end = find_select_end(bytes, i + 6);
          let into_idx = find_into(bytes, i + 6, select_end);
          if let Some(into_pos) = into_idx {
            let proj = &stripped[i + 6..into_pos];
            let into_body_end = find_into_end(bytes, into_pos + 4, select_end);
            let into_body = &stripped[into_pos + 4..into_body_end];
            let proj_count = top_level_comma_count(proj) + 1;
            let into_count = top_level_comma_count(into_body) + 1;
            // Skip the `*` / single-row shorthand case --
            // `SELECT * INTO row` is legal when row is composite.
            let proj_trim = proj.trim();
            if proj_trim != "*" && proj_count != into_count {
              let base = source.find(body_text).unwrap_or(start);
              let abs_start = base + i;
              let abs_end = base + into_body_end;
              out.push(Diagnostic {
                code: "sql037",
                severity: Severity::Error,
                message: format!("SELECT INTO shape mismatch: {proj_count} projection(s) vs {into_count} target(s)"),
                range: crate::range_at(abs_start, abs_end),
              });
            }
          }
        }
        i += 6;
      } else {
        i += 1;
      }
    }
  }
}

fn find_select_end(bytes: &[u8], from: usize) -> usize {
  let n = bytes.len();
  let mut i = from;
  let mut depth = 0i32;
  while i < n {
    match bytes[i] {
      b'(' => depth += 1,
      b')' => {
        if depth == 0 {
          return i;
        }
        depth -= 1;
      },
      b';' if depth == 0 => return i,
      b'\'' => {
        i += 1;
        while i < n && bytes[i] != b'\'' {
          i += 1;
        }
      },
      _ => {},
    }
    i += 1;
  }
  n
}

fn find_into(bytes: &[u8], from: usize, end: usize) -> Option<usize> {
  let n = end;
  let mut i = from;
  let mut depth = 0i32;
  while i + 4 <= n {
    match bytes[i] {
      b'(' => depth += 1,
      b')' => depth -= 1,
      b'\'' => {
        i += 1;
        while i < n && bytes[i] != b'\'' {
          i += 1;
        }
      },
      _ => {},
    }
    if depth == 0 && bytes[i..i + 4].eq_ignore_ascii_case(b"INTO") {
      let prev_ok = i == 0 || !is_word(bytes[i - 1] as char);
      let next_ok = i + 4 == n || !is_word(bytes[i + 4] as char);
      if prev_ok && next_ok {
        return Some(i);
      }
    }
    i += 1;
  }
  None
}

fn find_into_end(bytes: &[u8], from: usize, end: usize) -> usize {
  let n = end;
  let mut i = from;
  let mut depth = 0i32;
  while i < n {
    match bytes[i] {
      b'(' => depth += 1,
      b')' => {
        if depth == 0 {
          return i;
        }
        depth -= 1;
      },
      b';' if depth == 0 => return i,
      b'\'' => {
        i += 1;
        while i < n && bytes[i] != b'\'' {
          i += 1;
        }
      },
      _ => {},
    }
    if depth == 0
      && i + 4 <= n
      && bytes[i..i + 4].eq_ignore_ascii_case(b"FROM")
      && (i == 0 || !is_word(bytes[i - 1] as char))
      && (i + 4 == n || !is_word(bytes[i + 4] as char))
    {
      return i;
    }
    i += 1;
  }
  n
}

fn top_level_comma_count(s: &str) -> usize {
  let bytes = s.as_bytes();
  let n = bytes.len();
  let mut count = 0usize;
  let mut depth = 0i32;
  let mut i = 0;
  while i < n {
    match bytes[i] {
      b'(' => depth += 1,
      b')' => depth -= 1,
      b'\'' => {
        i += 1;
        while i < n && bytes[i] != b'\'' {
          i += 1;
        }
      },
      b',' if depth == 0 => count += 1,
      _ => {},
    }
    i += 1;
  }
  count
}

fn dollar_body(text: &str) -> Option<&str> {
  let start = text.find("$$")?;
  let after = start + 2;
  let end_rel = text[after..].find("$$")?;
  Some(&text[after..after + end_rel])
}

fn strip_comments(s: &str) -> String {
  let mut out = String::with_capacity(s.len());
  let bytes = s.as_bytes();
  let n = bytes.len();
  let mut i = 0;
  while i < n {
    if i + 1 < n && bytes[i] == b'-' && bytes[i + 1] == b'-' {
      while i < n && bytes[i] != b'\n' {
        i += 1;
      }
    } else if i + 1 < n && bytes[i] == b'/' && bytes[i + 1] == b'*' {
      i += 2;
      while i + 1 < n && !(bytes[i] == b'*' && bytes[i + 1] == b'/') {
        i += 1;
      }
      i = (i + 2).min(n);
    } else {
      out.push(bytes[i] as char);
      i += 1;
    }
  }
  out
}

