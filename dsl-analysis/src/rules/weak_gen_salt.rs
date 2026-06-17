//! sql634: `gen_salt('md5' | 'des' | 'xdes')` from pgcrypto. These algorithms
//! are weak for password hashing -- DES truncates to 8 characters and MD5 is
//! fast and broken. Use `gen_salt('bf', <rounds>)` (Blowfish/bcrypt) so each
//! hash is deliberately slow to brute-force.

use crate::clause_scan::split_top_level;
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

const WEAK: &[&str] = &["md5", "des", "xdes"];

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql634"
  }
  fn default_severity(&self) -> Severity {
    Severity::Warning
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, body) = crate::stmt_body(stmt, source);
    let lower = body.to_ascii_lowercase();
    let bytes = body.as_bytes();
    let n = bytes.len();
    let needle = "gen_salt(";
    let mut from = 0usize;
    while let Some(rel) = lower[from..].find(needle) {
      let at = from + rel;
      from = at + needle.len();
      if at > 0 && (bytes[at - 1].is_ascii_alphanumeric() || bytes[at - 1] == b'_') {
        continue;
      }
      let open = at + needle.len() - 1;
      let mut depth = 0i32;
      let mut j = open;
      let mut close = None;
      while j < n {
        match bytes[j] {
          b'(' => depth += 1,
          b')' => {
            depth -= 1;
            if depth == 0 {
              close = Some(j);
              break;
            }
          }
          _ => {}
        }
        j += 1;
      }
      let Some(close) = close else { continue };
      let args = &body[open + 1..close];
      let Some((first, off)) = split_top_level(args).into_iter().next() else {
        continue;
      };
      let t = first.trim();
      if t.len() >= 2 && t.starts_with('\'') && t.ends_with('\'') {
        let algo = t[1..t.len() - 1].trim().to_ascii_lowercase();
        if WEAK.contains(&algo.as_str()) {
          let lead = first.len() - first.trim_start().len();
          let abs = open + 1 + off + lead;
          out.push(Diagnostic {
            code: "sql634",
            severity: Severity::Warning,
            message: format!("`gen_salt('{algo}')` is weak for password hashing -- use `gen_salt('bf', <rounds>)` (bcrypt)"),
            range: crate::range_at(start + abs, start + abs + t.len()),
          });
        }
      }
    }
  }
}
