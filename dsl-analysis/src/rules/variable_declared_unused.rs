//! sql804: a PL/pgSQL `DECLARE x type;` variable that's never
//! referenced anywhere after `BEGIN`. Classic dead-code smell. Only
//! handles the simple single top-level `DECLARE ... BEGIN` block
//! shape; nested DECLARE blocks are out of scope.

use crate::clause_scan::is_word;
use crate::textutil::contains_word;
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql804"
  }
  fn default_severity(&self) -> Severity {
    Severity::Hint
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, body, upper) = crate::stmt_body_upper(stmt, source);
    let ub = upper.as_bytes();
    let Some(decl_rel) = find_word_from(ub, 0, b"DECLARE") else { return };
    let Some(begin_rel) = find_word_from(ub, decl_rel, b"BEGIN") else { return };
    let decls = &body[decl_rel + 7..begin_rel];
    let decls_upper = &upper[decl_rel + 7..begin_rel];
    let after_begin = &upper[begin_rel + 5..];
    for (name_start_rel, name) in declared_names(decls, decls_upper) {
      let name_upper = name.to_ascii_uppercase();
      if !contains_word(after_begin, &name_upper) {
        let abs = decl_rel + 7 + name_start_rel;
        out.push(Diagnostic {
          code: "sql804",
          severity: Severity::Hint,
          message: format!("declared variable `{name}` is never referenced"),
          range: crate::range_at(start + abs, start + abs + name.len()),
        });
      }
    }
  }
}

/// Walk `;`-separated declaration entries in a DECLARE section,
/// yielding `(offset_of_name_within_decls, name)` for each one whose
/// first token looks like a bare identifier.
fn declared_names<'a>(decls: &'a str, decls_upper: &str) -> Vec<(usize, &'a str)> {
  let mut out = Vec::new();
  let bytes = decls_upper.as_bytes();
  let mut i = 0usize;
  let mut entry_start = 0usize;
  let mut depth = 0i32;
  while i <= bytes.len() {
    let at_end = i == bytes.len();
    let c = if at_end { b';' } else { bytes[i] };
    match c {
      b'(' => depth += 1,
      b')' => depth -= 1,
      b';' if depth == 0 => {
        let raw = &decls[entry_start..i.min(decls.len())];
        let entry = raw.trim_start();
        let lead = raw.len() - entry.len();
        let end = entry.find(|ch: char| !(ch.is_alphanumeric() || ch == '_')).unwrap_or(entry.len());
        if end > 0 {
          out.push((entry_start + lead, &entry[..end]));
        }
        entry_start = i + 1;
      },
      _ => {},
    }
    i += 1;
  }
  out
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
