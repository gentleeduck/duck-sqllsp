//! `textDocument/semanticTokens/{full,range}` handler.
//!
//! We classify each lexed identifier as keyword / type / function /
//! catalog-table / catalog-column / parameter, plus literals and
//! comments. Output is the LSP relative-delta-encoded `SemanticTokens`.
//!
//! Resolution order per identifier:
//!   1. SQL keywords  (case-insensitive, from dsl-knowledge).
//!   2. A name being *introduced* -- see "Declarations" below.
//!   3. Type names    (also from dsl-knowledge).
//!   4. Catalog table names (from current snapshot).
//!   5. Catalog column names (anywhere in any table).
//!   6. Function-call (next non-space byte is `(`) -> FUNCTION.
//!   7. NEW / OLD / DECLARE locals -> VARIABLE.
//!
//! # Declarations
//!
//! A purely catalog-driven classifier can only colour things it has
//! already seen, which leaves the most important identifiers in a
//! migration file -- the ones the file is *creating* -- completely
//! unstyled. So a small amount of positional state rides along with the
//! lexer:
//!
//!   * `CREATE [OR REPLACE] <kind> <name>` marks `<name>` with
//!     `declaration | definition`, and picks the token type from the
//!     kind (`TABLE` -> class, `FUNCTION` -> function, `TYPE` -> type).
//!   * Inside the parenthesised body that follows, an identifier in the
//!     head position of a list item (directly after `(` or `,`, or after
//!     `IN` / `OUT` / `INOUT` / `VARIADIC`) is the column or parameter
//!     being declared -> `property` / `parameter` + `declaration`.
//!     Depth-tracked, so `PRIMARY KEY (a, b)` at depth 2 is left alone
//!     and `CONSTRAINT c CHECK (...)` is skipped because `c` follows a
//!     keyword rather than a delimiter.
//!   * `DECLARE x int; y text; BEGIN` marks each PL/pgSQL local ->
//!     `variable` + `declaration`.
//!
//! Body tracking is a heuristic, not a parse: `awaiting_body` is dropped
//! at `;` and at keywords that mean the body will never come
//! (`SELECT` for `CREATE TABLE ... AS SELECT`, `RETURNS`, `LANGUAGE`,
//! ...). Worst case a stray identifier picks up a `declaration` bit,
//! which is a colour difference, never a wrong edit.
//!
//! # Range requests
//!
//! `semanticTokens/range` cannot start lexing at the range: whether a
//! given byte sits inside a string, a dollar-quoted body, or a block
//! comment is only knowable from the top of the file. So we always scan
//! from byte 0, but stop as soon as we pass the requested end and then
//! keep the tokens that overlap. The saving is real on a large file
//! where the viewport is near the top, and the answer is identical to
//! the full request's -- which a test pins down.

use crate::state::ServerState;
use ropey::Rope;
use tower_lsp::lsp_types::{
  Position, Range, SemanticToken, SemanticTokens, SemanticTokensParams, SemanticTokensRangeParams,
  SemanticTokensRangeResult, SemanticTokensResult,
};

/// Order matches `SEMANTIC_LEGEND` in `capabilities.rs`.
#[repr(u32)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Tok {
  Keyword = 0,
  Type = 1,
  Function = 2,
  Class = 3,    // tables
  Property = 4, // columns
  Variable = 5, // NEW/OLD/locals
  Parameter = 6,
  String = 7,
  Number = 8,
  Comment = 9,
  Operator = 10,
}

/// Bit positions match `SEMANTIC_MODIFIERS` in `capabilities.rs`.
mod modbit {
  /// The name is being introduced here.
  pub const DECLARATION: u32 = 1 << 0;
  /// ...and this occurrence also carries the body/implementation.
  pub const DEFINITION: u32 = 1 << 1;
  /// Ships with the server/engine rather than the user's schema.
  pub const DEFAULT_LIBRARY: u32 = 1 << 2;
}

