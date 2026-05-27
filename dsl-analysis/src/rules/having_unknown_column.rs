//! sql405: HAVING references a column that doesn't exist.
//!
//! HAVING is an expression (not a comma-separated list like GROUP BY
//! or ORDER BY), so the scanner walks every word-shaped token inside
//! the clause and checks each as either a bare or qualified column
//! ref. Tokens that are function names (followed by `(`), SQL
//! keywords / boolean / null literals, type names commonly used in
//! casts, or projection aliases are skipped.

use crate::clause_scan::{find_clause, find_clause_end, is_word};
use crate::rules::unknown_column::column_exists;
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::{Projection, Statement, StatementKind};
use dsl_resolve::Scope;
use text_size::TextRange;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql405"
  }
  fn default_severity(&self) -> Severity {
    Severity::Error
  }

  fn check(&self, source: &str, stmt: &Statement, scope: &Scope, catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    if scope.is_empty() || catalog.tables().next().is_none() {
      return;
    }
    let StatementKind::Select(s) = &stmt.kind else {
      return;
    };
    let aliases: Vec<String> = s
      .projections
      .iter()
      .filter_map(|p| match p {
        Projection::Expr { alias: Some(a), .. } => Some(a.to_ascii_lowercase()),
        _ => None,
      })
      .collect();

    let start: usize = u32::from(stmt.range.start()) as usize;
    let end: usize = (u32::from(stmt.range.end()) as usize).min(source.len());
    let raw = &source[start..end];
    let cleaned = crate::textutil::strip_noise_full(raw);
    let upper = cleaned.to_ascii_uppercase();
    let bytes_u = upper.as_bytes();
    let Some(rel_clause_start) = find_clause(bytes_u, b"HAVING") else {
      return;
    };
    let clause_end = find_clause_end(bytes_u, rel_clause_start + 6, &["ORDER BY", "LIMIT", "OFFSET", "FOR", "FETCH", "WINDOW"]);
    let clause_start = rel_clause_start + 6;
    let bytes = cleaned.as_bytes();

    let mut i = clause_start;
    let mut emitted: std::collections::HashSet<String> = std::collections::HashSet::new();
    while i < clause_end {
      let c = bytes[i];
      // Skip non-word characters.
      if !is_word(c as char) || c.is_ascii_digit() {
        i += 1;
        continue;
      }
      // Read word.
      let word_start = i;
      while i < clause_end && (is_word(bytes[i] as char) || bytes[i] == b'"') {
        i += 1;
      }
      let word = &cleaned[word_start..i];
      // Followed by `(`? Function call -- skip.
      let mut j = i;
      while j < clause_end && bytes[j].is_ascii_whitespace() {
        j += 1;
      }
      if j < clause_end && bytes[j] == b'(' {
        continue;
      }
      let key = word.to_ascii_uppercase();
      if HAVING_KEYWORDS.binary_search(&key.as_str()).is_ok() {
        continue;
      }
      // Qualified: peek for `.` then another word.
      let (qualifier, name, name_end) = if j < clause_end && bytes[j] == b'.' {
        let mut k = j + 1;
        while k < clause_end && bytes[k].is_ascii_whitespace() {
          k += 1;
        }
        let n_start = k;
        while k < clause_end && (is_word(bytes[k] as char) || bytes[k] == b'"') {
          k += 1;
        }
        if k == n_start {
          continue;
        }
        let n = &cleaned[n_start..k];
        (Some(strip_quotes(word).to_string()), strip_quotes(n).to_string(), k)
      } else {
        (None, strip_quotes(word).to_string(), i)
      };
      if name.is_empty() {
        continue;
      }
      if qualifier.is_none() && aliases.iter().any(|a| a == &name.to_ascii_lowercase()) {
        continue;
      }
      if column_exists(scope, catalog, qualifier.as_deref(), &name) {
        // also advance i past the qualified second word
        i = name_end;
        continue;
      }
      let display = match &qualifier {
        Some(q) => format!("{q}.{name}"),
        None => name.clone(),
      };
      let dedup_key = display.to_ascii_lowercase();
      if !emitted.insert(dedup_key) {
        i = name_end;
        continue;
      }
      let abs_start = start + word_start;
      let abs_end = start + name_end;
      out.push(Diagnostic {
        code: "sql405",
        severity: Severity::Error,
        message: format!("unknown column `{display}` in HAVING"),
        range: TextRange::new((abs_start as u32).into(), (abs_end as u32).into()),
      });
      i = name_end;
    }
  }
}

fn strip_quotes(s: &str) -> &str {
  if s.len() >= 2 && s.starts_with('"') && s.ends_with('"') {
    &s[1..s.len() - 1]
  } else {
    s
  }
}

/// Uppercase, **sorted** keyword list. SQL keywords + boolean / null
/// literals + common type names that appear unparenthesized in HAVING
/// expressions (CAST targets, INTERVAL units, etc.). Must stay sorted
/// for the binary_search lookup above.
const HAVING_KEYWORDS: &[&str] = &[
  "ALL",
  "AND",
  "ANY",
  "ARRAY",
  "AS",
  "ASC",
  "BETWEEN",
  "BIGINT",
  "BOOLEAN",
  "BOTH",
  "BY",
  "CASE",
  "CAST",
  "COLLATE",
  "DATE",
  "DAY",
  "DAYS",
  "DESC",
  "DISTINCT",
  "DOUBLE",
  "ELSE",
  "END",
  "ESCAPE",
  "EXISTS",
  "FALSE",
  "FLOAT",
  "FROM",
  "HOUR",
  "HOURS",
  "ILIKE",
  "IN",
  "INT",
  "INTEGER",
  "INTERVAL",
  "IS",
  "JSON",
  "JSONB",
  "LEADING",
  "LIKE",
  "MINUTE",
  "MINUTES",
  "MONTH",
  "MONTHS",
  "NOT",
  "NULL",
  "NULLS",
  "NUMERIC",
  "OR",
  "PRECISION",
  "REAL",
  "SECOND",
  "SECONDS",
  "SIMILAR",
  "SMALLINT",
  "SOME",
  "SYMMETRIC",
  "TEXT",
  "THEN",
  "TIME",
  "TIMESTAMP",
  "TIMESTAMPTZ",
  "TIMETZ",
  "TO",
  "TRAILING",
  "TRUE",
  "UNKNOWN",
  "USING",
  "VARCHAR",
  "WHEN",
  "WITH",
  "WITHIN",
  "WITHOUT",
  "YEAR",
  "YEARS",
  "ZONE",
];
