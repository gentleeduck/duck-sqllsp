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
