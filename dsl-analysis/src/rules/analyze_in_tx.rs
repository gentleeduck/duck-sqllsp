//! sql283: `ANALYZE` (or `ANALYZE t`) inside an explicit transaction.
//! ANALYZE acquires ShareUpdateExclusiveLock per table; bundled in
//! a long-running tx those locks are held until COMMIT and block
//! autovacuum / other ANALYZE concurrently. PG accepts it, but
//! almost always you want ANALYZE outside the tx.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql283"
  }
  fn default_severity(&self) -> Severity {
    Severity::Hint
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let start: usize = u32::from(stmt.range.start()) as usize;
    let end: usize = (u32::from(stmt.range.end()) as usize).min(source.len());
    let body = &source[start..end];
    let upper = body.to_ascii_uppercase();
    let trim = upper.trim_start();
    if !trim.starts_with("ANALYZE") {
      return;
    }
    let prelude = source[..start].to_ascii_uppercase();
    let begins = count_kw(&prelude, "BEGIN") + count_phrase(&prelude, "START TRANSACTION");
    let closes = count_kw(&prelude, "COMMIT") + count_kw(&prelude, "ROLLBACK");
    if begins <= closes {
      return;
    }
    let lead = body.len() - body.trim_start().len();
    let abs_s = start + lead;
    let abs_e = abs_s + "ANALYZE".len();
    out.push(Diagnostic {
      code: "sql283",
      severity: Severity::Hint,
      message: "ANALYZE inside transaction -- ShareUpdateExclusiveLock held until COMMIT, blocks autovacuum; run outside BEGIN/COMMIT".into(),
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
