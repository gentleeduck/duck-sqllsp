//! sql486: `SELECT DISTINCT *` / `SELECT DISTINCT t.*` -- DISTINCT on
//! a whole-row projection is almost always a workaround for a join
//! that produced duplicates, not the intended filter. It forces a
//! full-row sort/hash and silently hides the underlying join bug.
//! Prefer fixing the join (EXISTS subquery, narrower SELECT list,
//! or aggregation) instead of deduplicating the result.

use crate::clause_scan::is_word;
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;
use text_size::TextRange;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql486"
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
    // Find `SELECT` at the top level (not nested inside a sub-select
    // -- we only flag the OUTER one). We scan all `SELECT`s for now;
    // each statement is checked independently.
    let Some(select_at) = find_word(ub, b"SELECT", 0, n) else {
      return;
    };
    let mut k = select_at + 6;
    while k < n && bytes[k].is_ascii_whitespace() {
      k += 1;
    }
    // Look for DISTINCT (but not DISTINCT ON)
    if !word_eq(ub, k, b"DISTINCT") {
      return;
    }
    let after_distinct = k + 8;
    let mut p = after_distinct;
    while p < n && bytes[p].is_ascii_whitespace() {
      p += 1;
    }
    if word_eq(ub, p, b"ON") {
      // DISTINCT ON -- sql101 handles its own concerns; out of scope.
      return;
    }
    // p now points at the first char of the projection. We need to
    // check if it's `*` or `<ident>.*` -- only flag exact whole-row
    // projection forms.
    let rest = &cleaned[p..];
    let trimmed = rest.trim_start();
    let lead = rest.len() - trimmed.len();
    let abs_proj_start = start + p + lead;
    // Compute the end of the first projection item (up to top-level
    // comma or end of token-run).
    let first_item_end = find_first_proj_end(bytes, p + lead, n);
    let first_item = cleaned[p + lead..first_item_end].trim();
    let abs_proj_end = abs_proj_start + first_item.len();
    let is_bare_star = first_item == "*";
    let is_qualified_star = first_item.ends_with(".*") && {
      // Check that everything before `.*` is a simple ident
      let head = &first_item[..first_item.len() - 2];
      !head.is_empty() && head.chars().all(is_word)
    };
    if !is_bare_star && !is_qualified_star {
      return;
    }
    out.push(Diagnostic {
      code: "sql486",
      severity: Severity::Hint,
      message: "`SELECT DISTINCT *` deduplicates on the entire row -- usually a workaround for a join that produced duplicates rather than the intended filter. Prefer fixing the join (EXISTS subquery, narrower SELECT list, or aggregation).".into(),
      range: TextRange::new((abs_proj_start as u32).into(), (abs_proj_end as u32).into()),
    });
  }
}

fn word_eq(ub: &[u8], i: usize, w: &[u8]) -> bool {
  let m = w.len();
  if i + m > ub.len() {
    return false;
  }
  if &ub[i..i + m] != w {
    return false;
  }
  let prev_ok = i == 0 || !is_word(ub[i - 1] as char);
  let next_ok = i + m == ub.len() || !is_word(ub[i + m] as char);
  prev_ok && next_ok
}

fn find_word(ub: &[u8], w: &[u8], from: usize, to: usize) -> Option<usize> {
  let m = w.len();
  let mut i = from;
  while i + m <= to {
    if word_eq(ub, i, w) {
      return Some(i);
    }
    i += 1;
  }
  None
}

fn find_first_proj_end(bytes: &[u8], from: usize, to: usize) -> usize {
  let mut depth: i32 = 0;
  let mut i = from;
  while i < to {
    let c = bytes[i];
    if c == b'\'' {
      i += 1;
      while i < to && bytes[i] != b'\'' {
        i += 1;
      }
      i = (i + 1).min(to);
      continue;
    }
    if c == b'(' {
      depth += 1;
    } else if c == b')' {
      depth -= 1;
    } else if depth == 0 && c == b',' {
      return i;
    }
    // Stop on FROM at depth 0
    if depth == 0 && i + 4 <= to && bytes[i].eq_ignore_ascii_case(&b'F') && {
      let s = &bytes[i..i + 4];
      s.eq_ignore_ascii_case(b"FROM")
        && (i == 0 || !is_word(bytes[i - 1] as char))
        && (i + 4 == to || !is_word(bytes[i + 4] as char))
    } {
      return i;
    }
    i += 1;
  }
  to
}
