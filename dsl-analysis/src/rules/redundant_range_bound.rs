//! sql550: `WHERE x > 5 AND x > 3` -- two bounds in the same direction on one
//! column. Only the tighter one matters (`x > 5` here); the looser bound is
//! dead weight. Usually a leftover from editing a range. (sql527 owns the
//! contradictory opposite-direction case; this is the same-direction one.)

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
    "sql550"
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

#[derive(Default)]
struct Dirs {
  lowers: u32, // `>` / `>=`
  uppers: u32, // `<` / `<=`
  lo: usize,
  hi: usize,
}

fn scan(body: &str, abs: usize, from: usize, to: usize, out: &mut Vec<Diagnostic>) {
  let mut cols: HashMap<String, Dirs> = HashMap::new();
  for (s, e) in split_on_and(body, from, to) {
    if let Some((col, is_lower)) = parse_dir(&body[s..e]) {
      let raw = &body[s..e];
      let span_s = s + (raw.len() - raw.trim_start().len());
      let span_e = s + raw.trim_end().len();
      let d = cols.entry(col).or_insert(Dirs { lowers: 0, uppers: 0, lo: span_s, hi: span_e });
      d.lo = d.lo.min(span_s);
      d.hi = d.hi.max(span_e);
      if is_lower {
        d.lowers += 1;
      } else {
        d.uppers += 1;
      }
    }
  }
  for (col, d) in cols {
    if d.lowers >= 2 || d.uppers >= 2 {
      let dir = if d.lowers >= 2 { "lower" } else { "upper" };
      out.push(Diagnostic {
        code: "sql550",
        severity: Severity::Hint,
        message: format!("`{col}` has multiple {dir} bounds -- only the tightest matters; the others are redundant"),
        range: crate::range_at(abs + d.lo, abs + d.hi),
      });
    }
  }
}

/// Parse `<simple_ident> {> | >= | < | <=} <number>` -> (col, is_lower).
fn parse_dir(term: &str) -> Option<(String, bool)> {
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
        found = Some((i, if bytes.get(i + 1) == Some(&b'=') { 2 } else { 1 }, true));
        break;
      },
      b'<' if depth == 0 => {
        if bytes.get(i + 1) == Some(&b'>') {
          return None;
        }
        found = Some((i, if bytes.get(i + 1) == Some(&b'=') { 2 } else { 1 }, false));
        break;
      },
      _ => {},
    }
    i += 1;
  }
  let (pos, oplen, is_lower) = found?;
  let lhs = term[..pos].trim();
  let rhs = term[pos + oplen..].trim();
  let (qual, name) = parse_simple_ident(lhs)?;
  // Only count literal-number bounds, so `x > a AND x > b` (column bounds,
  // which may be unrelated) doesn't get a false "redundant" call.
  rhs.parse::<f64>().ok()?;
  let key = match qual {
    Some(q) => format!("{}.{}", q.to_ascii_lowercase(), name.to_ascii_lowercase()),
    None => name.to_ascii_lowercase(),
  };
  Some((key, is_lower))
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
