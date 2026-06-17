//! sql651: a set-returning function (`generate_series`, `unnest`,
//! `jsonb_array_elements`, ...) in a `GROUP BY`, `HAVING`, or `ORDER BY` clause.
//! Like WHERE (sql645), these clauses don't allow SRFs; PostgreSQL raises 0A000.
//! Put the SRF in the FROM clause (often `LATERAL`) and group/order/filter over
//! its output. SRFs inside a `(SELECT ...)` subquery are legal and skipped.

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
    "sql651"
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

    for (needle, label) in [(&b"GROUP BY"[..], "GROUP BY"), (&b"HAVING"[..], "HAVING"), (&b"ORDER BY"[..], "ORDER BY")] {
      let mut from = 0usize;
      while let Some(rel) = find_clause(&ub[from..], needle).map(|p| p + from) {
        let pred_start = rel + needle.len();
        let pred_end = find_clause_end(ub, pred_start, STOP);
        scan(lb, pred_start, pred_end, start, label, out);
        from = pred_end.max(rel + needle.len());
      }
    }
  }
}

fn scan(lb: &[u8], lo: usize, hi: usize, start: usize, label: &str, out: &mut Vec<Diagnostic>) {
  let mut i = lo;
  while i < hi {
    if lb[i] == b'(' {
      let mut j = i + 1;
      while j < hi && lb[j].is_ascii_whitespace() {
        j += 1;
      }
      let sub = lb[j..hi.min(lb.len())].starts_with(b"select") || lb[j..hi.min(lb.len())].starts_with(b"with");
      if sub {
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
    if (i == lo || !is_word(lb[i - 1] as char)) && is_word(lb[i] as char) {
      for &srf in SRFS {
        let l = srf.len();
        if i + l <= hi && &lb[i..i + l] == srf.as_bytes() && lb.get(i + l).is_none_or(|&b| !is_word(b as char)) {
          let mut p = i + l;
          while p < hi && lb[p].is_ascii_whitespace() {
            p += 1;
          }
          if p < hi && lb[p] == b'(' {
            out.push(Diagnostic {
              code: "sql651",
              severity: Severity::Error,
              message: format!("`{srf}` is a set-returning function and isn't allowed in {label} (PG 0A000) -- move it into FROM (LATERAL) or a subquery"),
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
