//! sql336: `bytea` literal `'\\xFF'` without the `E''` escape-string
//! prefix. PG defaults to standard-conforming strings on PG9.1+, so a
//! bare backslash is *literal*, not an escape. Hex-bytea literals
//! need `'\xFF'::bytea` or `E'\\xFF'`.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql336"
  }
  fn default_severity(&self) -> Severity {
    Severity::Warning
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let start: usize = u32::from(stmt.range.start()) as usize;
    let end: usize = (u32::from(stmt.range.end()) as usize).min(source.len());
    let body = &source[start..end];
    let bytes = body.as_bytes();
    let mut i = 0usize;
    while i + 4 <= bytes.len() {
      if bytes[i] == b'\'' {
        // Look back for E or e prefix.
        let prefix_ok = i > 0 && (bytes[i - 1] == b'E' || bytes[i - 1] == b'e');
        let s_start = i + 1;
        let mut s_end = s_start;
        while s_end < bytes.len() && bytes[s_end] != b'\'' { s_end += 1 }
        if s_end >= bytes.len() { break }
        let lit = &body[s_start..s_end];
        // Heuristic: starts with `\x` and only hex digits after, looks like a hex-bytea.
        if !prefix_ok && lit.starts_with("\\x") && lit.len() >= 4 && lit[2..].bytes().all(|b| b.is_ascii_hexdigit()) {
          let abs_s = start + i;
          let abs_e = start + s_end + 1;
          out.push(Diagnostic {
            code: "sql336",
            severity: Severity::Warning,
            message: "bytea hex literal needs the `E''` escape prefix (or `::bytea` cast) -- standard strings treat `\\` as literal".into(),
            range: text_size::TextRange::new((abs_s as u32).into(), (abs_e as u32).into()),
          });
        }
        i = s_end + 1;
        continue;
      }
      i += 1;
    }
  }
}
