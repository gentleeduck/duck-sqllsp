//! `textDocument/semanticTokens/full` handler.
//!
//! We classify each lexed identifier as keyword / type / function /
//! catalog-table / catalog-column / parameter, plus literals and
//! comments. Output is the LSP relative-delta-encoded `SemanticTokens`.
//!
//! Resolution order per identifier:
//!   1. SQL keywords  (case-insensitive, from dsl-knowledge).
//!   2. Type names    (also from dsl-knowledge).
//!   3. Catalog table names (from current snapshot).
//!   4. Catalog column names (anywhere in any table).
//!   5. Function-call (next non-space byte is `(`) -> FUNCTION.
//!   6. NEW / OLD / DECLARE locals -> VARIABLE.

use crate::state::ServerState;
use ropey::Rope;
use tower_lsp::lsp_types::{Position, SemanticToken, SemanticTokens, SemanticTokensParams, SemanticTokensResult};

/// Order matches `LEGEND` in `capabilities.rs`.
#[repr(u32)]
enum Tok {
  Keyword = 0,
  Type = 1,
  Function = 2,
  Class = 3,    // tables
  Property = 4, // columns
  Variable = 5, // NEW/OLD/locals
  #[allow(dead_code)]
  Parameter = 6,
  String = 7,
  Number = 8,
  Comment = 9,
  Operator = 10,
}

pub fn run(state: &ServerState, params: SemanticTokensParams) -> Option<SemanticTokensResult> {
  let _g = crate::handlers::perf::Guard::with_uri("semantic_tokens", &params.text_document.uri);
  let doc = state.documents.get(&params.text_document.uri)?;
  let cat = state.catalog.read().clone();
  let kw = dsl_knowledge::keywords();
  let ty = dsl_knowledge::types();

  let bytes = doc.text.as_bytes();
  let n = bytes.len();
  let mut raw: Vec<(usize, usize, Tok)> = Vec::new();
  let mut i = 0usize;

  while i < n {
    let c = bytes[i] as char;

    // -- line comment
    if c == '-' && i + 1 < n && bytes[i + 1] == b'-' {
      let start = i;
      while i < n && bytes[i] != b'\n' {
        i += 1;
      }
      raw.push((start, i, Tok::Comment));
      continue;
    }
    // /* block */ comment
    if c == '/' && i + 1 < n && bytes[i + 1] == b'*' {
      let start = i;
      i += 2;
      while i + 1 < n && !(bytes[i] == b'*' && bytes[i + 1] == b'/') {
        i += 1;
      }
      i = (i + 2).min(n);
      raw.push((start, i, Tok::Comment));
      continue;
    }
    // single-quoted string
    if c == '\'' {
      let start = i;
      i += 1;
      while i < n {
        if bytes[i] == b'\'' {
          if i + 1 < n && bytes[i + 1] == b'\'' {
            i += 2;
            continue;
          }
          i += 1;
          break;
        }
        i += 1;
      }
      raw.push((start, i, Tok::String));
      continue;
    }
    // dollar-quoted: highlight the body as a string
    if c == '$'
      && let Some((after, tag)) = dollar_open(bytes, i)
    {
      let start = i;
      let mut j = after;
      while j + tag.len() <= n {
        if &bytes[j..j + tag.len()] == tag.as_bytes() {
          j += tag.len();
          break;
        }
        j += 1;
      }
      i = j.min(n);
      raw.push((start, i, Tok::String));
      continue;
    }
    // numbers (simple)
    if c.is_ascii_digit() {
      let start = i;
      while i < n && (bytes[i].is_ascii_digit() || bytes[i] == b'.') {
        i += 1;
      }
      raw.push((start, i, Tok::Number));
      continue;
    }
    // identifiers
    if c.is_alphabetic() || c == '_' {
      let start = i;
      while i < n && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_') {
        i += 1;
      }
      let word = &doc.text[start..i];
      let upper = word.to_ascii_uppercase();
      let kind = if kw.contains_key(upper.as_str()) {
        Tok::Keyword
      } else if ty.contains_key(upper.as_str()) {
        Tok::Type
      } else if matches!(upper.as_str(), "NEW" | "OLD") {
        Tok::Variable
      } else if is_function_call(bytes, i) {
        Tok::Function
      } else if cat.tables().any(|t| t.name.eq_ignore_ascii_case(word)) {
        Tok::Class
      } else if cat.tables().any(|t| t.columns.iter().any(|col| col.name.eq_ignore_ascii_case(word))) {
        Tok::Property
      } else {
        continue;
      };
      raw.push((start, i, kind));
      continue;
    }
    // operators -- `:` added so the PG cast operator `::` gets a
    // single Operator token (otherwise the two colons fell through
    // and the cast stayed unstyled).
    if matches!(c, '=' | '<' | '>' | '+' | '-' | '*' | '/' | '!' | '%' | '|' | '&' | ':') {
      let start = i;
      i += 1;
      while i < n && matches!(bytes[i] as char, '=' | '<' | '>' | '+' | '-' | '*' | '/' | '!' | '%' | '|' | '&' | ':') {
        i += 1;
      }
      raw.push((start, i, Tok::Operator));
      continue;
    }
    // Array subscript brackets / range-subscript colon: tag as
    // Operator so `arr[0]` and `arr[1:5]` get a consistent colour.
    if c == '[' || c == ']' {
      raw.push((i, i + 1, Tok::Operator));
      i += 1;
      continue;
    }

    i += 1;
  }

  // Post-pass: any identifier that immediately follows an operator
  // token whose text contains `::` is a cast target. Promote it from
  // whatever it landed on (often unmatched and dropped) to Type so
  // user-defined enum / domain casts colour correctly even when not
  // in the built-in type table.
  promote_cast_targets(&doc.text, &mut raw);

  Some(SemanticTokensResult::Tokens(SemanticTokens { result_id: None, data: encode(&doc.rope, raw) }))
}

