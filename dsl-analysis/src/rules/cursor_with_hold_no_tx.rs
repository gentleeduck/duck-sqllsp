//! sql271: `DECLARE c CURSOR WITH HOLD FOR ...` outside an explicit
//! transaction. WITH HOLD only matters if the cursor needs to survive
//! the tx that opened it; in autocommit mode there is no tx so PG
//! either errors or the HOLD is a no-op (depends on PG version).

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql271"
  }
  fn default_severity(&self) -> Severity {
    Severity::Warning
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, body, upper) = crate::stmt_body_upper(stmt, source);
    if !upper.contains("DECLARE") || !upper.contains("CURSOR") {
      return;
    }
    if !upper.contains("WITH HOLD") {
      return;
    }
    let prelude = source[..start].to_ascii_uppercase();
    let begins = count_kw(&prelude, "BEGIN") + count_phrase(&prelude, "START TRANSACTION");
    let closes = count_kw(&prelude, "COMMIT") + count_kw(&prelude, "ROLLBACK");
    if begins > closes {
      return;
    }
    let abs_s = start;
    let abs_e = start + body.find(';').unwrap_or(body.len());
    out.push(Diagnostic {
      code: "sql271",
      severity: Severity::Warning,
      message: "DECLARE CURSOR WITH HOLD outside transaction -- HOLD is meaningful only inside a tx; wrap in BEGIN/COMMIT or drop WITH HOLD".into(),
      range: crate::range_at(abs_s, abs_e),
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
