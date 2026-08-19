//! `textDocument/rangeFormatting` handler -- format just the selection.
//!
//! SQL has no sub-statement formatting unit that survives being formatted
//! in isolation: hand `WHERE a = 1` to sql-formatter on its own and it
//! reflows it as a standalone fragment with no idea what clause it came
//! from. So instead of formatting the literal byte range the editor asked
//! for, we *snap outward to statement boundaries*: every top-level
//! statement the selection touches is formatted whole, and the returned
//! TextEdit spans exactly those statements.
//!
//! Consequences of the snap, all deliberate:
//!   * Selecting the middle of one `SELECT` formats that whole `SELECT`.
//!   * Selecting across two statements formats both, plus the whitespace
//!     between them (so `linesBetweenQueries` applies).
//!   * Selecting only blank lines / a comment between statements formats
//!     nothing (returns None) rather than mangling the comment.
//!
//! The trailing `;` of the last touched statement is pulled into the
//! region so the formatter sees a terminated statement -- otherwise
//! `newlineBeforeSemicolon` and friends have nothing to act on and the
//! orphaned `;` ends up on its own line.
//!
//! Indentation: when the region starts mid-line behind pure whitespace
//! (an indented statement inside a migration block), that indent is
//! re-applied to every produced line after the first. Without it a
//! formatted nested statement would slam back to column 0.

use crate::handlers::position::{byte_to_lsp, to_offset};
use crate::state::ServerState;
use tower_lsp::lsp_types::{DocumentRangeFormattingParams, Range, TextEdit};

pub fn run(state: &ServerState, params: DocumentRangeFormattingParams) -> Option<Vec<TextEdit>> {
  let uri = &params.text_document.uri;
  let _g = crate::handlers::perf::Guard::with_uri("range_formatting", uri);
  let doc = state.documents.get(uri)?;
  if doc.too_large() {
    return None;
  }
  let src = &doc.text;

  let sel_start: usize = usize::from(to_offset(&doc.rope, params.range.start));
  let sel_end: usize = usize::from(to_offset(&doc.rope, params.range.end));
  let (sel_start, sel_end) = if sel_start <= sel_end { (sel_start, sel_end) } else { (sel_end, sel_start) };

  let (region_start, region_end) = snap_to_statements(src, sel_start, sel_end)?;
  let slice = &src[region_start..region_end];
  if slice.trim().is_empty() {
    return None;
  }

  let formatter_style = crate::handlers::formatting::resolve_style(state, &doc, &params.options);
  let cfg = state.config_snapshot();
  let formatted = dsl_format::format(slice, &formatter_style, &cfg.style.create_table);

  // `format` normalises to a trailing newline + may prepend blank
  // lines; neither belongs inside a mid-document splice.
  let formatted = formatted.trim_matches(|c| c == '\n' || c == '\r');
  let formatted = reindent(formatted, base_indent(src, region_start));

  if formatted == slice {
    return None;
  }

  Some(vec![TextEdit {
    range: Range { start: byte_to_lsp(&doc.rope, region_start), end: byte_to_lsp(&doc.rope, region_end) },
    new_text: formatted,
  }])
}

/// Grow `[sel_start, sel_end)` outward to cover whole top-level
/// statements. Returns `None` when the selection touches no statement
/// text at all (pure whitespace / trailing comment tail).
///
/// An empty selection (caret, no visual selection -- what nvim's
/// `vim.lsp.buf.format()` sends with no range and what VS Code sends
/// for "format selection" with nothing selected) resolves to the
/// statement the caret sits in or directly abuts.
fn snap_to_statements(src: &str, sel_start: usize, sel_end: usize) -> Option<(usize, usize)> {
  let stmts = dsl_parse::split::split_statements(src);
  let mut region: Option<(usize, usize)> = None;
  for (_, range) in &stmts {
    let s = usize::from(range.start());
    let e = usize::from(range.end());
    // Half-open overlap, with the empty-selection case widened to
    // inclusive so a caret parked on either edge still matches.
    let touches = if sel_start == sel_end { sel_start >= s && sel_start <= e } else { s < sel_end && e > sel_start };
    if !touches {
      continue;
    }
    region = Some(match region {
      Some((rs, re)) => (rs.min(s), re.max(e)),
      None => (s, e),
    });
  }
  let (start, mut end) = region?;
  // Pull in the terminating `;` (and only that -- not a trailing
  // comment on the same line, which the formatter would relocate).
  let tail = src.as_bytes();
  let mut probe = end;
  while probe < tail.len() && (tail[probe] == b' ' || tail[probe] == b'\t') {
    probe += 1;
  }
  if probe < tail.len() && tail[probe] == b';' {
    end = probe + 1;
  }
  Some((start, end))
}

