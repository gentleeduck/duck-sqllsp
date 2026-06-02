//! sql470: `NOT (col IN (...))` / `NOT (col LIKE ...)` / `NOT (col
//! BETWEEN ...)` -- less idiomatic than `col NOT IN (...)` / `col
//! NOT LIKE ...` / `col NOT BETWEEN ...`. PG accepts all forms but
//! the explicit NOT-prefix is conventional and easier to scan.
//!
//! Pairs with sql469 which handles the NOT (IS NULL) variant.

use crate::clause_scan::is_word;
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;
use text_size::TextRange;

pub struct Rule;

/// Predicates that have a dedicated `NOT` form -- and their idiomatic
/// rewrite.
const PREDS: &[(&[u8], &str)] = &[
  (b"IN", "NOT IN"),
  (b"LIKE", "NOT LIKE"),
  (b"ILIKE", "NOT ILIKE"),
  (b"BETWEEN", "NOT BETWEEN"),
  (b"SIMILAR TO", "NOT SIMILAR TO"),
];

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql470"
  }
  fn default_severity(&self) -> Severity {
    Severity::Hint
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, raw) = crate::stmt_body(stmt, source);
    let cleaned = crate::textutil::strip_noise_full(raw);
    let upper = cleaned.to_ascii_uppercase();
    let ub = upper.as_bytes();
    let bytes = cleaned.as_bytes();
    let n = ub.len();
    let mut i = 0usize;
    let mut emitted: std::collections::HashSet<usize> = std::collections::HashSet::new();
    while i + 3 <= n {
      if !(&ub[i..i + 3] == b"NOT" && (i == 0 || !is_word(ub[i - 1] as char)) && (i + 3 == n || !is_word(ub[i + 3] as char))) {
        i += 1;
        continue;
      }
      // Require an opening paren (the unparen form `NOT col IN ...` is
      // also less idiomatic, but it overlaps too much with patterns
      // like `WHERE NOT active OR active` -- keep this Hint narrow).
      let mut k = i + 3;
      while k < n && bytes[k].is_ascii_whitespace() {
        k += 1;
      }
      if k >= n || bytes[k] != b'(' {
        i += 1;
        continue;
      }
      k += 1;
      while k < n && bytes[k].is_ascii_whitespace() {
        k += 1;
      }
      // Read ident (possibly qualified).
      let id_start = k;
      while k < n && (is_word(bytes[k] as char) || bytes[k] == b'.') {
        k += 1;
      }
      let id_end = k;
      if id_start == id_end {
        i += 1;
        continue;
      }
      let ident = &cleaned[id_start..id_end];
      // Skip whitespace, expect one of the predicate keywords.
      while k < n && bytes[k].is_ascii_whitespace() {
        k += 1;
      }
      // Skip if `IS` -- that's sql469's job.
      if k + 2 <= n && &ub[k..k + 2] == b"IS" && (k + 2 == n || !is_word(ub[k + 2] as char)) {
        i += 1;
        continue;
      }
      let mut matched: Option<&'static str> = None;
      for (kw, label) in PREDS {
        let m = kw.len();
        if k + m <= n
          && &ub[k..k + m] == *kw
          && (k + m == n || !is_word(ub[k + m] as char))
        {
          matched = Some(*label);
          break;
        }
      }
      let Some(rewrite) = matched else {
        i += 1;
        continue;
      };
      if emitted.insert(i) {
        let kw_for_msg = rewrite.trim_start_matches("NOT ").to_string();
        let abs_s = start + i;
        // Range end at the close paren of the NOT-paren wrapper.
        // Conservatively, find the matching `)`.
        let abs_e_rel = match_paren(bytes, i + 3 + skip_ws_count(bytes, i + 3), n).unwrap_or(k);
        let abs_e = start + (abs_e_rel + 1).min(n);
        out.push(Diagnostic {
          code: "sql470",
          severity: Severity::Hint,
          message: format!(
            "`NOT ({ident} {kw_for_msg} ...)` is less idiomatic than `{ident} {rewrite} ...` -- both are semantically equivalent; the explicit NOT-prefix form is the convention"
          ),
          range: TextRange::new((abs_s as u32).into(), (abs_e as u32).into()),
        });
      }
      i = k;
    }
  }
}

fn skip_ws_count(bytes: &[u8], from: usize) -> usize {
  let mut k = from;
  while k < bytes.len() && bytes[k].is_ascii_whitespace() {
    k += 1;
  }
  k - from
}

fn match_paren(bytes: &[u8], open: usize, end: usize) -> Option<usize> {
  if open >= bytes.len() || bytes[open] != b'(' {
    return None;
  }
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
