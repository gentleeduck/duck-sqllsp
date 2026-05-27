//! Text-level fallback scope builder.
//!
//! Used when the parser cannot make sense of the surrounding statement
//! (which happens constantly while the user is typing, e.g. `SELECT u.`).
//! We scan the raw source for FROM / JOIN clauses with regex-style passes
//! and produce a [`Scope`] mapping aliases to table names. The result is
//! "best effort" -- nested subqueries and CTEs are not fully handled --
//! but enough to recover useful completion in the half-written state.

use dsl_parse::TableRef;
use dsl_resolve::{Binding, Scope};
use text_size::TextRange;

/// Yield the CTE names declared by a `WITH [RECURSIVE] name [(cols)] AS
/// (body), name2 AS (body2) ...` prefix on `src`. Pure text-level scan
/// so it still works when the parser bails on an unfinished trailing
/// statement (the common case while the user is typing the outer FROM).
pub fn cte_names_from_text(src: &str) -> Vec<String> {
  let bytes = src.as_bytes();
  let n = bytes.len();
  let mut out = Vec::new();
  let mut i = 0;
  // Skip whitespace / line+block comments to find the leading WITH.
  loop {
    while i < n && bytes[i].is_ascii_whitespace() {
      i += 1;
    }
    if i + 1 < n && bytes[i] == b'-' && bytes[i + 1] == b'-' {
      while i < n && bytes[i] != b'\n' {
        i += 1;
      }
      continue;
    }
    if i + 1 < n && bytes[i] == b'/' && bytes[i + 1] == b'*' {
      i += 2;
      while i + 1 < n && !(bytes[i] == b'*' && bytes[i + 1] == b'/') {
        i += 1;
      }
      i = (i + 2).min(n);
      continue;
    }
    break;
  }
  if i + 4 > n || !src[i..i + 4].eq_ignore_ascii_case("WITH") {
    return out;
  }
  i += 4;
  // Optional RECURSIVE.
  while i < n && bytes[i].is_ascii_whitespace() {
    i += 1;
  }
  if i + 9 <= n && src[i..i + 9].eq_ignore_ascii_case("RECURSIVE") {
    i += 9;
  }
  // Loop over CTE definitions: <name> [(<cols>)] AS [...]? (<body>) [,]
  loop {
    while i < n && bytes[i].is_ascii_whitespace() {
      i += 1;
    }
    // Read CTE name (word identifier or quoted).
    let name_start = i;
    let name: String;
    if i < n && bytes[i] == b'"' {
      i += 1;
      let inner = i;
      while i < n && bytes[i] != b'"' {
        i += 1;
      }
      if i >= n {
        return out;
      }
      name = src[inner..i].to_string();
      i += 1;
    } else {
      while i < n && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_') {
        i += 1;
      }
      if i == name_start {
        return out;
      }
      name = src[name_start..i].to_string();
    }
    // Skip whitespace, optional column list, AS, optional MATERIALIZED, then the body parens.
    while i < n && bytes[i].is_ascii_whitespace() {
      i += 1;
    }
    if i < n && bytes[i] == b'(' {
      let mut depth = 1i32;
      i += 1;
      while i < n && depth > 0 {
        match bytes[i] {
          b'(' => depth += 1,
          b')' => depth -= 1,
          _ => {},
        }
        i += 1;
      }
      while i < n && bytes[i].is_ascii_whitespace() {
        i += 1;
      }
    }
    // Require `AS` (case-insensitive); otherwise this isn't a real CTE def.
    if i + 2 > n || !src[i..i + 2].eq_ignore_ascii_case("AS") {
      // Still emit the name -- the user may not have typed AS yet.
      out.push(name);
      return out;
    }
    i += 2;
    while i < n && bytes[i].is_ascii_whitespace() {
      i += 1;
    }
    for kw in ["NOT MATERIALIZED", "MATERIALIZED"] {
      if i + kw.len() <= n && src[i..i + kw.len()].eq_ignore_ascii_case(kw) {
        i += kw.len();
        while i < n && bytes[i].is_ascii_whitespace() {
          i += 1;
        }
        break;
      }
    }
    out.push(name);
    if i >= n || bytes[i] != b'(' {
      return out;
    }
    let mut depth = 1i32;
    i += 1;
    while i < n && depth > 0 {
      match bytes[i] {
        b'(' => depth += 1,
        b')' => depth -= 1,
        b'\'' => {
          i += 1;
          while i < n && bytes[i] != b'\'' {
            i += 1;
          }
        },
        _ => {},
      }
      if i < n {
        i += 1;
      }
    }
    while i < n && bytes[i].is_ascii_whitespace() {
      i += 1;
    }
    if i < n && bytes[i] == b',' {
      i += 1;
      continue;
    }
    return out;
  }
}

