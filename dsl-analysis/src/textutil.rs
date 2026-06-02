//! Shared text helpers reused by many rules.
//!
//! Centralising the comment / string / dollar-block stripping function
//! here means we don't paste a 30-line `strip_noise` block into every
//! rule that needs it -- a recurring source of bugs where one rule's
//! local copy lags behind another's.

/// Replace `-- ... \n` line comments, `/* ... */` (nested) block
/// comments, `'...'` string literals, and `$tag$ ... $tag$` PG dollar-
/// quoted blocks with equal-length space runs. Byte offsets are
/// preserved 1:1 so callers can keep using positions from the cleaned
/// string against the original source.
pub fn strip_noise_full(s: &str) -> String {
  let mut out: Vec<u8> = s.as_bytes().to_vec();
  let n = out.len();
  let mut i = 0usize;
  while i < n {
    if i + 1 < n && out[i] == b'-' && out[i + 1] == b'-' {
      while i < n && out[i] != b'\n' {
        out[i] = b' ';
        i += 1;
      }
      continue;
    }
    if i + 1 < n && out[i] == b'/' && out[i + 1] == b'*' {
      let mut depth = 1u32;
      out[i] = b' ';
      out[i + 1] = b' ';
      i += 2;
      while i + 1 < n && depth > 0 {
        if out[i] == b'/' && out[i + 1] == b'*' {
          depth += 1;
          out[i] = b' ';
          out[i + 1] = b' ';
          i += 2;
        } else if out[i] == b'*' && out[i + 1] == b'/' {
          depth -= 1;
          out[i] = b' ';
          out[i + 1] = b' ';
          i += 2;
        } else {
          out[i] = b' ';
          i += 1;
        }
      }
      continue;
    }
    if out[i] == b'\'' {
      out[i] = b' ';
      i += 1;
      while i < n && out[i] != b'\'' {
        out[i] = b' ';
        i += 1;
      }
      if i < n {
        out[i] = b' ';
        i += 1;
      }
      continue;
    }
    if out[i] == b'$' {
      let mut k = i + 1;
      while k < n && (out[k].is_ascii_alphanumeric() || out[k] == b'_') {
        k += 1;
      }
      if k < n && out[k] == b'$' {
        let tag_bytes = &out[i + 1..k];
        let closer: Vec<u8> =
          std::iter::once(b'$').chain(tag_bytes.iter().copied()).chain(std::iter::once(b'$')).collect();
        let closer_len = closer.len();
        out[i..k + 1].fill(b' ');
        i = k + 1;
        while i + closer_len <= n {
          if out[i..i + closer_len] == *closer {
            break;
          }
          out[i] = b' ';
          i += 1;
        }
        if i + closer_len <= n {
          out[i..i + closer_len].fill(b' ');
          i += closer_len;
        }
        continue;
      }
    }
    i += 1;
  }
  String::from_utf8(out).unwrap_or_else(|_| s.to_string())
}

/// Like [`strip_noise_full`] but does NOT strip `$tag$...$tag$` dollar-
/// quoted blocks. Use this when the rule needs to inspect text inside
/// PL/pgSQL function bodies (e.g. checking for `RETURN` in a trigger
/// function), but still wants comment / string literal stripping for
/// the regular SQL.
pub fn strip_comments_strings(s: &str) -> String {
  let mut out: Vec<u8> = s.as_bytes().to_vec();
  let n = out.len();
  let mut i = 0usize;
  while i < n {
    if i + 1 < n && out[i] == b'-' && out[i + 1] == b'-' {
      while i < n && out[i] != b'\n' {
        out[i] = b' ';
        i += 1;
      }
      continue;
    }
    if i + 1 < n && out[i] == b'/' && out[i + 1] == b'*' {
      let mut depth = 1u32;
      out[i] = b' ';
      out[i + 1] = b' ';
      i += 2;
      while i + 1 < n && depth > 0 {
        if out[i] == b'/' && out[i + 1] == b'*' {
          depth += 1;
          out[i] = b' ';
          out[i + 1] = b' ';
          i += 2;
        } else if out[i] == b'*' && out[i + 1] == b'/' {
          depth -= 1;
          out[i] = b' ';
          out[i + 1] = b' ';
          i += 2;
        } else {
          out[i] = b' ';
          i += 1;
        }
      }
      continue;
    }
    if out[i] == b'\'' {
      out[i] = b' ';
      i += 1;
      while i < n && out[i] != b'\'' {
        out[i] = b' ';
        i += 1;
      }
      if i < n {
        out[i] = b' ';
        i += 1;
      }
      continue;
    }
    i += 1;
  }
  String::from_utf8(out).unwrap_or_else(|_| s.to_string())
}

