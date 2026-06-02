//! sql145: column `DEFAULT now()` (or any volatile expression) freezes
//! the value at insert time, which is usually fine -- but DEFAULT
//! random() / nextval() / etc. inside CREATE TABLE produces a fresh
//! value per row at insert. Surface as a Hint so the user is aware
//! the default is recomputed per row.

use crate::{Diagnostic, LintRule, Severity};
use crate::textutil::is_word;
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql145"
  }
  fn default_severity(&self) -> Severity {
    Severity::Hint
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, body, upper) = crate::stmt_body_upper(stmt, source);
    let trimmed = upper.trim_start();
    if !(trimmed.starts_with("CREATE TABLE") || trimmed.starts_with("ALTER TABLE")) {
      return;
    }
    let bytes = upper.as_bytes();
    let n = bytes.len();
    let mut i = 0;
    while i + 7 <= n {
      if &upper[i..i + 7] == "DEFAULT"
        && (i == 0 || !is_word(bytes[i - 1] as char))
        && (i + 7 == n || !is_word(bytes[i + 7] as char))
      {
        let mut j = i + 7;
        while j < n && bytes[j].is_ascii_whitespace() {
          j += 1;
        }
        let arg_start = j;
        while j < n && (is_word(bytes[j] as char) || bytes[j] == b'(' || bytes[j] == b')') {
          j += 1;
        }
        let arg = &upper[arg_start..j];
        // Whitelist the well-known volatile-by-design defaults:
        // gen_random_uuid, uuid_generate_v*, now, current_*,
        // clock_timestamp, nextval -- these are the *intended*
        // uses of a volatile default. Only flag random() (almost
        // never what you want as a default).
        let volatile = arg.starts_with("RANDOM(");
        if volatile {
          let abs_start = start + i;
          let abs_end = start + j;
          out.push(Diagnostic {
            code: "sql145",
            severity: Severity::Hint,
            message: format!(
              "DEFAULT `{}` is volatile -- the column gets a fresh value per inserted row, not a single fixed value",
              &body[arg_start..j]
            ),
            range: crate::range_at(abs_start, abs_end),
          });
          return;
        }
        i = j;
        continue;
      }
      i += 1;
    }
  }
}

