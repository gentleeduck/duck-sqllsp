//! `textDocument/onTypeFormatting` handler.
//!
//! Fires when the user types one of the configured trigger characters
//! (currently `\n`). Returns a TextEdit that supplies the right
//! indentation for the new line so cursor lands at the correct column:
//!
//!   * Newline after `(` inside CREATE TABLE / INSERT VALUES / function
//!     args: indent one tab beyond the line containing the `(`.
//!   * Newline inside a PL/pgSQL block (BEGIN..END, IF..END IF,
//!     LOOP..END LOOP, CASE..END CASE): indent matches the current
//!     block depth.
//!   * Newline inside a SELECT projection list / WHERE conjunction /
//!     etc: keep the existing indentation of the previous line so
//!     wrapped expressions line up.
//!
//! Pure text-scan; depends on the surrounding char only, not the
//! parser. Never emits a destructive edit -- if no indent rule applies,
//! returns None and the editor handles indentation itself.

use crate::state::ServerState;
use tower_lsp::lsp_types::{DocumentOnTypeFormattingParams, Range, TextEdit};

pub fn run(state: &ServerState, params: DocumentOnTypeFormattingParams) -> Option<Vec<TextEdit>> {
  let uri = &params.text_document_position.text_document.uri;
  let _g = crate::handlers::perf::Guard::with_uri("on_type_formatting", uri);
  let doc = state.documents.get(uri)?;
  let text = &doc.text;

  // Only handle the newline trigger.
  if params.ch != "\n" {
    return None;
  }

  let pos = params.text_document_position.position;
  // The cursor sits at the start of the new (empty) line. The
  // previous line is the one we read for context.
  if pos.line == 0 {
    return None;
  }
  let prev_line_idx = pos.line as usize - 1;
  let line_byte = doc.rope.line_to_byte(prev_line_idx);
  let line_end = doc.rope.line_to_byte(pos.line as usize);
  let prev_line = &text[line_byte..line_end];
  // Trim newline at end.
  let prev_line = prev_line.trim_end_matches('\n');

  let current_indent = leading_indent(prev_line);
  let indent_unit = indent_unit(&params.options);

  // Newline after `(` at end of line -> increase indent by one unit.
  let trimmed = prev_line.trim_end();
  let increase_indent = trimmed.ends_with('(')
    || ends_with_word_ci(trimmed, "BEGIN")
    || ends_with_word_ci(trimmed, "LOOP")
    || ends_with_word_ci(trimmed, "THEN")
    || ends_with_word_ci(trimmed, "ELSE");
  let new_indent = if increase_indent {
    format!("{current_indent}{indent_unit}")
  } else {
    // Plain wrap -- keep the same indent as the previous line.
    current_indent.to_string()
  };

  if new_indent.is_empty() {
    return None;
  }

  Some(vec![TextEdit { range: Range { start: pos, end: pos }, new_text: new_indent }])
}

fn leading_indent(line: &str) -> &str {
  let end = line.char_indices().find(|(_, c)| !c.is_whitespace() || *c == '\n').map(|(i, _)| i).unwrap_or(line.len());
  &line[..end]
}

fn indent_unit(opts: &tower_lsp::lsp_types::FormattingOptions) -> String {
  if opts.insert_spaces { " ".repeat(opts.tab_size.max(1) as usize) } else { "\t".to_string() }
}

fn ends_with_word_ci(s: &str, word: &str) -> bool {
  let s_upper = s.to_ascii_uppercase();
  let w_upper = word.to_ascii_uppercase();
  if !s_upper.ends_with(&w_upper) {
    return false;
  }
  let cut = s_upper.len() - w_upper.len();
  // The char before the word must be word-boundary (not alnum/_).
  cut == 0 || !s_upper.as_bytes()[cut - 1].is_ascii_alphanumeric() && s_upper.as_bytes()[cut - 1] != b'_'
}