/// Strip only line and block comments, leaving strings and dollar blocks
/// intact. Useful for rules that need to read string literal contents
/// (e.g. `COMMENT ON ... IS ''` empty-comment check).
pub fn strip_comments_only(s: &str) -> String {
  let mut out: Vec<u8> = s.as_bytes().to_vec();
  let n = out.len();
  let mut i = 0usize;
  while i < n {
    if i + 1 < n && out[i] == b'-' && out[i + 1] == b'-' {
      while i < n && out[i] != b'\n' {
        out[i] = b' ';
        i += 1;
      }
      continue;
    }
    if i + 1 < n && out[i] == b'/' && out[i + 1] == b'*' {
      let mut depth = 1u32;
      out[i] = b' ';
      out[i + 1] = b' ';
      i += 2;
      while i + 1 < n && depth > 0 {
        if out[i] == b'/' && out[i + 1] == b'*' {
          depth += 1;
          out[i] = b' ';
          out[i + 1] = b' ';
          i += 2;
        } else if out[i] == b'*' && out[i + 1] == b'/' {
          depth -= 1;
          out[i] = b' ';
          out[i + 1] = b' ';
          i += 2;
        } else {
          out[i] = b' ';
          i += 1;
        }
      }
      continue;
    }
    i += 1;
  }
  String::from_utf8(out).unwrap_or_else(|_| s.to_string())
}

/// True when `c` can be part of a SQL identifier (alphanumeric +
/// underscore). Recurs in dozens of rules; this helper dedupes them.
#[inline]
pub fn is_word(c: char) -> bool {
  c.is_alphanumeric() || c == '_'
}

/// True when `needle` appears in `haystack` as a whole word (alphanumeric/
/// underscore boundary). Case-sensitive -- callers that need case-insensitive
/// matching should pre-uppercase both inputs. Replaces dozens of local
/// `contains_word` / `has_word_kw` copies.
pub fn contains_word(haystack: &str, needle: &str) -> bool {
  let h = haystack.as_bytes();
  let n = needle.as_bytes();
  if n.is_empty() {
    return false;
  }
  let mut i = 0;
  while i + n.len() <= h.len() {
    if &h[i..i + n.len()] == n {
      let prev_ok = i == 0 || !is_word(h[i - 1] as char);
      let next_ok = i + n.len() == h.len() || !is_word(h[i + n.len()] as char);
      if prev_ok && next_ok {
        return true;
      }
    }
    i += 1;
  }
  false
}

/// First byte offset where `needle` occurs in `haystack` as a whole
/// word (alphanumeric/underscore boundary). Case-sensitive. Returns
/// None when not found or `needle` is empty. Sibling of
/// [`contains_word`] -- this one returns the position.
pub fn find_word(haystack: &str, needle: &str) -> Option<usize> {
  let h = haystack.as_bytes();
  let n = needle.as_bytes();
  if n.is_empty() {
    return None;
  }
  let mut i = 0usize;
  while i + n.len() <= h.len() {
    if &h[i..i + n.len()] == n {
      let prev_ok = i == 0 || !is_word(h[i - 1] as char);
      let next_ok = i + n.len() == h.len() || !is_word(h[i + n.len()] as char);
      if prev_ok && next_ok {
        return Some(i);
      }
    }
    i += 1;
  }
  None
}
