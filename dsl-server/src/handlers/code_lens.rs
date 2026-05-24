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
use tower_lsp::lsp_types::{
    CodeLens, CodeLensParams, Command, Position, Range,
};

pub fn run(state: &ServerState, params: CodeLensParams) -> Option<Vec<CodeLens>> {
    let doc = state.documents.get(&params.text_document.uri)?;
    let cache = doc.parsed();

    let mut out = Vec::new();
    for stmt in &cache.file.statements {
        let runnable = matches!(
            &stmt.kind,
            StatementKind::Select(_)
                | StatementKind::Insert(_)
                | StatementKind::Update(_)
                | StatementKind::Delete(_)
        );
        if !runnable { continue; }
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
    }
    if out.is_empty() { None } else { Some(out) }
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
    Range {
        start: byte_to_position(rope, s as usize),
        end:   byte_to_position(rope, (e as usize).min(rope.len_bytes())),
    }
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
        if bytes_seen >= bytes_in_line { break; }
        utf16 += c.len_utf16() as u32;
        bytes_seen += c.len_utf8();
    }
    Position { line: line as u32, character: utf16 }
}
