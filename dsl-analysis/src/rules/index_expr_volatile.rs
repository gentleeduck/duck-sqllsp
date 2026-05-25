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
    let body = &source[start..end];
    let upper = body.to_ascii_uppercase();
    if !upper.contains("CREATE") || !upper.contains("INDEX") { return }
    // Find first `(` after the INDEX keyword pair.
    let Some(idx_at) = upper.find("INDEX") else { return };
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
