//! sql021: prefer the declared alias over the bare table name.
//!
//! When a statement declares `FROM users AS u`, references to columns
//! should go through the alias (`u.id`), not through the raw table
//! (`users.id`). The aliased form is shorter, survives table renames,
//! and avoids ambiguity in multi-table SELECTs.
//!
//! We surface this as a hint (not a hard error) so the diagnostic
//! doesn't fight users who deliberately spelled the table name.

use crate::{Diagnostic, LintRule, Severity};
use crate::textutil::is_word;
use dsl_catalog::Catalog;
use dsl_parse::{Statement, StatementKind};
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql021"
  }
  fn default_severity(&self) -> Severity {
    Severity::Hint
  }

  fn check(&self, source: &str, stmt: &Statement, scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    // Only worth flagging when the statement has at least one
    // explicit alias.
    let aliases: Vec<(String, String)> =
      scope.tables().filter(|b| b.alias != b.table.name).map(|b| (b.table.name.clone(), b.alias.clone())).collect();
    if aliases.is_empty() {
      return;
    }

    // Limit the search to the statement body so we don't false-fire
    // on neighbouring statements that legitimately use the bare
    // name.
    let (_start, body) = crate::stmt_body(stmt, source);

    // Skip non-DML statements; CREATE/ALTER reference the bare name
    // on purpose.
    if !matches!(
      stmt.kind,
      StatementKind::Select(_) | StatementKind::Update(_) | StatementKind::Delete(_) | StatementKind::Insert(_)
    ) {
      return;
    }

    for (table, alias) in &aliases {
      for hit in find_qualified_uses(body, table) {
        out.push(Diagnostic {
          code: "sql021",
          severity: Severity::Hint,
          message: format!("use alias `{alias}` instead of `{table}` (declared in this statement)"),
          range: shift_range(stmt.range, hit.0, hit.1),
        });
      }
    }
  }
}

/// Find every `<table>.` occurrence in `body` that is a whole-word match
/// (not part of a longer identifier). Returns (start, end) byte ranges
/// relative to `body`.
fn find_qualified_uses(body: &str, table: &str) -> Vec<(usize, usize)> {
  let bytes = body.as_bytes();
  let needle: Vec<u8> = table.to_ascii_lowercase().into_bytes();
  let upper_body = body.to_ascii_lowercase();
  let mut out = Vec::new();
  let mut from = 0usize;
  while let Some(rel) = upper_body[from..].find(std::str::from_utf8(&needle).unwrap_or("")) {
    let i = from + rel;
    let end = i + needle.len();
    let before_ok = i == 0 || !is_word(bytes[i - 1] as char);
    let after_dot = bytes.get(end) == Some(&b'.');
    if before_ok && after_dot {
      out.push((i, end));
    }
    from = end;
  }
  out
}


fn shift_range(stmt: text_size::TextRange, rel_start: usize, rel_end: usize) -> text_size::TextRange {
  let base: usize = u32::from(stmt.start()) as usize;
  crate::range_at(base + rel_start, base + rel_end)
}
