//! sql253: `x NOT IN (SELECT col FROM t)` where `col` is nullable.
//! If the subquery returns even one NULL, the whole `NOT IN`
//! predicate evaluates to UNKNOWN -> filtered out. Almost always a
//! bug. Suggest `NOT EXISTS` or filter NULLs in the subquery.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql253"
  }
  fn default_severity(&self) -> Severity {
    Severity::Warning
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let start: usize = u32::from(stmt.range.start()) as usize;
    let end: usize = (u32::from(stmt.range.end()) as usize).min(source.len());
    let raw = &source[start..end];
    let body_owned = crate::textutil::strip_noise_full(raw);
    let body = body_owned.as_str();
    let upper = body.to_ascii_uppercase();
    let mut from = 0usize;
    while let Some(rel) = upper[from..].find("NOT IN (") {
      let at = from + rel;
      let open = at + "NOT IN ".len();
      let Some(close) = find_matching_paren(body, open) else { break };
      let inner = body[open + 1..close].trim();
      let inner_upper = inner.to_ascii_uppercase();
      if !inner_upper.starts_with("SELECT") {
        from = close + 1;
        continue;
      }
      // Single-column projection text-scan.
      let proj_end = inner_upper.find(" FROM ").unwrap_or(inner.len());
      let proj = inner[6..proj_end].trim();
      let col = proj.trim_matches('"');
      let bare = col.rsplit('.').next().unwrap_or(col);
      // Locate the FROM tail and extract the first identifier (table name).
      let tbl_start = proj_end + " FROM ".len();
      if tbl_start >= inner.len() {
        from = close + 1;
        continue;
      }
      let tail = &inner[tbl_start..];
      let tbl_end =
        tail.find(|c: char| !c.is_ascii_alphanumeric() && c != '_' && c != '.' && c != '"').unwrap_or(tail.len());
      let table = &tail[..tbl_end];
      let table_bare = table.rsplit('.').next().unwrap_or(table).trim_matches('"');
      let Some(t) = catalog.find_table(None, table_bare) else {
        from = close + 1;
        continue;
      };
      let Some(c) = t.columns.iter().find(|c| c.name.eq_ignore_ascii_case(bare)) else {
        from = close + 1;
        continue;
      };
      if !c.nullable {
        from = close + 1;
        continue;
      }
      out.push(Diagnostic {
        code: "sql253",
        severity: Severity::Warning,
        message: format!(
          "NOT IN subquery on nullable `{}.{}` -- any NULL silently filters out ALL rows; use NOT EXISTS or add `IS NOT NULL`",
          t.name, c.name,
        ),
        range: text_size::TextRange::new(((start + at) as u32).into(), ((start + close + 1) as u32).into()),
      });
      from = close + 1;
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
