//! sql566: `WHERE x = x + 1` -- a column compared to itself plus/minus a
//! non-zero constant. It reduces to `0 = 1`, so it's always false and the
//! query returns nothing. Almost always a typo (a different column, or the
//! wrong side of an update expression).

use crate::clause_scan::{find_clause, find_clause_end, is_word, parse_simple_ident};
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

const STOPWORDS: &[&str] =
  &["GROUP", "ORDER", "HAVING", "LIMIT", "OFFSET", "WINDOW", "RETURNING", "UNION", "INTERSECT", "EXCEPT", "FETCH", "FOR"];

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql566"
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
        for (s, e) in split_on_and(body, ps, pe) {
          if let Some(col) = is_self_offset(&body[s..e]) {
            let raw = &body[s..e];
            let lead = raw.len() - raw.trim_start().len();
            out.push(Diagnostic {
              code: "sql566",
              severity: Severity::Warning,
              message: format!("`{}` is always false -- `{col}` cannot equal itself plus a non-zero amount", raw.trim()),
              range: crate::range_at(start + s + lead, start + s + raw.trim_end().len()),
            });
          }
        }
        from = pe.max(ps);
      }
    }
  }
}

/// `<col> = <col> {+|-} <nonzero number>` -> Some(col).
fn is_self_offset(term: &str) -> Option<String> {
  let bytes = term.as_bytes();
  // First top-level `=` (not <=, >=, <>, !=).
  let mut depth = 0i32;
  let mut i = 0usize;
  let mut eq = None;
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
      b'=' if depth == 0 => {
        let prev = if i > 0 { bytes[i - 1] } else { b' ' };
        if !matches!(prev, b'<' | b'>' | b'!') && bytes.get(i + 1) != Some(&b'=') {
          eq = Some(i);
          break;
        }
      },
      _ => {},
    }
    i += 1;
  }
  let eq = eq?;
  let (_, lhs_name) = parse_simple_ident(term[..eq].trim())?;
  let rhs = term[eq + 1..].trim();
  // rhs = <ident> <op> <number>
  let rb = rhs.as_bytes();
  let mut j = 0usize;
  while j < rb.len() && (is_word(rb[j] as char) || rb[j] == b'.') {
    j += 1;
  }
  if j == 0 {
    return None;
  }
  let (_, rhs_name) = parse_simple_ident(rhs[..j].trim())?;
  if !rhs_name.eq_ignore_ascii_case(&lhs_name) {
    return None;
  }
  let rest = rhs[j..].trim_start();
  let rest_b = rest.as_bytes();
  if !matches!(rest_b.first(), Some(b'+') | Some(b'-')) {
    return None;
  }
  let num = rest[1..].trim();
  // Non-zero numeric literal.
  match num.parse::<f64>() {
    Ok(v) if v != 0.0 => Some(lhs_name),
    _ => None,
  }
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