/// Like [`cte_names_from_text`] but for a *single* named CTE: walk the
/// `WITH ... AS (body)` prefix and return the projected column names
/// of the CTE whose name matches `target` (case-insensitive). The body
/// is re-parsed as a standalone SELECT so the column names come from
/// the projection list -- including any aliases the user wrote.
///
/// Returns an empty vector when the CTE is found but its body doesn't
/// parse (the caller treats that as "I know the CTE exists but can't
/// enumerate its columns"). Returns `None` when no matching CTE is
/// declared in the leading WITH.
pub fn cte_columns_from_text(src: &str, target: &str) -> Option<Vec<String>> {
  let bytes = src.as_bytes();
  let n = bytes.len();
  let mut i = 0;
  // Skip whitespace / comments to find leading WITH.
  loop {
    while i < n && bytes[i].is_ascii_whitespace() {
      i += 1;
    }
    if i + 1 < n && bytes[i] == b'-' && bytes[i + 1] == b'-' {
      while i < n && bytes[i] != b'\n' {
        i += 1;
      }
      continue;
    }
    if i + 1 < n && bytes[i] == b'/' && bytes[i + 1] == b'*' {
      i += 2;
      while i + 1 < n && !(bytes[i] == b'*' && bytes[i + 1] == b'/') {
        i += 1;
      }
      i = (i + 2).min(n);
      continue;
    }
    break;
  }
  if i + 4 > n || !src[i..i + 4].eq_ignore_ascii_case("WITH") {
    return None;
  }
  i += 4;
  while i < n && bytes[i].is_ascii_whitespace() {
    i += 1;
  }
  if i + 9 <= n && src[i..i + 9].eq_ignore_ascii_case("RECURSIVE") {
    i += 9;
  }
  loop {
    while i < n && bytes[i].is_ascii_whitespace() {
      i += 1;
    }
    // CTE name.
    let name_start = i;
    let name: String;
    if i < n && bytes[i] == b'"' {
      i += 1;
      let inner = i;
      while i < n && bytes[i] != b'"' {
        i += 1;
      }
      if i >= n {
        return None;
      }
      name = src[inner..i].to_string();
      i += 1;
    } else {
      while i < n && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_') {
        i += 1;
      }
      if i == name_start {
        return None;
      }
      name = src[name_start..i].to_string();
    }
    while i < n && bytes[i].is_ascii_whitespace() {
      i += 1;
    }
    // Optional column list `(col1, col2, ...)` -- if present, this IS
    // the projected schema; no need to re-parse the body.
    let mut explicit_cols: Option<Vec<String>> = None;
    if i < n && bytes[i] == b'(' {
      let start = i + 1;
      let mut depth = 1i32;
      i += 1;
      while i < n && depth > 0 {
        match bytes[i] {
          b'(' => depth += 1,
          b')' => depth -= 1,
          _ => {},
        }
        i += 1;
      }
      let end = (i - 1).min(n);
      explicit_cols = Some(
        src[start..end]
          .split(',')
          .map(|s| s.trim().trim_matches('"').to_string())
          .filter(|s| !s.is_empty())
          .collect(),
      );
      while i < n && bytes[i].is_ascii_whitespace() {
        i += 1;
      }
    }
    if i + 2 > n || !src[i..i + 2].eq_ignore_ascii_case("AS") {
      return if name.eq_ignore_ascii_case(target) { Some(explicit_cols.unwrap_or_default()) } else { None };
    }
    i += 2;
    while i < n && bytes[i].is_ascii_whitespace() {
      i += 1;
    }
    for kw in ["NOT MATERIALIZED", "MATERIALIZED"] {
      if i + kw.len() <= n && src[i..i + kw.len()].eq_ignore_ascii_case(kw) {
        i += kw.len();
        while i < n && bytes[i].is_ascii_whitespace() {
          i += 1;
        }
        break;
      }
    }
    if i >= n || bytes[i] != b'(' {
      return if name.eq_ignore_ascii_case(target) { Some(explicit_cols.unwrap_or_default()) } else { None };
    }
    let body_start = i + 1;
    let mut depth = 1i32;
    i += 1;
    while i < n && depth > 0 {
      match bytes[i] {
        b'(' => depth += 1,
        b')' => depth -= 1,
        b'\'' => {
          i += 1;
          while i < n && bytes[i] != b'\'' {
            i += 1;
          }
        },
        _ => {},
      }
      if i < n {
        i += 1;
      }
    }
    let body_end = (i - 1).min(n);
    if name.eq_ignore_ascii_case(target) {
      if let Some(cols) = explicit_cols {
        return Some(cols);
      }
      let body = &src[body_start..body_end];
      return Some(projection_columns_of(body));
    }
    while i < n && bytes[i].is_ascii_whitespace() {
      i += 1;
    }
    if i < n && bytes[i] == b',' {
      i += 1;
      continue;
    }
    return None;
  }
}