/// Whitespace prefix of the line containing `at`, but only when
/// everything before `at` on that line is whitespace. Anything else
/// (e.g. `SELECT 1; SELECT 2` where the second statement starts
/// mid-line) yields an empty indent -- re-indenting there would be
/// guesswork.
fn base_indent(src: &str, at: usize) -> &str {
  let line_start = src[..at].rfind('\n').map(|i| i + 1).unwrap_or(0);
  let prefix = &src[line_start..at];
  if prefix.chars().all(|c| c == ' ' || c == '\t') { prefix } else { "" }
}

/// Prefix every line after the first with `indent`. Blank lines stay
/// blank so we don't introduce trailing whitespace.
fn reindent(text: &str, indent: &str) -> String {
  if indent.is_empty() || !text.contains('\n') {
    return text.to_string();
  }
  let mut out = String::with_capacity(text.len() + indent.len() * 4);
  for (i, line) in text.split('\n').enumerate() {
    if i > 0 {
      out.push('\n');
      if !line.trim().is_empty() {
        out.push_str(indent);
      }
    }
    out.push_str(line);
  }
  out
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn snaps_caret_inside_statement_to_whole_statement() {
    let src = "SELECT a FROM t;\nSELECT b FROM u;\n";
    let caret = src.find("FROM t").unwrap();
    let (s, e) = snap_to_statements(src, caret, caret).unwrap();
    assert_eq!(&src[s..e], "SELECT a FROM t;");
  }

  #[test]
  fn snaps_partial_selection_outward_over_both_statements() {
    let src = "SELECT a FROM t;\nSELECT b FROM u;\n";
    let start = src.find("a FROM").unwrap();
    let end = src.find("b FROM").unwrap();
    let (s, e) = snap_to_statements(src, start, end).unwrap();
    assert_eq!(&src[s..e], "SELECT a FROM t;\nSELECT b FROM u;");
  }

  #[test]
  fn selection_over_only_whitespace_between_statements_yields_none() {
    let src = "SELECT a FROM t;\n\n\nSELECT b FROM u;";
    let gap_start = src.find(";\n\n\n").unwrap() + 2;
    assert!(snap_to_statements(src, gap_start, gap_start + 1).is_none());
  }

  #[test]
  fn semicolon_only_pulled_in_when_it_directly_follows() {
    // No terminator at all -> region ends at the statement text.
    let src = "SELECT a FROM t";
    let (s, e) = snap_to_statements(src, 0, src.len()).unwrap();
    assert_eq!(&src[s..e], "SELECT a FROM t");
  }

  #[test]
  fn dollar_quoted_body_is_one_statement() {
    let src = "CREATE FUNCTION f() RETURNS int AS $$ BEGIN RETURN 1; END $$ LANGUAGE plpgsql;\nSELECT 1;";
    let caret = src.find("RETURN 1").unwrap();
    let (s, e) = snap_to_statements(src, caret, caret).unwrap();
    assert!(src[s..e].starts_with("CREATE FUNCTION"), "got {:?}", &src[s..e]);
    assert!(src[s..e].ends_with("plpgsql;"), "got {:?}", &src[s..e]);
  }

  #[test]
  fn base_indent_only_when_prefix_is_pure_whitespace() {
    let src = "  SELECT 1;";
    assert_eq!(base_indent(src, 2), "  ");
    let src2 = "SELECT 1; SELECT 2;";
    assert_eq!(base_indent(src2, 10), "");
  }

  #[test]
  fn reindent_skips_first_line_and_blank_lines() {
    let got = reindent("SELECT a\nFROM t\n\nWHERE x", "  ");
    assert_eq!(got, "SELECT a\n  FROM t\n\n  WHERE x");
  }
}
