//! sql343: `percent_rank() OVER (ORDER BY <col>)` /
//! `cume_dist() OVER (ORDER BY <col>)` where `<col>` is a non-numeric,
//! non-temporal type. The window function still runs but yields
//! lexicographic ranking, which is rarely what was meant.

use crate::typing::{TypeFamily, column_family};
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql343"
  }
  fn default_severity(&self) -> Severity {
    Severity::Hint
  }

  fn check(&self, source: &str, stmt: &Statement, scope: &Scope, catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let start: usize = u32::from(stmt.range.start()) as usize;
    let end: usize = (u32::from(stmt.range.end()) as usize).min(source.len());
    let body = &source[start..end];
    let upper = body.to_ascii_uppercase();
    for needle in ["PERCENT_RANK()", "CUME_DIST()"] {
      let Some(rel) = upper.find(needle) else { continue };
      // Find `OVER (ORDER BY <col>` after.
      let after = rel + needle.len();
      let Some(over_at) = upper[after..].find("OVER").map(|p| after + p) else { continue };
      let over_rest = &body[over_at..];
      let Some(open) = over_rest.find('(') else { continue };
      let Some(close_rel) = matched_close(over_rest.as_bytes(), open) else { continue };
      let window_body = &over_rest[open + 1..close_rel];
      let win_upper = window_body.to_ascii_uppercase();
      let Some(ob_at) = win_upper.find("ORDER BY ") else { continue };
      let ob_rest = &window_body[ob_at + 9..];
      let first = ob_rest.split(|c: char| c == ',' || c == ')' || c.is_whitespace()).next().unwrap_or("").trim();
      if first.is_empty() {
        continue;
      }
      let (qual, col) = split_dotted(first);
      let Some(fam) = column_family(scope, catalog, qual.as_deref(), &col) else { continue };
      let comparable = fam.is_numeric()
        || matches!(fam, TypeFamily::Date | TypeFamily::Time | TypeFamily::Timestamp | TypeFamily::Interval);
      if comparable {
        continue;
      }
      let abs_s = start + rel;
      let abs_e = start + over_at + open + 1 + close_rel + 1;
      out.push(Diagnostic {
        code: "sql343",
        severity: Severity::Hint,
        message: format!(
          "{} OVER (ORDER BY {}) -- `{}` has family `{}`, ranking will be lexicographic not numeric",
          &needle[..needle.len() - 2],
          first,
          col,
          fam.name()
        ),
        range: text_size::TextRange::new((abs_s as u32).into(), (abs_e as u32).into()),
      });
      return;
    }
  }
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
