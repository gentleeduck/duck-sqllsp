//! sql766: `JSON_TABLE(... COLUMNS (a ..., a ...))` -- the same output
//! column name used twice. PostgreSQL rejects duplicate JSON_TABLE
//! column names. Replaces the spec's original sql766 (empty COLUMNS
//! list), which pg_query rejects as a hard parse error before any
//! LintRule sees it -- verified empirically.

use crate::clause_scan::{find_clause, is_word, split_top_level};
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql766"
  }
  fn default_severity(&self) -> Severity {
    Severity::Warning
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, body, upper) = crate::stmt_body_upper(stmt, source);
    let ub = upper.as_bytes();
    let mut i = 0usize;
    while i < ub.len() {
      if !word_at(ub, i, b"JSON_TABLE") {
        i += 1;
        continue;
      }
      let p = skip_ws(ub, i + "JSON_TABLE".len());
      if ub.get(p) != Some(&b'(') {
        i += "JSON_TABLE".len();
        continue;
      }
      let Some(close) = match_paren(ub, p) else { break };
      // Search strictly INSIDE the call's own parens so `COLUMNS`
      // registers at depth 0 relative to this inner slice.
      let inner = &upper[p + 1..close];
      if let Some(cols_rel) = find_clause(inner.as_bytes(), b"COLUMNS") {
        let cols_abs = p + 1 + cols_rel;
        let paren_pos = skip_ws(ub, cols_abs + "COLUMNS".len());
        if ub.get(paren_pos) == Some(&b'(')
          && let Some(cols_close) = match_paren(ub, paren_pos)
        {
          let list = &body[paren_pos + 1..cols_close];
          let mut seen: Vec<String> = Vec::new();
          for (entry, off) in split_top_level(list) {
            let Some(name) = leading_ident(entry) else { continue };
            if seen.iter().any(|s| s.eq_ignore_ascii_case(&name)) {
              let lead = entry.len() - entry.trim_start().len();
              let abs = paren_pos + 1 + off + lead;
              out.push(Diagnostic {
                code: "sql766",
                severity: Severity::Warning,
                message: format!("JSON_TABLE output column `{name}` is defined more than once"),
                range: crate::range_at(start + abs, start + abs + name.len()),
              });
            } else {
              seen.push(name);
            }
          }
        }
      }
      i = close + 1;
    }
  }
}

/// Leading identifier of a JSON_TABLE column-def entry (bare or
/// double-quoted), ignoring the rest of the entry (`int PATH '...'`).
fn leading_ident(s: &str) -> Option<String> {
  let t = s.trim_start();
  if let Some(rest) = t.strip_prefix('"') {
    let end = rest.find('"')?;
    return Some(rest[..end].to_string());
  }
  let end = t.find(|c: char| !(c.is_alphanumeric() || c == '_')).unwrap_or(t.len());
  if end == 0 {
    return None;
  }
  Some(t[..end].to_string())
}

fn match_paren(ub: &[u8], open: usize) -> Option<usize> {
  let mut depth = 0i32;
  let mut i = open;
  while i < ub.len() {
    match ub[i] {
      b'(' => depth += 1,
      b')' => {
        depth -= 1;
        if depth == 0 {
          return Some(i);
        }
      },
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