/// Walk `raw` looking for `<Operator containing "::">` followed by an
/// identifier-shaped region of `text`; if no identifier token currently
/// covers that region (because it was dropped), insert one as Type.
/// When an identifier token does cover it (e.g. a known type like
/// `text`), re-tag the existing token to Type.
fn promote_cast_targets(text: &str, raw: &mut Vec<(usize, usize, Tok)>) {
  let bytes = text.as_bytes();
  let n = bytes.len();
  // Indices of operator tokens whose text contains `::`.
  let cast_ops: Vec<usize> = raw
    .iter()
    .enumerate()
    .filter_map(|(idx, (s, e, k))| {
      if matches!(k, Tok::Operator) && text.get(*s..*e).is_some_and(|t| t.contains("::")) { Some(idx) } else { None }
    })
    .collect();
  for op_idx in cast_ops {
    let (_, op_end, _) = raw[op_idx];
    // Skip whitespace after the `::`.
    let mut k = op_end;
    while k < n && bytes[k].is_ascii_whitespace() {
      k += 1;
    }
    if k >= n {
      continue;
    }
    if !(bytes[k].is_ascii_alphabetic() || bytes[k] == b'_') {
      continue;
    }
    let ident_start = k;
    while k < n && (bytes[k].is_ascii_alphanumeric() || bytes[k] == b'_') {
      k += 1;
    }
    let ident_end = k;
    if ident_end == ident_start {
      continue;
    }
    // Existing token covering this region?
    if let Some(existing) = raw.iter_mut().find(|(s, e, _)| *s == ident_start && *e == ident_end) {
      existing.2 = Tok::Type;
    } else {
      raw.push((ident_start, ident_end, Tok::Type));
    }
  }
}

fn is_function_call(bytes: &[u8], end: usize) -> bool {
  let mut j = end;
  while j < bytes.len() && (bytes[j] == b' ' || bytes[j] == b'\t') {
    j += 1;
  }
  j < bytes.len() && bytes[j] == b'('
}

fn dollar_open(bytes: &[u8], i: usize) -> Option<(usize, String)> {
  let n = bytes.len();
  if bytes[i] != b'$' {
    return None;
  }
  let mut j = i + 1;
  while j < n && (bytes[j].is_ascii_alphanumeric() || bytes[j] == b'_') {
    j += 1;
  }
  if j >= n || bytes[j] != b'$' {
    return None;
  }
  Some((j + 1, std::str::from_utf8(&bytes[i..=j]).ok()?.to_string()))
}

/// Encode (start, end, kind) byte ranges as LSP delta-encoded tokens.
fn encode(rope: &Rope, mut raw: Vec<(usize, usize, Tok)>) -> Vec<SemanticToken> {
  raw.sort_by_key(|(s, _, _)| *s);
  let mut prev_line = 0u32;
  let mut prev_char = 0u32;
  let mut out = Vec::with_capacity(raw.len());
  for (start, end, kind) in raw {
    let pos = byte_to_position(rope, start);
    let len = (end - start) as u32;
    let delta_line = pos.line - prev_line;
    let delta_char = if delta_line == 0 { pos.character - prev_char } else { pos.character };
    out.push(SemanticToken {
      delta_line,
      delta_start: delta_char,
      length: len,
      token_type: kind as u32,
      token_modifiers_bitset: 0,
    });
    prev_line = pos.line;
    prev_char = pos.character;
  }
  out
}

fn byte_to_position(rope: &Rope, byte: usize) -> Position {
  let byte = byte.min(rope.len_bytes());
  let line = rope.byte_to_line(byte);
  let line_start_byte = rope.line_to_byte(line);
  let line_slice = rope.line(line);
  let mut utf16 = 0u32;
  let mut bytes_seen = 0usize;
  let bytes_in_line = byte.saturating_sub(line_start_byte);
  for c in line_slice.chars() {
    if bytes_seen >= bytes_in_line {
      break;
    }
    utf16 += c.len_utf16() as u32;
    bytes_seen += c.len_utf8();
  }
  Position { line: line as u32, character: utf16 }
}
