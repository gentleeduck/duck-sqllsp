//! sql235: `pg_sleep(n)` inside an explicit transaction block. The
//! sleeping backend keeps every lock + snapshot acquired so far,
//! easily stalls other writers, and consumes a slot. Hint at the
//! risk and suggest sleeping outside the tx.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql235"
  }
  fn default_severity(&self) -> Severity {
    Severity::Warning
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let start: usize = u32::from(stmt.range.start()) as usize;
    let end: usize = (u32::from(stmt.range.end()) as usize).min(source.len());
    let raw = &source[start..end];
    let body_owned = crate::textutil::strip_noise_full(raw);
    let body = body_owned.as_str();
    let lower = body.to_ascii_lowercase();
    for fname in ["pg_sleep(", "pg_sleep_for(", "pg_sleep_until("] {
      let Some(rel) = lower.find(fname) else { continue };
      if rel > 0 {
        let prev = body.as_bytes()[rel - 1] as char;
        if prev.is_ascii_alphanumeric() || prev == '_' { continue }
      }
      let prelude = source[..start].to_ascii_uppercase();
      let begins = count_kw(&prelude, "BEGIN") + count_phrase(&prelude, "START TRANSACTION");
      let closes = count_kw(&prelude, "COMMIT") + count_kw(&prelude, "ROLLBACK");
      if begins <= closes { continue }
      let abs_s = start + rel;
      let abs_e = abs_s + fname.len() - 1;
      out.push(Diagnostic {
        code: "sql235",
        severity: Severity::Warning,
        message: format!(
          "`{}` inside transaction block -- holds locks + snapshot for the whole sleep; sleep outside tx",
          fname.trim_end_matches('(')
        ),
        range: text_size::TextRange::new((abs_s as u32).into(), (abs_e as u32).into()),
      });
      return;
    }
  }
}

fn count_kw(s: &str, needle: &str) -> usize {
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

fn count_phrase(s: &str, needle: &str) -> usize { s.matches(needle).count() }
