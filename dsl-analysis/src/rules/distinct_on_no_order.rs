//! sql101: `SELECT DISTINCT ON (x) ... FROM t` without an `ORDER BY`
//! that starts with `x` -- which row PG returns is undefined.

use crate::{Diagnostic, LintRule, Severity};
use crate::textutil::is_word;
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql101"
  }
  fn default_severity(&self) -> Severity {
    Severity::Warning
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, raw) = crate::stmt_body(stmt, source);
    let body_owned = crate::textutil::strip_noise_full(raw);
    let body = body_owned.as_str();
    let upper = body.to_ascii_uppercase();
    let bytes = upper.as_bytes();
    let n = bytes.len();
    // Find `DISTINCT ON (...)`.
    let Some(dist_at) = upper.find("DISTINCT ON") else { return };
    let prev_ok = dist_at == 0 || !is_word(bytes[dist_at - 1] as char);
    let next_ok = dist_at + 11 == n || !is_word(bytes[dist_at + 11] as char);
    if !(prev_ok && next_ok) {
      return;
    }
    let mut j = dist_at + 11;
    while j < n && bytes[j].is_ascii_whitespace() {
      j += 1;
    }
    if j >= n || bytes[j] != b'(' {
      return;
    }
    // Collect first identifier inside the parens (good enough --
    // matching all keys is harder than worth here).
    let mut k = j + 1;
    while k < n && bytes[k].is_ascii_whitespace() {
      k += 1;
    }
    let id_start = k;
    while k < n && (is_word(bytes[k] as char) || bytes[k] == b'.') {
      k += 1;
    }
    let id_end = k;
    if id_end == id_start {
      return;
    }
    let first_key = &upper[id_start..id_end];
    // Now look for ORDER BY <first_key>... in the rest of the stmt.
    let rest = &upper[id_end..];
    let has_order = rest.find("ORDER BY").is_some();
    let matches_order = if has_order {
      let ob = rest.find("ORDER BY").unwrap();
      let after = rest[ob + 8..].trim_start();
      // The first ORDER BY key should start with first_key (allow
      // qualified col like a.id matching id, or vice-versa).
      after
        .split(|c: char| c == ',' || c.is_ascii_whitespace())
        .next()
        .map(|tok| {
          let tok_tail = tok.rsplit('.').next().unwrap_or(tok);
          let key_tail = first_key.rsplit('.').next().unwrap_or(first_key);
          tok == first_key || tok_tail == key_tail
        })
        .unwrap_or(false)
    } else {
      false
    };
    if matches_order {
      return;
    }
    let abs_start = start + dist_at;
    let abs_end = start + dist_at + 11;
    out.push(Diagnostic {
      code: "sql101",
      severity: Severity::Warning,
      message: "DISTINCT ON without matching ORDER BY -- which row PG returns is undefined".into(),
      range: crate::range_at(abs_start, abs_end),
    });
  }
}

