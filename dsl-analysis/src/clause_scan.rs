//! Shared text-scanning helpers for ORDER BY / GROUP BY / HAVING
//! diagnostics. These rules can't lean on the AST (it doesn't model
//! those clauses), so they walk the (noise-stripped) source byte slice
//! and need a common notion of clause boundaries, top-level comma
//! splitting, and bare-identifier parsing.

/// Find the byte offset of `needle` (uppercase ASCII) as a whole word
/// at paren depth 0. Returns None if absent. The input must already be
/// uppercased -- callers pass `to_ascii_uppercase()` once and reuse.
pub fn find_clause(bytes: &[u8], needle: &[u8]) -> Option<usize> {
  let n = bytes.len();
  let m = needle.len();
  if m == 0 || n < m {
    return None;
  }
  let mut depth: i32 = 0;
  let mut i = 0;
  while i + m <= n {
    let c = bytes[i];
    if c == b'(' {
      depth += 1;
    } else if c == b')' {
      depth -= 1;
    }
    if depth == 0
      && bytes[i..i + m] == *needle
      && (i == 0 || !is_word(bytes[i - 1] as char))
      && (i + m == n || !is_word(bytes[i + m] as char))
    {
      return Some(i);
    }
    i += 1;
  }
  None
}

/// Walk forward from `from` until a top-level (depth 0) `;`, `)`, or
/// one of `stopwords` (matched as whole words). Returns the offset of
/// the boundary (or input length).
pub fn find_clause_end(bytes: &[u8], from: usize, stopwords: &[&str]) -> usize {
  let n = bytes.len();
  let mut depth: i32 = 0;
  let mut i = from;
  while i < n {
    let c = bytes[i];
    if c == b'(' {
      depth += 1;
    } else if c == b')' {
      if depth == 0 {
        return i;
      }
      depth -= 1;
    } else if c == b';' && depth == 0 {
      return i;
    } else if depth == 0 && (i == from || !is_word(bytes[i - 1] as char)) {
      for w in stopwords {
        let wb = w.as_bytes();
        if i + wb.len() <= n
          && bytes[i..i + wb.len()] == *wb
          && (i + wb.len() == n || !is_word(bytes[i + wb.len()] as char))
        {
          return i;
        }
      }
    }
    i += 1;
  }
  n
}

/// Split a clause body on top-level commas (depth-aware across both
/// `()` and `[]`). Yields `(slice, offset_within_body)`.
pub fn split_top_level(s: &str) -> Vec<(&str, usize)> {
  let mut out = Vec::new();
  let bytes = s.as_bytes();
  let mut depth: i32 = 0;
  let mut last = 0usize;
  for (i, &b) in bytes.iter().enumerate() {
    match b {
      b'(' | b'[' => depth += 1,
      b')' | b']' => depth -= 1,
      b',' if depth == 0 => {
        out.push((&s[last..i], last));
        last = i + 1;
      },
      _ => {},
    }
  }
  if last < bytes.len() {
    out.push((&s[last..], last));
  }
  out
}

/// Parse `ident` or `qual.ident` (each part optionally double-quoted),
/// rejecting anything containing operators / parens / function calls.
/// Returns None when the item isn't a bare reference.
pub fn parse_simple_ident(s: &str) -> Option<(Option<String>, String)> {
  let s = s.trim();
  let bytes = s.as_bytes();
  if bytes.is_empty() {
    return None;
  }
  for &b in bytes {
    if !(is_word(b as char) || b == b'.' || b == b'"') {
      return None;
    }
  }
  if let Some(dot) = s.find('.') {
    let q = trim_quotes(&s[..dot]);
    let n = trim_quotes(&s[dot + 1..]);
    if q.is_empty() || n.is_empty() {
      return None;
    }
    return Some((Some(q.to_string()), n.to_string()));
  }
  let n = trim_quotes(s);
  if n.is_empty() {
    return None;
  }
  Some((None, n.to_string()))
}

fn trim_quotes(s: &str) -> &str {
  let s = s.trim();
  if s.len() >= 2 && s.starts_with('"') && s.ends_with('"') {
    &s[1..s.len() - 1]
  } else {
    s
  }
}

pub fn is_word(c: char) -> bool {
  c.is_alphanumeric() || c == '_'
}

/// A single `WITH RECURSIVE name[(cols)] AS ( base UNION [ALL]
/// recursive )` CTE, split into its parts. Shared by the recursive-CTE
/// rule family (sql769-sql773) -- each of those rules needs the same
/// "where does the recursive term start/end" answer, so it lives here
/// instead of being re-derived five times.
pub struct RecursiveCte {
  pub name_start: usize,
  pub name_end: usize,
  pub cols: Vec<String>,
  pub term_start: usize,
  pub term_end: usize,
}

/// Find the (single) recursive CTE in `body`, given its ASCII-
/// uppercased twin `upper`. Returns `None` when there's no `WITH
/// RECURSIVE`, the CTE body has no top-level `UNION`, or the shape
/// doesn't match `name[(cols)] AS ( ... )` -- a `WITH RECURSIVE a AS
/// (...), b AS (...)` multi-CTE list is out of scope (`None`), kept
/// simple to stay conservative.
pub fn find_recursive_cte(body: &str, upper: &str) -> Option<RecursiveCte> {
  let ub = upper.as_bytes();
  let with_rel = find_clause(ub, b"WITH RECURSIVE")?;
  let mut i = skip_ws(ub, with_rel + "WITH RECURSIVE".len());
  let name_start = i;
  while i < ub.len() && is_word(ub[i] as char) {
    i += 1;
  }
  let name_end = i;
  if name_end == name_start {
    return None;
  }
  i = skip_ws(ub, i);
  let mut cols = Vec::new();
  if ub.get(i) == Some(&b'(') {
    let close = match_paren(ub, i)?;
    for (c, _) in split_top_level(&body[i + 1..close]) {
      if let Some((_, n)) = parse_simple_ident(c) {
        cols.push(n);
      }
    }
    i = skip_ws(ub, close + 1);
  }
  if !upper[i..].starts_with("AS") {
    return None;
  }
  i = skip_ws(ub, i + 2);
  if ub.get(i) != Some(&b'(') {
    return None;
  }
  let body_open = i;
  let body_close = match_paren(ub, body_open)?;
  let inner = &upper[body_open + 1..body_close];
  let union_rel = find_clause(inner.as_bytes(), b"UNION")?;
  let mut term_start = body_open + 1 + union_rel + "UNION".len();
  let after_union = &upper[term_start..body_close];
  let trimmed_len = after_union.len() - after_union.trim_start().len();
  if after_union.trim_start().starts_with("ALL") {
    let ws_and_all = trimmed_len + "ALL".len();
    term_start += ws_and_all;
  }
  Some(RecursiveCte { name_start, name_end, cols, term_start, term_end: body_close })
}

/// Strip matching wrapping parens repeatedly, e.g. `((x))` -> `x`.
/// Used to normalize a recursive term that's fully parenthesized
/// (`UNION ALL (SELECT ... ORDER BY ...)`) before depth-0 scans.
pub fn unwrap_parens(ub: &[u8], start: usize, end: usize) -> (usize, usize) {
  let mut s = start;
  let mut e = end;
  while s < e && ub[s].is_ascii_whitespace() {
    s += 1;
  }
  while e > s && ub[e - 1].is_ascii_whitespace() {
    e -= 1;
  }
  if s < e
    && ub[s] == b'('
    && let Some(close) = match_paren(ub, s)
    && close == e - 1
  {
    return unwrap_parens(ub, s + 1, e - 1);
  }
  (s, e)
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
