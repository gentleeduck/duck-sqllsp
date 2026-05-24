//! `textDocument/codeLens` handler.
//!
//! Emits a `Run` and `EXPLAIN` lens above every SELECT / INSERT /
//! UPDATE / DELETE statement. The lens command name (`duck-sqllsp.runQuery`,
//! `duck-sqllsp.explainQuery`) is what the editor binds to a Lua handler;
//! the LSP itself doesn't execute the query -- the editor uses dadbod
//! (or whatever the user has bound) to run the statement text returned
//! in `arguments`.

use crate::state::ServerState;
use dsl_parse::StatementKind;
use ropey::Rope;
use text_size::TextRange;
use tower_lsp::lsp_types::{CodeLens, CodeLensParams, Command, Position, Range};

pub fn run(state: &ServerState, params: CodeLensParams) -> Option<Vec<CodeLens>> {
  let _g = crate::handlers::perf::Guard::with_uri("code_lens", &params.text_document.uri);
  let doc = state.documents.get(&params.text_document.uri)?;
  let cache = doc.parsed();

  let mut out = Vec::new();
  for stmt in &cache.file.statements {
    let runnable = matches!(
      &stmt.kind,
      StatementKind::Select(_) | StatementKind::Insert(_) | StatementKind::Update(_) | StatementKind::Delete(_)
    );
    if !runnable {
      continue;
    }
    let range = to_lsp_range(&doc.rope, stmt.range);
    let text = slice_of(&doc.text, stmt.range);
    out.push(CodeLens {
      range,
      command: Some(Command {
        title: "Run".into(),
        command: "duck-sqllsp.runQuery".into(),
        arguments: Some(vec![serde_json::json!(text)]),
      }),
      data: None,
    });
    out.push(CodeLens {
      range,
      command: Some(Command {
        title: "EXPLAIN".into(),
        command: "duck-sqllsp.explainQuery".into(),
        arguments: Some(vec![serde_json::json!(text)]),
      }),
      data: None,
    });
    // Slow-query nudge: a SELECT with 3+ JOINs and no LIMIT clause
    // is likely to scan a lot of rows. Surface inline LIMIT 100 and
    // EXPLAIN ANALYZE shortcuts so the user can quickly bound the
    // scope or get a cost estimate.
    if let StatementKind::Select(_) = &stmt.kind {
      if is_slow_select(&text) {
        out.push(CodeLens {
          range,
          command: Some(Command {
            title: "+ LIMIT 100".into(),
            command: "duck-sqllsp.addLimit".into(),
            arguments: Some(vec![serde_json::json!(text), serde_json::json!(100)]),
          }),
          data: None,
        });
        out.push(CodeLens {
          range,
          command: Some(Command {
            title: "EXPLAIN ANALYZE".into(),
            command: "duck-sqllsp.explainAnalyzeQuery".into(),
            arguments: Some(vec![serde_json::json!(text)]),
          }),
          data: None,
        });
      }
    }
  }
  if out.is_empty() { None } else { Some(out) }
}

/// SELECT with >= 3 JOIN tokens and no LIMIT (case-insensitive, word-
/// bounded). Crude but covers the cases worth nudging on.
fn is_slow_select(text: &str) -> bool {
  let upper = text.to_ascii_uppercase();
  let join_count = upper.split_whitespace().filter(|w| *w == "JOIN").count();
  if join_count < 3 { return false; }
  // Reject if any LIMIT keyword survives, as a whole word.
  !upper.split(|c: char| !c.is_ascii_alphanumeric() && c != '_').any(|w| w == "LIMIT")
}

fn slice_of(text: &str, r: TextRange) -> String {
  let s: u32 = r.start().into();
  let e: u32 = r.end().into();
  let end = (e as usize).min(text.len());
  text[s as usize..end].to_string()
}

fn to_lsp_range(rope: &Rope, r: TextRange) -> Range {
  let s: u32 = r.start().into();
  let e: u32 = r.end().into();
  Range { start: byte_to_position(rope, s as usize), end: byte_to_position(rope, (e as usize).min(rope.len_bytes())) }
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
