//! Locate the identifier or word run at the cursor.
//!
//! Two helpers:
//!   - [`token_at`]  -- single (possibly dotted) identifier; used for
//!                     table / column resolution.
//!   - [`window_at`] -- up to 5 adjacent words spanning the cursor (2
//!                     before, the cursor's word, 2 after), plus the
//!                     index of the cursor word. Lets the resolver try
//!                     every multi-word sliding window that includes the
//!                     cursor, so hovering on the second word of
//!                     `INNER JOIN` or the middle word of `IS NOT NULL`
//!                     still resolves to the multi-word entry.

use text_size::TextSize;

pub fn token_at(src: &str, offset: TextSize) -> Option<String> {
  let pos: usize = offset.into();
  if pos > src.len() {
    return None;
  }
  let bytes = src.as_bytes();
  let mut start = pos;
  while start > 0 {
    let c = bytes[start - 1] as char;
    if is_token(c) {
      start -= 1;
    } else {
      break;
    }
  }
  let mut end = pos;
  while end < bytes.len() {
    let c = bytes[end] as char;
    if is_token(c) {
      end += 1;
    } else {
      break;
    }
  }
  if start == end {
    return None;
  }
  Some(src[start..end].to_string())
}

fn is_token(c: char) -> bool {
  c.is_alphanumeric() || c == '_' || c == '.'
}

pub struct Window {
  pub words: Vec<String>,
  /// Index inside `words` of the word the cursor sits in. Equal to
  /// `words.len()` if the cursor is on trailing whitespace.
  pub cursor: usize,
}

pub fn window_at(src: &str, offset: TextSize) -> Window {
  let chars: Vec<char> = src.chars().collect();
  let pos: usize = offset.into();
  let pos = pos.min(chars.len());

  // Cursor word: walk left to its start, right to its end.
  let mut cur_start = pos;
  while cur_start > 0 && is_word(chars[cur_start - 1]) {
    cur_start -= 1;
  }
  let mut cur_end = pos;
  while cur_end < chars.len() && is_word(chars[cur_end]) {
    cur_end += 1;
  }

  // Up to 2 preceding words.
  let mut prefix: Vec<(usize, usize)> = Vec::new();
  let mut i = cur_start;
  for _ in 0..2 {
    let prev_end = back_skip_ws(&chars, i);
    if prev_end == 0 {
      break;
    }
    let prev_start = back_word_start(&chars, prev_end);
    if prev_start == prev_end {
      break;
    }
    prefix.push((prev_start, prev_end));
    i = prev_start;
  }
  prefix.reverse();

  // Up to 2 following words.
  let mut suffix: Vec<(usize, usize)> = Vec::new();
  let mut j = cur_end;
  for _ in 0..2 {
    let next_start = fwd_skip_ws(&chars, j);
    if next_start >= chars.len() {
      break;
    }
    let next_end = fwd_word_end(&chars, next_start);
    if next_end == next_start {
      break;
    }
    suffix.push((next_start, next_end));
    j = next_end;
  }

  let mut words: Vec<String> = prefix.iter().map(|(s, e)| chars[*s..*e].iter().collect()).collect();
  let cursor_idx = words.len();
  if cur_start < cur_end {
    words.push(chars[cur_start..cur_end].iter().collect());
  }
  for (s, e) in &suffix {
    words.push(chars[*s..*e].iter().collect());
  }
  Window { words, cursor: cursor_idx }
}

fn back_skip_ws(chars: &[char], mut i: usize) -> usize {
  while i > 0 && chars[i - 1].is_whitespace() {
    i -= 1;
  }
  i
}
fn back_word_start(chars: &[char], mut end: usize) -> usize {
  while end > 0 && is_word(chars[end - 1]) {
    end -= 1;
  }
  end
}
fn fwd_skip_ws(chars: &[char], mut i: usize) -> usize {
  while i < chars.len() && chars[i].is_whitespace() {
    i += 1;
  }
  i
}
fn fwd_word_end(chars: &[char], mut start: usize) -> usize {
  while start < chars.len() && is_word(chars[start]) {
    start += 1;
  }
  start
}
fn is_word(c: char) -> bool {
  c.is_alphanumeric() || c == '_'
}

/// Back-compat shim.
pub fn words_at(src: &str, offset: TextSize) -> Vec<String> {
  window_at(src, offset).words
}
