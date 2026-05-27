//! sql482: `HAVING <constant>` -- a constant HAVING is either
//! pointless (`HAVING TRUE`) or empties the result (`HAVING FALSE`
//! / `HAVING NULL`). Counterpart to the WHERE always-true/false
//! family for the HAVING clause.

use crate::clause_scan::{find_clause, find_clause_end};
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;
use text_size::TextRange;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql482"
  }
  fn default_severity(&self) -> Severity {
    Severity::Warning
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let start: usize = u32::from(stmt.range.start()) as usize;
    let end: usize = (u32::from(stmt.range.end()) as usize).min(source.len());
    let raw = &source[start..end];
    let cleaned = crate::textutil::strip_noise_full(raw);
    let upper = cleaned.to_ascii_uppercase();
    let bytes = upper.as_bytes();
    let Some(rel) = find_clause(bytes, b"HAVING") else {
      return;
    };
    let clause_end = find_clause_end(bytes, rel + 6, &["ORDER BY", "LIMIT", "OFFSET", "FOR", "FETCH", "WINDOW"]);
    let raw_clause = raw[rel + 6..clause_end].trim();
    if raw_clause.is_empty() {
      return;
    }
    let Some((kind, sev)) = classify(raw_clause) else {
      return;
    };
    let leading_ws = raw[rel + 6..clause_end].len() - raw[rel + 6..clause_end].trim_start().len();
    let abs_s = start + rel + 6 + leading_ws;
    let abs_e = abs_s + raw_clause.len();
    let msg = match kind {
      "TRUE" => "`HAVING TRUE` -- pointless filter, every group passes; drop the clause".into(),
      "FALSE" => "`HAVING FALSE` -- empties the result set; the query returns no rows".into(),
      "NULL" => "`HAVING NULL` -- NULL is treated as FALSE, empties the result set; the query returns no rows".into(),
      "STRING" => "`HAVING '<literal>'` -- a string-literal HAVING is a no-op (PG coerces it to bool TRUE if non-empty) or empties the result; almost certainly a placeholder".into(),
      _ => return,
    };
    out.push(Diagnostic {
      code: "sql482",
      severity: sev,
      message: msg,
      range: TextRange::new((abs_s as u32).into(), (abs_e as u32).into()),
    });
  }
}

fn classify(s: &str) -> Option<(&'static str, Severity)> {
  let t = s.trim();
  let u = t.to_ascii_uppercase();
  if u == "TRUE" {
    return Some(("TRUE", Severity::Hint));
  }
  if u == "FALSE" {
    return Some(("FALSE", Severity::Warning));
  }
  if u == "NULL" {
    return Some(("NULL", Severity::Warning));
  }
  if is_lone_string_literal(t) {
    return Some(("STRING", Severity::Warning));
  }
  None
}

/// True iff `t` is exactly one string literal -- no trailing operators,
/// no other expression after the closing quote. Distinguishes
/// `HAVING 'x'` (lone literal, sql482's target) from
/// `HAVING 'a' = 'a'` (a comparison; out of sql482's scope).
fn is_lone_string_literal(t: &str) -> bool {
  let bytes = t.as_bytes();
  let n = bytes.len();
  if n < 2 || bytes[0] != b'\'' {
    return false;
  }
  let mut i = 1usize;
  while i < n {
    if bytes[i] == b'\'' {
      // `''` is an escaped quote inside the literal.
      if i + 1 < n && bytes[i + 1] == b'\'' {
        i += 2;
        continue;
      }
      // Closing quote found at i. Everything after must be ws.
      return t[i + 1..].trim().is_empty();
    }
    i += 1;
  }
  false
}
