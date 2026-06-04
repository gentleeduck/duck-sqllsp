//! Pick the right hover renderer for the token / word run under the cursor.

use crate::render;
use crate::token::Window;
use dsl_catalog::Catalog;
use dsl_knowledge as kb;

/// Resolve a single (possibly dotted) token against catalog + knowledge.
pub fn from_token(token: &str, catalog: &Catalog) -> Option<String> {
  if let Some((left, right)) = token.split_once('.') {
    if let Some(t) = catalog.find_table(Some(left), right) {
      return Some(render::table(t));
    }
    if let Some(t) = catalog.find_table(None, left)
      && let Some(c) = t.columns.iter().find(|c| c.name.eq_ignore_ascii_case(right))
    {
      return Some(render::column(t, c));
    }
  }
  if let Some(t) = catalog.find_table(None, token) {
    return Some(render::table(t));
  }
  let columns = catalog.columns_named(token);
  if !columns.is_empty() {
    return Some(render::column_in_tables(&columns));
  }
  if let Some(entry) = kb::lookup(token) {
    return Some(kb::render_markdown(entry));
  }
  None
}

/// Try every contiguous 2- or 3-word window inside `window.words` that
/// includes the cursor word. Longest wins (`IS NOT NULL` beats `IS NULL`
/// beats `IS`). Returns the rendered markdown when a multi-word keyword
/// matches.
pub fn from_window(window: &Window) -> Option<String> {
  if window.words.is_empty() {
    return None;
  }
  let table = kb::keywords();
  let upper: Vec<String> = window.words.iter().map(|w| w.to_uppercase()).collect();
  let n = upper.len();
  let cur = window.cursor.min(n.saturating_sub(1));
  for size in [3usize, 2] {
    if n < size {
      continue;
    }
    let start_lo = cur.saturating_sub(size - 1);
    let start_hi = cur.min(n - size);
    for start in start_lo..=start_hi {
      let candidate = upper[start..start + size].join(" ");
      if let Some(entry) = table.get(candidate.as_str()) {
        return Some(kb::render_markdown(entry));
      }
    }
  }
  None
}

/// Older single-direction helper kept for tests.
pub fn from_words(words: &[String]) -> Option<String> {
  if words.is_empty() {
    return None;
  }
  let table = kb::keywords();
  let upper: Vec<String> = words.iter().map(|w| w.to_uppercase()).collect();
  for n in (1..=3.min(upper.len())).rev() {
    let candidate = upper[..n].join(" ");
    if let Some(entry) = table.get(candidate.as_str()) {
      return Some(kb::render_markdown(entry));
    }
  }
  None
}

pub fn resolve(token: &str, catalog: &Catalog) -> Option<String> {
  from_token(token, catalog)
}
