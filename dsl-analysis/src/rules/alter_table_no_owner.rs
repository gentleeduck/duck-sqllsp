//! sql129: `ALTER TABLE x OWNER TO` -- without an OWNER clause the
//! table keeps whatever owner was current at creation time, which is
//! often the migration user rather than the application role.
//!
//! We only flag when the ALTER TABLE *exists in a migration-style file*
//! and obviously *should* set ownership (CREATE TABLE in the same
//! statement) but doesn't.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql129"
  }
  fn default_severity(&self) -> Severity {
    Severity::Hint
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let start: usize = u32::from(stmt.range.start()) as usize;
    let end: usize = (u32::from(stmt.range.end()) as usize).min(source.len());
    let body = &source[start..end];
    let upper = body.to_ascii_uppercase();
    let trimmed = upper.trim_start();
    if !trimmed.starts_with("CREATE TABLE") {
      return;
    }
    // Look in the rest of the source for a matching `ALTER TABLE
    // <same_name> OWNER TO`. If the table is created but no OWNER
    // ever set in this file, emit the hint.
    // Extract table name from CREATE TABLE.
    let rest = trimmed[12..].trim_start();
    let rest_after_ine = rest.strip_prefix("IF NOT EXISTS").unwrap_or(rest).trim_start();
    let name: String = rest_after_ine.chars().take_while(|c| c.is_alphanumeric() || *c == '_' || *c == '.').collect();
    if name.is_empty() {
      return;
    }
    let bare = name.rsplit('.').next().unwrap_or(&name).to_ascii_uppercase();
    // Search rest of source for `ALTER TABLE ... OWNER TO ...`
    // referencing this table.
    let after_stmt = &source[end..];
    let after_upper = after_stmt.to_ascii_uppercase();
    let needle_owner = format!("ALTER TABLE");
    let mut i = 0;
    while let Some(rel) = after_upper[i..].find(&needle_owner) {
      let at = i + rel;
      let chunk = &after_upper[at..];
      // Read table name token after ALTER TABLE.
      let after_kw = chunk[11..].trim_start();
      let tok: String =
        after_kw.chars().take_while(|c| c.is_alphanumeric() || *c == '_' || *c == '.' || *c == '"').collect();
      let tok_bare = tok.trim_matches('"').rsplit('.').next().unwrap_or("").to_ascii_uppercase();
      if tok_bare == bare && chunk.contains("OWNER TO") {
        return;
      }
      i = at + 11;
    }
    let leading = upper.len() - trimmed.len();
    let abs_start = start + leading;
    let abs_end = abs_start + 12;
    out.push(Diagnostic {
      code: "sql129",
      severity: Severity::Hint,
      message: format!(
        "CREATE TABLE `{name}` has no following `ALTER TABLE ... OWNER TO` -- ownership defaults to the migration user"
      ),
      range: text_size::TextRange::new((abs_start as u32).into(), (abs_end as u32).into()),
    });
  }
}
