//! sql214: `CREATE INDEX CONCURRENTLY` (or `DROP INDEX CONCURRENTLY`)
//! inside an explicit transaction block. PG raises 25001 "CREATE
//! INDEX CONCURRENTLY cannot run inside a transaction block" at
//! runtime. Counts BEGIN/START TRANSACTION minus COMMIT/ROLLBACK in
//! the source before this statement.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql214"
  }
  fn default_severity(&self) -> Severity {
    Severity::Error
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let start: usize = u32::from(stmt.range.start()) as usize;
    let end: usize = (u32::from(stmt.range.end()) as usize).min(source.len());
    let body = &source[start..end];
    let upper = body.to_ascii_uppercase();
    let trim = upper.trim_start();
    let is_target = (trim.starts_with("CREATE INDEX")
      || trim.starts_with("CREATE UNIQUE INDEX")
      || trim.starts_with("DROP INDEX")
      || trim.starts_with("REINDEX INDEX")
      || trim.starts_with("REINDEX TABLE"))
      && upper.contains("CONCURRENTLY");
    if !is_target {
      return;
    }
    let prelude = &source[..start].to_ascii_uppercase();
    let begins = count_kw(prelude, "BEGIN") + count_phrase(prelude, "START TRANSACTION");
    let closes = count_kw(prelude, "COMMIT") + count_kw(prelude, "ROLLBACK") + count_phrase(prelude, "END TRANSACTION");
    if begins <= closes {
      return;
    }
    let lead = upper.len() - trim.len();
    let abs_s = start + lead;
    let abs_e = abs_s + (trim.find(';').unwrap_or(trim.len()));
    out.push(Diagnostic {
      code: "sql214",
      severity: Severity::Error,
      message: "CONCURRENTLY index op inside transaction block -- PG raises 25001; run outside BEGIN/COMMIT".into(),
      range: text_size::TextRange::new((abs_s as u32).into(), (abs_e as u32).into()),
    });
  }
}

fn count_kw(s: &str, needle: &str) -> usize {
  let bytes = s.as_bytes();
  let mut from = 0usize;
  let mut n = 0usize;
  while let Some(rel) = s[from..].find(needle) {
    let at = from + rel;
    let before_ok = at == 0
      || !{
        let p = bytes[at - 1] as char;
        p.is_ascii_alphanumeric() || p == '_'
      };
    let after = at + needle.len();
    let after_ok = after >= bytes.len()
      || !{
        let p = bytes[after] as char;
        p.is_ascii_alphanumeric() || p == '_'
      };
    if before_ok && after_ok {
      n += 1
    }
    from = at + needle.len();
  }
  n
}

fn count_phrase(s: &str, needle: &str) -> usize {
  s.matches(needle).count()
}
