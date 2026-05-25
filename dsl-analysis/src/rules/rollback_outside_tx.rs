//! sql211: bare `ROLLBACK;` / `COMMIT;` with no preceding BEGIN /
//! START TRANSACTION in the source. PG emits a WARNING ("there is no
//! transaction in progress") and the statement is a no-op.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql211"
  }
  fn default_severity(&self) -> Severity {
    Severity::Warning
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let start: usize = u32::from(stmt.range.start()) as usize;
    let end: usize = (u32::from(stmt.range.end()) as usize).min(source.len());
    let body = &source[start..end];
    let upper = body.to_ascii_uppercase();
    let trimmed = upper.trim_start();
    let (kw, kw_len) = if trimmed.starts_with("ROLLBACK") {
      ("ROLLBACK", "ROLLBACK".len())
    } else if trimmed.starts_with("COMMIT") {
      ("COMMIT", "COMMIT".len())
    } else { return };
    // Skip ROLLBACK TO SAVEPOINT (always valid where SAVEPOINT was set).
    if trimmed.starts_with("ROLLBACK TO") { return }
    // Walk source up to this stmt start, look for unclosed BEGIN/START TRANSACTION.
    let prelude = &source[..start].to_ascii_uppercase();
    let begins = count_occurrences(prelude, "BEGIN") + count_occurrences(prelude, "START TRANSACTION");
    let closes = count_occurrences(prelude, "COMMIT") + count_occurrences(prelude, "ROLLBACK");
    if begins > closes { return }
    let lead = upper.len() - trimmed.len();
    let abs_s = start + lead;
    let abs_e = abs_s + kw_len;
    out.push(Diagnostic {
      code: "sql211",
      severity: Severity::Warning,
      message: format!("`{kw}` with no open transaction -- PG emits warning + no-op"),
      range: text_size::TextRange::new((abs_s as u32).into(), (abs_e as u32).into()),
    });
  }
}

fn count_occurrences(s: &str, needle: &str) -> usize {
  let bytes = s.as_bytes();
  let mut from = 0usize;
  let mut n = 0usize;
  while let Some(rel) = s[from..].find(needle) {
    let at = from + rel;
    let before_ok = at == 0 || !{ let p = bytes[at - 1] as char; p.is_ascii_alphanumeric() || p == '_' };
    let after = at + needle.len();
    let after_ok = after >= bytes.len() || !{ let p = bytes[after] as char; p.is_ascii_alphanumeric() || p == '_' };
    if before_ok && after_ok { n += 1 }
    from = at + needle.len();
  }
  n
}
