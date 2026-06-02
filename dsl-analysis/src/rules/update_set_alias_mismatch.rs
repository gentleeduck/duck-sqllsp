//! sql204: `UPDATE users u SET other.col = ...` -- the qualifier on
//! the SET target doesn't match the updated table (`u` or `users`).
//! PG raises 42703 / 42P01 -- only the updated table is in scope on
//! the SET left-hand side. Catches the common bug where folks alias
//! the update target then accidentally use a JOINed table alias.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::{Statement, StatementKind};
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql204"
  }
  fn default_severity(&self) -> Severity {
    Severity::Error
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let StatementKind::Update(u) = &stmt.kind else { return };
    let (start, body, upper) = crate::stmt_body_upper(stmt, source);
    let Some(set_at) = upper.find(" SET ") else { return };
    let after_set = set_at + " SET ".len();
    let from_pos = upper[after_set..].find(" FROM ").map(|p| after_set + p).unwrap_or(body.len());
    let where_pos = upper[after_set..].find(" WHERE ").map(|p| after_set + p).unwrap_or(body.len());
    let limit = from_pos.min(where_pos);
    let set_text = &body[after_set..limit];
    let allowed: Vec<String> = [u.table.name.clone()].into_iter().collect();
    // Also accept any alias the user wrote in the UPDATE clause itself.
    // Detect `UPDATE <name> <alias>` form.
    let upd_idx = upper.find("UPDATE ").unwrap_or(0) + "UPDATE ".len();
    let head = &body[upd_idx..upd_idx + body[upd_idx..].find('\n').unwrap_or(body[upd_idx..].len())];
    let tokens: Vec<&str> = head.split_whitespace().take(3).collect();
    let mut allowed = allowed;
    if tokens.len() >= 2 {
      let second = tokens[1].trim_end_matches(',').trim_matches('"');
      if !second.eq_ignore_ascii_case("SET") {
        allowed.push(second.to_string());
      }
    }
    if tokens.len() >= 3 && tokens[1].eq_ignore_ascii_case("AS") {
      allowed.push(tokens[2].trim_end_matches(',').trim_matches('"').to_string());
    }
    for raw in split_top_level(set_text) {
      let frag = set_text[raw.start..raw.end].trim_start();
      let Some(eq_at) = frag.find('=') else { continue };
      let lhs = frag[..eq_at].trim();
      if !lhs.contains('.') {
        continue;
      }
      let Some((qual, _col)) = lhs.split_once('.') else { continue };
      let qual = qual.trim().trim_matches('"');
      if allowed.iter().any(|a| a.eq_ignore_ascii_case(qual)) {
        continue;
      }
      // Composite-column field assignment: `SET col.field = ...` where
      // `col` is a composite-typed column of the target table. PG
      // accepts this and updates the named field of the composite.
      if let Some(t) = catalog.find_table(u.table.schema.as_deref(), &u.table.name)
        && t.columns.iter().any(|c| c.name.eq_ignore_ascii_case(qual))
      {
        continue;
      }
      let off = set_text.find(lhs).unwrap_or(0);
      let abs_s = start + after_set + off;
      let abs_e = abs_s + lhs.len();
      out.push(Diagnostic {
        code: "sql204",
        severity: Severity::Error,
        message: format!(
          "UPDATE SET target `{lhs}` -- qualifier `{qual}` is not the updated table; only `{}` is in scope on the SET left-hand side",
          allowed.join("` / `"),
        ),
        range: crate::range_at(abs_s, abs_e),
      });
    }
  }
}

struct Span {
  start: usize,
  end: usize,
}

fn split_top_level(text: &str) -> Vec<Span> {
  let mut out = Vec::new();
  let bytes = text.as_bytes();
  let mut depth = 0i32;
  let mut start = 0usize;
  let mut i = 0usize;
  while i < bytes.len() {
    match bytes[i] {
      b'(' => depth += 1,
      b')' => depth -= 1,
      b',' if depth == 0 => {
        out.push(Span { start, end: i });
        start = i + 1
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
  out.push(Span { start, end: bytes.len() });
  out
}
