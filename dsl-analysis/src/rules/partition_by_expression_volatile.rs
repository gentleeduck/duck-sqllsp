//! sql759: `PARTITION BY RANGE/LIST/HASH (some_volatile_fn(col))` --
//! PostgreSQL requires partition key expressions to be immutable and
//! rejects non-immutable functions ("functions in partition key
//! expression must be marked IMMUTABLE"). Flags the common cases where
//! the expression obviously calls a well-known volatile/stable
//! builtin; anything else is left alone (no catalog volatility lookup
//! available for user-defined functions).

use crate::clause_scan::{find_clause, split_top_level};
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

const VOLATILE_FNS: &[&str] =
  &["NOW", "CLOCK_TIMESTAMP", "STATEMENT_TIMESTAMP", "TRANSACTION_TIMESTAMP", "RANDOM", "GEN_RANDOM_UUID", "NEXTVAL"];
const VOLATILE_KEYWORDS: &[&str] = &["CURRENT_TIMESTAMP", "LOCALTIMESTAMP"];

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql759"
  }
  fn default_severity(&self) -> Severity {
    Severity::Error
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, _body, upper) = crate::stmt_body_upper(stmt, source);
    let ub = upper.as_bytes();
    let Some(kw) = find_clause(ub, b"PARTITION BY") else { return };
    let mut i = skip_ws(ub, kw + "PARTITION BY".len());
    for strategy in ["RANGE", "LIST", "HASH"] {
      if upper[i..].starts_with(strategy) {
        i += strategy.len();
        break;
      }
    }
    i = skip_ws(ub, i);
    if ub.get(i) != Some(&b'(') {
      return;
    }
    let Some(close) = match_paren(ub, i) else { return };
    let list = &upper[i + 1..close];
    for (entry, off) in split_top_level(list) {
      let trimmed = entry.trim_start();
      let lead = entry.len() - trimmed.len();
      let hit = VOLATILE_FNS
        .iter()
        .find(|f| trimmed.starts_with(**f) && trimmed[f.len()..].trim_start().starts_with('('))
        .or_else(|| VOLATILE_KEYWORDS.iter().find(|f| trimmed.starts_with(**f)));
      if let Some(fname) = hit {
        let abs = i + 1 + off + lead;
        out.push(Diagnostic {
          code: "sql759",
          severity: Severity::Error,
          message: format!(
            "partition key expression calls {fname}, which is not IMMUTABLE -- PostgreSQL rejects non-immutable partition key expressions"
          ),
          range: crate::range_at(start + abs, start + abs + fname.len()),
        });
      }
    }
  }
}

fn match_paren(ub: &[u8], open: usize) -> Option<usize> {
  let mut depth = 0i32;
  let mut i = open;
  while i < ub.len() {
    match ub[i] {
      b'(' => depth += 1,
      b')' => {
        depth -= 1;
        if depth == 0 {
          return Some(i);
        }
      },
      b'\'' => {
        i += 1;
        while i < ub.len() && ub[i] != b'\'' {
          i += 1;
        }
      },
      _ => {},
    }
    i += 1;
  }
  None
}

fn skip_ws(ub: &[u8], mut i: usize) -> usize {
  while i < ub.len() && ub[i].is_ascii_whitespace() {
    i += 1;
  }
  i
}
