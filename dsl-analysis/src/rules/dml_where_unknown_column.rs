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
    // Build an effective column set: catalog cols + ALTER ADD COLUMN
    // + ALTER RENAME COLUMN new-side. Without this, migrations that
    // ADD or RENAME a column and then UPDATE/DELETE it falsely
    // trigger "unknown column".
    let mut valid_cols: std::collections::HashSet<String> = t.columns
      .iter()
      .map(|c| c.name.to_ascii_lowercase())
      .collect();
    for col in alter_added_or_renamed(source, &table_ref.name) {
      valid_cols.insert(col);
    }
    let start: usize = u32::from(stmt.range.start()) as usize;
    let end: usize = (u32::from(stmt.range.end()) as usize).min(source.len());
    let body = &source[start..end];
    // Strip line comments so `-- WHERE col` doesn't pollute the
    // predicate walk (was matching `col` inside the comment text).
    let cleaned = strip_line_comments(body);
    let upper = cleaned.to_ascii_uppercase();
    // CTE / subquery in the body -- columns from `WITH t AS (...)`
    // and aliases from `FROM (SELECT ...) sub` aren't in this
    // statement's resolver scope; bail rather than false-flag.
    // Match SELECT word-boundary so `FROM (SELECT ...)` triggers too
    // (the earlier ` SELECT ` substring missed it -- no space before).
    if upper.contains("WITH ") || contains_word(&upper, "SELECT") {
      return;
    }
    let body = cleaned.as_str();
    let Some(where_at) = upper.find(" WHERE ") else { return };
    let after = where_at + 7;
    let rest = &body[after..];
    // Stop at the first of `;`, RETURNING, ORDER BY, GROUP BY, LIMIT,
    // OFFSET, FOR (UPDATE/SHARE), HAVING. Earlier code used `or_else`
    // which preferred `;` even if a RETURNING came first -- the alias
    // in RETURNING then got scanned as a WHERE column.
    let upper_rest = &upper[after..];
    let stop = [
      rest.find(';'),
      find_word_pos(upper_rest, "RETURNING"),
      find_word_pos(upper_rest, "ORDER"),
      find_word_pos(upper_rest, "GROUP"),
      find_word_pos(upper_rest, "HAVING"),
      find_word_pos(upper_rest, "LIMIT"),
      find_word_pos(upper_rest, "OFFSET"),
      find_word_pos(upper_rest, "FETCH"),
    ]
    .into_iter()
    .flatten()
    .min()
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
      // Cast operator `::type`: skip the type identifier so it's not
      // treated as a bare column ref. Was firing on `WHERE c = 'x'::myenum`.
      if i + 1 < bytes.len() && bytes[i] == b':' && bytes[i + 1] == b':' {
        i += 2;
        while i < bytes.len() && bytes[i].is_ascii_whitespace() { i += 1 }
        while i < bytes.len() && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_' || bytes[i] == b'.') { i += 1 }
        // Optional `(N)` size spec.
        if i < bytes.len() && bytes[i] == b'(' {
          let mut depth = 1i32;
          i += 1;
          while i < bytes.len() && depth > 0 {
            match bytes[i] { b'(' => depth += 1, b')' => depth -= 1, _ => {} }
            i += 1;
          }
        }
        // Optional `[]` array suffix.
        while i + 1 < bytes.len() && bytes[i] == b'[' && bytes[i + 1] == b']' { i += 2 }
        continue;
      }
      if !(bytes[i].is_ascii_alphabetic() || bytes[i] == b'_') { i += 1; continue }
      let s = i;
      while i < bytes.len() && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_') { i += 1 }
      let token = &predicate[s..i];
      // Qualified ref like `b.a_id` -- skip over the whole `b.a_id`.
      // Without this, the inner walk left `i` at the `.`, then resumed
      // at `a_id` and treated it as an unqualified column on the
      // UPDATE/DELETE target, false-firing whenever the WHERE
      // references a column of the USING/FROM table.
      if i < bytes.len() && bytes[i] == b'.' {
        i += 1;
        while i < bytes.len() && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_') { i += 1 }
        continue;
      }
      // Function call shape `foo(...)` -- skip.
      if i < bytes.len() && bytes[i] == b'(' { continue }
      let upper_tok = token.to_ascii_uppercase();
      if is_keyword(&upper_tok) { continue }
      if valid_cols.contains(&token.to_ascii_lowercase()) { continue }
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

/// First position where `needle` appears as a whole word in `haystack`,
/// at depth 0 with respect to parens (so `(SELECT ... ORDER BY x)` in
/// a subquery doesn't stop the predicate scan early). Case-sensitive
/// match against an upper-case haystack.
fn find_word_pos(haystack_upper: &str, needle_upper: &str) -> Option<usize> {
  let h = haystack_upper.as_bytes();
  let n = needle_upper.len();
  let len = h.len();
  if n == 0 { return None; }
  let mut depth = 0i32;
  let mut i = 0usize;
  while i + n <= len {
    match h[i] {
      b'(' => { depth += 1; i += 1; continue; }
      b')' => { depth -= 1; i += 1; continue; }
      b'\'' => {
        i += 1;
        while i < len && h[i] != b'\'' { i += 1 }
        if i < len { i += 1 }
        continue;
      }
      _ => {}
    }
    if depth == 0 && haystack_upper[i..i + n].eq_ignore_ascii_case(needle_upper) {
      let prev_ok = i == 0 || !(h[i - 1].is_ascii_alphanumeric() || h[i - 1] == b'_');
      let next_ok = i + n == len || !(h[i + n].is_ascii_alphanumeric() || h[i + n] == b'_');
      if prev_ok && next_ok { return Some(i); }
    }
    i += 1;
  }
  None
}

