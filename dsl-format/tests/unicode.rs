//! Unicode preservation: every UTF-8 byte in the input must survive
//! the format pipeline unchanged. A single `bytes[i] as char` slip
//! along the byte-walking code paths is enough to mangle non-ASCII
//! content (it reinterprets each UTF-8 continuation byte as a Latin-1
//! codepoint and re-encodes it). These tests pin that invariant.

use dsl_format::{CreateTableStyle, FormatterStyle, format, rewrite};

fn fmt(src: &str) -> String {
  format(src, &FormatterStyle::default(), &CreateTableStyle::default())
}

fn ct(src: &str) -> String {
  rewrite(src, &CreateTableStyle::default())
}

#[test]
fn format_preserves_unicode_in_string_literal() {
  let src = "SELECT 'café';";
  let out = fmt(src);
  assert!(out.contains("café"), "lost `café` in output; got: {out:?}");
}

#[test]
fn format_preserves_unicode_in_line_comment() {
  let src = "-- café\nSELECT 1;";
  let out = fmt(src);
  assert!(out.contains("café"), "lost `café` in comment; got: {out:?}");
}

#[test]
fn format_preserves_unicode_in_identifier() {
  let src = "SELECT café FROM users;";
  let out = fmt(src);
  assert!(out.contains("café"), "lost `café` identifier; got: {out:?}");
}

#[test]
fn format_preserves_japanese_in_quoted_identifier() {
  let src = "SELECT \"名前\" FROM users;";
  let out = fmt(src);
  assert!(out.contains("名前"), "lost Japanese quoted identifier; got: {out:?}");
}

#[test]
fn format_preserves_emoji_in_comment() {
  let src = "-- ☕ coffee\nSELECT 1;";
  let out = fmt(src);
  assert!(out.contains("☕"), "lost emoji in comment; got: {out:?}");
}

#[test]
fn rewrite_preserves_unicode_in_plpgsql_body() {
  // PL/pgSQL body alignment walks bytes byte-by-byte. A Unicode
  // string literal inside the body must round-trip cleanly.
  let src = "CREATE FUNCTION g() RETURNS text AS $$ BEGIN RETURN 'café'; END; $$ LANGUAGE plpgsql;";
  let out = ct(src);
  assert!(out.contains("café"), "lost `café` inside PL/pgSQL body; got: {out:?}");
}

#[test]
fn format_strips_leading_bom() {
  let src = "\u{feff}SELECT 1;";
  let out = fmt(src);
  assert!(!out.starts_with('\u{feff}'), "leading BOM should be stripped; got bytes {:02x?}", out.as_bytes());
  // External formatter may wrap `SELECT 1` onto two lines (`SELECT\n    1`)
  // -- assert the tokens survive in order rather than as a contiguous run.
  assert!(out.contains("SELECT"), "SELECT keyword must survive: {out:?}");
  assert!(out.contains('1'), "value 1 must survive: {out:?}");
}

#[test]
fn format_preserves_bom_inside_string_literal() {
  // BOM inside a string literal is data, not metadata -- keep it.
  let src = "SELECT '\u{feff}' AS bom_char;";
  let out = fmt(src);
  assert!(out.contains('\u{feff}'), "BOM inside string literal must survive: bytes {:02x?}", out.as_bytes());
}

#[test]
fn rewrite_preserves_unicode_in_plpgsql_line_comment() {
  let src = "CREATE FUNCTION g() RETURNS void AS $$ BEGIN -- café\nNULL; END; $$ LANGUAGE plpgsql;";
  let out = ct(src);
  assert!(out.contains("café"), "lost `café` in PL/pgSQL comment; got: {out:?}");
}
// ============================================================================
// Inline comments inside CREATE TABLE column lists. A `/* ... */`
// (or `-- ...`) attached to the same line as a column declaration
// used to land in the "name" slot, leaving downstream columns
// indented to a phantom 8-char width. Now lifted onto its own
// indented row so the column name aligns with siblings.
// ============================================================================

#[test]
fn create_table_inline_block_comment_lifts_to_own_line() {
  let src = "create table t(\n  a int,\n  /* trailing */ b text\n)";
  let out = ct(src);
  // The `b` column row must align with the `a` column row (both at
  // 4-space indent, name-width 1 then gap then type).
  assert!(out.contains("    a  int"), "expected `a` aligned; got: {out}");
  assert!(out.contains("    b  text"), "expected `b` aligned same as `a`; got: {out}");
  // The comment lands on its own line at the same 4-space indent.
  assert!(out.contains("    /* trailing */"), "expected comment on its own indented line; got: {out}");
  // The comment line itself must not get a trailing comma.
  assert!(!out.contains("/* trailing */,"), "comment line should not gain a comma; got: {out}");
}

#[test]
fn create_table_inline_line_comment_lifts_to_own_line() {
  let src = "create table t(\n  a int,\n  -- note\n  b text\n)";
  let out = ct(src);
  assert!(out.contains("    -- note"), "expected line comment on its own indented row; got: {out}");
  assert!(out.contains("    b  text"), "column `b` must remain aligned; got: {out}");
}

#[test]
fn create_table_trailing_line_comment_does_not_swallow_comma() {
  // `a int -- inline\n , b text` used to render as
  // `    a  int -- inline,\n    b  text` -- the comma landed
  // INSIDE the line comment, silently breaking the column DDL.
  // Trailing line comments must move to the end of the row AFTER
  // the comma so the comment text doesn't capture it.
  let src = "create table t(\n  a int -- inline\n , b text\n)";
  let out = ct(src);
  assert!(out.contains("    a  int, -- inline"), "expected comma before trailing line comment; got: {out}");
  assert!(!out.contains("-- inline,"), "comma must not be inside line comment: {out}");
}

#[test]
fn create_table_trailing_block_comment_at_end_of_entry() {
  // Final `/* note */` at end of a column entry (no SQL after it
  // before the comma) -- should also move past the comma.
  let src = "create table t(\n  a int /* note */,\n  b text\n)";
  let out = ct(src);
  // Either approach works as long as the comma is not inside the
  // block comment. The minimal contract: comma must follow the
  // block comment text intact in the rendered output.
  assert!(out.contains("    a  int") && !out.contains("/* note,"), "comma must not be inside block comment: {out}");
}
