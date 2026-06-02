//! sql475: `INSERT INTO t SELECT ... FROM t` -- the SELECT reads
//! from the same table being inserted into. Each execution doubles
//! the row count (or grows it unboundedly when the new rows feed
//! the next iteration via triggers). Almost always a typo for a
//! different source table, or it should be guarded by an
//! `ON CONFLICT DO NOTHING` / `WHERE NOT EXISTS (...)` predicate
//! to keep idempotent.

use crate::clause_scan::is_word;
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::{Statement, StatementKind};
use dsl_resolve::Scope;
use text_size::TextRange;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql475"
  }
  fn default_severity(&self) -> Severity {
    Severity::Warning
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let StatementKind::Insert(ins) = &stmt.kind else {
      return;
    };
    let target = ins.table.name.to_ascii_lowercase();
    let (start, raw) = crate::stmt_body(stmt, source);
    let cleaned = crate::textutil::strip_noise_full(raw);
    let upper = cleaned.to_ascii_uppercase();
    let ub = upper.as_bytes();
    let bytes = cleaned.as_bytes();
    let n = ub.len();
    // Find the SELECT clause's FROM body; scan tables.
    let Some(select_at) = find_word(ub, b"SELECT") else {
      return;
    };
    let Some(from_at) = find_word_from(ub, b"FROM", select_at + 6) else {
      return;
    };
    let body_start = from_at + 4;
    // Walk the FROM body looking for the target table name.
    let mut i = body_start;
    let mut found = false;
    while i < n {
      let c = bytes[i];
      if !(c.is_ascii_alphabetic() || c == b'_' || c == b'"') {
        i += 1;
        continue;
      }
      // Read identifier (possibly schema-qualified or quoted).
      let id_start = i;
      // Handle quoted ident.
      if c == b'"' {
        i += 1;
        while i < n && bytes[i] != b'"' {
          i += 1;
        }
        i = (i + 1).min(n);
      } else {
        while i < n && (is_word(bytes[i] as char) || bytes[i] == b'.') {
          i += 1;
        }
      }
      let ident = &cleaned[id_start..i];
      // Strip leading schema if `schema.table`.
      let bare = ident.rsplit('.').next().unwrap_or(ident).trim_matches('"');
      if bare.eq_ignore_ascii_case(&target) {
        found = true;
        break;
      }
    }
    if !found {
      return;
    }
    // Skip if there's an `ON CONFLICT` clause (idempotency guard).
    if find_word(ub, b"ON CONFLICT").is_some() {
      return;
    }
    let abs_s = start;
    let abs_e = start + n.min(raw.len());
    let _ = TextRange::new((abs_s as u32).into(), (abs_e as u32).into());
    out.push(Diagnostic {
      code: "sql475",
      severity: Severity::Warning,
      message: format!(
        "INSERT INTO `{target}` SELECT ... FROM `{target}` -- the source and target are the same table. Each execution doubles the rows (or grows unboundedly with triggers). If this is intentional duplication, add `ON CONFLICT DO NOTHING` or a `WHERE NOT EXISTS (...)` guard"
      ),
      range: TextRange::new((abs_s as u32).into(), (abs_e as u32).into()),
    });
  }
}

fn find_word(ub: &[u8], w: &[u8]) -> Option<usize> {
  find_word_from(ub, w, 0)
}

fn find_word_from(ub: &[u8], w: &[u8], from: usize) -> Option<usize> {
  let m = w.len();
  let n = ub.len();
  let mut i = from;
  while i + m <= n {
    if &ub[i..i + m] == w
      && (i == 0 || !is_word(ub[i - 1] as char))
      && (i + m == n || !is_word(ub[i + m] as char))
    {
      return Some(i);
    }
    i += 1;
  }
  None
}
