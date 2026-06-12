//! sql523: `WHERE col IS NULL OR col IS NOT NULL` -- the two halves cover
//! every possible value of `col`, so the disjunction is always true and the
//! whole predicate is a no-op filter. Usually a leftover from refactoring a
//! real condition, or a misunderstanding of three-valued logic. (Pairs with
//! sql435, which catches the always-false `IS NULL AND <strict op>` form.)

use crate::clause_scan::{find_clause, find_clause_end, is_word};
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;
use std::collections::HashMap;

const STOPWORDS: &[&str] =
  &["GROUP", "ORDER", "HAVING", "LIMIT", "OFFSET", "WINDOW", "RETURNING", "UNION", "INTERSECT", "EXCEPT", "FETCH", "FOR"];

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql523"
  }
  fn default_severity(&self) -> Severity {
    Severity::Warning
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, body, upper) = crate::stmt_body_upper(stmt, source);
    let ub = upper.as_bytes();

    for needle in [&b"WHERE"[..], &b"ON"[..], &b"HAVING"[..]] {
      let mut from = 0usize;
      while let Some(rel) = find_clause(&ub[from..], needle).map(|p| p + from) {
        let pred_start = rel + needle.len();
        let pred_end = find_clause_end(ub, pred_start, STOPWORDS);
        scan(body, upper.as_str(), start, pred_start, pred_end, out);
        from = pred_end.max(pred_start);
      }
    }
  }
}

fn scan(body: &str, upper: &str, abs: usize, from: usize, to: usize, out: &mut Vec<Diagnostic>) {
  // operand (lowercased) -> (saw_is_null, saw_is_not_null, span_lo, span_hi)
  let mut seen: HashMap<String, (bool, bool, usize, usize)> = HashMap::new();
  for (s, e) in split_on_or(body, from, to) {
    let raw = &body[s..e];
    let Some((operand, is_not)) = null_check(raw, &upper[s..e]) else { continue };
    let span_s = s + (raw.len() - raw.trim_start().len());
    let span_e = s + raw.trim_end().len();
    let key = operand.to_ascii_lowercase();
    let entry = seen.entry(key).or_insert((false, false, span_s, span_e));
    if is_not {
      entry.1 = true;
    } else {
      entry.0 = true;
    }
    entry.2 = entry.2.min(span_s);
    entry.3 = entry.3.max(span_e);
  }
  for (operand, (null, notnull, lo, hi)) in seen {
    if null && notnull {
      out.push(Diagnostic {
        code: "sql523",
        severity: Severity::Warning,
        message: format!("`{operand} IS NULL OR {operand} IS NOT NULL` is always true -- this filter does nothing"),
        range: crate::range_at(abs + lo, abs + hi),
      });
    }
  }
}

/// If `raw` (with its byte-aligned uppercased twin `raw_u`) is exactly
/// `<operand> IS [NOT] NULL`, return `(operand, is_not)`. `raw` and `raw_u`
/// share offsets because the body is ASCII-uppercased (length-preserving).
fn null_check<'a>(raw: &'a str, raw_u: &str) -> Option<(&'a str, bool)> {
  let tu = raw_u.trim();
  let (kw, is_not) = if tu.ends_with("IS NOT NULL") {
    ("IS NOT NULL", true)
  } else if tu.ends_with("IS NULL") {
    ("IS NULL", false)
  } else {
    return None;
  };
  let lead = raw_u.len() - raw_u.trim_start().len();
  let operand = raw[lead..lead + tu.len() - kw.len()].trim();
  if operand.is_empty() {
    return None;
  }
  Some((operand, is_not))
}

fn split_on_or(body: &str, from: usize, to: usize) -> Vec<(usize, usize)> {
  let bytes = body.as_bytes();
  let mut out = Vec::new();
  let mut depth = 0i32;
  let mut last = from;
  let mut i = from;
  while i < to {
    match bytes[i] {
      b'(' | b'[' => depth += 1,
      b')' | b']' => depth -= 1,
      b'\'' => {
        i += 1;
        while i < to && bytes[i] != b'\'' {
          i += 1;
        }
      },
      b'O' | b'o' if depth == 0 => {
        let is_or = bytes.get(i + 1).is_some_and(|b| b.eq_ignore_ascii_case(&b'R'));
        let prev_ok = i == from || !is_word(bytes[i - 1] as char);
        let next_ok = bytes.get(i + 2).is_none_or(|&b| !is_word(b as char));
        if is_or && prev_ok && next_ok {
          out.push((last, i));
          i += 2;
          last = i;
          continue;
        }
      },
      _ => {},
    }
    i += 1;
  }
  out.push((last, to));
  out
}
