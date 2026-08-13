//! JSON key completion for `col->'<cursor>'` / `col->>'<cursor>'`
//! chains.
//!
//! Two sources of keys: same-buffer jsonb literal examples (harvested
//! by a cheap non-parsing scanner), and -- when a live catalog is
//! available -- `Column.json_keys` recorded for the resolved column.
//! Extracted from `engine.rs` (was inline there) -- pure code motion,
//! no behavior change.

use text_size::TextSize;

/// When the cursor sits inside the literal of `<expr>->'<cursor>` or
/// `<expr>->>'<cursor>`, return the JSON keys observed in same-buffer
/// jsonb literals (`'{"key":...}'`) so the user can autocomplete
/// instead of guessing. Handles chained paths: `col->'a'->'b'->'<cursor>'`
/// walks into nested objects of the harvested literals and surfaces
/// only the keys present at depth a.b. Returns None outside this
/// context.
pub fn json_path_keys_at(source: &str, offset: TextSize) -> Option<Vec<String>> {
  let pos: usize = u32::from(offset) as usize;
  let bytes = source.as_bytes();
  let n = bytes.len().min(source.len());
  if pos > n {
    return None;
  }
  // Walk back to the opening `'` of the string the cursor is in.
  let mut s = pos;
  while s > 0 && bytes[s - 1] != b'\'' {
    s -= 1;
  }
  if s == 0 || bytes[s - 1] != b'\'' {
    return None;
  }
  // The string must be preceded by `->` or `->>`. Skip whitespace.
  let mut k = s - 1; // points at the `'`
  while k > 0 && bytes[k - 1].is_ascii_whitespace() {
    k -= 1;
  }
  if k < 2 {
    return None;
  }
  // `->>` form: bytes[k-3]='-', bytes[k-2]='>', bytes[k-1]='>'.
  // `->`  form: bytes[k-2]='-', bytes[k-1]='>'.
  let has_double = k >= 3 && bytes[k - 1] == b'>' && bytes[k - 2] == b'>' && bytes[k - 3] == b'-';
  let has_single = bytes[k - 1] == b'>' && bytes[k - 2] == b'-';
  if !has_double && !has_single {
    return None;
  }
  // Walk further back to harvest the chain of preceding `->'KEY'`
  // segments so we know what depth to look up in the JSON blobs.
  let chain_end = if has_double { k - 3 } else { k - 2 };
  let chain = collect_json_path_chain(source, chain_end);

  // Harvest jsonb keys from same-buffer literals -- at the requested depth.
  let mut keys: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
  let mut i = 0;
  while i < n {
    if bytes[i] == b'\'' && i + 1 < n && (bytes[i + 1] == b'{' || bytes[i + 1] == b'[') {
      let lit_start = i + 1;
      let mut j = lit_start;
      while j < n && bytes[j] != b'\'' {
        j += 1;
      }
      if j < n {
        let blob = &source[lit_start..j];
        let nested = navigate_json(blob, &chain).unwrap_or(blob);
        harvest_json_keys(nested, &mut keys);
        i = j + 1;
        continue;
      }
    }
    i += 1;
  }
  if keys.is_empty() {
    return None;
  }
  Some(keys.into_iter().collect())
}

/// Like [`json_path_keys_at`] but also consults `Column.json_keys` from
/// the catalog. When the cursor sits on a `col->'...'` chain whose head
/// resolves to a known jsonb column with stored top-level keys, surface
/// those even when the buffer has no example literal to harvest.
pub fn json_path_keys_at_with_catalog(
  source: &str,
  offset: TextSize,
  catalog: &dsl_catalog::Catalog,
) -> Option<Vec<String>> {
  if let Some(keys) = json_path_keys_at(source, offset) {
    return Some(keys);
  }
  // Walk back to the column name at the head of the `->'k'` chain.
  let pos: usize = u32::from(offset) as usize;
  let bytes = source.as_bytes();
  if pos > bytes.len() {
    return None;
  }
  let mut s = pos;
  while s > 0 && bytes[s - 1] != b'\'' {
    s -= 1
  }
  if s == 0 {
    return None;
  }
  let mut k = s - 1;
  while k > 0 && bytes[k - 1].is_ascii_whitespace() {
    k -= 1
  }
  let has_double = k >= 3 && bytes[k - 1] == b'>' && bytes[k - 2] == b'>' && bytes[k - 3] == b'-';
  let has_single = bytes[k - 1] == b'>' && bytes[k - 2] == b'-';
  if !has_double && !has_single {
    return None;
  }
  let arrow_at = if has_double { k - 3 } else { k - 2 };
  // Identifier just before the first arrow.
  let mut j = arrow_at;
  while j > 0 && bytes[j - 1].is_ascii_whitespace() {
    j -= 1
  }
  let id_end = j;
  while j > 0
    && (bytes[j - 1].is_ascii_alphanumeric() || bytes[j - 1] == b'_' || bytes[j - 1] == b'.' || bytes[j - 1] == b'"')
  {
    j -= 1;
  }
  if j == id_end {
    return None;
  }
  let col_full = &source[j..id_end];
  let col_bare = col_full.rsplit('.').next().unwrap_or(col_full).trim_matches('"');
  for t in catalog.tables() {
    if let Some(c) = t.columns.iter().find(|c| c.name.eq_ignore_ascii_case(col_bare))
      && let Some(keys) = &c.json_keys
      && !keys.is_empty()
    {
      return Some(keys.clone());
    }
  }
  None
}

