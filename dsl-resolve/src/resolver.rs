//! Walk a parsed statement and produce its [`Scope`].
//!
//! Resolution is one pass: every FROM / JOIN reference adds a binding
//! under both its alias (if present) and its bare table name. Subsequent
//! lookups by either form resolve to the same row.

use crate::binding::Binding;
use crate::scope::Scope;
use dsl_parse::{Statement, StatementKind, TableRef};

/// Resolve every statement in `stmts`. Returns one [`Scope`] per statement,
/// in matching order, so callers can index by statement position.
///
/// CTE column projections are populated as empty `Vec`s -- use
/// [`resolve_with_source`] if you have the raw source text and want the
/// resolver to extract projection column names from each CTE body.
pub fn resolve(stmts: &[Statement]) -> Vec<Scope> {
  stmts.iter().map(|s| resolve_one(s, None)).collect()
}

/// Resolve as above, but use `source` to populate `Scope.cte_columns`
/// with the projection column names declared by each CTE body's outer
/// SELECT. Best-effort -- subqueries / function calls / `*` show up as
/// empty Vecs.
pub fn resolve_with_source(stmts: &[Statement], source: &str) -> Vec<Scope> {
  stmts.iter().map(|s| resolve_one(s, Some(source))).collect()
}

fn resolve_one(stmt: &Statement, source: Option<&str>) -> Scope {
  let mut scope = Scope::default();
  match &stmt.kind {
    StatementKind::Select(s) => {
      // Bind CTE names first so they're visible to FROM lookups
      // when the same CTE appears later in the same SELECT.
      for name in &s.cte_names {
        add_synthetic(&mut scope, name);
        let cols = match source {
          Some(src) => extract_cte_columns(src, stmt, name),
          None => Vec::new(),
        };
        scope.cte_columns.entry(name.clone()).or_insert(cols);
      }
      for t in &s.from {
        add(&mut scope, t);
      }
      for j in &s.joins {
        add(&mut scope, &j.table);
      }
    },
    StatementKind::Update(u) => add(&mut scope, &u.table),
    StatementKind::Delete(d) => add(&mut scope, &d.table),
    StatementKind::Insert(i) => add(&mut scope, &i.table),
    _ => {},
  }
  scope
}

/// Find `WITH [RECURSIVE] <name> [(col, ...)] AS [MATERIALIZED] (...)`
/// inside the statement source and return the projection column names.
///
/// Two cases:
///   - explicit column list after the CTE name -- use those names
///   - else parse the inner SELECT projection and extract aliases
fn extract_cte_columns(source: &str, stmt: &Statement, name: &str) -> Vec<String> {
  let start: usize = u32::from(stmt.range.start()) as usize;
  let end: usize = (u32::from(stmt.range.end()) as usize).min(source.len());
  let body = &source[start..end];
  let upper = body.to_ascii_uppercase();
  let bytes = body.as_bytes();
  let n = bytes.len();
  let name_up = name.to_ascii_uppercase();
  // Skip `WITH` (and optional `RECURSIVE`).
  let mut i = match upper.find("WITH") {
    Some(p) => p + 4,
    None => return Vec::new(),
  };
  loop {
    while i < n && bytes[i].is_ascii_whitespace() {
      i += 1;
    }
    if i + 9 <= n && upper[i..i + 9] == *"RECURSIVE" {
      i += 9;
      while i < n && bytes[i].is_ascii_whitespace() {
        i += 1;
      }
    }
    // Read CTE name.
    let name_start = i;
    while i < n && is_word(bytes[i] as char) {
      i += 1;
    }
    if name_start == i {
      return Vec::new();
    }
    let cur_name = &upper[name_start..i];
    let is_target = cur_name == name_up;
    while i < n && bytes[i].is_ascii_whitespace() {
      i += 1;
    }
    // Optional `(col, ...)`.
    let mut explicit_cols: Vec<String> = Vec::new();
    if i < n && bytes[i] == b'(' {
      let list_start = i + 1;
      let mut depth = 1i32;
      i += 1;
      while i < n && depth > 0 {
        match bytes[i] {
          b'(' => depth += 1,
          b')' => depth -= 1,
          _ => {},
        }
        if depth == 0 {
          break;
        }
        i += 1;
      }
      if is_target {
        let raw = &body[list_start..i];
        explicit_cols =
          raw.split(',').map(|s| s.trim().trim_matches('"').to_string()).filter(|s| !s.is_empty()).collect();
      }
      i += 1; // past `)`
      while i < n && bytes[i].is_ascii_whitespace() {
        i += 1;
      }
    }
    // `AS`.
    if i + 2 > n || !body[i..i + 2].eq_ignore_ascii_case("AS") {
      return explicit_cols;
    }
    i += 2;
    while i < n && bytes[i].is_ascii_whitespace() {
      i += 1;
    }
    for kw in ["NOT MATERIALIZED", "MATERIALIZED"] {
      if i + kw.len() <= n && body[i..i + kw.len()].eq_ignore_ascii_case(kw) {
        i += kw.len();
        while i < n && bytes[i].is_ascii_whitespace() {
          i += 1;
        }
        break;
      }
    }
    if i >= n || bytes[i] != b'(' {
      return explicit_cols;
    }
    let body_open = i + 1;
    let mut depth = 1i32;
    let mut j = body_open;
    while j < n && depth > 0 {
      match bytes[j] {
        b'(' => depth += 1,
        b')' => depth -= 1,
        b'\'' => {
          j += 1;
          while j < n && bytes[j] != b'\'' {
            j += 1;
          }
        },
        _ => {},
      }
      if depth == 0 {
        break;
      }
      j += 1;
    }
    if j >= n {
      return explicit_cols;
    }
    if is_target {
      if !explicit_cols.is_empty() {
        return explicit_cols;
      }
      let cte_body = &body[body_open..j];
      return projection_columns(cte_body);
    }
    // Not our CTE -- skip past `,` and continue with the next CTE.
    i = j + 1;
    while i < n && bytes[i].is_ascii_whitespace() {
      i += 1;
    }
    if i >= n || bytes[i] != b',' {
      return Vec::new();
    }
    i += 1;
  }
}