/// Collect all column names added or renamed-to by ALTER TABLE
/// statements in source for `table`. Lenient text scan.
fn alter_added_or_renamed(source: &str, table: &str) -> Vec<String> {
  let cleaned = strip_noise_full(source);
  let source = cleaned.as_str();
  let upper = source.to_ascii_uppercase();
  let bytes = source.as_bytes();
  let n = bytes.len();
  let needle = "ALTER TABLE";
  let table_lc = table.to_ascii_lowercase();
  let mut out = Vec::new();
  let mut from = 0usize;
  while let Some(rel) = upper[from..].find(needle) {
    let at = from + rel;
    let mut k = at + needle.len();
    while k < n && bytes[k].is_ascii_whitespace() { k += 1 }
    for kw in ["ONLY ", "IF EXISTS "] {
      if upper[k..].starts_with(kw) {
        k += kw.len();
        while k < n && bytes[k].is_ascii_whitespace() { k += 1 }
      }
    }
    let id_start = k;
    while k < n && (bytes[k].is_ascii_alphanumeric() || bytes[k] == b'_' || bytes[k] == b'.' || bytes[k] == b'"') { k += 1 }
    let id = source[id_start..k].trim_matches('"').to_ascii_lowercase();
    let bare = id.rsplit('.').next().unwrap_or(&id).to_string();
    from = k;
    if bare != table_lc { continue }
    let stmt_end = source[k..].find(';').map(|i| k + i).unwrap_or(n);
    let stmt_body_upper = &upper[k..stmt_end];
    let stmt_body = &source[k..stmt_end];
    let pb = stmt_body.as_bytes();
    // ADD COLUMN <name>
    let mut local = 0usize;
    while let Some(p_rel) = stmt_body_upper[local..].find("ADD COLUMN") {
      let p = local + p_rel + "ADD COLUMN".len();
      let mut q = p;
      while q < pb.len() && pb[q].is_ascii_whitespace() { q += 1 }
      if stmt_body_upper[q..].starts_with("IF NOT EXISTS") {
        q += "IF NOT EXISTS".len();
        while q < pb.len() && pb[q].is_ascii_whitespace() { q += 1 }
      }
      let name_start = q;
      while q < pb.len() && (pb[q].is_ascii_alphanumeric() || pb[q] == b'_' || pb[q] == b'"') { q += 1 }
      if q > name_start {
        out.push(stmt_body[name_start..q].trim_matches('"').to_ascii_lowercase());
      }
      local = q;
    }
    // RENAME COLUMN <old> TO <new> -- new name only.
    let mut local = 0usize;
    while let Some(p_rel) = stmt_body_upper[local..].find("RENAME COLUMN") {
      let p = local + p_rel + "RENAME COLUMN".len();
      let mut q = p;
      while q < pb.len() && pb[q].is_ascii_whitespace() { q += 1 }
      while q < pb.len() && (pb[q].is_ascii_alphanumeric() || pb[q] == b'_' || pb[q] == b'"') { q += 1 }
      while q < pb.len() && pb[q].is_ascii_whitespace() { q += 1 }
      if stmt_body_upper[q..].starts_with("TO") {
        q += 2;
        while q < pb.len() && pb[q].is_ascii_whitespace() { q += 1 }
        let name_start = q;
        while q < pb.len() && (pb[q].is_ascii_alphanumeric() || pb[q] == b'_' || pb[q] == b'"') { q += 1 }
        if q > name_start {
          out.push(stmt_body[name_start..q].trim_matches('"').to_ascii_lowercase());
        }
      }
      local = q;
    }
  }
  out
}

fn strip_noise_full(s: &str) -> String {
  let mut out: Vec<u8> = s.as_bytes().to_vec();
  let n = out.len();
  let mut i = 0usize;
  while i < n {
    if i + 1 < n && out[i] == b'-' && out[i + 1] == b'-' {
      while i < n && out[i] != b'\n' { out[i] = b' '; i += 1 }
      continue;
    }
    if i + 1 < n && out[i] == b'/' && out[i + 1] == b'*' {
      let mut depth = 1u32;
      out[i] = b' '; out[i + 1] = b' '; i += 2;
      while i + 1 < n && depth > 0 {
        if out[i] == b'/' && out[i + 1] == b'*' { depth += 1; out[i] = b' '; out[i + 1] = b' '; i += 2; }
        else if out[i] == b'*' && out[i + 1] == b'/' { depth -= 1; out[i] = b' '; out[i + 1] = b' '; i += 2; }
        else { out[i] = b' '; i += 1; }
      }
      continue;
    }
    if out[i] == b'\'' {
      out[i] = b' '; i += 1;
      while i < n && out[i] != b'\'' { out[i] = b' '; i += 1 }
      if i < n { out[i] = b' '; i += 1 }
      continue;
    }
    i += 1;
  }
  String::from_utf8(out).unwrap_or_else(|_| s.to_string())
}

fn contains_word(haystack: &str, needle: &str) -> bool {
  let h = haystack.as_bytes();
  let n = needle.as_bytes();
  if n.is_empty() { return false; }
  let mut i = 0;
  while i + n.len() <= h.len() {
    if h[i..i + n.len()] == *n {
      let prev_ok = i == 0 || !(h[i - 1].is_ascii_alphanumeric() || h[i - 1] == b'_');
      let next_ok = i + n.len() == h.len() || !(h[i + n.len()].is_ascii_alphanumeric() || h[i + n.len()] == b'_');
      if prev_ok && next_ok { return true; }
    }
    i += 1;
  }
  false
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
