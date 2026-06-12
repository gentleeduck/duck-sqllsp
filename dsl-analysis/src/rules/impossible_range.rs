//! sql527: `WHERE col > 5 AND col < 3` -- the lower and upper bounds on a
//! column don't overlap, so the range is empty and the query returns nothing.
//! Type-independent: only flagged when the bounds are empty for *any* numeric
//! domain (`lo > hi`, or `lo == hi` with a strict `<`/`>` on either side), so
//! `col > 5 AND col < 6` (empty for ints, not for numerics) is left alone.

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
    "sql527"
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
        let ps = rel + needle.len();
        let pe = find_clause_end(ub, ps, STOPWORDS);
        scan(body, start, ps, pe, out);
        from = pe.max(ps);
      }
    }
  }
}

// (value, strict?)
#[derive(Default)]
struct Bounds {
  lowers: Vec<(f64, bool)>,
  uppers: Vec<(f64, bool)>,
}

fn scan(body: &str, abs: usize, from: usize, to: usize, out: &mut Vec<Diagnostic>) {
  let mut cols: HashMap<String, Bounds> = HashMap::new();
  for (s, e) in split_on_and(body, from, to) {
    if let Some((col, lower, val, strict)) = parse_range(&body[s..e]) {
      let b = cols.entry(col).or_default();
      if lower {
        b.lowers.push((val, strict));
      } else {
        b.uppers.push((val, strict));
      }
    }
  }
  for (col, b) in cols {
    let (Some(lo), Some(hi)) = (tightest(&b.lowers, true), tightest(&b.uppers, false)) else { continue };
    let empty = lo.0 > hi.0 || (lo.0 == hi.0 && (lo.1 || hi.1));
    if empty {
      out.push(Diagnostic {
        code: "sql527",
        severity: Severity::Warning,
        message: format!("`{col}` lower and upper bounds don't overlap -- this range is always empty"),
        range: crate::range_at(abs + from, abs + body[..to].trim_end().len()),
      });
    }
  }
}

/// The binding bound: the max value for lowers / min value for uppers, strict
/// if any entry at that extreme is strict.
fn tightest(bounds: &[(f64, bool)], lower: bool) -> Option<(f64, bool)> {
  let val = bounds.iter().map(|(v, _)| *v).fold(None, |acc: Option<f64>, v| {
    Some(match acc {
      None => v,
      Some(a) if lower => a.max(v),
      Some(a) => a.min(v),
    })
  })?;
  let strict = bounds.iter().any(|(v, s)| *v == val && *s);
  Some((val, strict))
}

/// Parse `<simple_ident> {> | >= | < | <=} <number>` -> (col, is_lower, value,
/// strict). Equality / not-equal are left to other rules.
fn parse_range(term: &str) -> Option<(String, bool, f64, bool)> {
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
        let eq = bytes.get(i + 1) == Some(&b'=');
        found = Some((i, if eq { 2 } else { 1 }, true, !eq));
        break;
      },
      b'<' if depth == 0 => {
        // `<>` is not-equal, not a range op.
        if bytes.get(i + 1) == Some(&b'>') {
          return None;
        }
        let eq = bytes.get(i + 1) == Some(&b'=');
        found = Some((i, if eq { 2 } else { 1 }, false, !eq));
        break;
      },
      _ => {},
    }
    i += 1;
  }
  let (pos, oplen, is_lower, strict) = found?;
  let lhs = term[..pos].trim();
  let rhs = term[pos + oplen..].trim();
  let (qual, name) = parse_simple_ident(lhs)?;
  let key = match qual {
    Some(q) => format!("{}.{}", q.to_ascii_lowercase(), name.to_ascii_lowercase()),
    None => name.to_ascii_lowercase(),
  };
  let val: f64 = rhs.parse().ok()?;
  Some((key, is_lower, val, strict))
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
