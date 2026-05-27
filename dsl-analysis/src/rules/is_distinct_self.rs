//! sql464: `x IS DISTINCT FROM x` -- always FALSE (the NULL-safe
//! equality says x is NOT distinct from itself even when NULL).
//! Likewise `x IS NOT DISTINCT FROM x` is always TRUE. Almost
//! always a copy-paste typo for two different operands.

use crate::clause_scan::is_word;
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;
use text_size::TextRange;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql464"
  }
  fn default_severity(&self) -> Severity {
    Severity::Warning
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let start: usize = u32::from(stmt.range.start()) as usize;
    let end: usize = (u32::from(stmt.range.end()) as usize).min(source.len());
    let raw = &source[start..end];
    let cleaned = crate::textutil::strip_noise_full(raw);
    let upper = cleaned.to_ascii_uppercase();
    let ub = upper.as_bytes();
    let bytes = cleaned.as_bytes();
    let n = ub.len();
    let mut i = 0usize;
    while i + 2 <= n {
      // Locate ` IS DISTINCT FROM ` or ` IS NOT DISTINCT FROM ` after
      // an identifier. Walk backward to capture the lhs ident.
      if &ub[i..i + 2] != b"IS" || (i > 0 && is_word(ub[i - 1] as char)) {
        i += 1;
        continue;
      }
      // Word boundary after IS.
      if i + 2 < n && is_word(ub[i + 2] as char) {
        i += 1;
        continue;
      }
      // Scan forward over whitespace + optional NOT + whitespace + DISTINCT + whitespace + FROM + whitespace.
      let mut p = i + 2;
      while p < n && bytes[p].is_ascii_whitespace() {
        p += 1;
      }
      let is_not = if p + 3 <= n && &ub[p..p + 3] == b"NOT" && (p + 3 == n || !is_word(ub[p + 3] as char)) {
        p += 3;
        while p < n && bytes[p].is_ascii_whitespace() {
          p += 1;
        }
        true
      } else {
        false
      };
      if !(p + 8 <= n && &ub[p..p + 8] == b"DISTINCT") {
        i += 1;
        continue;
      }
      p += 8;
      while p < n && bytes[p].is_ascii_whitespace() {
        p += 1;
      }
      if !(p + 4 <= n && &ub[p..p + 4] == b"FROM") {
        i += 1;
        continue;
      }
      p += 4;
      while p < n && bytes[p].is_ascii_whitespace() {
        p += 1;
      }
      // Read LHS ident backward from i.
      let mut lhs_end = i;
      while lhs_end > 0 && bytes[lhs_end - 1].is_ascii_whitespace() {
        lhs_end -= 1;
      }
      let lhs_start = walk_ident_back(bytes, lhs_end);
      if lhs_start == lhs_end {
        i = p;
        continue;
      }
      let lhs = &cleaned[lhs_start..lhs_end];
      // Read RHS ident forward from p.
      let rhs_start = p;
      let rhs_end = walk_ident_forward(bytes, rhs_start, n);
      if rhs_end == rhs_start {
        i = p;
        continue;
      }
      let rhs = &cleaned[rhs_start..rhs_end];
      if lhs.eq_ignore_ascii_case(rhs) && looks_like_column(lhs) {
        let kw = if is_not { "IS NOT DISTINCT FROM" } else { "IS DISTINCT FROM" };
        let outcome = if is_not { "always TRUE" } else { "always FALSE" };
        let abs_s = start + lhs_start;
        let abs_e = start + rhs_end;
        out.push(Diagnostic {
          code: "sql464",
          severity: Severity::Warning,
          message: format!(
            "`{lhs} {kw} {rhs}` -- the NULL-safe equality on a column with itself is {outcome}; almost always a copy-paste typo for two different operands"
          ),
          range: TextRange::new((abs_s as u32).into(), (abs_e as u32).into()),
        });
      }
      i = rhs_end;
    }
  }
}

fn walk_ident_back(bytes: &[u8], end: usize) -> usize {
  let mut k = end;
  while k > 0 {
    let c = bytes[k - 1];
    if c.is_ascii_alphanumeric() || c == b'_' || c == b'.' {
      k -= 1;
    } else {
      break;
    }
  }
  k
}

fn walk_ident_forward(bytes: &[u8], start: usize, n: usize) -> usize {
  let mut k = start;
  while k < n {
    let c = bytes[k];
    if c.is_ascii_alphanumeric() || c == b'_' || c == b'.' {
      k += 1;
    } else {
      break;
    }
  }
  k
}

fn looks_like_column(s: &str) -> bool {
  if s.is_empty() || s.starts_with('.') || s.ends_with('.') {
    return false;
  }
  // Reject pure numerics and keyword literals.
  if s.chars().all(|c| c.is_ascii_digit() || c == '.') {
    return false;
  }
  let upper = s.to_ascii_uppercase();
  if matches!(upper.as_str(), "NULL" | "TRUE" | "FALSE") {
    return false;
  }
  true
}