/// Extract projection column names from a CTE body. Looks for the
/// outermost `SELECT ...` and parses each comma-separated projection.
///
/// The scan tracks paren depth and skips string literals, double-quoted
/// identifiers, dollar-quoted strings (`$tag$...$tag$`), line comments
/// (`-- ...`), and block comments (`/* ... */`) so that keywords or
/// commas inside any of those don't affect projection boundaries.
fn projection_columns(cte_body: &str) -> Vec<String> {
  let bytes = cte_body.as_bytes();
  let n = bytes.len();
  // Find outer SELECT, skipping comments and strings.
  let Some(sel) = find_keyword_outside(bytes, 0, b"SELECT") else { return Vec::new() };
  let proj_start = sel + 6;
  // Find top-level FROM at depth 0, outside strings/comments.
  let from_at = find_keyword_outside(bytes, proj_start, b"FROM").unwrap_or(n);
  let proj_text = &cte_body[proj_start..from_at];
  // Split on top-level commas, also skipping strings/comments.
  let pbytes = proj_text.as_bytes();
  let pn = pbytes.len();
  let mut out: Vec<String> = Vec::new();
  let mut depth = 0i32;
  let mut start = 0usize;
  let mut i = 0usize;
  while i < pn {
    if let Some(skip) = skip_string_or_comment(pbytes, i) {
      i = skip;
      continue;
    }
    match pbytes[i] {
      b'(' => depth += 1,
      b')' => depth -= 1,
      b',' if depth == 0 => {
        out.push(alias_of(&proj_text[start..i]));
        start = i + 1;
      },
      _ => {},
    }
    i += 1;
  }
  if start < pn {
    out.push(alias_of(&proj_text[start..]));
  }
  out.into_iter().filter(|s| !s.is_empty()).collect()
}

/// Return new cursor past a string literal, identifier quote, dollar-quote,
/// or comment starting at `i`. Returns `None` if `i` is not such a span.
fn skip_string_or_comment(bytes: &[u8], i: usize) -> Option<usize> {
  let n = bytes.len();
  if i >= n {
    return None;
  }
  match bytes[i] {
    // Single-quoted string. Doubled '' is an escaped quote.
    b'\'' => {
      let mut j = i + 1;
      while j < n {
        if bytes[j] == b'\'' {
          if j + 1 < n && bytes[j + 1] == b'\'' {
            j += 2;
            continue;
          }
          return Some(j + 1);
        }
        j += 1;
      }
      Some(n)
    },
    // Double-quoted identifier. Doubled "" is an escaped quote.
    b'"' => {
      let mut j = i + 1;
      while j < n {
        if bytes[j] == b'"' {
          if j + 1 < n && bytes[j + 1] == b'"' {
            j += 2;
            continue;
          }
          return Some(j + 1);
        }
        j += 1;
      }
      Some(n)
    },
    // Line comment `-- ...` to end of line.
    b'-' if i + 1 < n && bytes[i + 1] == b'-' => {
      let mut j = i + 2;
      while j < n && bytes[j] != b'\n' {
        j += 1;
      }
      Some(j)
    },
    // Block comment `/* ... */` (non-nested -- good enough for projection scan).
    b'/' if i + 1 < n && bytes[i + 1] == b'*' => {
      let mut j = i + 2;
      while j + 1 < n && !(bytes[j] == b'*' && bytes[j + 1] == b'/') {
        j += 1;
      }
      Some((j + 2).min(n))
    },
    // Dollar-quoted string `$tag$ ... $tag$` (tag may be empty).
    b'$' => {
      // Read optional tag of [A-Za-z0-9_]* until next `$`.
      let mut j = i + 1;
      while j < n && (bytes[j].is_ascii_alphanumeric() || bytes[j] == b'_') {
        j += 1;
      }
      if j >= n || bytes[j] != b'$' {
        return None;
      }
      let tag = &bytes[i..j + 1]; // includes both `$`s
      let body_start = j + 1;
      let mut k = body_start;
      while k + tag.len() <= n {
        if &bytes[k..k + tag.len()] == tag {
          return Some(k + tag.len());
        }
        k += 1;
      }
      Some(n)
    },
    _ => None,
  }
}