/// Walk backwards from `end` collecting `->'KEY'` (or `->>'KEY'`)
/// segments in left-to-right order. Stops at the first non-segment
/// token. Whitespace between segments is tolerated.
fn collect_json_path_chain(source: &str, end: usize) -> Vec<String> {
  let bytes = source.as_bytes();
  let mut chain: Vec<String> = Vec::new();
  let mut k = end;
  loop {
    // Skip trailing whitespace.
    while k > 0 && bytes[k - 1].is_ascii_whitespace() {
      k -= 1;
    }
    // Need a closing `'`.
    if k == 0 || bytes[k - 1] != b'\'' {
      break;
    }
    let close = k - 1;
    // Walk back to the opening `'`.
    let mut open = close;
    while open > 0 && bytes[open - 1] != b'\'' {
      open -= 1;
    }
    if open == 0 {
      break;
    }
    let key = &source[open..close];
    let preceded_by_arrow = {
      let mut p = open.saturating_sub(1); // points at the opening `'`
      while p > 0 && bytes[p - 1].is_ascii_whitespace() {
        p -= 1;
      }
      let double = p >= 3 && bytes[p - 1] == b'>' && bytes[p - 2] == b'>' && bytes[p - 3] == b'-';
      let single = p >= 2 && bytes[p - 1] == b'>' && bytes[p - 2] == b'-';
      if double || single { Some(if double { p - 3 } else { p - 2 }) } else { None }
    };
    if let Some(next_end) = preceded_by_arrow {
      chain.push(key.to_string());
      k = next_end;
    } else {
      break;
    }
  }
  chain.reverse();
  chain
}

/// Given a JSON object literal (no surrounding quotes), navigate down
/// the `keys` path and return a sub-blob that the caller can re-scan
/// for keys. Returns None when the path doesn't resolve.
fn navigate_json<'a>(blob: &'a str, keys: &[String]) -> Option<&'a str> {
  if keys.is_empty() {
    return Some(blob);
  }
  let mut current = blob;
  for key in keys {
    current = find_value_for_key(current, key)?;
  }
  Some(current)
}

/// Locate `"key":` in `blob` and return the slice starting at the
/// value's first byte and ending at the matching close (`}` for
/// objects, `]` for arrays, or the next top-level `,`).
fn find_value_for_key<'a>(blob: &'a str, key: &str) -> Option<&'a str> {
  let needle = format!("\"{key}\"");
  let bytes = blob.as_bytes();
  let n = bytes.len();
  let mut i = 0;
  while i + needle.len() <= n {
    if blob[i..i + needle.len()] == needle {
      let mut j = i + needle.len();
      while j < n && bytes[j].is_ascii_whitespace() {
        j += 1;
      }
      if j >= n || bytes[j] != b':' {
        i += 1;
        continue;
      }
      j += 1;
      while j < n && bytes[j].is_ascii_whitespace() {
        j += 1;
      }
      if j >= n {
        return None;
      }
      let value_start = j;
      let value_end = scan_value_end(bytes, value_start);
      return Some(&blob[value_start..value_end]);
    }
    i += 1;
  }
  None
}

fn scan_value_end(bytes: &[u8], start: usize) -> usize {
  let n = bytes.len();
  if start >= n {
    return n;
  }
  match bytes[start] {
    b'{' | b'[' => {
      let open = bytes[start];
      let close = if open == b'{' { b'}' } else { b']' };
      let mut depth = 1i32;
      let mut i = start + 1;
      while i < n && depth > 0 {
        match bytes[i] {
          b'"' => {
            i += 1;
            while i < n && bytes[i] != b'"' {
              if bytes[i] == b'\\' && i + 1 < n {
                i += 2;
              } else {
                i += 1;
              }
            }
          },
          c if c == open => depth += 1,
          c if c == close => depth -= 1,
          _ => {},
        }
        i += 1;
      }
      i.min(n)
    },
    b'"' => {
      let mut i = start + 1;
      while i < n && bytes[i] != b'"' {
        if bytes[i] == b'\\' && i + 1 < n {
          i += 2;
        } else {
          i += 1;
        }
      }
      (i + 1).min(n)
    },
    _ => {
      let mut i = start;
      while i < n && bytes[i] != b',' && bytes[i] != b'}' && bytes[i] != b']' {
        i += 1;
      }
      i
    },
  }
}

fn harvest_json_keys(blob: &str, out: &mut std::collections::BTreeSet<String>) {
  // Cheap key-scanner: find each `"<key>":` pair without a real JSON
  // parser. Good enough for completion -- if the blob is malformed,
  // we miss a key or two, no harm done.
  let b = blob.as_bytes();
  let n = b.len();
  let mut i = 0;
  while i < n {
    if b[i] == b'"' {
      let key_start = i + 1;
      let mut j = key_start;
      while j < n && b[j] != b'"' {
        j += 1;
      }
      if j >= n {
        return;
      }
      let key = &blob[key_start..j];
      // Look forward for `:`.
      let mut k = j + 1;
      while k < n && b[k].is_ascii_whitespace() {
        k += 1;
      }
      if k < n && b[k] == b':' && !key.is_empty() && key.chars().all(|c| c.is_alphanumeric() || c == '_' || c == '-') {
        out.insert(key.to_string());
      }
      i = j + 1;
      continue;
    }
    i += 1;
  }
}
