//! sql551: redundantly nested functions whose outer call subsumes the inner
//! one:
//!   * `upper(lower(x))` / `lower(upper(x))` / `upper(upper(x))` -- the outer
//!     case-fold wins; the inner one does nothing.
//!   * `trim(trim(x))` / `btrim(btrim(x))` / `abs(abs(x))` -- idempotent, so
//!     the second application is a no-op.
//!   * `reverse(reverse(x))` -- two reverses cancel; it's just `x`.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

const OUTER_FNS: &[&str] = &["upper", "lower", "trim", "btrim", "abs", "reverse"];

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql551"
  }
  fn default_severity(&self) -> Severity {
    Severity::Hint
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, body) = crate::stmt_body(stmt, source);
    let lower = body.to_ascii_lowercase();
    let bytes = body.as_bytes();
    for outer in OUTER_FNS {
      let needle = format!("{outer}(");
      let mut from = 0usize;
      while let Some(rel) = lower[from..].find(&needle) {
        let at = from + rel;
        if at > 0 && (bytes[at - 1].is_ascii_alphanumeric() || bytes[at - 1] == b'_') {
          from = at + needle.len();
          continue;
        }
        let open = at + needle.len() - 1;
        let Some(close) = match_paren(bytes, open) else { break };
        let arg = body[open + 1..close].trim();
        if let Some(inner) = sole_inner_call(arg)
          && let Some(msg) = redundancy(outer, &inner)
        {
          out.push(Diagnostic {
            code: "sql551",
            severity: Severity::Hint,
            message: msg,
            range: crate::range_at(start + at, start + close + 1),
          });
        }
        from = close + 1;
      }
    }
  }
}

const CASE_FOLD: &[&str] = &["upper", "lower"];
const IDEMPOTENT: &[&str] = &["trim", "btrim", "abs"];

fn redundancy(outer: &str, inner: &str) -> Option<String> {
  if CASE_FOLD.contains(&outer) && CASE_FOLD.contains(&inner) {
    Some(format!("redundant nested case-fold -- `{outer}({inner}(...))` is just `{outer}(...)`"))
  } else if outer == inner && IDEMPOTENT.contains(&outer) {
    Some(format!("redundant nesting -- `{outer}` is idempotent, so `{outer}({outer}(...))` is just `{outer}(...)`"))
  } else if outer == "reverse" && inner == "reverse" {
    Some("`reverse(reverse(x))` cancels out -- it's just `x`".to_string())
  } else {
    None
  }
}

/// If `arg` is exactly a single function call `fname(...)` (its matching paren
/// closes at the end), return the lowercased `fname`.
fn sole_inner_call(arg: &str) -> Option<String> {
  let bytes = arg.as_bytes();
  let mut i = 0usize;
  while i < bytes.len() && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_') {
    i += 1;
  }
  if i == 0 || bytes.get(i) != Some(&b'(') {
    return None;
  }
  let close = match_paren(bytes, i)?;
  if close != bytes.len() - 1 {
    return None;
  }
  Some(arg[..i].to_ascii_lowercase())
}

fn match_paren(bytes: &[u8], open: usize) -> Option<usize> {
  let mut depth = 0i32;
  let mut i = open;
  while i < bytes.len() {
    match bytes[i] {
      b'(' => depth += 1,
      b')' => {
        depth -= 1;
        if depth == 0 {
          return Some(i);
        }
      },
      b'\'' => {
        i += 1;
        while i < bytes.len() && bytes[i] != b'\'' {
          i += 1
        }
      },
      _ => {},
    }
    i += 1;
  }
  None
}