#[derive(Clone, Copy, Debug)]
struct Raw {
  start: usize,
  end: usize,
  kind: Tok,
  mods: u32,
}

/// What the last significant (non-whitespace, non-comment) token was.
/// Only the distinctions that matter for list-head detection.
#[derive(Clone, Copy, PartialEq, Eq)]
enum PrevSig {
  Open,
  Comma,
  /// `IN` / `OUT` / `INOUT` / `VARIADIC` -- a parameter mode, so the
  /// next identifier is still the parameter name.
  ParamMode,
  Other,
}

/// What a parenthesised body declares.
#[derive(Clone, Copy, PartialEq, Eq)]
enum BodyKind {
  Column,
  Param,
}

pub fn run(state: &ServerState, params: SemanticTokensParams) -> Option<SemanticTokensResult> {
  let _g = crate::handlers::perf::Guard::with_uri("semantic_tokens", &params.text_document.uri);
  let doc = state.documents.get(&params.text_document.uri)?;
  // Oversized buffer: bail rather than block the editor. See
  // `documents::MAX_DOC_BYTES`.
  if doc.too_large() {
    return None;
  }
  let cat = state.catalog.read().clone();
  let raw = collect(&doc.text, &cat, None);
  Some(SemanticTokensResult::Tokens(SemanticTokens { result_id: None, data: encode(&doc.rope, raw) }))
}

pub fn run_range(state: &ServerState, params: SemanticTokensRangeParams) -> Option<SemanticTokensRangeResult> {
  let _g = crate::handlers::perf::Guard::with_uri("semantic_tokens_range", &params.text_document.uri);
  let doc = state.documents.get(&params.text_document.uri)?;
  // Oversized buffer: bail rather than block the editor. See
  // `documents::MAX_DOC_BYTES`.
  if doc.too_large() {
    return None;
  }
  let cat = state.catalog.read().clone();
  let (lo, hi) = byte_span(&doc.rope, params.range);
  // Lex from the top (context is not resumable mid-file) but stop once
  // past the viewport, then keep only overlapping tokens.
  let raw: Vec<Raw> = collect(&doc.text, &cat, Some(hi)).into_iter().filter(|t| t.start < hi && t.end > lo).collect();
  Some(SemanticTokensRangeResult::Tokens(SemanticTokens { result_id: None, data: encode(&doc.rope, raw) }))
}

fn byte_span(rope: &Rope, range: Range) -> (usize, usize) {
  let a = usize::from(crate::handlers::position::to_offset(rope, range.start));
  let b = usize::from(crate::handlers::position::to_offset(rope, range.end));
  if a <= b { (a, b) } else { (b, a) }
}

