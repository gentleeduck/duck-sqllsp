//! sql094: `CASE` expressions nested more than 3 deep -- usually
//! signals a lookup table or function refactor is needed.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

const MAX_DEPTH: i32 = 3;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql094"
  }
  fn default_severity(&self) -> Severity {
    Severity::Hint
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let start: usize = u32::from(stmt.range.start()) as usize;
    let end: usize = (u32::from(stmt.range.end()) as usize).min(source.len());
    let body = &source[start..end];
    let upper = body.to_ascii_uppercase();
    let bytes = upper.as_bytes();
    let n = bytes.len();

    let mut depth = 0i32;
    let mut max_depth = 0i32;
    let mut deepest_pos: Option<usize> = None;
    let mut i = 0;
    while i < n {
      if i + 4 <= n
        && &upper[i..i + 4] == "CASE"
        && (i == 0 || !is_word(bytes[i - 1] as char))
        && (i + 4 == n || !is_word(bytes[i + 4] as char))
      {
        depth += 1;
        if depth > max_depth {
          max_depth = depth;
          deepest_pos = Some(i);
        }
        i += 4;
        continue;
      }
      if i + 3 <= n
        && &upper[i..i + 3] == "END"
        && (i == 0 || !is_word(bytes[i - 1] as char))
        && (i + 3 == n || !is_word(bytes[i + 3] as char))
        && depth > 0
      {
        depth -= 1;
        i += 3;
        continue;
      }
      i += 1;
    }
    if max_depth > MAX_DEPTH {
      let range = if let Some(pos) = deepest_pos {
        let abs_start = start + pos;
        let abs_end = abs_start + 4;
        text_size::TextRange::new((abs_start as u32).into(), (abs_end as u32).into())
      } else {
        stmt.range
      };
      out.push(Diagnostic {
        code: "sql094",
        severity: Severity::Hint,
        message: format!("CASE expressions nested {max_depth} deep -- consider a lookup table or helper function"),
        range,
      });
    }
  }
}

fn is_word(c: char) -> bool {
  c.is_alphanumeric() || c == '_'
}
