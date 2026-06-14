//! sql544: `WHERE col >= 5 AND col <= 5` -- inclusive lower and upper bounds
//! on the same value. The range admits exactly one value, so it's just
//! `col = 5`, written more directly. (sql527 owns the *empty* cases like
//! `> 5 AND < 5`; this is the single-point case it deliberately leaves alone.)

use crate::clause_scan::{find_clause, find_clause_end, is_word, parse_simple_ident};
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
    "sql544"
  }
  fn default_severity(&self) -> Severity {
    Severity::Hint
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, body, upper) = crate::stmt_body_upper(stmt, source);
    let ub = upper.as_bytes();

    for needle in [&b"WHERE"[..], &b"ON"[..], &b"HAVING"[..]] {
      let mut from = 0usize;
      while let Some(rel) = find_clause(&ub[from..], needle).map(|p| p + from) {
        let ps = rel + needle.len();
        let pe = find_clause_end(ub, ps, STOPWORDS);
        scan(body, start, ps, pe, out);
        from = pe.max(ps);
      }
    }
  }
}

struct Bounds {
  ge: Option<f64>, // an inclusive `>=` value
  le: Option<f64>, // an inclusive `<=` value
  has_strict: bool, // any `<`/`>` seen -> not a pure inclusive single point
  span: (usize, usize),
}

fn scan(body: &str, abs: usize, from: usize, to: usize, out: &mut Vec<Diagnostic>) {
  let mut cols: HashMap<String, Bounds> = HashMap::new();
  for (s, e) in split_on_and(body, from, to) {
    if let Some((col, inclusive, is_lower, val)) = parse_bound(&body[s..e]) {
      let raw = &body[s..e];
      let span_s = s + (raw.len() - raw.trim_start().len());
      let span_e = s + raw.trim_end().len();
      let b = cols.entry(col).or_insert(Bounds { ge: None, le: None, has_strict: false, span: (span_s, span_e) });
      b.span.0 = b.span.0.min(span_s);
      b.span.1 = b.span.1.max(span_e);
      if !inclusive {
        b.has_strict = true;
      } else if is_lower {
        b.ge = Some(val);
      } else {
        b.le = Some(val);
      }
    }
  }
  for (col, b) in cols {
    if !b.has_strict
      && let (Some(lo), Some(hi)) = (b.ge, b.le)
      && lo == hi
    {
      out.push(Diagnostic {
        code: "sql544",
        severity: Severity::Hint,
        message: format!("`{col} >= {0} AND {col} <= {0}` admits one value -- write `{col} = {0}`", trim_f64(lo)),
        range: crate::range_at(abs + b.span.0, abs + b.span.1),
      });
    }
  }
}

fn trim_f64(v: f64) -> String {
  if v.fract() == 0.0 { format!("{}", v as i64) } else { format!("{v}") }
}

/// Parse `<col> {>= | <=} <number>` -> (col, inclusive=true, is_lower, value),
/// or a strict `<`/`>` bound -> (col, inclusive=false, ...) so the caller can
/// disqualify the column.
fn parse_bound(term: &str) -> Option<(String, bool, bool, f64)> {
  let bytes = term.as_bytes();
  let mut depth = 0i32;
  let mut i = 0usize;
  let mut found = None;
  while i < bytes.len() {
    match bytes[i] {
      b'(' | b'[' => depth += 1,
      b')' | b']' => depth -= 1,
      b'\'' => {
        i += 1;
        while i < bytes.len() && bytes[i] != b'\'' {
          i += 1;
        }
      },
      b'>' if depth == 0 => {
        let inc = bytes.get(i + 1) == Some(&b'=');
        found = Some((i, if inc { 2 } else { 1 }, inc, true));
        break;
      },
      b'<' if depth == 0 => {
        if bytes.get(i + 1) == Some(&b'>') {
          return None;
        }
        let inc = bytes.get(i + 1) == Some(&b'=');
        found = Some((i, if inc { 2 } else { 1 }, inc, false));
        break;
      },
      _ => {},
    }
    i += 1;
  }
  let (pos, oplen, inclusive, is_lower) = found?;
  let lhs = term[..pos].trim();
  let rhs = term[pos + oplen..].trim();
  let (qual, name) = parse_simple_ident(lhs)?;
  let key = match qual {
    Some(q) => format!("{}.{}", q.to_ascii_lowercase(), name.to_ascii_lowercase()),
    None => name.to_ascii_lowercase(),
  };
  let val: f64 = rhs.parse().ok()?;
  Some((key, inclusive, is_lower, val))
}

fn split_on_and(body: &str, from: usize, to: usize) -> Vec<(usize, usize)> {
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
      b'A' | b'a' if depth == 0 => {
        let is_and = i + 3 <= to && body[i..i + 3].eq_ignore_ascii_case("AND");
        let prev_ok = i == from || !is_word(bytes[i - 1] as char);
        let next_ok = bytes.get(i + 3).is_none_or(|&b| !is_word(b as char));
        if is_and && prev_ok && next_ok {
          out.push((last, i));
          i += 3;
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
