//! sql568: `col ~ 'abc'` -- a regex match against a pattern with no regex
//! metacharacters at all. It just tests "contains the substring abc", which
//! is `col LIKE '%abc%'` (or `ILIKE` / `NOT LIKE` for `~*` / `!~`). The LIKE
//! form is clearer and can use a `text_pattern_ops` index. (sql423 handles the
//! anchored `^prefix` form; this is the no-metacharacter substring case.)

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

const METACHARS: &[u8] = b".^$*+?()[]{}|\\";

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql568"
  }
  fn default_severity(&self) -> Severity {
    Severity::Hint
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, body) = crate::stmt_body(stmt, source);
    let bytes = body.as_bytes();
    let n = bytes.len();
    let mut i = 0usize;
    while i < n {
      match bytes[i] {
        b'\'' => {
          i += 1;
          while i < n && bytes[i] != b'\'' {
            i += 1;
          }
        },
        b'~' => {
          // Exclude `~~` (LIKE operator, sql554) and geometric `~=`/`~>`.
          if bytes.get(i + 1) == Some(&b'~') || (i > 0 && bytes[i - 1] == b'~') {
            i += 1;
            continue;
          }
          let neg = i > 0 && bytes[i - 1] == b'!';
          let ci = bytes.get(i + 1) == Some(&b'*');
          let op_start = if neg { i - 1 } else { i };
          let after_op = i + 1 + ci as usize;
          // Operand must be a string literal.
          let mut p = after_op;
          while p < n && bytes[p].is_ascii_whitespace() {
            p += 1;
          }
          if bytes.get(p) == Some(&b'\'')
            && let Some((content, lit_end)) = read_string(bytes, p)
            && !content.is_empty()
            && !content.bytes().any(|b| METACHARS.contains(&b))
          {
            let kw = match (neg, ci) {
              (false, false) => "LIKE",
              (false, true) => "ILIKE",
              (true, false) => "NOT LIKE",
              (true, true) => "NOT ILIKE",
            };
            out.push(Diagnostic {
              code: "sql568",
              severity: Severity::Hint,
              message: format!("regex `'{content}'` has no metacharacters -- use `{kw} '%{content}%'` instead"),
              range: crate::range_at(start + op_start, start + lit_end),
            });
            i = lit_end;
            continue;
          }
        },
        _ => {},
      }
      i += 1;
    }
  }
}

fn read_string(bytes: &[u8], open: usize) -> Option<(String, usize)> {
  let mut content = String::new();
  let mut i = open + 1;
  while i < bytes.len() {
    if bytes[i] == b'\'' {
      if bytes.get(i + 1) == Some(&b'\'') {
        content.push('\'');
        i += 2;
        continue;
      }
      return Some((content, i + 1));
    }
    content.push(bytes[i] as char);
    i += 1;
  }
  None
}
