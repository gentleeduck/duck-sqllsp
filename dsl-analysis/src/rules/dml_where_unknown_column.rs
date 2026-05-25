//! sql351: `DELETE/UPDATE FROM t WHERE bogus` -- WHERE column not
//! found on the target table. Fills the sql002 gap (which is
//! SELECT-only).

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::{Statement, StatementKind, TableRef};
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql351"
  }
  fn default_severity(&self) -> Severity {
    Severity::Error
  }

  fn check(&self, source: &str, stmt: &Statement, scope: &Scope, catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let table_ref: &TableRef = match &stmt.kind {
      StatementKind::Update(u) => &u.table,
      StatementKind::Delete(d) => &d.table,
      _ => return,
    };
    let Some(t) = catalog.find_table(table_ref.schema.as_deref(), &table_ref.name) else { return };
    let start: usize = u32::from(stmt.range.start()) as usize;
    let end: usize = (u32::from(stmt.range.end()) as usize).min(source.len());
    let body = &source[start..end];
    // Strip line comments so `-- WHERE col` doesn't pollute the
    // predicate walk (was matching `col` inside the comment text).
    let cleaned = strip_line_comments(body);
    let upper = cleaned.to_ascii_uppercase();
    // CTE / subquery in the body -- columns from `WITH t AS (...)`
    // aren't in this statement's resolver scope; bail rather than
    // false-flag every CTE name as an unknown column.
    if upper.contains("WITH ") || upper.contains(" SELECT ") {
      return;
    }
    let body = cleaned.as_str();
    let Some(where_at) = upper.find(" WHERE ") else { return };
    let after = where_at + 7;
    let rest = &body[after..];
    let stop = rest
      .find(|c: char| c == ';')
      .or_else(|| upper[after..].find(" RETURNING ").map(|p| p))
      .unwrap_or(rest.len());
    let predicate = &rest[..stop];
    // Walk identifiers; skip strings, qualified refs, function calls.
    let bytes = predicate.as_bytes();
    let mut i = 0usize;
    while i < bytes.len() {
      if bytes[i] == b'\'' {
        i += 1;
        while i < bytes.len() && bytes[i] != b'\'' { i += 1 }
        if i < bytes.len() { i += 1 }
        continue;
      }
      if !(bytes[i].is_ascii_alphabetic() || bytes[i] == b'_') { i += 1; continue }
      let s = i;
      while i < bytes.len() && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_') { i += 1 }
      let token = &predicate[s..i];
      // Skip qualified ref (a.b), function call (foo(...)), keywords.
      if i < bytes.len() && (bytes[i] == b'.' || bytes[i] == b'(') { continue }
      let upper_tok = token.to_ascii_uppercase();
      if is_keyword(&upper_tok) { continue }
      if t.columns.iter().any(|c| c.name.eq_ignore_ascii_case(token)) { continue }
      // Token may be a table name (subquery references) -- skip.
      if catalog.tables().any(|tb| tb.name.eq_ignore_ascii_case(token)) { continue }
      // Token may be a CTE name or alias the scope knows about.
      if scope.get(token).is_some() { continue }
      let abs_s = start + after + s;
      let abs_e = abs_s + token.len();
      out.push(Diagnostic {
        code: "sql351",
        severity: Severity::Error,
        message: format!("unknown column `{token}` in WHERE on `{}`", table_ref.name),
        range: text_size::TextRange::new((abs_s as u32).into(), (abs_e as u32).into()),
      });
      return;
    }
  }
}

/// Replace `-- comment` runs with spaces so byte offsets stay 1:1.
fn strip_line_comments(s: &str) -> String {
  let mut out = String::with_capacity(s.len());
  let bytes = s.as_bytes();
  let n = bytes.len();
  let mut i = 0usize;
  while i < n {
    if i + 1 < n && bytes[i] == b'-' && bytes[i + 1] == b'-' {
      while i < n && bytes[i] != b'\n' {
        out.push(' ');
        i += 1;
      }
    } else if bytes[i].is_ascii() {
      out.push(bytes[i] as char);
      i += 1;
    } else {
      out.push(' ');
      i += 1;
    }
  }
  out
}

fn is_keyword(t: &str) -> bool {
  matches!(t,
    "AND" | "OR" | "NOT" | "IN" | "BETWEEN" | "LIKE" | "ILIKE" | "SIMILAR" | "IS" | "NULL" |
    "TRUE" | "FALSE" | "ANY" | "ALL" | "SOME" | "EXISTS" | "DISTINCT" | "FROM" | "JOIN" |
    "LEFT" | "RIGHT" | "INNER" | "OUTER" | "CROSS" | "FULL" | "ON" | "USING" | "AS" |
    "ASC" | "DESC" | "NULLS" | "FIRST" | "LAST" | "LIMIT" | "OFFSET" | "CASE" | "WHEN" |
    "THEN" | "ELSE" | "END" | "RETURNING" | "CAST" | "ARRAY" | "ROW" | "CURRENT" | "DATE" |
    "TIME" | "TIMESTAMP" | "INTERVAL" |
    "SELECT" | "UPDATE" | "INSERT" | "DELETE" | "WITH" | "VALUES" |
    "INTO" | "SET" | "WHERE" | "GROUP" | "BY" | "HAVING" | "ORDER" |
    "UNION" | "INTERSECT" | "EXCEPT" | "OFFSET" | "FETCH" | "FOR" |
    "OVER" | "PARTITION" | "WINDOW" | "FILTER" | "LATERAL" | "NATURAL"
  )
}
