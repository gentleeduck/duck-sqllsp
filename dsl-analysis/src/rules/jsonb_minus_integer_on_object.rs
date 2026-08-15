//! sql782: `'{"a":1}'::jsonb - 0` -- the integer-index form of the `-`
//! operator deletes an array element by position and is only defined
//! for arrays; the literal here is an object. PostgreSQL raises an
//! error ("cannot delete from object using integer index") at
//! runtime. Scoped to jsonb literals only -- a jsonb *column*'s
//! runtime shape (object vs array) isn't visible in its static
//! catalog type, so this can't be checked for columns.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql782"
  }
  fn default_severity(&self) -> Severity {
    Severity::Error
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, body, upper) = crate::stmt_body_upper(stmt, source);
    let b = body.as_bytes();
    let ub = upper.as_bytes();
    let mut i = 0usize;
    while i < b.len() {
      if b[i] != b'\'' {
        i += 1;
        continue;
      }
      let lit_s = i;
      i += 1;
      while i < b.len() && b[i] != b'\'' {
        i += 1;
      }
      if i >= b.len() {
        break;
      }
      let lit_e = i;
      let content = body[lit_s + 1..lit_e].trim();
      if !content.starts_with('{') {
        i += 1;
        continue;
      }
      let after_lit = skip_ws(ub, lit_e + 1);
      if !upper[after_lit..].starts_with("::JSONB") {
        i += 1;
        continue;
      }
      let mut j = skip_ws(ub, after_lit + "::JSONB".len());
      if ub.get(j) != Some(&b'-') {
        i += 1;
        continue;
      }
      j = skip_ws(ub, j + 1);
      let num_start = j;
      while j < ub.len() && ub[j].is_ascii_digit() {
        j += 1;
      }
      if j == num_start {
        i += 1;
        continue;
      }
      out.push(Diagnostic {
        code: "sql782",
        severity: Severity::Error,
        message: "integer-index `-` deletes an array element and is only defined for arrays -- this literal is an object".into(),
        range: crate::range_at(start + lit_s, start + j),
      });
      i = j;
    }
  }
}

fn skip_ws(ub: &[u8], mut i: usize) -> usize {
  while i < ub.len() && ub[i].is_ascii_whitespace() {
    i += 1;
  }
  i
}
