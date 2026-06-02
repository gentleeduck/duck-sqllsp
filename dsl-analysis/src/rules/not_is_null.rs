//! sql469: `NOT (col IS NULL)` and `NOT col IS NULL` -- less
//! idiomatic than `col IS NOT NULL`. Both forms are semantically
//! equivalent in PG but the negated form is harder to scan and a
//! common pattern after a refactor that dropped the inner predicate.
//! Same with the inverse `NOT (col IS NOT NULL)` -> `col IS NULL`.

use crate::clause_scan::is_word;
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;
use text_size::TextRange;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql469"
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
      // Find word-bounded NOT.
      if !(&ub[i..i + 3] == b"NOT" && (i == 0 || !is_word(ub[i - 1] as char)) && (i + 3 == n || !is_word(ub[i + 3] as char))) {
        i += 1;
        continue;
      }
      let mut k = i + 3;
      while k < n && bytes[k].is_ascii_whitespace() {
        k += 1;
      }
      // Optional opening `(`.
      let had_paren = k < n && bytes[k] == b'(';
      if had_paren {
        k += 1;
        while k < n && bytes[k].is_ascii_whitespace() {
          k += 1;
        }
      }
      // Read ident.
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
      // Skip whitespace, expect IS [NOT] NULL.
      while k < n && bytes[k].is_ascii_whitespace() {
        k += 1;
      }
      if !(k + 2 <= n && &ub[k..k + 2] == b"IS" && (k + 2 == n || !is_word(ub[k + 2] as char))) {
        i += 1;
        continue;
      }
      k += 2;
      while k < n && bytes[k].is_ascii_whitespace() {
        k += 1;
      }
      let inner_is_not = k + 3 <= n && &ub[k..k + 3] == b"NOT" && (k + 3 == n || !is_word(ub[k + 3] as char));
      if inner_is_not {
        k += 3;
        while k < n && bytes[k].is_ascii_whitespace() {
          k += 1;
        }
      }
      if !(k + 4 <= n && &ub[k..k + 4] == b"NULL" && (k + 4 == n || !is_word(ub[k + 4] as char))) {
        i += 1;
        continue;
      }
      k += 4;
      if had_paren {
        while k < n && bytes[k].is_ascii_whitespace() {
          k += 1;
        }
        if k >= n || bytes[k] != b')' {
          i += 1;
          continue;
        }
        k += 1;
      }
      if emitted.insert(i) {
        // Skip if the ident is keyword-shaped (NULL/TRUE/FALSE).
        let u = ident.to_ascii_uppercase();
        if matches!(u.as_str(), "NULL" | "TRUE" | "FALSE") {
          i = k;
          continue;
        }
        let inner_kw = if inner_is_not { "IS NOT NULL" } else { "IS NULL" };
        let suggestion = if inner_is_not { format!("{ident} IS NULL") } else { format!("{ident} IS NOT NULL") };
        let abs_s = start + i;
        let abs_e = start + k;
        out.push(Diagnostic {
          code: "sql469",
          severity: Severity::Hint,
          message: format!(
            "`NOT ({ident} {inner_kw})` is less idiomatic than `{suggestion}` -- rewrite using the IS-NOT-NULL/IS-NULL form for clearer intent"
          ),
          range: TextRange::new((abs_s as u32).into(), (abs_e as u32).into()),
        });
      }
      i = k;
    }
  }
}
