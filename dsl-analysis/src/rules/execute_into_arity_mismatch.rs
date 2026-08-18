//! sql802: `EXECUTE '<literal SELECT>' INTO a, b` where the
//! statically-known SELECT-list column count doesn't match the number
//! of INTO targets. Only fires when the EXECUTE target is a single
//! plain string literal immediately followed by INTO (not `format()`/
//! concatenation, and not combined with USING) whose content is
//! itself statically a bare `SELECT <items> [FROM ...]`.

use crate::clause_scan::{find_clause, is_word, split_top_level};
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql802"
  }
  fn default_severity(&self) -> Severity {
    Severity::Warning
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, body, upper) = crate::stmt_body_upper(stmt, source);
    let ub = upper.as_bytes();
    let mut i = 0usize;
    while i < ub.len() {
      if !word_at(ub, i, b"EXECUTE") {
        i += 1;
        continue;
      }
      let p = skip_ws(ub, i + "EXECUTE".len());
      if ub.get(p) != Some(&b'\'') {
        i += "EXECUTE".len();
        continue;
      }
      let Some(lit_end) = find_string_literal_end(ub, p) else { break };
      let after_lit = skip_ws(ub, lit_end + 1);
      let Some(into_rel) = find_word_from(ub, after_lit, b"INTO") else {
        i = lit_end + 1;
        continue;
      };
      if into_rel != after_lit {
        // Something (e.g. USING) sits between the literal and INTO --
        // don't attempt arity counting in that shape.
        i = lit_end + 1;
        continue;
      }
      let literal_upper = &upper[p + 1..lit_end];
      let Some(select_cols) = static_select_column_count(literal_upper) else {
        i = lit_end + 1;
        continue;
      };
      let into_start = skip_ws(ub, into_rel + 4);
      let into_end = find_stmt_end(ub, into_start);
      let into_count =
        split_top_level(&body[into_start..into_end]).iter().filter(|(s, _)| !s.trim().is_empty()).count();
      if select_cols != into_count {
        out.push(Diagnostic {
          code: "sql802",
          severity: Severity::Warning,
          message: format!(
            "dynamic SELECT has {select_cols} column(s) but INTO has {into_count} target(s) -- raises \"wrong number of columns\" at runtime"
          ),
          range: crate::range_at(start + into_rel, start + into_end),
        });
      }
      i = into_end;
    }
  }
}

/// If `literal` (already uppercased) is statically a bare `SELECT
/// <items> [FROM ...]`, return the top-level item count. None
/// otherwise (doesn't start with SELECT, or the item list is empty).
fn static_select_column_count(literal: &str) -> Option<usize> {
  let t = literal.trim_start();
  let after = t.strip_prefix("SELECT")?;
  if after.starts_with(|c: char| c.is_alphanumeric() || c == '_') {
    return None;
  }
  let rest = after.trim_start();
  let end = find_clause(rest.as_bytes(), b"FROM").unwrap_or(rest.len());
  let items = rest[..end].trim();
  if items.is_empty() {
    return None;
  }
  Some(split_top_level(items).len())
}

fn find_string_literal_end(ub: &[u8], quote_pos: usize) -> Option<usize> {
  let mut i = quote_pos + 1;
  while i < ub.len() {
    if ub[i] == b'\'' {
      return Some(i);
    }
    i += 1;
  }
  None
}

fn find_stmt_end(ub: &[u8], from: usize) -> usize {
  let mut depth = 0i32;
  let mut i = from;
  while i < ub.len() {
    match ub[i] {
      b'(' => depth += 1,
      b')' => depth -= 1,
      b';' if depth == 0 => return i,
      b'\'' => {
        i += 1;
        while i < ub.len() && ub[i] != b'\'' {
          i += 1;
        }
      },
      _ => {},
    }
    i += 1;
  }
  ub.len()
}

fn find_word_from(ub: &[u8], from: usize, w: &[u8]) -> Option<usize> {
  let mut i = from;
  while i + w.len() <= ub.len() {
    if word_at(ub, i, w) {
      return Some(i);
    }
    i += 1;
  }
  None
}

fn word_at(ub: &[u8], i: usize, w: &[u8]) -> bool {
  i + w.len() <= ub.len()
    && &ub[i..i + w.len()] == w
    && (i == 0 || !is_word(ub[i - 1] as char))
    && (i + w.len() == ub.len() || !is_word(ub[i + w.len()] as char))
}

fn skip_ws(ub: &[u8], mut i: usize) -> usize {
  while i < ub.len() && ub[i].is_ascii_whitespace() {
    i += 1;
  }
  i
}
