//! sql767: `col IS JSON` where the catalog already types `col` as
//! `json` or `jsonb` -- always true, the predicate is redundant.
//! `IS JSON OBJECT`/`ARRAY`/`SCALAR` are narrower checks and are left
//! alone (being jsonb-typed doesn't guarantee a specific JSON kind).

use crate::clause_scan::is_word;
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql767"
  }
  fn default_severity(&self) -> Severity {
    Severity::Hint
  }

  fn check(&self, source: &str, stmt: &Statement, scope: &Scope, catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    if scope.is_empty() || catalog.tables().next().is_none() {
      return;
    }
    let (start, body) = crate::stmt_body(stmt, source);
    let cleaned = crate::textutil::strip_noise_full(body);
    let bytes = cleaned.as_bytes();
    let n = bytes.len();
    let mut i = 0usize;
    while i < n {
      let Some(rel) = find_word_ci(bytes, i, b"IS") else { break };
      let after = skip_ws(bytes, rel + 2);
      if !word_at(bytes, after, b"JSON") {
        i = rel + 2;
        continue;
      }
      let after_json = skip_ws(bytes, after + 4);
      if word_at(bytes, after_json, b"OBJECT")
        || word_at(bytes, after_json, b"ARRAY")
        || word_at(bytes, after_json, b"SCALAR")
      {
        i = rel + 2;
        continue;
      }
      let mut left_end = rel;
      while left_end > 0 && bytes[left_end - 1].is_ascii_whitespace() {
        left_end -= 1;
      }
      let mut left_start = left_end;
      while left_start > 0 {
        let b = bytes[left_start - 1];
        if b.is_ascii_alphanumeric() || b == b'_' || b == b'.' {
          left_start -= 1;
        } else {
          break;
        }
      }
      let col_text = &cleaned[left_start..left_end];
      if !col_text.is_empty() {
        let (qualifier, name) = match col_text.split_once('.') {
          Some((q, nm)) => (Some(q), nm),
          None => (None, col_text),
        };
        if let Some(ty) = lookup_column_type(scope, catalog, qualifier, name) {
          let bare = ty.trim().to_ascii_lowercase();
          if bare == "json" || bare == "jsonb" {
            out.push(Diagnostic {
              code: "sql767",
              severity: Severity::Hint,
              message: format!("`{col_text} IS JSON` is always true -- column is already `{ty}`"),
              range: crate::range_at(start + left_start, start + after + 4),
            });
          }
        }
      }
      i = after + 4;
    }
  }
}

fn lookup_column_type(scope: &Scope, catalog: &Catalog, qualifier: Option<&str>, name: &str) -> Option<String> {
  if let Some(q) = qualifier {
    if let Some((schema, table)) = q.split_once('.')
      && let Some(t) = catalog.find_table(Some(schema), table)
    {
      return t.columns.iter().find(|c| c.name.eq_ignore_ascii_case(name)).map(|c| c.data_type.clone());
    }
    if let Some(b) = scope.get(q)
      && let Some(t) = catalog.find_table(b.table.schema.as_deref(), &b.table.name)
    {
      return t.columns.iter().find(|c| c.name.eq_ignore_ascii_case(name)).map(|c| c.data_type.clone());
    }
    return None;
  }
  let mut hit: Option<String> = None;
  for b in scope.tables() {
    let Some(t) = catalog.find_table(b.table.schema.as_deref(), &b.table.name) else { continue };
    for c in &t.columns {
      if c.name.eq_ignore_ascii_case(name) {
        if hit.is_some() {
          return None;
        }
        hit = Some(c.data_type.clone());
      }
    }
  }
  hit
}

fn find_word_ci(bytes: &[u8], from: usize, w: &[u8]) -> Option<usize> {
  let mut i = from;
  while i + w.len() <= bytes.len() {
    if bytes[i..i + w.len()].eq_ignore_ascii_case(w) && word_at(bytes, i, w) {
      return Some(i);
    }
    i += 1;
  }
  None
}

fn word_at(bytes: &[u8], i: usize, w: &[u8]) -> bool {
  i + w.len() <= bytes.len()
    && bytes[i..i + w.len()].eq_ignore_ascii_case(w)
    && (i == 0 || !is_word(bytes[i - 1] as char))
    && (i + w.len() == bytes.len() || !is_word(bytes[i + w.len()] as char))
}

fn skip_ws(bytes: &[u8], mut i: usize) -> usize {
  while i < bytes.len() && bytes[i].is_ascii_whitespace() {
    i += 1;
  }
  i
}