/// Lex `text` into classified byte ranges. `limit` stops the scan once
/// the cursor passes that byte (range requests); `None` scans it all.
fn collect(text: &str, cat: &dsl_catalog::Catalog, limit: Option<usize>) -> Vec<Raw> {
  let kw = dsl_knowledge::keywords();
  let ty = dsl_knowledge::types();
  let builtin_fns = dsl_knowledge::functions();

  let bytes = text.as_bytes();
  let n = bytes.len();
  let mut raw: Vec<Raw> = Vec::new();
  let mut i = 0usize;

  // Declaration-tracking state -- see the module docs.
  let mut decl_target: Option<Tok> = None;
  let mut declare_block = false;
  let mut awaiting_body: Option<BodyKind> = None;
  let mut body: Option<(u32, BodyKind)> = None;
  let mut paren_depth: u32 = 0;
  let mut prev_sig = PrevSig::Other;

  while i < n {
    if let Some(stop) = limit
      && i >= stop
    {
      break;
    }
    let c = bytes[i] as char;

    // -- line comment
    if c == '-' && i + 1 < n && bytes[i + 1] == b'-' {
      let start = i;
      while i < n && bytes[i] != b'\n' {
        i += 1;
      }
      raw.push(Raw { start, end: i, kind: Tok::Comment, mods: 0 });
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
      raw.push(Raw { start, end: i, kind: Tok::Comment, mods: 0 });
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
      raw.push(Raw { start, end: i, kind: Tok::String, mods: 0 });
      prev_sig = PrevSig::Other;
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
      raw.push(Raw { start, end: i, kind: Tok::String, mods: 0 });
      prev_sig = PrevSig::Other;
      continue;
    }
    // numbers (simple)
    if c.is_ascii_digit() {
      let start = i;
      while i < n && (bytes[i].is_ascii_digit() || bytes[i] == b'.') {
        i += 1;
      }
      raw.push(Raw { start, end: i, kind: Tok::Number, mods: 0 });
      prev_sig = PrevSig::Other;
      continue;
    }
    // identifiers
    if c.is_alphabetic() || c == '_' {
      let start = i;
      while i < n && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_') {
        i += 1;
      }
      let word = &text[start..i];
      let upper = word.to_ascii_uppercase();

      if kw.contains_key(upper.as_str()) {
        apply_keyword(&upper, &mut decl_target, &mut declare_block, &mut awaiting_body);
        raw.push(Raw { start, end: i, kind: Tok::Keyword, mods: 0 });
        prev_sig = if matches!(upper.as_str(), "IN" | "OUT" | "INOUT" | "VARIADIC") {
          PrevSig::ParamMode
        } else {
          PrevSig::Other
        };
        continue;
      }

      // A name being introduced by CREATE / DECLARE.
      if let Some(target) = decl_target.take() {
        let mods = if declare_block { modbit::DECLARATION } else { modbit::DECLARATION | modbit::DEFINITION };
        raw.push(Raw { start, end: i, kind: target, mods });
        prev_sig = PrevSig::Other;
        continue;
      }

      // Head of a column / parameter list item inside a CREATE body.
      if let Some((depth, body_kind)) = body
        && paren_depth == depth
        && matches!(prev_sig, PrevSig::Open | PrevSig::Comma | PrevSig::ParamMode)
      {
        let kind = match body_kind {
          BodyKind::Column => Tok::Property,
          BodyKind::Param => Tok::Parameter,
        };
        raw.push(Raw { start, end: i, kind, mods: modbit::DECLARATION });
        prev_sig = PrevSig::Other;
        continue;
      }

      prev_sig = PrevSig::Other;
      let (kind, mods) = if ty.contains_key(upper.as_str()) {
        (Tok::Type, modbit::DEFAULT_LIBRARY)
      } else if matches!(upper.as_str(), "NEW" | "OLD") {
        (Tok::Variable, 0)
      } else if is_function_call(bytes, i) {
        let m = if builtin_fns.contains_key(word.to_ascii_lowercase().as_str()) { modbit::DEFAULT_LIBRARY } else { 0 };
        (Tok::Function, m)
      } else if cat.tables().any(|t| t.name.eq_ignore_ascii_case(word)) {
        (Tok::Class, 0)
      } else if cat.tables().any(|t| t.columns.iter().any(|col| col.name.eq_ignore_ascii_case(word))) {
        (Tok::Property, 0)
      } else {
        continue;
      };
      raw.push(Raw { start, end: i, kind, mods });
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
      raw.push(Raw { start, end: i, kind: Tok::Operator, mods: 0 });
      prev_sig = PrevSig::Other;
      continue;
    }
    // Array subscript brackets / range-subscript colon: tag as
    // Operator so `arr[0]` and `arr[1:5]` get a consistent colour.
    if c == '[' || c == ']' {
      raw.push(Raw { start: i, end: i + 1, kind: Tok::Operator, mods: 0 });
      i += 1;
      prev_sig = PrevSig::Other;
      continue;
    }

    // Structural bytes. Untokenised, but they drive declaration state.
    match c {
      '(' => {
        paren_depth += 1;
        if let Some(k) = awaiting_body.take() {
          body = Some((paren_depth, k));
        }
        prev_sig = PrevSig::Open;
      },
      ')' => {
        if let Some((depth, _)) = body
          && paren_depth == depth
        {
          body = None;
        }
        paren_depth = paren_depth.saturating_sub(1);
        prev_sig = PrevSig::Other;
      },
      ',' => prev_sig = PrevSig::Comma,
      ';' => {
        awaiting_body = None;
        body = None;
        paren_depth = 0;
        // Each `;` inside a DECLARE section starts another local.
        decl_target = if declare_block { Some(Tok::Variable) } else { None };
        prev_sig = PrevSig::Other;
      },
      _ => {},
    }
    i += 1;
  }

  // Post-pass: any identifier that immediately follows an operator
  // token whose text contains `::` is a cast target. Promote it from
  // whatever it landed on (often unmatched and dropped) to Type so
  // user-defined enum / domain casts colour correctly even when not
  // in the built-in type table.
  promote_cast_targets(text, &mut raw);
  raw
}