/// Re-parse `body` (a single SELECT) and return its projection column
/// names. Bare `Column { name }` projections use the column name;
/// `Expr { alias: Some(a) }` uses the alias; other shapes are skipped.
fn projection_columns_of(body: &str) -> Vec<String> {
  let file = dsl_parse::parse(body, dsl_parse::Dialect::Postgres);
  for stmt in &file.statements {
    if let dsl_parse::StatementKind::Select(s) = &stmt.kind {
      let mut out = Vec::new();
      for p in &s.projections {
        if let dsl_parse::Projection::Expr { expr, alias } = p {
          if let Some(a) = alias {
            out.push(a.clone());
          } else if let dsl_parse::Expr::Column { name, .. } = expr {
            out.push(name.clone());
          }
        }
      }
      return out;
    }
  }
  Vec::new()
}

/// Return a fallback scope, or None if no FROM / JOIN matches were found.
pub fn scope_from_text(src: &str) -> Option<Scope> {
  let mut scope = Scope::default();
  for (schema, table, alias) in iter_table_bindings(src) {
    let table_ref =
      TableRef { schema: schema.clone(), name: table.clone(), alias: Some(alias.clone()), range: TextRange::default() };
    let binding = Binding { alias: alias.clone(), table: table_ref.clone() };
    scope.bindings.insert(alias, binding.clone());
    // Bind by unaliased name too so qualified refs like `users.id` work.
    scope.bindings.entry(table).or_insert(Binding { alias: table_ref.name.clone(), table: table_ref });
  }
  if scope.is_empty() { None } else { Some(scope) }
}

/// Yield (schema, table, alias) tuples from FROM / JOIN / UPDATE / DELETE
/// / INTO clauses in the source. Consumes comma-separated relation lists
/// after FROM / UPDATE so `FROM users u, orders o` exposes both bindings.
/// `schema` is `None` for bare table refs and `Some(...)` for the dotted
/// `schema.table` form.
fn iter_table_bindings(src: &str) -> Vec<(Option<String>, String, String)> {
  let mut out = Vec::new();
  let chars: Vec<char> = src.chars().collect();
  let mut i = 0;
  while i < chars.len() {
    if let Some(consumed) = match_keyword(&chars, i, &["FROM", "JOIN", "UPDATE", "INTO"]) {
      i += consumed;
      // Read the first table, then any comma-continuations.
      loop {
        if let Some((schema, table, alias, consumed2)) = read_table_with_alias(&chars, i) {
          out.push((schema, table, alias));
          i += consumed2;
          // Skip whitespace; require an immediate `,` to take
          // another table in the same FROM/UPDATE list.
          let ws = skip_ws(&chars, i);
          if chars.get(i + ws) == Some(&',') {
            i += ws + 1;
            continue;
          }
        }
        break;
      }
      continue;
    }
    i += 1;
  }
  out
}

/// Return the number of consumed chars if any of `keywords` matches at
/// `pos`, as a whole word (preceded and followed by non-word chars).
fn match_keyword(chars: &[char], pos: usize, keywords: &[&str]) -> Option<usize> {
  // Must be at a word boundary.
  if pos > 0 && is_word(chars[pos - 1]) {
    return None;
  }
  for &kw in keywords {
    if matches_ci(chars, pos, kw) && chars.get(pos + kw.len()).is_none_or(|c| !is_word(*c)) {
      return Some(kw.len());
    }
  }
  None
}

