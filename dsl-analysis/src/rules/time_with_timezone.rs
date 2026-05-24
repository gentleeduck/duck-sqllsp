//! sql075: column declared as `TIME WITH TIME ZONE` (alias `TIMETZ`).
//! PG docs recommend against TIMETZ -- it's almost never what you want.
//! Use `TIMESTAMP WITH TIME ZONE` (`TIMESTAMPTZ`) instead. Hint.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::{Statement, StatementKind};
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql075"
  }
  fn default_severity(&self) -> Severity {
    Severity::Hint
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    if !matches!(stmt.kind, StatementKind::CreateTable(_) | StatementKind::AlterTable(_)) {
      return;
    }
    let start: usize = u32::from(stmt.range.start()) as usize;
    let end: usize = (u32::from(stmt.range.end()) as usize).min(source.len());
    let body = &source[start..end];
    let upper = body.to_ascii_uppercase();

    let (token_start, token_len) = if let Some(p) = upper.find("TIME WITH TIME ZONE") {
      (p, "TIME WITH TIME ZONE".len())
    } else if let Some(p) = find_word_pos(&upper, "TIMETZ") {
      (p, "TIMETZ".len())
    } else {
      return;
    };
    let abs_start = start + token_start;
    let abs_end = abs_start + token_len;
    out.push(Diagnostic {
      code: "sql075",
      severity: Severity::Hint,
      message: "TIME WITH TIME ZONE (TIMETZ) is rarely what you want -- prefer TIMESTAMPTZ".into(),
      range: text_size::TextRange::new((abs_start as u32).into(), (abs_end as u32).into()),
    });
  }
}

fn find_word_pos(haystack: &str, needle: &str) -> Option<usize> {
  let bytes = haystack.as_bytes();
  let nb = needle.as_bytes();
  let mut i = 0;
  while i + nb.len() <= bytes.len() {
    if &bytes[i..i + nb.len()] == nb {
      let prev_ok = i == 0 || !is_word(bytes[i - 1] as char);
      let next_ok = i + nb.len() == bytes.len() || !is_word(bytes[i + nb.len()] as char);
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
