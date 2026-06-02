//! sql159: `CREATE TRIGGER ... FOR EACH STATEMENT ... NEW` -- only
//! row-level triggers have NEW/OLD. Statement-level triggers cannot
//! reference them.

use crate::{Diagnostic, LintRule, Severity};
use crate::textutil::is_word;
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql159"
  }
  fn default_severity(&self) -> Severity {
    Severity::Error
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, _body, upper) = crate::stmt_body_upper(stmt, source);
    if !upper.contains("CREATE TRIGGER") {
      return;
    }
    if !upper.contains("FOR EACH STATEMENT") {
      return;
    }
    // Look for NEW. / OLD. in any subsequent WHEN clause / function
    // body referenced. Best-effort: scan for bare NEW or OLD as a
    // word in the trigger source.
    let bytes = upper.as_bytes();
    let n = bytes.len();
    let mut i = 0;
    while i + 3 <= n {
      for kw in ["NEW", "OLD"] {
        if &upper[i..i + 3] == kw
          && (i == 0 || !is_word(bytes[i - 1] as char))
          && (i + 3 == n || !is_word(bytes[i + 3] as char))
        {
          // Only flag when the keyword is used as a row reference --
          // `NEW.col` / `OLD.col` / `:= NEW` (assignment). In particular
          // `REFERENCING NEW TABLE AS x` and `REFERENCING OLD TABLE AS y`
          // are statement-level transition-table aliases (PG10+) and
          // are completely legal.
          let mut k = i + 3;
          while k < n && bytes[k].is_ascii_whitespace() {
            k += 1
          }
          let followed_by_dot = k < n && bytes[k] == b'.';
          // What word came right before NEW/OLD?
          let mut p = i;
          while p > 0 && bytes[p - 1].is_ascii_whitespace() {
            p -= 1
          }
          let word_end = p;
          while p > 0 && (bytes[p - 1].is_ascii_alphanumeric() || bytes[p - 1] == b'_') {
            p -= 1
          }
          let prev_word = &upper[p..word_end];
          let is_referencing_kind = matches!(prev_word, "REFERENCING");
          // What word follows NEW/OLD ignoring whitespace?
          let next_word_start = k;
          let mut q = next_word_start;
          while q < n && (bytes[q].is_ascii_alphanumeric() || bytes[q] == b'_') {
            q += 1
          }
          let next_word = &upper[next_word_start..q];
          let is_followed_by_table_or_row = matches!(next_word, "TABLE" | "ROW");
          if is_referencing_kind || is_followed_by_table_or_row || !followed_by_dot {
            i += 1;
            continue;
          }
          let abs_start = start + i;
          let abs_end = start + i + 3;
          out.push(Diagnostic {
            code: "sql159",
            severity: Severity::Error,
            message: format!(
              "FOR EACH STATEMENT trigger references {kw} -- only row-level (FOR EACH ROW) triggers have NEW/OLD"
            ),
            range: crate::range_at(abs_start, abs_end),
          });
          return;
        }
      }
      i += 1;
    }
  }
}

