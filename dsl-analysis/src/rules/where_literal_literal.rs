//! sql474: `WHERE 'a' = 'a'` (tautology), `WHERE 2 = 2` (tautology),
//! `WHERE 'a' = 'b'` (contradiction) -- a constant on both sides of
//! an equality is independent of any row's data. Tautologies are
//! noise; contradictions silently return zero rows. Pairs with
//! sql282 which handles the narrow `WHERE 1=1` placeholder case and
//! sql407 which handles `1 = 0` numeric contradictions.

use crate::clause_scan::{find_clause, find_clause_end, is_word};
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;
use text_size::TextRange;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql474"
  }
  fn default_severity(&self) -> Severity {
    Severity::Warning
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, raw) = crate::stmt_body(stmt, source);
    let cleaned = crate::textutil::strip_noise_full(raw);
    let upper = cleaned.to_ascii_uppercase();
    let ub = upper.as_bytes();
    let stopwords = ["GROUP BY", "ORDER BY", "LIMIT", "OFFSET", "HAVING", "FOR", "FETCH", "WINDOW", "RETURNING", "UNION", "INTERSECT", "EXCEPT"];
    let Some(rel_where) = find_clause(ub, b"WHERE") else {
      return;
    };
    let pred_start = rel_where + 5;
    let pred_end = find_clause_end(ub, pred_start, &stopwords);
    let pred = &raw[pred_start..pred_end.min(raw.len())];
    let conjuncts = split_top_level_and(pred);
    for (c, c_rel_off) in conjuncts {
      let trimmed = c.trim();
      // Match `<lit> = <lit>` (also `<>`, `!=`).
      let Some((op, lhs, rhs)) = split_eq(trimmed) else { continue };
      let Some(l_kind) = classify_literal(lhs.trim()) else { continue };
      let Some(r_kind) = classify_literal(rhs.trim()) else { continue };
      // Both must be the SAME kind of literal for the comparison to be
      // a true constant relation.
      if l_kind.kind != r_kind.kind {
        continue;
      }
      let equal = l_kind.norm == r_kind.norm;
      let is_eq = op == "=";
      let is_tautology = (equal && is_eq) || (!equal && !is_eq);
      let (severity, message) = if is_tautology {
        (
          Severity::Hint,
          format!(
            "`{trimmed}` is always TRUE -- both sides are constant literals; the predicate has no filter effect"
          ),
        )
      } else {
        (
          Severity::Warning,
          format!(
            "`{trimmed}` is always FALSE -- both sides are constant literals; the query returns zero rows regardless of data"
          ),
        )
      };
      let leading = c.len() - c.trim_start().len();
      let abs_s = start + pred_start + c_rel_off + leading;
      let abs_e = abs_s + trimmed.len();
      out.push(Diagnostic {
        code: "sql474",
        severity,
        message,
        range: TextRange::new((abs_s as u32).into(), (abs_e as u32).into()),
      });
    }
  }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum LitKind {
  Numeric,
  String,
  Boolean,
}

struct Lit {
  kind: LitKind,
  norm: String,
}

fn classify_literal(s: &str) -> Option<Lit> {
  let t = s.trim();
  if t.is_empty() {
    return None;
  }
  // String literal: `'...'` with no embedded unquoted text.
  if t.starts_with('\'') && t.ends_with('\'') && t.len() >= 2 {
    return Some(Lit { kind: LitKind::String, norm: t.to_string() });
  }
  // Boolean.
  let u = t.to_ascii_uppercase();
  if u == "TRUE" || u == "FALSE" {
    return Some(Lit { kind: LitKind::Boolean, norm: u });
  }
  // Numeric.
  if t.parse::<f64>().is_ok() {
    return Some(Lit { kind: LitKind::Numeric, norm: t.to_string() });
  }
  None
}

/// Split on top-level `=`, `<>`, or `!=`. Returns (op, lhs, rhs).
fn split_eq(s: &str) -> Option<(&'static str, &str, &str)> {
  let bytes = s.as_bytes();
  let n = bytes.len();
  let mut depth: i32 = 0;
  let mut i = 0;
  while i < n {
    let c = bytes[i];
    if c == b'\'' {
      i += 1;
      while i < n && bytes[i] != b'\'' {
        i += 1;
      }
      i = (i + 1).min(n);
      continue;
    }
    if c == b'(' {
      depth += 1;
      i += 1;
      continue;
    }
    if c == b')' {
      depth -= 1;
      i += 1;
      continue;
    }
    if depth == 0 {
      // Two-char ops first.
      if i + 2 <= n {
        let two = &bytes[i..i + 2];
        if two == b"<>" || two == b"!=" {
          return Some((if two == b"<>" { "<>" } else { "!=" }, &s[..i], &s[i + 2..]));
        }
      }
      if c == b'=' {
        // Avoid matching `==`, `<=`, `>=`, `!=`, `:` (assignment).
        let prev_bad = i > 0 && matches!(bytes[i - 1], b'!' | b'<' | b'>' | b'=' | b':');
        let next_bad = i + 1 < n && bytes[i + 1] == b'=';
        if !prev_bad && !next_bad {
          return Some(("=", &s[..i], &s[i + 1..]));
        }
      }
    }
    i += 1;
  }
  None
}

fn split_top_level_and(s: &str) -> Vec<(String, usize)> {
  let bytes = s.as_bytes();
  let upper: String = s.to_ascii_uppercase();
  let ub = upper.as_bytes();
  let n = bytes.len();
  let mut out: Vec<(String, usize)> = Vec::new();
  let mut last = 0usize;
  let mut depth: i32 = 0;
  let mut i = 0;
  while i < n {
    let c = bytes[i];
    if c == b'\'' {
      i += 1;
      while i < n && bytes[i] != b'\'' {
        i += 1;
      }
      i = (i + 1).min(n);
      continue;
    }
    if c == b'(' {
      depth += 1;
      i += 1;
      continue;
    }
    if c == b')' {
      depth -= 1;
      i += 1;
      continue;
    }
    if depth == 0
      && i + 3 <= n
      && &ub[i..i + 3] == b"AND"
      && (i == 0 || !is_word(ub[i - 1] as char))
      && (i + 3 == n || !is_word(ub[i + 3] as char))
    {
      out.push((s[last..i].to_string(), last));
      last = i + 3;
      i += 3;
      continue;
    }
    i += 1;
  }
  out.push((s[last..].to_string(), last));
  out
}
