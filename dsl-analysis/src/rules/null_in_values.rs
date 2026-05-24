//! sql061: bare `NULL` inside `VALUES (...)` without an explicit cast.
//!
//! PG infers NULL's type from context. In a multi-row VALUES block, an
//! untyped NULL on the first row can pin the column to TEXT and force
//! later rows to cast. Hint: `NULL::<type>` instead.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::{Statement, StatementKind};
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql061"
  }
  fn default_severity(&self) -> Severity {
    Severity::Hint
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let StatementKind::Insert(_) = &stmt.kind else {
      return;
    };
    let start: usize = u32::from(stmt.range.start()) as usize;
    let end: usize = (u32::from(stmt.range.end()) as usize).min(source.len());
    let body = &source[start..end];
    let upper = body.to_ascii_uppercase();
    let bytes = body.as_bytes();
    let Some(values_at) = upper.find("VALUES") else { return };
    let n = body.len();
    let mut i = values_at + 6;
    while i < n && bytes[i].is_ascii_whitespace() {
      i += 1;
    }
    if i >= n || bytes[i] != b'(' {
      return;
    }
    let Some(close) = match_paren(bytes, i) else { return };
    let tuple = &body[i + 1..close];
    let upper_tuple = tuple.to_ascii_uppercase();
    let tb = upper_tuple.as_bytes();
    // Scan for `NULL` not followed by `::`.
    let mut k = 0;
    while k + 4 <= tb.len() {
      if &upper_tuple[k..k + 4] == "NULL"
        && (k == 0 || !is_word(tb[k - 1] as char))
        && (k + 4 == tb.len() || !is_word(tb[k + 4] as char))
      {
        // Check for ::
        let mut j = k + 4;
        while j < tb.len() && tb[j].is_ascii_whitespace() {
          j += 1;
        }
        if j + 1 < tb.len() && tb[j] == b':' && tb[j + 1] == b':' {
          k = j + 2;
          continue;
        }
        // Narrow to the NULL token inside the VALUES tuple.
        let abs_start = start + i + 1 + k;
        let abs_end = abs_start + 4;
        out.push(Diagnostic {
          code: "sql061",
          severity: Severity::Hint,
          message: "NULL in VALUES without explicit cast -- prefer `NULL::<type>` for predictable type inference"
            .into(),
          range: text_size::TextRange::new((abs_start as u32).into(), (abs_end as u32).into()),
        });
        return;
      }
      k += 1;
    }
  }
}

fn match_paren(bytes: &[u8], open: usize) -> Option<usize> {
  let n = bytes.len();
  let mut depth = 0i32;
  let mut i = open;
  while i < n {
    match bytes[i] {
      b'(' => depth += 1,
      b')' => {
        depth -= 1;
        if depth == 0 {
          return Some(i);
        }
      },
      b'\'' => {
        i += 1;
        while i < n && bytes[i] != b'\'' {
          i += 1;
        }
      },
      _ => {},
    }
    i += 1;
  }
  None
}

fn is_word(c: char) -> bool {
  c.is_alphanumeric() || c == '_'
}
