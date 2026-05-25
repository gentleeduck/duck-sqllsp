//! sql213: `CREATE INDEX ... (expr)` where `expr` calls a known-
//! volatile function (random / now / clock_timestamp / nextval /
//! gen_random_uuid / etc). PG raises 42P17 "functions in index
//! expression must be marked IMMUTABLE" at runtime.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

const VOLATILE: &[&str] = &[
  "random", "now", "clock_timestamp", "statement_timestamp", "transaction_timestamp",
  "current_timestamp", "current_time", "current_date", "localtime", "localtimestamp",
  "gen_random_uuid", "uuid_generate_v1", "uuid_generate_v4", "nextval", "currval",
  "lastval", "setval", "txid_current", "pg_backend_pid", "pg_advisory_lock",
];

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql213"
  }
  fn default_severity(&self) -> Severity {
    Severity::Error
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let start: usize = u32::from(stmt.range.start()) as usize;
    let end: usize = (u32::from(stmt.range.end()) as usize).min(source.len());
    let raw = &source[start..end];
    // Strip comments + strings before scanning -- prevents matching
    // `INDEX` inside `-- INCLUDE, partial indexes, ...` header
    // comments (the comment contains `INDEXES` which substring-matches
    // `INDEX`).
    let body_owned = strip_noise(raw);
    let body = body_owned.as_str();
    let upper = body.to_ascii_uppercase();
    // Verify the statement actually starts with CREATE [UNIQUE] INDEX.
    let trimmed = upper.trim_start();
    if !(trimmed.starts_with("CREATE INDEX")
      || trimmed.starts_with("CREATE UNIQUE INDEX")
      || trimmed.starts_with("CREATE OR REPLACE INDEX"))
    {
      return;
    }
    // Word-bounded `INDEX` search (avoid hitting `INDEXES` in residual
    // text -- defensive after strip_noise).
    let Some(idx_at) = find_word(&upper, "INDEX") else { return };
    let after_idx = idx_at + "INDEX".len();
    let Some(open_rel) = body[after_idx..].find('(') else { return };
    let open = after_idx + open_rel;
    let Some(close) = find_matching_paren(body, open) else { return };
    let cols = &body[open + 1..close];
    let cols_lc = cols.to_ascii_lowercase();
    for v in VOLATILE {
      let needle = format!("{v}(");
      if let Some(rel) = cols_lc.find(&needle) {
        let abs_s = start + open + 1 + rel;
        let abs_e = abs_s + v.len();
        out.push(Diagnostic {
          code: "sql213",
          severity: Severity::Error,
          message: format!(
            "CREATE INDEX expression calls volatile `{v}()` -- PG raises 42P17, functions in index expr must be IMMUTABLE"
          ),
          range: text_size::TextRange::new((abs_s as u32).into(), (abs_e as u32).into()),
        });
        return;
      }
    }
  }
}

fn find_word(haystack: &str, needle: &str) -> Option<usize> {
  let h = haystack.as_bytes();
  let n = needle.as_bytes();
  if n.is_empty() { return None }
  let mut i = 0usize;
  while i + n.len() <= h.len() {
    if h[i..i + n.len()] == *n {
      let prev_ok = i == 0 || !(h[i - 1].is_ascii_alphanumeric() || h[i - 1] == b'_');
      let next_ok = i + n.len() == h.len() || !(h[i + n.len()].is_ascii_alphanumeric() || h[i + n.len()] == b'_');
      if prev_ok && next_ok { return Some(i) }
    }
    i += 1;
  }
  None
}

fn strip_noise(s: &str) -> String {
  let mut out: Vec<u8> = s.as_bytes().to_vec();
  let n = out.len();
  let mut i = 0usize;
  while i < n {
    if i + 1 < n && out[i] == b'-' && out[i + 1] == b'-' {
      while i < n && out[i] != b'\n' { out[i] = b' '; i += 1 }
      continue;
    }
    if i + 1 < n && out[i] == b'/' && out[i + 1] == b'*' {
      let mut depth = 1u32;
      out[i] = b' '; out[i + 1] = b' '; i += 2;
      while i + 1 < n && depth > 0 {
        if out[i] == b'/' && out[i + 1] == b'*' { depth += 1; out[i] = b' '; out[i + 1] = b' '; i += 2; }
        else if out[i] == b'*' && out[i + 1] == b'/' { depth -= 1; out[i] = b' '; out[i + 1] = b' '; i += 2; }
        else { out[i] = b' '; i += 1; }
      }
      continue;
    }
    if out[i] == b'\'' {
      out[i] = b' '; i += 1;
      while i < n && out[i] != b'\'' { out[i] = b' '; i += 1 }
      if i < n { out[i] = b' '; i += 1 }
      continue;
    }
    i += 1;
  }
  String::from_utf8(out).unwrap_or_else(|_| s.to_string())
}

fn find_matching_paren(s: &str, open: usize) -> Option<usize> {
  let bytes = s.as_bytes();
  let mut depth = 0i32;
  let mut i = open;
  while i < bytes.len() {
    match bytes[i] {
      b'(' => depth += 1,
      b')' => { depth -= 1; if depth == 0 { return Some(i); } }
      b'\'' => {
        i += 1;
        while i < bytes.len() && bytes[i] != b'\'' { i += 1 }
      }
      _ => {}
    }
    i += 1;
  }
  None
}
