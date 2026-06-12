//! sql526: `WHERE col = 1 AND col = 2` -- the same column is required to equal
//! two different constants at once, so the predicate is always false and the
//! query returns nothing. Also catches `col = 1 AND col <> 1` (same value
//! demanded and forbidden). Usually a copy-paste slip or a bad codegen
//! template. (Pairs with sql407, which handles literal-only `1 = 2`.)

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
    "sql526"
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

#[derive(Clone, PartialEq)]
enum Lit {
  Num(f64),
  Str(String),
  Bool(bool),
}

fn scan(body: &str, abs: usize, from: usize, to: usize, out: &mut Vec<Diagnostic>) {
  // col -> (equal values seen, not-equal values seen), with the clause span.
  let mut eqs: HashMap<String, Vec<Lit>> = HashMap::new();
  let mut neqs: HashMap<String, Vec<Lit>> = HashMap::new();
  let mut reported: std::collections::HashSet<String> = Default::default();

  for (s, e) in split_on_and(body, from, to) {
    let Some((col, op, lit)) = parse_eq(&body[s..e]) else { continue };
    let bucket = if op == Op::Eq { &mut eqs } else { &mut neqs };
    bucket.entry(col.clone()).or_default().push(lit.clone());

    // Contradiction checks against what we've already seen for this column.
    let contradiction = match op {
      Op::Eq => {
        eqs.get(&col).is_some_and(|vs| vs.iter().any(|v| lit_conflicts(v, &lit)))
          || neqs.get(&col).is_some_and(|vs| vs.iter().any(|v| v == &lit))
      },
      Op::Neq => eqs.get(&col).is_some_and(|vs| vs.iter().any(|v| v == &lit)),
    };
    if contradiction && reported.insert(col.clone()) {
      out.push(Diagnostic {
        code: "sql526",
        severity: Severity::Warning,
        message: format!("`{col}` is constrained to conflicting values -- this predicate is always false"),
        range: crate::range_at(abs + from, abs + body[..to].trim_end().len()),
      });
    }
  }
}

#[derive(PartialEq)]
enum Op {
  Eq,
  Neq,
}

/// Two `=` literals conflict iff they're the same kind but different value
/// (so `col = 1 AND col = 1.0` does *not* conflict).
fn lit_conflicts(a: &Lit, b: &Lit) -> bool {
  match (a, b) {
    (Lit::Num(x), Lit::Num(y)) => x != y,
    (Lit::Str(x), Lit::Str(y)) => x != y,
    (Lit::Bool(x), Lit::Bool(y)) => x != y,
    _ => false,
  }
}

/// Parse `<simple_ident> {= | <> | !=} <literal>` -> (col_key, op, literal).
/// Range operators (`<`, `>`, `<=`, `>=`) yield None -- sql527 owns those.
fn parse_eq(term: &str) -> Option<(String, Op, Lit)> {
  let bytes = term.as_bytes();
  let mut depth = 0i32;
  let mut i = 0usize;
  let mut op_at = None;
  while i < bytes.len() {
    let b = bytes[i];
    match b {
      b'(' | b'[' => depth += 1,
      b')' | b']' => depth -= 1,
      b'\'' => {
        i += 1;
        while i < bytes.len() && bytes[i] != b'\'' {
          i += 1;
        }
      },
      b'<' if depth == 0 && bytes.get(i + 1) == Some(&b'>') => {
        op_at = Some((i, Op::Neq, 2));
        break;
      },
      b'!' if depth == 0 && bytes.get(i + 1) == Some(&b'=') => {
        op_at = Some((i, Op::Neq, 2));
        break;
      },
      b'=' if depth == 0 => {
        let prev = if i > 0 { bytes[i - 1] } else { b' ' };
        // Not part of `<=`, `>=`, or `==`.
        if !matches!(prev, b'<' | b'>' | b'!') && bytes.get(i + 1) != Some(&b'=') {
          op_at = Some((i, Op::Eq, 1));
          break;
        }
      },
      _ => {},
    }
    i += 1;
  }
  let (pos, op, oplen) = op_at?;
  let lhs = term[..pos].trim();
  let rhs = term[pos + oplen..].trim();
  let (qual, name) = parse_simple_ident(lhs)?;
  let key = match qual {
    Some(q) => format!("{}.{}", q.to_ascii_lowercase(), name.to_ascii_lowercase()),
    None => name.to_ascii_lowercase(),
  };
  let lit = classify_literal(rhs)?;
  Some((key, op, lit))
}

fn classify_literal(s: &str) -> Option<Lit> {
  let t = s.trim();
  if t.len() >= 2 && t.starts_with('\'') && t.ends_with('\'') {
    return Some(Lit::Str(t[1..t.len() - 1].to_string()));
  }
  if t.eq_ignore_ascii_case("TRUE") {
    return Some(Lit::Bool(true));
  }
  if t.eq_ignore_ascii_case("FALSE") {
    return Some(Lit::Bool(false));
  }
  if let Ok(n) = t.parse::<f64>() {
    return Some(Lit::Num(n));
  }
  None
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
