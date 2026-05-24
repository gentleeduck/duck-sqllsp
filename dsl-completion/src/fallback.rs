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

/// Return a fallback scope, or None if no FROM / JOIN matches were found.
pub fn scope_from_text(src: &str) -> Option<Scope> {
  let mut scope = Scope::default();
  for (table, alias) in iter_table_bindings(src) {
    let table_ref =
      TableRef { schema: None, name: table.clone(), alias: Some(alias.clone()), range: TextRange::default() };
    let binding = Binding { alias: alias.clone(), table: table_ref.clone() };
    scope.bindings.insert(alias, binding.clone());
    // Bind by unaliased name too so qualified refs like `users.id` work.
    scope.bindings.entry(table).or_insert(Binding { alias: table_ref.name.clone(), table: table_ref });
  }
  if scope.is_empty() { None } else { Some(scope) }
}

/// Yield (table, alias) pairs from FROM / JOIN / UPDATE / DELETE / INTO
/// clauses in the source. Consumes comma-separated relation lists after
/// FROM / UPDATE so `FROM users u, orders o` exposes both bindings.
fn iter_table_bindings(src: &str) -> Vec<(String, String)> {
  let mut out = Vec::new();
  let chars: Vec<char> = src.chars().collect();
  let mut i = 0;
  while i < chars.len() {
    if let Some(consumed) = match_keyword(&chars, i, &["FROM", "JOIN", "UPDATE", "INTO"]) {
      i += consumed;
      // Read the first table, then any comma-continuations.
      loop {
        if let Some((table, alias, consumed2)) = read_table_with_alias(&chars, i) {
          out.push((table, alias));
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
    if matches_ci(chars, pos, kw) && chars.get(pos + kw.len()).map_or(true, |c| !is_word(*c)) {
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

/// Read `[whitespace] <table> [AS <alias>] | <alias>` after a FROM/JOIN.
/// Returns (table, alias, consumed chars).
fn read_table_with_alias(chars: &[char], pos: usize) -> Option<(String, String, usize)> {
  let mut i = pos;
  let ws = skip_ws(chars, i);
  i += ws;

  let (table, t_len) = read_ident(chars, i)?;
  if table.is_empty() {
    return None;
  }
  i += t_len;

  // Optional AS alias  |  bare alias  |  no alias.
  let mut alias = table.clone();
  let after_table = i;
  i += skip_ws(chars, i);
  if matches_ci(chars, i, "AS") && chars.get(i + 2).map_or(true, |c| !is_word(*c)) {
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

  Some((table, alias, i - pos))
}

fn skip_ws(chars: &[char], pos: usize) -> usize {
  let mut n = 0;
  while pos + n < chars.len() && chars[pos + n].is_whitespace() {
    n += 1;
  }
  n
}

fn read_ident(chars: &[char], pos: usize) -> Option<(String, usize)> {
  let mut n = 0;
  while pos + n < chars.len() && is_word(chars[pos + n]) {
    n += 1;
  }
  if n == 0 {
    return None;
  }
  Some((chars[pos..pos + n].iter().collect(), n))
}
