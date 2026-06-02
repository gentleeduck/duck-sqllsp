//! sql419: `NULLIF(x, NULL)` and `NULLIF(NULL, x)` are pointless --
//! `NULLIF(x, NULL)` collapses to just `x` (NULL compared to anything
//! is NULL, so the equality never holds), and `NULLIF(NULL, x)` is
//! always NULL. Likely a typo or unfinished thought.

use crate::{Diagnostic, LintRule, Severity};
use crate::textutil::is_word;
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;
use text_size::TextRange;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql419"
  }
  fn default_severity(&self) -> Severity {
    Severity::Hint
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, body, upper) = crate::stmt_body_upper(stmt, source);
    let bytes = body.as_bytes();
    let n = bytes.len();
    let mut i = 0;
    while i + 6 <= n {
      if &upper[i..i + 6] != "NULLIF" {
        i += 1;
        continue;
      }
      let prev_ok = i == 0 || !is_word(bytes[i - 1] as char);
      if !prev_ok {
        i += 6;
        continue;
      }
      let mut j = i + 6;
      while j < n && bytes[j].is_ascii_whitespace() {
        j += 1;
      }
      if j >= n || bytes[j] != b'(' {
        i = j.max(i + 6);
        continue;
      }
      let Some(close) = match_paren(bytes, j) else {
        i = j + 1;
        continue;
      };
      let inner = &body[j + 1..close];
      let parts = split_top_level_commas(inner);
      if parts.len() != 2 {
        i = close + 1;
        continue;
      }
      let a = parts[0].trim();
      let b = parts[1].trim();
      let a_null = a.eq_ignore_ascii_case("NULL");
      let b_null = b.eq_ignore_ascii_case("NULL");
      if a_null || b_null {
        let msg = if a_null && b_null {
          "`NULLIF(NULL, NULL)` always returns NULL -- pointless".into()
        } else if b_null {
          format!("`NULLIF({a}, NULL)` is a no-op -- collapses to `{a}` (NULL never equals anything)")
        } else {
          format!("`NULLIF(NULL, {b})` always returns NULL -- the first arg is the only possible result")
        };
        out.push(Diagnostic {
          code: "sql419",
          severity: Severity::Hint,
          message: msg,
          range: TextRange::new(((start + i) as u32).into(), ((start + close + 1) as u32).into()),
        });
      }
      i = close + 1;
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

fn split_top_level_commas(s: &str) -> Vec<String> {
  let bytes = s.as_bytes();
  let n = bytes.len();
  let mut out = Vec::new();
  let mut start = 0;
  let mut depth = 0i32;
  let mut i = 0;
  while i < n {
    match bytes[i] {
      b'(' => depth += 1,
      b')' => depth -= 1,
      b'\'' => {
        i += 1;
        while i < n && bytes[i] != b'\'' {
          i += 1;
        }
      },
      b',' if depth == 0 => {
        out.push(s[start..i].to_string());
        start = i + 1;
      },
      _ => {},
    }
    i += 1;
  }
  out.push(s[start..].to_string());
  out
}

