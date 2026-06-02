//! sql176: `WHERE col IS NULL` where the catalog says `col` is
//! NOT NULL. The predicate can never be true so the query returns
//! zero rows.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::{Statement, StatementKind};
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql176"
  }
  fn default_severity(&self) -> Severity {
    Severity::Warning
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let target = match &stmt.kind {
      StatementKind::Select(s) => s.from.first(),
      StatementKind::Update(u) => Some(&u.table),
      StatementKind::Delete(d) => Some(&d.table),
      _ => return,
    };
    let Some(target) = target else { return };
    let Some(t) = catalog.find_table(target.schema.as_deref(), &target.name) else { return };

    let (start, body, upper) = crate::stmt_body_upper(stmt, source);

    let mut from = 0usize;
    while let Some(rel) = upper[from..].find("IS NULL") {
      let at = from + rel;
      // Word-boundary check on both sides.
      let after = at + "IS NULL".len();
      let prev = upper.as_bytes().get(at.saturating_sub(1)).copied();
      let next = upper.as_bytes().get(after).copied();
      let word_prev = prev.is_some_and(|b| b.is_ascii_alphanumeric() || b == b'_');
      let word_next = next.is_some_and(|b| b.is_ascii_alphanumeric() || b == b'_');
      // Skip IS NOT NULL by checking the preceding word.
      let prefix = &upper[..at];
      let prefix_trim = prefix.trim_end();
      if prefix_trim.ends_with("NOT") {
        from = after;
        continue;
      }
      if word_prev || word_next {
        from = after;
        continue;
      }
      // Read the identifier preceding the keyword.
      let mut k = at;
      while k > 0 && body.as_bytes()[k - 1].is_ascii_whitespace() {
        k -= 1;
      }
      let id_end = k;
      while k > 0 {
        let b = body.as_bytes()[k - 1];
        if b.is_ascii_alphanumeric() || b == b'_' || b == b'.' || b == b'"' {
          k -= 1;
        } else {
          break;
        }
      }
      if id_end == k {
        from = after;
        continue;
      }
      let raw = &body[k..id_end];
      let bare = raw.rsplit('.').next().unwrap_or(raw).trim_matches('"');
      let Some(col) = t.columns.iter().find(|c| c.name.eq_ignore_ascii_case(bare)) else {
        from = after;
        continue;
      };
      if col.nullable {
        from = after;
        continue;
      }
      let abs_s = start + k;
      let abs_e = start + after;
      out.push(Diagnostic {
        code: "sql176",
        severity: Severity::Warning,
        message: format!(
          "`{}` is NOT NULL -- `{} IS NULL` is always false; the predicate returns zero rows",
          col.name, col.name
        ),
        range: crate::range_at(abs_s, abs_e),
      });
      from = after;
    }
  }
}
