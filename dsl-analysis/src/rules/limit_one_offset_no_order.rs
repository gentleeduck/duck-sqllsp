//! sql411: `LIMIT 1 OFFSET N` (with N > 0) without ORDER BY picks
//! a deliberately non-first row, but without ORDER BY there's no
//! defined notion of "the Nth row" -- the planner is free to return
//! anything. Distinct from sql051 which exempts `LIMIT 1` (the
//! common "any one matching row" idiom): the OFFSET makes the intent
//! position-sensitive so the missing ORDER BY is the bug.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::{Statement, StatementKind};
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql411"
  }
  fn default_severity(&self) -> Severity {
    Severity::Warning
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let StatementKind::Select(_) = &stmt.kind else {
      return;
    };
    let start: usize = u32::from(stmt.range.start()) as usize;
    let end: usize = (u32::from(stmt.range.end()) as usize).min(source.len());
    let raw = &source[start..end];
    let body_owned = crate::textutil::strip_noise_full(raw);
    let upper = body_owned.to_ascii_uppercase();
    if !upper.contains("LIMIT") || !upper.contains("OFFSET") {
      return;
    }
    if contains_word(&upper, "ORDER BY") {
      return;
    }
    // Pull the OFFSET literal. PG accepts a numeric literal or
    // a parameter ($1). We only flag the numeric form -- a parameter
    // could legitimately be 0.
    let Some(off_at) = find_word(&upper, "OFFSET") else {
      return;
    };
    let after = &body_owned[off_at + 6..];
    let trimmed = after.trim_start();
    let mut chars = trimmed.chars();
    let mut buf = String::new();
    for c in chars.by_ref() {
      if c.is_ascii_digit() {
        buf.push(c);
      } else {
        break;
      }
    }
    let Ok(n) = buf.parse::<u64>() else {
      return;
    };
    if n == 0 {
      return;
    }
    // Highlight the LIMIT keyword for the diagnostic range.
    let Some(lim_at) = find_word(&upper, "LIMIT") else {
      return;
    };
    let abs_start = start + lim_at;
    let abs_end = abs_start + 5;
    out.push(Diagnostic {
      code: "sql411",
      severity: Severity::Warning,
      message: format!(
        "LIMIT with OFFSET {n} but no ORDER BY -- the skipped/returned row is non-deterministic; add ORDER BY or use ROW_NUMBER() OVER (ORDER BY ...)"
      ),
      range: text_size::TextRange::new((abs_start as u32).into(), (abs_end as u32).into()),
    });
  }
}

fn contains_word(hay: &str, needle: &str) -> bool {
  find_word(hay, needle).is_some()
}

fn find_word(hay: &str, needle: &str) -> Option<usize> {
  let bytes = hay.as_bytes();
  let nb = needle.as_bytes();
  let n = bytes.len();
  let m = nb.len();
  let mut i = 0;
  while i + m <= n {
    if &bytes[i..i + m] == nb {
      let prev_ok = i == 0 || !is_word(bytes[i - 1] as char);
      let next_ok = i + m == n || !is_word(bytes[i + m] as char);
      if prev_ok && next_ok {
        return Some(i);
      }
    }
    i += 1;
  }
  None
}

fn is_word(c: char) -> bool {
  c.is_alphanumeric() || c == '_'
}
