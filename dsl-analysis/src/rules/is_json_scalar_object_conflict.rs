//! sql768: `<expr> IS JSON OBJECT AND <same expr> IS JSON ARRAY` (or
//! any two different IS JSON kinds directly ANDed together) -- a JSON
//! value is exactly one of object/array/scalar, so requiring two
//! different kinds of the same expression is always false. Only the
//! direct `x IS JSON K1 AND x IS JSON K2` adjacency is matched
//! (nothing else between the two checks) to stay conservative.

use crate::clause_scan::is_word;
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

const KINDS: &[&str] = &["OBJECT", "ARRAY", "SCALAR"];

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql768"
  }
  fn default_severity(&self) -> Severity {
    Severity::Warning
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, _body, upper) = crate::stmt_body_upper(stmt, source);
    let ub = upper.as_bytes();
    let mut i = 0usize;
    while i < ub.len() {
      let Some(hit1) = match_expr_is_json(ub, i) else { break };
      let mut j = skip_ws(ub, hit1.end);
      if word_at(ub, j, b"AND") {
        j = skip_ws(ub, j + 3);
        if let Some(hit2) = match_expr_is_json_at(ub, j)
          && hit1.expr(ub).eq_ignore_ascii_case(hit2.expr(ub))
          && hit1.kind != hit2.kind
        {
          out.push(Diagnostic {
            code: "sql768",
            severity: Severity::Warning,
            message: format!(
              "`{}` cannot be both IS JSON {} and IS JSON {} -- always false",
              hit1.expr(ub),
              hit1.kind,
              hit2.kind
            ),
            range: crate::range_at(start + hit1.expr_start, start + hit2.end),
          });
        }
      }
      i = hit1.end;
    }
  }
}

struct Hit {
  expr_start: usize,
  expr_end: usize,
  kind: &'static str,
  end: usize,
}

impl Hit {
  fn expr<'a>(&self, ub: &'a [u8]) -> &'a str {
    std::str::from_utf8(&ub[self.expr_start..self.expr_end]).unwrap_or("")
  }
}

/// Scan forward from `from` for the next `<ident> IS JSON <KIND>`.
fn match_expr_is_json(ub: &[u8], from: usize) -> Option<Hit> {
  let mut i = from;
  while i < ub.len() {
    if is_ident_byte(ub[i])
      && (i == 0 || !is_ident_byte(ub[i - 1]))
      && let Some(h) = match_expr_is_json_at(ub, i)
    {
      return Some(h);
    }
    i += 1;
  }
  None
}

/// `<ident> IS JSON <KIND>` starting exactly at `at`, or None.
fn match_expr_is_json_at(ub: &[u8], at: usize) -> Option<Hit> {
  if !ub.get(at).is_some_and(|b| is_ident_byte(*b)) {
    return None;
  }
  let mut e = at;
  while e < ub.len() && is_ident_byte(ub[e]) {
    e += 1;
  }
  let after = skip_ws(ub, e);
  if !word_at(ub, after, b"IS") {
    return None;
  }
  let after_is = skip_ws(ub, after + 2);
  if !word_at(ub, after_is, b"JSON") {
    return None;
  }
  let after_json = skip_ws(ub, after_is + 4);
  let kind = KINDS.iter().find(|k| word_at(ub, after_json, k.as_bytes())).copied()?;
  Some(Hit { expr_start: at, expr_end: e, kind, end: after_json + kind.len() })
}

fn is_ident_byte(b: u8) -> bool {
  b.is_ascii_alphanumeric() || b == b'_'
}

fn word_at(ub: &[u8], i: usize, w: &[u8]) -> bool {
  i + w.len() <= ub.len()
    && &ub[i..i + w.len()] == w
    && (i == 0 || !is_word(ub[i - 1] as char))
    && (i + w.len() == ub.len() || !is_word(ub[i + w.len()] as char))
}

fn skip_ws(ub: &[u8], mut i: usize) -> usize {
  while i < ub.len() && ub[i].is_ascii_whitespace() {
    i += 1;
  }
  i
}