/// Fold one keyword into the declaration-tracking state machine.
fn apply_keyword(
  upper: &str,
  decl_target: &mut Option<Tok>,
  declare_block: &mut bool,
  awaiting_body: &mut Option<BodyKind>,
) {
  match upper {
    "CREATE" => {
      // Default to class; a following kind keyword refines it.
      *decl_target = Some(Tok::Class);
      *awaiting_body = None;
    },
    "DECLARE" => {
      *declare_block = true;
      *decl_target = Some(Tok::Variable);
    },
    "BEGIN" => {
      *declare_block = false;
      *decl_target = None;
    },
    // Kind keywords only refine an armed CREATE -- `DROP TABLE t` and
    // `ALTER TABLE t` must not mark `t` as a declaration.
    "TABLE" | "VIEW" if decl_target.is_some() => {
      *decl_target = Some(Tok::Class);
      *awaiting_body = Some(BodyKind::Column);
    },
    "FUNCTION" | "PROCEDURE" if decl_target.is_some() => {
      *decl_target = Some(Tok::Function);
      *awaiting_body = Some(BodyKind::Param);
    },
    "TYPE" | "DOMAIN" if decl_target.is_some() => {
      *decl_target = Some(Tok::Type);
      // `CREATE TYPE x AS (a int, ...)` declares columns; `AS ENUM
      // (...)` has none, and its string literals are unaffected.
      *awaiting_body = Some(BodyKind::Column);
    },
    // Everything else that can follow CREATE (INDEX, TRIGGER, POLICY,
    // SCHEMA, ROLE, EXTENSION, SEQUENCE, ...) keeps the default class
    // token but declares nothing inside its parens -- `CREATE INDEX i
    // ON t (col)` lists *existing* columns.
    "INDEX" | "TRIGGER" | "POLICY" | "SCHEMA" | "ROLE" | "USER" | "EXTENSION" | "DATABASE" | "SEQUENCE"
    | "AGGREGATE" | "OPERATOR" | "RULE" | "SERVER" | "PUBLICATION" | "SUBSCRIPTION"
      if decl_target.is_some() =>
    {
      *awaiting_body = None;
    },
    // `AS` always ends the name position (`CREATE TABLE x AS ...`,
    // `CREATE TYPE x AS (...)`). It must disarm the pending name --
    // otherwise a keyword-shaped object name like `snapshot` leaves
    // `decl_target` armed and the next bare identifier steals the
    // declaration. It must *not* disarm the body, or the composite
    // form `CREATE TYPE x AS (a int, b text)` loses its columns.
    "AS" => {
      *decl_target = None;
    },
    // Keywords that mean the parenthesised body is never coming.
    "SELECT" | "VALUES" | "EXECUTE" | "RETURNS" | "LANGUAGE" | "INHERITS" | "PARTITION" => {
      *decl_target = None;
      *awaiting_body = None;
    },
    _ => {},
  }
}