/// Find `kw` (case-insensitive) as a word-bounded match at depth 0,
/// outside strings/comments. `kw` must be uppercase ASCII.
/// For `FROM`, requires whitespace on the left side; right side allows
/// whitespace, `(`, or end-of-input.
fn find_keyword_outside(bytes: &[u8], from: usize, kw: &[u8]) -> Option<usize> {
  let n = bytes.len();
  let klen = kw.len();
  let mut depth = 0i32;
  let mut i = from;
  while i + klen <= n {
    if let Some(skip) = skip_string_or_comment(bytes, i) {
      i = skip;
      continue;
    }
    match bytes[i] {
      b'(' => {
        depth += 1;
        i += 1;
        continue;
      },
      b')' => {
        depth -= 1;
        i += 1;
        continue;
      },
      _ => {},
    }
    if depth == 0 {
      // Word-boundary check on the left.
      let left_ok = i == from || !is_ident_byte(bytes[i - 1]);
      if left_ok {
        let mut matched = true;
        for (off, kb) in kw.iter().enumerate() {
          if bytes[i + off].to_ascii_uppercase() != *kb {
            matched = false;
            break;
          }
        }
        if matched {
          let right_ok = i + klen == n || !is_ident_byte(bytes[i + klen]);
          if right_ok {
            return Some(i);
          }
        }
      }
    }
    i += 1;
  }
  None
}

fn is_ident_byte(b: u8) -> bool {
  b.is_ascii_alphanumeric() || b == b'_'
}

/// Return the alias of a projection expression: `expr AS alias` ->
/// `alias`; `t.col` -> `col`; `col` -> `col`; `*` -> empty.
fn alias_of(proj: &str) -> String {
  let trimmed = proj.trim();
  if trimmed == "*" {
    return String::new();
  }
  let upper = trimmed.to_ascii_uppercase();
  if let Some(at) = upper.rfind(" AS ") {
    return trimmed[at + 4..].trim().trim_matches('"').to_string();
  }
  // Trailing identifier-only alias (no `AS`).
  let bytes = trimmed.as_bytes();
  let n = bytes.len();
  // Walk back over an identifier.
  let mut end = n;
  while end > 0 && (bytes[end - 1].is_ascii_alphanumeric() || bytes[end - 1] == b'_') {
    end -= 1;
  }
  let tail = &trimmed[end..];
  // If preceded by whitespace and there's a non-id chunk before, treat
  // tail as alias.
  if !tail.is_empty()
    && end > 0
    && bytes[end - 1].is_ascii_whitespace()
    && tail.chars().next().is_some_and(|c| c.is_alphabetic() || c == '_')
  {
    return tail.trim().trim_matches('"').to_string();
  }
  // Otherwise treat the whole projection as a column reference.
  let tail = trimmed.rsplit('.').next().unwrap_or(trimmed).trim().trim_matches('"');
  tail.to_string()
}

fn is_word(c: char) -> bool {
  c.is_alphanumeric() || c == '_'
}

/// Bind a synthetic table reference (CTE / subquery alias). Columns of
/// the underlying body aren't resolved yet -- a future pass can promote
/// these from name-only to fully-typed bindings.
fn add_synthetic(scope: &mut Scope, name: &str) {
  if name.is_empty() {
    return;
  }
  let table = dsl_parse::TableRef { name: name.to_string(), ..Default::default() };
  scope.bindings.entry(name.to_string()).or_insert(Binding { alias: name.to_string(), table });
}

fn add(scope: &mut Scope, table: &TableRef) {
  if table.name.is_empty() {
    return;
  }
  let entry = Binding { alias: table.alias.clone().unwrap_or_else(|| table.name.clone()), table: table.clone() };
  if let Some(alias) = &table.alias {
    scope.bindings.insert(alias.clone(), entry.clone());
  }
  // Always also bind by the unaliased name so users can reference the
  // table without an alias inside the same query.
  scope.bindings.entry(table.name.clone()).or_insert(entry);
}