fn matches_ci(chars: &[char], pos: usize, target: &str) -> bool {
  let tchars: Vec<char> = target.chars().collect();
  if chars.len() < pos + tchars.len() {
    return false;
  }
  for (i, t) in tchars.iter().enumerate() {
    if !chars[pos + i].eq_ignore_ascii_case(t) {
      return false;
    }
  }
  true
}

fn is_word(c: char) -> bool {
  c.is_alphanumeric() || c == '_'
}

/// Read `[whitespace] [<schema>.]<table> [AS <alias>] | <alias>` after a
/// FROM/JOIN. Returns (schema, table, alias, consumed chars). `schema`
/// is `Some(...)` only for the dotted form `schema.table`.
fn read_table_with_alias(chars: &[char], pos: usize) -> Option<(Option<String>, String, String, usize)> {
  let mut i = pos;
  let ws = skip_ws(chars, i);
  i += ws;

  let (first, f_len) = read_ident(chars, i)?;
  if first.is_empty() {
    return None;
  }
  i += f_len;
  // Schema-qualified form: `schema.table`. The first ident becomes the
  // schema, the ident after the dot becomes the table.
  let mut schema: Option<String> = None;
  let mut table = first;
  if chars.get(i) == Some(&'.')
    && let Some((after, a_len)) = read_ident(chars, i + 1)
    && !after.is_empty()
  {
    schema = Some(table);
    table = after;
    i += 1 + a_len;
  }

  // Optional AS alias  |  bare alias  |  no alias.
  let mut alias = table.clone();
  let after_table = i;
  i += skip_ws(chars, i);
  if matches_ci(chars, i, "AS") && chars.get(i + 2).is_none_or(|c| !is_word(*c)) {
    i += 2;
    i += skip_ws(chars, i);
    if let Some((ident, len)) = read_ident(chars, i) {
      alias = ident;
      i += len;
    }
  } else if let Some((ident, len)) = read_ident(chars, i) {
    // Bare alias only if the identifier doesn't look like a SQL keyword.
    // We catch the common keywords that legitimately follow a table
    // without an alias: ON, WHERE, JOIN, GROUP, ORDER, LIMIT, etc.
    let upper = ident.to_uppercase();
    const STOPWORDS: &[&str] = &[
      "ON",
      "WHERE",
      "JOIN",
      "INNER",
      "LEFT",
      "RIGHT",
      "FULL",
      "OUTER",
      "CROSS",
      "LATERAL",
      "USING",
      "GROUP",
      "ORDER",
      "LIMIT",
      "OFFSET",
      "HAVING",
      "UNION",
      "INTERSECT",
      "EXCEPT",
      "AS",
      // DML clause keywords that follow `UPDATE <table>` / `DELETE FROM
      // <table>` / `INSERT INTO <table>` -- without these, the next
      // token (`SET`, `VALUES`, `RETURNING`, ...) was being captured as
      // a bogus alias for the target table.
      "SET",
      "VALUES",
      "RETURNING",
      "DEFAULT",
      "FROM",
      "INTO",
      "WITH",
      "TABLESAMPLE",
      "FETCH",
      "FOR",
      "WINDOW",
      "NATURAL",
    ];
    if !STOPWORDS.contains(&upper.as_str()) {
      alias = ident;
      i += len;
    } else {
      i = after_table;
    }
  } else {
    i = after_table;
  }

  Some((schema, table, alias, i - pos))
}

fn skip_ws(chars: &[char], pos: usize) -> usize {
  let mut n = 0;
  while pos + n < chars.len() && chars[pos + n].is_whitespace() {
    n += 1;
  }
  n
}

fn read_ident(chars: &[char], pos: usize) -> Option<(String, usize)> {
  // Double-quoted identifier: `"Anything Goes"` -- return the inner
  // text (without quotes) so the rest of the pipeline can match it
  // against scope keys, which the resolver stores unquoted.
  if chars.get(pos) == Some(&'"') {
    let mut n = 1;
    while pos + n < chars.len() && chars[pos + n] != '"' {
      n += 1;
    }
    if pos + n >= chars.len() {
      return None;
    }
    let inner: String = chars[pos + 1..pos + n].iter().collect();
    if inner.is_empty() {
      return None;
    }
    return Some((inner, n + 1));
  }
  let mut n = 0;
  while pos + n < chars.len() && is_word(chars[pos + n]) {
    n += 1;
  }
  if n == 0 {
    return None;
  }
  Some((chars[pos..pos + n].iter().collect(), n))
}