/// Walk `raw` looking for `<Operator containing "::">` followed by an
/// identifier-shaped region of `text`; if no identifier token currently
/// covers that region (because it was dropped), insert one as Type.
/// When an identifier token does cover it (e.g. a known type like
/// `text`), re-tag the existing token to Type.
fn promote_cast_targets(text: &str, raw: &mut Vec<Raw>) {
  let bytes = text.as_bytes();
  let n = bytes.len();
  // Indices of operator tokens whose text contains `::`.
  let cast_ops: Vec<usize> = raw
    .iter()
    .enumerate()
    .filter_map(|(idx, t)| {
      if t.kind == Tok::Operator && text.get(t.start..t.end).is_some_and(|s| s.contains("::")) {
        Some(idx)
      } else {
        None
      }
    })
    .collect();
  for op_idx in cast_ops {
    let op_end = raw[op_idx].end;
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
    if let Some(existing) = raw.iter_mut().find(|t| t.start == ident_start && t.end == ident_end) {
      existing.kind = Tok::Type;
    } else {
      raw.push(Raw { start: ident_start, end: ident_end, kind: Tok::Type, mods: 0 });
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

/// Encode classified byte ranges as LSP delta-encoded tokens.
fn encode(rope: &Rope, mut raw: Vec<Raw>) -> Vec<SemanticToken> {
  raw.sort_by_key(|t| t.start);
  let mut prev_line = 0u32;
  let mut prev_char = 0u32;
  let mut out = Vec::with_capacity(raw.len());
  for t in raw {
    let pos = byte_to_position(rope, t.start);
    let len = (t.end - t.start) as u32;
    let delta_line = pos.line - prev_line;
    let delta_char = if delta_line == 0 { pos.character - prev_char } else { pos.character };
    out.push(SemanticToken {
      delta_line,
      delta_start: delta_char,
      length: len,
      token_type: t.kind as u32,
      token_modifiers_bitset: t.mods,
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

#[cfg(test)]
mod tests {
  use super::*;

  fn scan(src: &str) -> Vec<Raw> {
    collect(src, &dsl_catalog::Catalog::default(), None)
  }

  fn find<'a>(raw: &'a [Raw], src: &str, word: &str) -> &'a Raw {
    let at = src.find(word).unwrap_or_else(|| panic!("{word:?} not in source"));
    raw.iter().find(|t| t.start == at).unwrap_or_else(|| panic!("no token at {word:?}"))
  }

  #[test]
  fn create_table_name_is_a_declaration_and_definition() {
    let src = "CREATE TABLE users (id int);";
    let raw = scan(src);
    let t = find(&raw, src, "users");
    assert_eq!(t.kind, Tok::Class);
    assert_eq!(t.mods, modbit::DECLARATION | modbit::DEFINITION);
  }

  #[test]
  fn drop_and_alter_targets_are_not_declarations() {
    for src in ["DROP TABLE users;", "ALTER TABLE users ADD COLUMN a int;"] {
      let raw = scan(src);
      let at = src.find("users").unwrap();
      if let Some(t) = raw.iter().find(|t| t.start == at) {
        assert_eq!(t.mods & modbit::DECLARATION, 0, "{src:?} must not declare `users`");
      }
    }
  }

  #[test]
  fn create_table_columns_are_declared_properties() {
    let src = "CREATE TABLE t (id int, email text);";
    let raw = scan(src);
    for col in ["id", "email"] {
      let t = find(&raw, src, col);
      assert_eq!(t.kind, Tok::Property, "{col}");
      assert_eq!(t.mods, modbit::DECLARATION, "{col}");
    }
  }

  #[test]
  fn table_constraint_name_is_not_a_column() {
    // `c` follows the CONSTRAINT keyword, not a `(` or `,`.
    let src = "CREATE TABLE t (id int, CONSTRAINT c CHECK (id > 0));";
    let raw = scan(src);
    let at = src.find(" c ").unwrap() + 1;
    let tok = raw.iter().find(|t| t.start == at);
    assert!(tok.is_none() || tok.unwrap().mods & modbit::DECLARATION == 0, "constraint name must not be a column");
  }

  #[test]
  fn nested_key_list_columns_are_not_redeclared() {
    let src = "CREATE TABLE t (a int, b int, PRIMARY KEY (a, b));";
    let raw = scan(src);
    // The `a` inside PRIMARY KEY (...) sits at depth 2.
    let second_a = src.rfind("(a, b)").unwrap() + 1;
    let tok = raw.iter().find(|t| t.start == second_a);
    assert!(tok.is_none() || tok.unwrap().mods & modbit::DECLARATION == 0);
  }

  #[test]
  fn function_params_are_declared_parameters() {
    let src = "CREATE FUNCTION f(a int, OUT b text) RETURNS int AS $$ SELECT 1 $$ LANGUAGE sql;";
    let raw = scan(src);
    assert_eq!(find(&raw, src, "f").kind, Tok::Function);
    for p in ["a int", "b text"] {
      let name = &p[..1];
      let at = src.find(p).unwrap();
      let t = raw.iter().find(|t| t.start == at).unwrap_or_else(|| panic!("no token for {name}"));
      assert_eq!(t.kind, Tok::Parameter, "{name}");
      assert_eq!(t.mods, modbit::DECLARATION, "{name}");
    }
  }

  #[test]
  fn plpgsql_declare_locals_are_declared_variables() {
    // Names must not themselves be SQL keywords -- the keyword table is
    // consulted first, so a local called `label` colours as a keyword.
    let src = "DECLARE\n  total int;\n  msg text;\nBEGIN\n  total := 1;\nEND";
    let raw = scan(src);
    for local in ["total", "msg"] {
      let t = find(&raw, src, local);
      assert_eq!(t.kind, Tok::Variable, "{local}");
      assert_eq!(t.mods, modbit::DECLARATION, "{local}");
    }
  }

  #[test]
  fn builtin_types_and_functions_carry_default_library() {
    let src = "SELECT count(x)::int FROM t;";
    let raw = scan(src);
    assert_eq!(find(&raw, src, "count").mods, modbit::DEFAULT_LIBRARY);
    let int_at = src.find("::int").unwrap() + 2;
    let t = raw.iter().find(|t| t.start == int_at).unwrap();
    assert_eq!(t.kind, Tok::Type);
  }

  #[test]
  fn user_defined_function_call_has_no_default_library_bit() {
    let src = "SELECT my_custom_thing(1);";
    let raw = scan(src);
    let t = find(&raw, src, "my_custom_thing");
    assert_eq!(t.kind, Tok::Function);
    assert_eq!(t.mods, 0);
  }

  #[test]
  fn create_table_as_select_does_not_declare_projection_columns() {
    let src = "CREATE TABLE snapshot AS SELECT (a), (b) FROM src;";
    let raw = scan(src);
    let a_at = src.find("(a)").unwrap() + 1;
    let tok = raw.iter().find(|t| t.start == a_at);
    assert!(tok.is_none() || tok.unwrap().mods & modbit::DECLARATION == 0, "AS SELECT has no column body");
  }

  #[test]
  fn limit_stops_the_scan_early() {
    let src = "SELECT 1;\nSELECT 2;\nSELECT 3;";
    let cut = src.find("SELECT 3").unwrap();
    let all = scan(src);
    let limited = collect(src, &dsl_catalog::Catalog::default(), Some(cut));
    assert!(limited.len() < all.len());
    assert!(limited.iter().all(|t| t.start < cut));
  }

  #[test]
  fn declaration_state_resets_across_statements() {
    let src = "CREATE TABLE a (id int);\nSELECT other FROM a;";
    let raw = scan(src);
    let other_at = src.find("other").unwrap();
    let tok = raw.iter().find(|t| t.start == other_at);
    assert!(tok.is_none() || tok.unwrap().mods == 0, "state must not leak past the `;`");
  }
}
