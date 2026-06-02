//! sql150: `CASE WHEN ... THEN ... END` without an `ELSE` branch.
//! Unmatched rows return NULL silently. Hint to add `ELSE` explicitly
//! so the author's intent is on the page.

use crate::{Diagnostic, LintRule, Severity};
use crate::textutil::is_word;
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql150"
  }
  fn default_severity(&self) -> Severity {
    Severity::Hint
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, _body, upper) = crate::stmt_body_upper(stmt, source);
    let bytes = upper.as_bytes();
    let n = bytes.len();
    let mut i = 0;
    while i + 4 <= n {
      if &upper[i..i + 4] == "CASE"
        && (i == 0 || !is_word(bytes[i - 1] as char))
        && (i + 4 == n || !is_word(bytes[i + 4] as char))
      {
        // Find matching END at the same nesting depth.
        let mut depth = 1i32;
        let mut j = i + 4;
        let mut has_else = false;
        while j < n && depth > 0 {
          // Skip strings.
          if bytes[j] == b'\'' {
            j += 1;
            while j < n && bytes[j] != b'\'' {
              j += 1;
            }
            if j < n {
              j += 1;
            }
            continue;
          }
          if j + 4 <= n
            && &upper[j..j + 4] == "CASE"
            && (j == 0 || !is_word(bytes[j - 1] as char))
            && (j + 4 == n || !is_word(bytes[j + 4] as char))
          {
            depth += 1;
            j += 4;
            continue;
          }
          if j + 3 <= n
            && &upper[j..j + 3] == "END"
            && (j == 0 || !is_word(bytes[j - 1] as char))
            && (j + 3 == n || !is_word(bytes[j + 3] as char))
          {
            depth -= 1;
            if depth == 0 {
              break;
            }
            j += 3;
            continue;
          }
          if depth == 1
            && j + 4 <= n
            && &upper[j..j + 4] == "ELSE"
            && (j == 0 || !is_word(bytes[j - 1] as char))
            && (j + 4 == n || !is_word(bytes[j + 4] as char))
          {
            has_else = true;
          }
          j += 1;
        }
        if !has_else && depth == 0 {
          let abs_start = start + i;
          let abs_end = start + i + 4;
          out.push(Diagnostic {
                        code: "sql150",
                        severity: Severity::Hint,
                        message: "CASE without an ELSE branch -- unmatched rows return NULL silently; add `ELSE NULL` to make intent explicit".into(),
                        range: crate::range_at(abs_start, abs_end),
                    });
          return;
        }
        i = j.saturating_add(3);
        continue;
      }
      i += 1;
    }
  }
}

