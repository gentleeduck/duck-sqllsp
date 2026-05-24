//! sql111: `LOCK TABLE` outside an explicit transaction has no effect
//! beyond the single statement -- usually a bug.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql111"
  }
  fn default_severity(&self) -> Severity {
    Severity::Error
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let start: usize = u32::from(stmt.range.start()) as usize;
    let end: usize = (u32::from(stmt.range.end()) as usize).min(source.len());
    let body = &source[start..end];
    let upper = body.to_ascii_uppercase();
    let trimmed = upper.trim_start();
    if !trimmed.starts_with("LOCK") {
      return;
    }
    // Look earlier in the *full* source for an unmatched BEGIN.
    // Cheap heuristic: count BEGIN vs COMMIT/ROLLBACK before this
    // statement.
    let before = &source[..start];
    let before_upper = before.to_ascii_uppercase();
    let begins = count_word(&before_upper, "BEGIN") + count_word(&before_upper, "START TRANSACTION");
    let commits = count_word(&before_upper, "COMMIT") + count_word(&before_upper, "ROLLBACK");
    if begins > commits {
      return;
    }
    let leading_ws = upper.len() - trimmed.len();
    let abs_start = start + leading_ws;
    let abs_end = start + leading_ws + 4;
    out.push(Diagnostic {
      code: "sql111",
      severity: Severity::Error,
      message: "LOCK TABLE outside a transaction -- the lock releases as soon as the statement finishes".into(),
      range: text_size::TextRange::new((abs_start as u32).into(), (abs_end as u32).into()),
    });
  }
}

fn count_word(haystack: &str, needle: &str) -> usize {
  let h = haystack.as_bytes();
  let n = h.len();
  let w = needle.len();
  let mut c = 0;
  let mut i = 0;
  while i + w <= n {
    if &haystack[i..i + w] == needle {
      let prev_ok = i == 0 || !is_word(h[i - 1] as char);
      let next_ok = i + w == n || !is_word(h[i + w] as char);
      if prev_ok && next_ok {
        c += 1;
        i += w;
        continue;
      }
    }
    i += 1;
  }
  c
}

fn is_word(c: char) -> bool {
  c.is_alphanumeric() || c == '_'
}
