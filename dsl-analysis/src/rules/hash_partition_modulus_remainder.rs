//! sql762: `FOR VALUES WITH (MODULUS m, REMAINDER r)` where the
//! remainder is not less than the modulus. PostgreSQL requires the
//! remainder to be in [0, modulus) and raises an error ("remainder for
//! hash partition must be less than modulus") otherwise.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql762"
  }
  fn default_severity(&self) -> Severity {
    Severity::Error
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, _body, upper) = crate::stmt_body_upper(stmt, source);
    let Some(mod_rel) = upper.find("MODULUS") else { return };
    let Some(modulus) = read_int_after(&upper, mod_rel + "MODULUS".len()) else { return };
    let Some(rem_rel_offset) = upper[mod_rel..].find("REMAINDER") else { return };
    let rem_rel = mod_rel + rem_rel_offset;
    let Some(remainder) = read_int_after(&upper, rem_rel + "REMAINDER".len()) else { return };
    if remainder >= modulus {
      out.push(Diagnostic {
        code: "sql762",
        severity: Severity::Error,
        message: format!("REMAINDER ({remainder}) must be less than MODULUS ({modulus}) for a hash partition"),
        range: crate::range_at(start + rem_rel, start + rem_rel + "REMAINDER".len()),
      });
    }
  }
}

/// Skip whitespace after `from`, then parse a run of ASCII digits.
fn read_int_after(upper: &str, from: usize) -> Option<i64> {
  let ub = upper.as_bytes();
  let mut i = from;
  while i < ub.len() && ub[i].is_ascii_whitespace() {
    i += 1;
  }
  let digit_start = i;
  while i < ub.len() && ub[i].is_ascii_digit() {
    i += 1;
  }
  if i == digit_start {
    return None;
  }
  upper[digit_start..i].parse().ok()
}
