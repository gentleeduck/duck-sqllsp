//! sql280: `ALTER TABLE t ADD CONSTRAINT c CHECK (...)` without
//! `NOT VALID`. PG scans every existing row to validate, holding
//! AccessExclusiveLock the whole time. On big tables that's a
//! sustained outage. Pattern: ADD CONSTRAINT ... NOT VALID + later
//! `VALIDATE CONSTRAINT` (only ShareUpdateExclusiveLock).

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql280"
  }
  fn default_severity(&self) -> Severity {
    Severity::Hint
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, raw) = crate::stmt_body(stmt, source);
    let body_owned = crate::textutil::strip_noise_full(raw);
    let body = body_owned.as_str();
    let upper = body.to_ascii_uppercase();
    if !upper.trim_start().starts_with("ALTER TABLE") {
      return;
    }
    let has_add_constraint_check = upper.contains("ADD CONSTRAINT") && upper.contains("CHECK");
    let has_add_constraint_fk = upper.contains("ADD CONSTRAINT") && upper.contains("FOREIGN KEY");
    // Also catch the unnamed form `ALTER TABLE t ADD CHECK (...)` /
    // `ADD FOREIGN KEY (...)` -- the validation cost is identical.
    let has_add_check_unnamed = !has_add_constraint_check && upper.contains("ADD CHECK");
    let has_add_fk_unnamed = !has_add_constraint_fk && upper.contains("ADD FOREIGN KEY");
    if !(has_add_constraint_check || has_add_constraint_fk || has_add_check_unnamed || has_add_fk_unnamed) {
      return;
    }
    if upper.contains("NOT VALID") {
      return;
    }
    // Suppress when the target table was CREATE TABLE'd earlier in the
    // same buffer. Migrations almost always follow `CREATE TABLE t
    // (...)` with `ALTER TABLE t ADD CONSTRAINT ...`, and at the moment
    // the ALTER runs the table has zero rows -- the outage warning is
    // bogus. Only the cross-buffer (existing-table) case is at risk.
    if let Some(target) = extract_alter_table_name(body)
      && buffer_has_prior_create_table(source, start, &target)
    {
      return;
    }
    let at = upper.find("ADD CONSTRAINT").or_else(|| upper.find("ADD CHECK")).or_else(|| upper.find("ADD FOREIGN KEY"));
    let Some(at) = at else { return };
    let abs_s = start + at;
    let abs_e = start + body.find(';').unwrap_or(body.len());
    out.push(Diagnostic {
      code: "sql280",
      severity: Severity::Hint,
      message: "ADD CONSTRAINT CHECK / FOREIGN KEY without NOT VALID -- scans every row under AccessExclusiveLock; use `... NOT VALID` then `ALTER TABLE t VALIDATE CONSTRAINT c` to avoid the outage".into(),
      range: crate::range_at(abs_s, abs_e),
    });
  }
}

/// Pull `<name>` from `ALTER TABLE [IF EXISTS] [ONLY] [schema.]<name>`.
/// Returns the bare table name (last component) lowercased, with
/// surrounding quotes stripped.
fn extract_alter_table_name(body: &str) -> Option<String> {
  let upper = body.to_ascii_uppercase();
  let at = upper.find("ALTER TABLE")? + "ALTER TABLE".len();
  let mut rest = body[at..].trim_start();
  // Skip optional modifiers.
  for kw in ["IF EXISTS", "ONLY"] {
    if rest.to_ascii_uppercase().starts_with(kw) {
      rest = rest[kw.len()..].trim_start();
    }
  }
  // Read schema-or-name, then optional `.name`.
  let (first, after_first) = read_ident(rest)?;
  rest = after_first.trim_start();
  let name = if let Some(after_dot) = rest.strip_prefix('.') {
    let (second, _) = read_ident(after_dot.trim_start())?;
    second
  } else {
    first
  };
  Some(name.trim_matches('"').to_ascii_lowercase())
}

fn read_ident(s: &str) -> Option<(String, &str)> {
  let s = s.trim_start();
  if let Some(rest) = s.strip_prefix('"') {
    let end = rest.find('"')?;
    return Some((format!("\"{}\"", &rest[..end]), &rest[end + 1..]));
  }
  let end = s.find(|c: char| !(c.is_ascii_alphanumeric() || c == '_')).unwrap_or(s.len());
  if end == 0 {
    return None;
  }
  Some((s[..end].to_string(), &s[end..]))
}

/// True when the buffer slice before `at` contains a `CREATE TABLE
/// [IF NOT EXISTS] [schema.]<name>` whose last component matches
/// `target` (case-insensitive, quote-insensitive). Only the prefix
/// up to `at` is scanned so a later CREATE TABLE doesn't satisfy an
/// earlier ALTER (would never happen in real PG semantics anyway).
fn buffer_has_prior_create_table(source: &str, at: usize, target: &str) -> bool {
  let prefix = &source[..at.min(source.len())];
  let stripped = crate::textutil::strip_noise_full(prefix);
  let upper = stripped.to_ascii_uppercase();
  let mut search_from = 0;
  while let Some(rel) = upper[search_from..].find("CREATE TABLE") {
    let pos = search_from + rel + "CREATE TABLE".len();
    let mut rest = stripped[pos..].trim_start();
    for kw in ["IF NOT EXISTS", "UNLOGGED", "TEMP", "TEMPORARY"] {
      if rest.to_ascii_uppercase().starts_with(kw) {
        rest = rest[kw.len()..].trim_start();
      }
    }
    if let Some((first, after_first)) = read_ident(rest) {
      let after_trim = after_first.trim_start();
      let name = if let Some(after_dot) = after_trim.strip_prefix('.') {
        read_ident(after_dot.trim_start()).map(|(n, _)| n).unwrap_or(first)
      } else {
        first
      };
      let bare = name.trim_matches('"').to_ascii_lowercase();
      if bare == target {
        return true;
      }
    }
    search_from = pos;
  }
  false
}
