//! sql097: `SELECT col FROM nothing` -- i.e. `SELECT x;` without a
//! FROM clause and without an aggregate. Usually a typo.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::{Statement, StatementKind};
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql097"
  }
  fn default_severity(&self) -> Severity {
    Severity::Hint
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    if !matches!(stmt.kind, StatementKind::Select(_)) {
      return;
    }
    let start: usize = u32::from(stmt.range.start()) as usize;
    let end: usize = (u32::from(stmt.range.end()) as usize).min(source.len());
    let body = &source[start..end];
    let upper = body.to_ascii_uppercase();
    // Skip when FROM present (whole-word match -- ` FROM ` failed on
    // `\nFROM\n` / `\tFROM` because the surrounding chars were newlines
    // or tabs not spaces; user-formatted SQL routinely indents FROM
    // on its own line).
    if has_word(&upper, "FROM") {
      return;
    }
    // Skip standalone VALUES / WITH ORDINALITY -- they carry their own
    // data source even without a FROM clause.
    if has_word(&upper, "VALUES") {
      return;
    }
    if has_word(&upper, "ORDINALITY") {
      return;
    }
    // Skip common no-FROM expressions: literals, casts, function
    // calls that look like aggregates / time / random / version.
    const OK_FUNCS: &[&str] = &[
      "NOW(",
      "CURRENT_DATE",
      "CURRENT_TIMESTAMP",
      "CURRENT_USER",
      "CURRENT_SCHEMA",
      "VERSION(",
      "RANDOM(",
      "PG_BACKEND_PID(",
      "TXID_CURRENT(",
      "USER",
      "SESSION_USER",
      // Set-returning functions and ARRAY constructors that legitimately
      // produce rows without needing a FROM.
      "UNNEST(",
      "GENERATE_SERIES(",
      "GENERATE_SUBSCRIPTS(",
      "JSONB_ARRAY_ELEMENTS(",
      "JSONB_ARRAY_ELEMENTS_TEXT(",
      "JSONB_EACH(",
      "JSONB_EACH_TEXT(",
      "JSONB_OBJECT_KEYS(",
      "JSON_ARRAY_ELEMENTS(",
      "JSON_EACH(",
      "JSON_EACH_TEXT(",
      "JSON_OBJECT_KEYS(",
      "REGEXP_MATCHES(",
      "REGEXP_SPLIT_TO_TABLE(",
      "STRING_TO_TABLE(",
      "ARRAY[",
      "ARRAY (",
      "PG_SLEEP(",
      "PG_TYPEOF(",
      "TO_CHAR(",
      "TO_TIMESTAMP(",
      "TO_DATE(",
      "TO_NUMBER(",
      "MAKE_DATE(",
      "MAKE_TIMESTAMP(",
      "MAKE_INTERVAL(",
      "CONCAT(",
      "CONCAT_WS(",
      "FORMAT(",
      "GREATEST(",
      "LEAST(",
      "COALESCE(",
      "NULLIF(",
      "CAST(",
      "EXTRACT(",
    ];
    if OK_FUNCS.iter().any(|f| upper.contains(f)) {
      return;
    }
    // Skip pure literal SELECTs (`SELECT 1`, `SELECT 'x'`, ...).
    let after_select = upper.trim_start_matches(|c: char| c == ' ' || c == '\n' || c == '\t');
    if !after_select.starts_with("SELECT") {
      return;
    }
    let proj = after_select[6..].trim_start();
    if proj.starts_with('\'') || proj.chars().next().map_or(false, |c| c.is_ascii_digit() || c == '-') {
      return;
    }
    // Skip when the projection is plain `*` (no FROM => syntax error
    // already; we don't pile on).
    if proj.starts_with('*') {
      return;
    }
    // Skip when the projection contains ANY function call -- the user
    // is calling a SELECT-expression form, not querying a missing
    // column. Covers `SELECT length('x')`, `SELECT to_tsvector(...)`,
    // `SELECT regexp_replace(...)`, `SELECT lower(col)`, etc. without
    // maintaining a hand-curated OK_FUNCS list.
    if proj.contains('(') {
      return;
    }
    // Skip casts: `SELECT '42'::int;` -- expression form.
    if proj.contains("::") {
      return;
    }
    // Skip typed-literal forms: `SELECT INTERVAL '1 day'`, `DATE '...'`,
    // `TIMESTAMP '...'`, `TIME '...'`, `B'...'`, `X'...'`.
    let proj_upper_trim = proj.trim_start();
    if proj_upper_trim.starts_with("INTERVAL ")
      || proj_upper_trim.starts_with("DATE ")
      || proj_upper_trim.starts_with("TIME ")
      || proj_upper_trim.starts_with("TIMESTAMP ")
      || proj_upper_trim.starts_with("TIMESTAMPTZ ")
      || proj_upper_trim.starts_with("B'")
      || proj_upper_trim.starts_with("X'")
      || proj_upper_trim.starts_with("E'")
      || proj_upper_trim.starts_with("$$")
    {
      return;
    }
    let abs_start = start;
    let abs_end = start + 6;
    out.push(Diagnostic {
      code: "sql097",
      severity: Severity::Hint,
      message: "SELECT without FROM and without a built-in -- did you mean to add a FROM clause?".into(),
      range: text_size::TextRange::new((abs_start as u32).into(), (abs_end as u32).into()),
    });
  }
}

fn has_word(upper: &str, needle: &str) -> bool {
  let h = upper.as_bytes();
  let n = needle.as_bytes();
  if n.is_empty() { return false }
  let mut i = 0usize;
  while i + n.len() <= h.len() {
    if h[i..i + n.len()] == *n {
      let prev_ok = i == 0 || !is_id(h[i - 1] as char);
      let next_ok = i + n.len() == h.len() || !is_id(h[i + n.len()] as char);
      if prev_ok && next_ok { return true }
    }
    i += 1;
  }
  false
}

fn is_id(c: char) -> bool { c.is_alphanumeric() || c == '_' }
