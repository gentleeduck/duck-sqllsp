//! sql459: `COUNT(col)` where `col` is declared NOT NULL -- the
//! expression is identical to `COUNT(*)`. Both yield the same row
//! count, but `COUNT(*)` is the conventional spelling and lets the
//! planner skip column-extraction work. (sql174 handles the inverse
//! "nullable column" case where COUNT(col) silently skips NULL rows
//! -- that's a semantic bug; this one is only a clarity issue.)
//!
//! Does NOT fire on COUNT(DISTINCT col) -- semantic difference.

use crate::clause_scan::is_word;
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;
use text_size::TextRange;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql459"
  }
  fn default_severity(&self) -> Severity {
    Severity::Hint
  }

  fn check(&self, source: &str, stmt: &Statement, scope: &Scope, catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    if scope.is_empty() {
      return;
    }
    let (start, raw) = crate::stmt_body(stmt, source);
    let cleaned = crate::textutil::strip_noise_full(raw);
    let upper = cleaned.to_ascii_uppercase();
    let ub = upper.as_bytes();
    let bytes = cleaned.as_bytes();
    let n = ub.len();
    let needle = b"COUNT";
    let m = needle.len();
    let mut i = 0usize;
    while i + m <= n {
      if !(&ub[i..i + m] == needle && (i == 0 || !is_word(ub[i - 1] as char)) && (i + m == n || !is_word(ub[i + m] as char))) {
        i += 1;
        continue;
      }
      let mut k = i + m;
      while k < n && bytes[k].is_ascii_whitespace() {
        k += 1;
      }
      if k >= n || bytes[k] != b'(' {
        i += m;
        continue;
      }
      let Some(close) = match_paren(bytes, k, n) else {
        i += m;
        continue;
      };
      let inner = cleaned[k + 1..close].trim();
      // Skip COUNT(*), COUNT(DISTINCT ...), COUNT(ALL ...), or
      // anything more complex than a bare identifier.
      if inner == "*"
        || inner.to_ascii_uppercase().starts_with("DISTINCT ")
        || inner.to_ascii_uppercase().starts_with("ALL ")
      {
        i = close + 1;
        continue;
      }
      let Some((qualifier, name)) = parse_bare_ident(inner) else {
        i = close + 1;
        continue;
      };
      if let Some((data_type, nullable)) = resolve_column(scope, catalog, qualifier, name) {
        let _ = data_type;
        if !nullable {
          let abs_s = start + i;
          let abs_e = start + close + 1;
          out.push(Diagnostic {
            code: "sql459",
            severity: Severity::Hint,
            message: format!(
              "`COUNT({name})` on NOT NULL column is identical to `COUNT(*)` -- prefer `COUNT(*)` (conventional, skips column-extraction work)"
            ),
            range: TextRange::new((abs_s as u32).into(), (abs_e as u32).into()),
          });
        }
      }
      i = close + 1;
    }
  }
}

fn parse_bare_ident(s: &str) -> Option<(Option<&str>, &str)> {
  let t = s.trim();
  if t.is_empty() {
    return None;
  }
  for c in t.chars() {
    if !(c.is_alphanumeric() || c == '_' || c == '.') {
      return None;
    }
  }
  if t.starts_with('.') || t.ends_with('.') || t.contains("..") {
    return None;
  }
  if t.chars().all(|c| c.is_ascii_digit() || c == '.') {
    return None;
  }
  if let Some(dot) = t.rfind('.') {
    Some((Some(&t[..dot]), &t[dot + 1..]))
  } else {
    Some((None, t))
  }
}

fn resolve_column(scope: &Scope, catalog: &Catalog, qualifier: Option<&str>, name: &str) -> Option<(String, bool)> {
  let lname = name.to_ascii_lowercase();
  for binding in scope.tables() {
    if let Some(q) = qualifier {
      let key_matches = binding.alias.eq_ignore_ascii_case(q) || binding.table.name.eq_ignore_ascii_case(q);
      if !key_matches {
        continue;
      }
    }
    if let Some(t) = catalog.find_table(binding.table.schema.as_deref(), &binding.table.name) {
      for col in &t.columns {
        if col.name.eq_ignore_ascii_case(&lname) {
          return Some((col.data_type.clone(), col.nullable));
        }
      }
    }
  }
  None
}

fn match_paren(bytes: &[u8], open: usize, end: usize) -> Option<usize> {
  let mut depth: i32 = 0;
  let mut i = open;
  while i < end {
    let c = bytes[i];
    if c == b'\'' {
      i += 1;
      while i < end && bytes[i] != b'\'' {
        i += 1;
      }
      i = (i + 1).min(end);
      continue;
    }
    if c == b'(' {
      depth += 1;
    } else if c == b')' {
      depth -= 1;
      if depth == 0 {
        return Some(i);
      }
    }
    i += 1;
  }
  None
}
