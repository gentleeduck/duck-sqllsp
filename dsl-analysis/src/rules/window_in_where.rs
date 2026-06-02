//! sql425: window function in WHERE / HAVING / JOIN ON. PG raises
//! 42P20 ("window functions are not allowed in WHERE"). Move the
//! window into a subquery and filter the result, or use HAVING for
//! aggregates.

use crate::clause_scan::{find_clause, find_clause_end, is_word};
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;
use text_size::TextRange;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql425"
  }
  fn default_severity(&self) -> Severity {
    Severity::Error
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, raw) = crate::stmt_body(stmt, source);
    let cleaned = crate::textutil::strip_noise_full(raw);
    let upper = cleaned.to_ascii_uppercase();
    let bytes_u = upper.as_bytes();
    let bytes = cleaned.as_bytes();
    let stopwords = ["GROUP BY", "ORDER BY", "LIMIT", "OFFSET", "HAVING", "FOR", "FETCH", "WINDOW", "RETURNING", "UNION", "INTERSECT", "EXCEPT"];

    for (needle, label) in [
      (&b"WHERE"[..], "WHERE"),
      (&b"ON"[..], "JOIN ON"),
      (&b"HAVING"[..], "HAVING"),
      (&b"GROUP BY"[..], "GROUP BY"),
    ] {
      let mut from = 0usize;
      while let Some(rel) = find_clause(&bytes_u[from..], needle).map(|p| p + from) {
        let pred_start = rel + needle.len();
        let pred_end = find_clause_end(bytes_u, pred_start, &stopwords);
        scan_over(bytes, bytes_u, pred_start, pred_end, start, label, out);
        from = pred_end.max(rel + needle.len());
      }
    }
  }
}

fn scan_over(bytes: &[u8], upper: &[u8], from: usize, to: usize, abs_off: usize, clause_label: &str, out: &mut Vec<Diagnostic>) {
  // Walk tracking subquery scope (same as sql424).
  let mut stack: Vec<bool> = Vec::new();
  let mut i = from;
  let mut emitted_at: std::collections::HashSet<usize> = std::collections::HashSet::new();
  while i < to {
    let c = bytes[i];
    if c == b'\'' {
      i += 1;
      while i < to && bytes[i] != b'\'' {
        i += 1;
      }
      i = (i + 1).min(to);
      continue;
    }
    if c == b'(' {
      let mut j = i + 1;
      while j < to && bytes[j].is_ascii_whitespace() {
        j += 1;
      }
      let is_subquery = (j + 6 <= to && upper[j..j + 6] == *b"SELECT" && (j + 6 == to || !is_word(upper[j + 6] as char)))
        || (j + 4 <= to && upper[j..j + 4] == *b"WITH" && (j + 4 == to || !is_word(upper[j + 4] as char)));
      stack.push(is_subquery);
      i += 1;
      continue;
    }
    if c == b')' {
      stack.pop();
      i += 1;
      continue;
    }
    // Look for word-bounded OVER followed by `(`.
    if i + 4 <= to
      && upper[i..i + 4] == *b"OVER"
      && (i == 0 || !is_word(upper[i - 1] as char))
      && (i + 4 == to || !is_word(upper[i + 4] as char))
    {
      let mut j = i + 4;
      while j < to && bytes[j].is_ascii_whitespace() {
        j += 1;
      }
      if j < to && bytes[j] == b'(' && !stack.iter().any(|&s| s) && emitted_at.insert(i) {
        let abs_s = abs_off + i;
        let abs_e = abs_off + j + 1;
        out.push(Diagnostic {
          code: "sql425",
          severity: Severity::Error,
          message: format!(
            "window function (`OVER (...)`) cannot appear in {clause_label} -- PG raises 42P20; wrap the query in a subquery and filter the windowed column, or use HAVING for aggregates"
          ),
          range: TextRange::new((abs_s as u32).into(), (abs_e as u32).into()),
        });
      }
      i = j + 1;
      continue;
    }
    i += 1;
  }
}
