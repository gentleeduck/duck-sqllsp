//! sql087: `x BETWEEN <high> AND <low>` -- bounds are flipped and the
//! expression matches nothing. Only catches obvious numeric literal
//! cases (constant low > constant high).

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql087"
  }
  fn default_severity(&self) -> Severity {
    Severity::Error
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let start: usize = u32::from(stmt.range.start()) as usize;
    let end: usize = (u32::from(stmt.range.end()) as usize).min(source.len());
    let body = &source[start..end];
    let upper = body.to_ascii_uppercase();
    let bytes = upper.as_bytes();
    let n = bytes.len();
    let mut i = 0;
    while i + 7 <= n {
      if &upper[i..i + 7] == "BETWEEN" {
        let prev_ok = i == 0 || !is_word(bytes[i - 1] as char);
        let next_ok = i + 7 == n || !is_word(bytes[i + 7] as char);
        if prev_ok && next_ok {
          // Read the low literal, the AND, then the high literal.
          let mut j = i + 7;
          while j < n && bytes[j].is_ascii_whitespace() {
            j += 1;
          }
          let low_start = j;
          while j < n && (bytes[j].is_ascii_digit() || bytes[j] == b'-' || bytes[j] == b'.') {
            j += 1;
          }
          let low = &upper[low_start..j];
          while j < n && bytes[j].is_ascii_whitespace() {
            j += 1;
          }
          if j + 3 > n || &upper[j..j + 3] != "AND" {
            i += 1;
            continue;
          }
          j += 3;
          while j < n && bytes[j].is_ascii_whitespace() {
            j += 1;
          }
          let high_start = j;
          while j < n && (bytes[j].is_ascii_digit() || bytes[j] == b'-' || bytes[j] == b'.') {
            j += 1;
          }
          let high = &upper[high_start..j];
          if let (Ok(lo), Ok(hi)) = (low.parse::<f64>(), high.parse::<f64>()) {
            if lo > hi {
              let abs_start = start + i;
              let abs_end = start + j;
              out.push(Diagnostic {
                code: "sql087",
                severity: Severity::Error,
                message: format!("BETWEEN {lo} AND {hi}: low > high, the expression matches no rows"),
                range: text_size::TextRange::new((abs_start as u32).into(), (abs_end as u32).into()),
              });
              return;
            }
          }
        }
      }
      i += 1;
    }
  }
}

fn is_word(c: char) -> bool {
  c.is_alphanumeric() || c == '_'
}
