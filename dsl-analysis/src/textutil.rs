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
        let closer: Vec<u8> = std::iter::once(b'$')
          .chain(tag_bytes.iter().copied())
          .chain(std::iter::once(b'$'))
          .collect();
        let closer_len = closer.len();
        for j in i..k + 1 {
          out[j] = b' ';
        }
        i = k + 1;
        while i + closer_len <= n {
          if out[i..i + closer_len] == *closer {
            break;
          }
          out[i] = b' ';
          i += 1;
        }
        if i + closer_len <= n {
          for j in i..i + closer_len {
            out[j] = b' ';
          }
          i += closer_len;
        }
        continue;
      }
    }
    i += 1;
  }
  String::from_utf8(out).unwrap_or_else(|_| s.to_string())
}
