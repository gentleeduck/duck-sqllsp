//! sql497: `array_agg(DISTINCT a ORDER BY b)` and similar -- PG
//! requires that, when DISTINCT is used inside an aggregate, every
//! ORDER BY expression must also appear in the aggregate's argument
//! list. Mismatch raises a runtime error:
//!   `in an aggregate with DISTINCT, ORDER BY expressions must appear
//!    in argument list`
//!
//! Covers array_agg / string_agg / json_agg / jsonb_agg /
//! json_object_agg / jsonb_object_agg / xmlagg.

use crate::clause_scan::is_word;
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;
use text_size::TextRange;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql497"
  }
  fn default_severity(&self) -> Severity {
    Severity::Error
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, raw) = crate::stmt_body(stmt, source);
    let cleaned = crate::textutil::strip_noise_full(raw);
    let lower = cleaned.to_ascii_lowercase();
    let lb = lower.as_bytes();
    let bytes = cleaned.as_bytes();
    let n = lb.len();
    let funcs: &[&[u8]] = &[
      b"jsonb_object_agg",
      b"json_object_agg",
      b"array_agg",
      b"string_agg",
      b"jsonb_agg",
      b"json_agg",
      b"xmlagg",
    ];
    let mut i = 0usize;
    while i < n {
      let mut name_len: Option<usize> = None;
      for kw in funcs {
        if i + kw.len() <= n
          && &lb[i..i + kw.len()] == *kw
          && (i == 0 || !is_word(lb[i - 1] as char))
          && (i + kw.len() == n || !is_word(lb[i + kw.len()] as char))
        {
          name_len = Some(kw.len());
          break;
        }
      }
      let Some(name_len) = name_len else {
        i += 1;
        continue;
      };
      let mut k = i + name_len;
      while k < n && bytes[k].is_ascii_whitespace() {
        k += 1;
      }
      if k >= n || bytes[k] != b'(' {
        i += name_len;
        continue;
      }
      let Some(close) = match_paren(bytes, k, n) else {
        i += name_len;
        continue;
      };
      let inner_start = k + 1;
      let inner_end = close;
      // Must contain a `DISTINCT` keyword at depth 0.
      let Some(distinct_at) = find_word_in_range(lb, b"distinct", inner_start, inner_end) else {
        i = close + 1;
        continue;
      };
      // Must contain an `ORDER BY` (two-word) at depth 0.
      let Some(order_at) = find_two_word(lb, b"order", b"by", inner_start, inner_end) else {
        i = close + 1;
        continue;
      };
      if order_at <= distinct_at {
        i = close + 1;
        continue;
      }
      // Extract DISTINCT arg(s) -- between distinct+8 and order_at.
      // For simplicity, take the comma-split list of identifiers
      // appearing there.
      let distinct_args_raw = cleaned[distinct_at + 8..order_at].trim();
      let distinct_idents = collect_idents(distinct_args_raw);
      // Extract ORDER BY columns -- everything after `order by`
      // until the close paren.
      let order_args_raw = cleaned[order_at + 8..inner_end].trim();
      let order_idents = collect_idents(order_args_raw);
      // If any order ident isn't in the distinct args, flag.
      let mismatch = order_idents.iter().any(|oi| !distinct_idents.iter().any(|di| di.eq_ignore_ascii_case(oi)));
      if mismatch {
        let abs_s = start + i;
        let abs_e = start + close + 1;
        out.push(Diagnostic {
          code: "sql497",
          severity: Severity::Error,
          message: "aggregate with `DISTINCT` requires every `ORDER BY` expression to also appear in the argument list -- PG raises `in an aggregate with DISTINCT, ORDER BY expressions must appear in argument list` at runtime. Either drop DISTINCT or order by the same expression.".into(),
          range: TextRange::new((abs_s as u32).into(), (abs_e as u32).into()),
        });
      }
      i = close + 1;
    }
  }
}

/// Pull bare identifiers (a, b.c, etc.) from a snippet -- used to
/// compare DISTINCT args to ORDER BY columns.
fn collect_idents(s: &str) -> Vec<String> {
  let mut out = Vec::new();
  let bytes = s.as_bytes();
  let n = bytes.len();
  let mut i = 0usize;
  while i < n {
    let c = bytes[i];
    if c.is_ascii_alphabetic() || c == b'_' {
      let start = i;
      while i < n && (is_word(bytes[i] as char) || bytes[i] == b'.') {
        i += 1;
      }
      let word = std::str::from_utf8(&bytes[start..i]).unwrap_or("");
      // Skip SQL keywords that aren't column refs.
      let up = word.to_ascii_uppercase();
      if !matches!(up.as_str(), "ASC" | "DESC" | "NULLS" | "FIRST" | "LAST") {
        out.push(word.to_string());
      }
      continue;
    }
    if c == b'\'' {
      i += 1;
      while i < n && bytes[i] != b'\'' {
        i += 1;
      }
      i = (i + 1).min(n);
      continue;
    }
    i += 1;
  }
  out
}

fn find_word_in_range(lb: &[u8], w: &[u8], from: usize, to: usize) -> Option<usize> {
  let m = w.len();
  let mut depth: i32 = 0;
  let mut i = from;
  while i + m <= to {
    let c = lb[i];
    if c == b'\'' {
      i += 1;
      while i < to && lb[i] != b'\'' {
        i += 1;
      }
      i = (i + 1).min(to);
      continue;
    }
    if c == b'(' {
      depth += 1;
    } else if c == b')' {
      depth -= 1;
    } else if depth == 0
      && &lb[i..i + m] == w
      && (i == 0 || !is_word(lb[i - 1] as char))
      && (i + m == lb.len() || !is_word(lb[i + m] as char))
    {
      return Some(i);
    }
    i += 1;
  }
  None
}

fn find_two_word(lb: &[u8], w1: &[u8], w2: &[u8], from: usize, to: usize) -> Option<usize> {
  let mut search_from = from;
  while let Some(p) = find_word_in_range(lb, w1, search_from, to) {
    let mut k = p + w1.len();
    while k < to && lb[k].is_ascii_whitespace() {
      k += 1;
    }
    if k + w2.len() <= to
      && &lb[k..k + w2.len()] == w2
      && (k + w2.len() == lb.len() || !is_word(lb[k + w2.len()] as char))
    {
      // Return position of w1 + len(w1) + 1 + len(w2) for "order by"
      // so caller can split correctly. Actually return position so
      // caller can use `+ w1.len() + 1 + w2.len()` = 8 for "order by".
      return Some(p);
    }
    search_from = p + w1.len();
  }
  None
}

fn match_paren(bytes: &[u8], open: usize, end: usize) -> Option<usize> {
  let mut depth: i32 = 0;
  let mut i = open;
  while i < end {
    let c = bytes[i];
    if c == b'\'' {
      i += 1;
      while i < end && bytes[i] != b'\'' {
        i += 1;
      }
      i = (i + 1).min(end);
      continue;
    }
    if c == b'(' {
      depth += 1;
    } else if c == b')' {
      depth -= 1;
      if depth == 0 {
        return Some(i);
      }
    }
    i += 1;
  }
  None
}
