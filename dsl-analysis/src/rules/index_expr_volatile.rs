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
  "random",
  "now",
  "clock_timestamp",
  "statement_timestamp",
  "transaction_timestamp",
  "current_timestamp",
  "current_time",
  "current_date",
  "localtime",
  "localtimestamp",
  "gen_random_uuid",
  "uuid_generate_v1",
  "uuid_generate_v4",
  "nextval",
  "currval",
  "lastval",
  "setval",
  "txid_current",
  "pg_backend_pid",
  "pg_advisory_lock",
];

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql213"
  }
  fn default_severity(&self) -> Severity {
    Severity::Error
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, raw) = crate::stmt_body(stmt, source);
    // Strip comments + strings before scanning -- prevents matching
    // `INDEX` inside `-- INCLUDE, partial indexes, ...` header
    // comments (the comment contains `INDEXES` which substring-matches
    // `INDEX`).
    let body_owned = crate::textutil::strip_comments_only(raw);
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
    let Some(idx_at) = crate::textutil::find_word(&upper, "INDEX") else { return };
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
          range: crate::range_at(abs_s, abs_e),
        });
        return;
      }
    }
  }
}
fn find_matching_paren(s: &str, open: usize) -> Option<usize> {
  let bytes = s.as_bytes();
  let mut depth = 0i32;
  let mut i = open;
  while i < bytes.len() {
    match bytes[i] {
      b'(' => depth += 1,
      b')' => {
        depth -= 1;
        if depth == 0 {
          return Some(i);
        }
      },
      b'\'' => {
        i += 1;
        while i < bytes.len() && bytes[i] != b'\'' {
          i += 1
        }
      },
      _ => {},
    }
    i += 1;
  }
  None
}
