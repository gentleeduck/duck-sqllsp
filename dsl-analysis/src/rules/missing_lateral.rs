//! sql151: `SELECT ... FROM t, generate_series(t.col, 10)` -- the
//! function reads from `t.col` but no `LATERAL` keyword. PG rejects:
//! "missing FROM-clause entry for table t".

use crate::{Diagnostic, LintRule, Severity};
use crate::textutil::is_word;
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql151"
  }
  fn default_severity(&self) -> Severity {
    Severity::Warning
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, body, upper) = crate::stmt_body_upper(stmt, source);
    // Need `FROM <tbl> [<alias>] , <fn>(<dotted ref>)`.
    let Some(from_at) = upper.find(" FROM ") else { return };
    let after = &body[from_at + 6..];
    let after_up = &upper[from_at + 6..];
    // Find a function call invocation after a comma in the FROM list.
    let bytes = after.as_bytes();
    let n = bytes.len();
    let mut depth = 0i32;
    let mut i = 0;
    while i < n {
      match bytes[i] {
        b'(' => depth += 1,
        b')' => depth -= 1,
        b',' if depth == 0 => {
          // Look forward: skip ws, identifier, `(`.
          let mut j = i + 1;
          while j < n && bytes[j].is_ascii_whitespace() {
            j += 1;
          }
          let id_start = j;
          while j < n && (is_word(bytes[j] as char)) {
            j += 1;
          }
          if j == id_start {
            i += 1;
            continue;
          }
          let id_up = &after_up[id_start..j];
          // Skip table refs without parens (no function call).
          while j < n && bytes[j].is_ascii_whitespace() {
            j += 1;
          }
          if j >= n || bytes[j] != b'(' {
            i += 1;
            continue;
          }
          // Walk to close paren, collect inner text.
          let inner_start = j + 1;
          let mut k = inner_start;
          let mut d = 1i32;
          while k < n && d > 0 {
            match bytes[k] {
              b'(' => d += 1,
              b')' => d -= 1,
              _ => {},
            }
            if d == 0 {
              break;
            }
            k += 1;
          }
          if k >= n {
            break;
          }
          let inner = &after[inner_start..k];
          let inner_up = inner.to_ascii_uppercase();
          // Has dotted reference inside? Skip if LATERAL is
          // word-bounded before id_start.
          if inner.contains('.') {
            let before = &after_up[..id_start];
            let has_lateral = before
              .rsplit_once("LATERAL")
              .map(|(_, tail)| tail.trim().is_empty() || tail.chars().all(|c| c.is_whitespace()))
              .unwrap_or(false);
            // PG documents that for **functions** in the FROM clause
            // the LATERAL keyword is OPTIONAL -- a function-call FROM
            // can reference earlier FROM items without it. Only flag
            // subselects (`FROM a, (SELECT a.x ...)`) that omit LATERAL.
            if !has_lateral && inner_up.trim_start().starts_with("SELECT") {
              let abs_start = start + from_at + 6 + id_start;
              let abs_end = start + from_at + 6 + j;
              out.push(Diagnostic {
                                code: "sql151",
                                severity: Severity::Warning,
                                message: format!("`{}(...)` in FROM references an outer-table column without LATERAL -- PG rejects with 'missing FROM-clause entry'", id_up),
                                range: crate::range_at(abs_start, abs_end),
                            });
              return;
            }
          }
          i = k + 1;
          continue;
        },
        _ => {},
      }
      i += 1;
    }
  }
}

