//! sql607: a length/precision modifier on a type that doesn't accept one --
//! e.g. `text(50)`, `bytea(16)`, `jsonb(1)`, `boolean(1)`, `uuid(16)`.
//! PostgreSQL rejects the statement ("type modifier is not allowed for type
//! ..."). These types are unbounded (or fixed-width); drop the modifier, or use
//! `varchar(n)` if you actually want a length-limited string.
//!
//! Only a purely numeric modifier is treated as an error, so a same-named
//! constructor call with non-numeric arguments is never misread.

use crate::clause_scan::is_word;
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

/// Types that take no length/precision modifier and have no numeric-argument
/// constructor function of the same name.
const NO_MOD: &[&str] = &["TEXT", "BYTEA", "JSONB", "JSON", "UUID", "BOOLEAN", "BOOL"];

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql607"
  }
  fn default_severity(&self) -> Severity {
    Severity::Error
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, _body, upper) = crate::stmt_body_upper(stmt, source);
    let ub = upper.as_bytes();
    let n = ub.len();
    for &ty in NO_MOD {
      let len = ty.len();
      let mut i = 0usize;
      while i + len <= n {
        if &ub[i..i + len] == ty.as_bytes()
          && (i == 0 || !is_word(ub[i - 1] as char))
        {
          // optional whitespace, then `(digits[,digits])`
          let mut j = i + len;
          while j < n && ub[j].is_ascii_whitespace() {
            j += 1;
          }
          if j < n && ub[j] == b'(' && self.numeric_modifier(&ub[j..]) {
            out.push(Diagnostic {
              code: "sql607",
              severity: Severity::Error,
              message: format!("`{ty}` does not take a length/precision modifier in PostgreSQL -- drop it (use `varchar(n)` for a bounded string)"),
              range: crate::range_at(start + i, start + i + len),
            });
            i = j;
          }
        }
        i += 1;
      }
    }
  }
}

impl Rule {
  /// True when `s` starts with `(` and contains only digits, commas and spaces
  /// up to the matching `)` -- i.e. a real type modifier, not a function call.
  fn numeric_modifier(&self, s: &[u8]) -> bool {
    let mut k = 1usize; // past '('
    let mut saw_digit = false;
    while k < s.len() {
      match s[k] {
        b')' => return saw_digit,
        b'0'..=b'9' => saw_digit = true,
        b',' | b' ' => {}
        _ => return false,
      }
      k += 1;
    }
    false
  }
}
