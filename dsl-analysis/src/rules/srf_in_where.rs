//! sql645: a set-returning function (`generate_series`, `unnest`,
//! `jsonb_array_elements`, `regexp_split_to_table`, `json_each`, ...) called
//! directly in a `WHERE` clause. PostgreSQL forbids SRFs outside the SELECT
//! list and FROM clause and raises 0A000 ("set-returning functions are not
//! allowed in WHERE"). Move the SRF into the FROM clause (often `LATERAL`) or a
//! subquery and filter its output.
//!
//! SRFs inside a `(SELECT ...)` / `(WITH ...)` subquery within the WHERE are
//! legal and are skipped.

use crate::clause_scan::{find_clause, find_clause_end, is_word};
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

const SRFS: &[&str] = &[
  "generate_series",
  "generate_subscripts",
  "unnest",
  "regexp_split_to_table",
  "regexp_matches",
  "jsonb_array_elements",
  "jsonb_array_elements_text",
  "json_array_elements",
  "json_array_elements_text",
  "jsonb_each",
  "jsonb_each_text",
  "json_each",
  "json_each_text",
  "jsonb_object_keys",
  "json_object_keys",
  "string_to_table",
];

const STOP: &[&str] = &[
  "GROUP BY", "ORDER BY", "LIMIT", "OFFSET", "HAVING", "FOR", "FETCH", "WINDOW", "RETURNING", "UNION",
  "INTERSECT", "EXCEPT",
];

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql645"
  }
  fn default_severity(&self) -> Severity {
    Severity::Error
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, raw) = crate::stmt_body(stmt, source);
    let cleaned = crate::textutil::strip_noise_full(raw);
    let upper = cleaned.to_ascii_uppercase();
    let ub = upper.as_bytes();
    let lower = cleaned.to_ascii_lowercase();
    let lb = lower.as_bytes();

    let mut from = 0usize;
    while let Some(rel) = find_clause(&ub[from..], b"WHERE").map(|p| p + from) {
      let pred_start = rel + 5;
      let pred_end = find_clause_end(ub, pred_start, STOP);
      self.scan(lb, pred_start, pred_end, start, out);
      from = pred_end.max(rel + 5);
    }
  }
}

impl Rule {
  fn scan(&self, lb: &[u8], lo: usize, hi: usize, start: usize, out: &mut Vec<Diagnostic>) {
    let mut i = lo;
    while i < hi {
      let c = lb[i];
      if c == b'(' {
        // skip a nested subquery wholesale
        let mut j = i + 1;
        while j < hi && lb[j].is_ascii_whitespace() {
          j += 1;
        }
        let is_subquery = lb[j..hi.min(lb.len())].starts_with(b"select") || lb[j..hi.min(lb.len())].starts_with(b"with");
        if is_subquery {
          let mut depth = 0i32;
          let mut k = i;
          while k < hi {
            match lb[k] {
              b'(' => depth += 1,
              b')' => {
                depth -= 1;
                if depth == 0 {
                  break;
                }
              }
              _ => {}
            }
            k += 1;
          }
          i = k + 1;
          continue;
        }
        i += 1;
        continue;
      }
      // word-start: try to match an SRF name followed by `(`
      if (i == lo || !is_word(lb[i - 1] as char)) && is_word(c as char) {
        for &srf in SRFS {
          let l = srf.len();
          if i + l <= hi
            && &lb[i..i + l] == srf.as_bytes()
            && lb.get(i + l).is_none_or(|&b| !is_word(b as char))
          {
            let mut p = i + l;
            while p < hi && lb[p].is_ascii_whitespace() {
              p += 1;
            }
            if p < hi && lb[p] == b'(' {
              out.push(Diagnostic {
                code: "sql645",
                severity: Severity::Error,
                message: format!("`{srf}` is a set-returning function and isn't allowed in WHERE (PG 0A000) -- move it into FROM (LATERAL) or a subquery"),
                range: crate::range_at(start + i, start + i + l),
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
