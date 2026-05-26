//! sql054: `WHERE x = true` / `WHERE x = false` -- redundant boolean
//! comparison.
//!
//! `WHERE active = true` should be `WHERE active`. The shorter form
//! reads better and the planner sometimes picks different paths for
//! boolean expressions in predicate position.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::{Statement, StatementKind};
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql054"
  }
  fn default_severity(&self) -> Severity {
    Severity::Hint
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    // Scan original body bytes case-insensitively. We skip over
    // single-quoted strings so `'true'` doesn't trip the rule.
    let start: usize = u32::from(stmt.range.start()) as usize;
    let end: usize = (u32::from(stmt.range.end()) as usize).min(source.len());
    let body = &source[start..end];
    // Skip SET statements + DDL with key=value option lists (PROVIDER /
    // LOCALE / DETERMINISTIC for COLLATION; storage parameters for
    // tables; FDW OPTIONS etc). Strip comments first so a leading
    // `-- header` line doesn't mask the keyword anchor.
    let cleaned = strip_quoted_and_comments(body);
    let trimmed = cleaned.trim_start().to_ascii_uppercase();
    if trimmed.starts_with("SET ") || trimmed.starts_with("RESET ") || trimmed.starts_with("ALTER SYSTEM ") {
      return;
    }
    // CREATE / ALTER COLLATION/EXTENSION/SUBSCRIPTION/PUBLICATION/SERVER/
    // FOREIGN TABLE/INDEX/USER MAPPING all carry `(key = value, ...)`
    // option lists where `key = false` is a config value, not a
    // predicate. Treat any CREATE/ALTER stmt that contains `OPTIONS (`
    // or `WITH (` followed by k=v pairs as opt-out from this rule.
    if (trimmed.starts_with("CREATE ") || trimmed.starts_with("ALTER ") || trimmed.starts_with("COPY "))
      && contains_with_or_options_paren(&trimmed)
    {
      return;
    }
    if trimmed.starts_with("CREATE COLLATION") || trimmed.starts_with("CREATE EXTENSION") {
      return;
    }
    let bytes = body.as_bytes();
    let n = bytes.len();
    let needles: &[(&[u8], &str)] = &[
      (b"= TRUE", "drop `= true`"),
      (b"=TRUE", "drop `=true`"),
      (b"= FALSE", "use `NOT <expr>` instead of `= false`"),
      (b"=FALSE", "use `NOT <expr>` instead of `=false`"),
    ];
    let mut i = 0;
    while i < n {
      // Skip single-quoted string contents.
      if bytes[i] == b'\'' {
        i += 1;
        while i < n && bytes[i] != b'\'' {
          i += 1;
        }
        if i < n {
          i += 1;
        }
        continue;
      }
      for (needle, advice) in needles {
        if i + needle.len() > n {
          continue;
        }
        if !bytes[i..i + needle.len()].eq_ignore_ascii_case(needle) {
          continue;
        }
        let end_pos = i + needle.len();
        let next_ok = end_pos == n || !is_word(bytes[end_pos] as char);
        if !next_ok {
          continue;
        }
        let abs_start = start + i;
        let abs_end = start + end_pos;
        out.push(Diagnostic {
          code: "sql054",
          severity: Severity::Hint,
          message: format!("redundant boolean comparison -- {}", advice),
          range: text_size::TextRange::new((abs_start as u32).into(), (abs_end as u32).into()),
        });
        return;
      }
      i += 1;
    }
  }
}

fn strip_quoted_and_comments(s: &str) -> String {
  let mut out = String::with_capacity(s.len());
  let bytes = s.as_bytes();
  let n = bytes.len();
  let mut i = 0;
  while i < n {
    if i + 1 < n && bytes[i] == b'-' && bytes[i + 1] == b'-' {
      while i < n && bytes[i] != b'\n' {
        i += 1;
      }
    } else if i + 1 < n && bytes[i] == b'/' && bytes[i + 1] == b'*' {
      i += 2;
      while i + 1 < n && !(bytes[i] == b'*' && bytes[i + 1] == b'/') {
        i += 1;
      }
      i = (i + 2).min(n);
    } else if bytes[i] == b'\'' {
      i += 1;
      while i < n && bytes[i] != b'\'' {
        i += 1;
      }
      if i < n {
        i += 1;
      }
    } else {
      out.push(bytes[i] as char);
      i += 1;
    }
  }
  out
}

fn is_word(c: char) -> bool {
  c.is_alphanumeric() || c == '_'
}

/// True if `s` contains `WITH (` or `OPTIONS (` as a whole keyword
/// (i.e. preceded by whitespace / start, optionally with whitespace
/// before the `(`). Catches multi-line forms like
/// `CREATE VIEW v\nWITH (security_barrier = true)`.
fn contains_with_or_options_paren(s: &str) -> bool {
  let bytes = s.as_bytes();
  let n = bytes.len();
  for kw in ["WITH", "OPTIONS"] {
    let mut from = 0usize;
    while let Some(rel) = s[from..].find(kw) {
      let at = from + rel;
      let prev_ok = at == 0 || !{ let c = bytes[at - 1] as char; c.is_ascii_alphanumeric() || c == '_' };
      let after = at + kw.len();
      let next_ok = after < n && !{ let c = bytes[after] as char; c.is_ascii_alphanumeric() || c == '_' };
      if prev_ok && next_ok {
        // Walk forward through whitespace; require `(` next.
        let mut k = after;
        while k < n && bytes[k].is_ascii_whitespace() { k += 1 }
        if k < n && bytes[k] == b'(' { return true; }
      }
      from = at + kw.len();
    }
  }
  false
}
