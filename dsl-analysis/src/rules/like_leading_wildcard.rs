//! sql088: `LIKE '%foo'` -- leading wildcard prevents B-tree index use.
//! Suggest `text_pattern_ops` index, `pg_trgm`, or full-text search.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql088"
  }
  fn default_severity(&self) -> Severity {
    Severity::Warning
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let start: usize = u32::from(stmt.range.start()) as usize;
    let end: usize = (u32::from(stmt.range.end()) as usize).min(source.len());
    let body = &source[start..end];
    let upper = body.to_ascii_uppercase();
    let bytes = body.as_bytes();
    let n = bytes.len();
    let mut i = 0;
    while i + 5 <= n {
      // Find `LIKE` or `ILIKE`.
      let kw_len: usize;
      let here = &upper.as_bytes()[i..];
      if here.starts_with(b"ILIKE")
        && (i == 0 || !is_word(bytes[i - 1] as char))
        && (i + 5 == n || !is_word(bytes[i + 5] as char))
      {
        kw_len = 5;
      } else if here.starts_with(b"LIKE")
        && (i == 0 || !is_word(bytes[i - 1] as char))
        && (i + 4 == n || !is_word(bytes[i + 4] as char))
      {
        kw_len = 4;
      } else {
        i += 1;
        continue;
      }
      let mut j = i + kw_len;
      while j < n && bytes[j].is_ascii_whitespace() {
        j += 1;
      }
      if j < n && bytes[j] == b'\'' {
        if j + 1 < n && bytes[j + 1] == b'%' {
          // Span the literal: from opening `'` through closing `'`.
          let str_start = j;
          let mut k = j + 1;
          while k < n && bytes[k] != b'\'' {
            k += 1;
          }
          let abs_start = start + str_start;
          let abs_end = start + (k + 1).min(n);
          out.push(Diagnostic {
            code: "sql088",
            severity: Severity::Warning,
            message: "LIKE/ILIKE with leading `%` prevents B-tree index use -- consider pg_trgm or full-text search"
              .into(),
            range: text_size::TextRange::new((abs_start as u32).into(), (abs_end as u32).into()),
          });
          return;
        }
      }
      i += kw_len;
    }
  }
}

fn is_word(c: char) -> bool {
  c.is_alphanumeric() || c == '_'
}
