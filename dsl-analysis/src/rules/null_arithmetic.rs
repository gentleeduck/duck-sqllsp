//! sql462: `x + NULL` (or `-`, `*`, `/`, `%`) -- arithmetic with a
//! literal NULL operand always returns NULL. Almost always a typo
//! or a leftover placeholder (the user dropped a real value). In
//! WHERE / ON the row will silently disappear; in projection the
//! row will quietly show NULL.

use crate::clause_scan::is_word;
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;
use text_size::TextRange;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql462"
  }
  fn default_severity(&self) -> Severity {
    Severity::Warning
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let start: usize = u32::from(stmt.range.start()) as usize;
    let end: usize = (u32::from(stmt.range.end()) as usize).min(source.len());
    let raw = &source[start..end];
    let cleaned = crate::textutil::strip_noise_full(raw);
    let upper = cleaned.to_ascii_uppercase();
    let ub = upper.as_bytes();
    let bytes = cleaned.as_bytes();
    let n = ub.len();
    let mut i = 0usize;
    let mut emitted_at: std::collections::HashSet<usize> = std::collections::HashSet::new();
    while i + 4 <= n {
      if !(&ub[i..i + 4] == b"NULL"
        && (i == 0 || !is_word(ub[i - 1] as char))
        && (i + 4 == n || !is_word(ub[i + 4] as char)))
      {
        i += 1;
        continue;
      }
      // Look BEFORE NULL: scan back over whitespace, then check for
      // an arithmetic op preceded by a non-operator char.
      let mut p = i;
      while p > 0 && bytes[p - 1].is_ascii_whitespace() {
        p -= 1;
      }
      let prev_op = if p > 0 {
        let c = bytes[p - 1];
        if matches!(c, b'+' | b'-' | b'*' | b'/' | b'%') {
          // Avoid `**` (not a PG op anyway), `||`, `::`, `<>` etc:
          // single-char ops only count when the char before isn't
          // another op char or another `=`/`<`/`>`/`!`/`|`/`:`.
          let two_back_ok = if p >= 2 {
            !matches!(bytes[p - 2], b'+' | b'-' | b'*' | b'/' | b'%' | b'=' | b'<' | b'>' | b'!' | b'|' | b':')
          } else {
            true
          };
          if two_back_ok {
            Some(c as char)
          } else {
            None
          }
        } else {
          None
        }
      } else {
        None
      };
      // Look AFTER NULL: scan past whitespace, check for arithmetic op
      // not followed by another op char (avoids `||`, `::`, `**`, etc.).
      let mut q = i + 4;
      while q < n && bytes[q].is_ascii_whitespace() {
        q += 1;
      }
      let next_op = if q < n {
        let c = bytes[q];
        if matches!(c, b'+' | b'-' | b'*' | b'/' | b'%') {
          let next_next_ok = if q + 1 < n {
            !matches!(bytes[q + 1], b'+' | b'-' | b'*' | b'/' | b'%' | b'=' | b'<' | b'>' | b'!' | b'|' | b':')
          } else {
            true
          };
          if next_next_ok {
            Some(c as char)
          } else {
            None
          }
        } else {
          None
        }
      } else {
        None
      };
      if let Some(op) = prev_op.or(next_op)
        && emitted_at.insert(i)
      {
        let abs_s = start + i;
        let abs_e = start + i + 4;
        out.push(Diagnostic {
          code: "sql462",
          severity: Severity::Warning,
          message: format!(
            "arithmetic with NULL operand (`{op} NULL` or `NULL {op}`) always returns NULL -- in WHERE/ON the row silently filters out, in projection it silently yields NULL. Drop the NULL or COALESCE it to a real value"
          ),
          range: TextRange::new((abs_s as u32).into(), (abs_e as u32).into()),
        });
      }
      i += 4;
    }
  }
}
