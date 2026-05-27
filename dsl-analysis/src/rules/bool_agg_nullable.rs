//! sql342: `BOOL_AND(col)` / `BOOL_OR(col)` / `EVERY(col)` on a nullable
//! boolean column. PG silently ignores NULL inputs, so the result hides
//! the fact that some rows had no opinion. Suggest COALESCE(col, false)
//! or an explicit IS NULL filter.

use crate::typing::column_nullable;
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql342"
  }
  fn default_severity(&self) -> Severity {
    Severity::Hint
  }

  fn check(&self, source: &str, stmt: &Statement, scope: &Scope, catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let start: usize = u32::from(stmt.range.start()) as usize;
    let end: usize = (u32::from(stmt.range.end()) as usize).min(source.len());
    let body = &source[start..end];
    let upper = body.to_ascii_uppercase();
    for needle in ["BOOL_AND(", "BOOL_OR(", "EVERY("] {
      let mut from = 0usize;
      while let Some(rel) = upper[from..].find(needle) {
        let at = from + rel;
        let prev_ok = at == 0 || !is_word(body.as_bytes()[at - 1] as char);
        if !prev_ok {
          from = at + needle.len();
          continue;
        }
        let inner_start = at + needle.len();
        let Some(close) = matched_close(body.as_bytes(), inner_start - 1) else {
          from = inner_start;
          continue;
        };
        let arg = body[inner_start..close].trim();
        let (qual, col) = split_dotted(arg);
        let Some(nullable) = column_nullable(scope, catalog, qual.as_deref(), &col) else {
          from = close + 1;
          continue;
        };
        if nullable {
          let abs_s = start + at;
          let abs_e = abs_s + needle.len() - 1;
          out.push(Diagnostic {
            code: "sql342",
            severity: Severity::Hint,
            message: format!(
              "{} ignores NULL inputs silently -- column `{}` is nullable, wrap in COALESCE({}, false) if you mean it",
              &needle[..needle.len() - 1],
              col,
              arg
            ),
            range: text_size::TextRange::new((abs_s as u32).into(), (abs_e as u32).into()),
          });
          return;
        }
        from = close + 1;
      }
    }
  }
}

fn is_word(c: char) -> bool {
  c.is_alphanumeric() || c == '_'
}

fn split_dotted(s: &str) -> (Option<String>, String) {
  let s = s.trim().trim_matches('"');
  if let Some(dot) = s.find('.') {
    let q = s[..dot].trim_matches('"').to_string();
    let c = s[dot + 1..].trim_matches('"').to_string();
    (Some(q), c)
  } else {
    (None, s.to_string())
  }
}

fn matched_close(bytes: &[u8], open: usize) -> Option<usize> {
  let mut depth = 0i32;
  let mut i = open;
  while i < bytes.len() {
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
        while i < bytes.len() && bytes[i] != b'\'' {
          i += 1
        }
      },
      _ => {},
    }
    i += 1;
  }
  None
}
