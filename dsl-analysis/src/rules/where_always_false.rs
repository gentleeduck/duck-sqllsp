//! sql407: `WHERE 1=2` / `WHERE FALSE` / `WHERE 1<>1` -- the entire
//! predicate is a trivially-false literal comparison; the query
//! returns zero rows regardless of input.
//!
//! Usually a leftover from copy-paste-and-edit ("kill the rows for
//! now") or a debugging placeholder that escaped review. PG happily
//! accepts and executes it; we surface a warning so it surfaces in
//! review.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql407"
  }
  fn default_severity(&self) -> Severity {
    Severity::Warning
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let start: usize = u32::from(stmt.range.start()) as usize;
    let end: usize = (u32::from(stmt.range.end()) as usize).min(source.len());
    let body = &source[start..end];
    let upper = body.to_ascii_uppercase();
    // Walk for word-bounded WHERE.
    let bytes = upper.as_bytes();
    let n = bytes.len();
    let mut i = 0;
    while i + 6 <= n {
      if &upper[i..i + 5] == "WHERE"
        && bytes[i + 5].is_ascii_whitespace()
        && (i == 0 || !is_word(bytes[i - 1] as char))
      {
        // Take everything from after WHERE until clause boundary
        // (top-level GROUP, ORDER, LIMIT, OFFSET, HAVING, FOR, FETCH,
        // RETURNING, UNION, semicolon, end). Bail if we run into
        // an opening paren before any of those (this WHERE could
        // be inside a subquery; let outer scan handle).
        let pred_start = i + 6;
        let pred_end = find_predicate_end(bytes, pred_start);
        let pred = body[pred_start..pred_end].trim();
        if is_always_false_literal(pred) {
          let abs_s = start + pred_start + (body[pred_start..pred_end].len() - body[pred_start..pred_end].trim_start().len());
          let abs_e = abs_s + pred.len();
          out.push(Diagnostic {
            code: "sql407",
            severity: Severity::Warning,
            message: format!("WHERE predicate `{pred}` is trivially false -- query returns zero rows regardless of data"),
            range: text_size::TextRange::new((abs_s as u32).into(), (abs_e as u32).into()),
          });
        }
        i = pred_end;
        continue;
      }
      i += 1;
    }
  }
}

fn find_predicate_end(bytes: &[u8], from: usize) -> usize {
  let stopwords: &[&str] =
    &["GROUP BY", "ORDER BY", "LIMIT", "OFFSET", "HAVING", "FOR ", "FETCH ", "RETURNING", "UNION", "INTERSECT", "EXCEPT", "WINDOW"];
  let n = bytes.len();
  let mut depth: i32 = 0;
  let mut i = from;
  while i < n {
    let c = bytes[i];
    if c == b'(' {
      depth += 1;
    } else if c == b')' {
      if depth == 0 {
        return i;
      }
      depth -= 1;
    } else if c == b';' && depth == 0 {
      return i;
    } else if depth == 0 && (i == from || !is_word(bytes[i - 1] as char)) {
      for w in stopwords {
        let wb = w.as_bytes();
        if i + wb.len() <= n && bytes[i..i + wb.len()] == *wb {
          return i;
        }
      }
    }
    i += 1;
  }
  n
}

/// True for predicate strings that are unconditionally false. The
/// match is intentionally narrow -- only obvious constants, no
/// inferred contradictions. False positives here would be confusing.
fn is_always_false_literal(pred: &str) -> bool {
  let canon = pred
    .trim_matches(|c: char| c == '(' || c == ')' || c.is_whitespace())
    .to_ascii_uppercase();
  let stripped: String = canon.chars().filter(|c| !c.is_whitespace()).collect();
  matches!(
    stripped.as_str(),
    "FALSE" | "1=2" | "2=1" | "0=1" | "1=0" | "1<>1" | "1!=1" | "TRUE=FALSE" | "FALSE=TRUE"
  )
}

fn is_word(c: char) -> bool {
  c.is_alphanumeric() || c == '_'
}
