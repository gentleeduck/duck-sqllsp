//! `textDocument/documentSymbol` handler.
//!
//! Walks the parsed statements and surfaces an outline entry per
//! recognised top-level statement. For CREATE TABLE we nest each
//! defined column as a child symbol so the editor's outline panel
//! shows the full schema at a glance.

use crate::handlers::position;
use crate::state::ServerState;
use dsl_parse::{Statement, StatementKind};
use ropey::Rope;
use text_size::TextRange;
use tower_lsp::lsp_types::{
    DocumentSymbol, DocumentSymbolParams, DocumentSymbolResponse, Position, Range, SymbolKind,
};

pub fn run(state: &ServerState, params: DocumentSymbolParams) -> Option<DocumentSymbolResponse> {
    let doc = state.documents.get(&params.text_document.uri)?;
    let cache = doc.parsed();
    let mut out = Vec::new();
    for s in &cache.file.statements {
        if let Some(sym) = symbol_for(s, &doc.text, &doc.rope) {
            out.push(sym);
        }
    }
    Some(DocumentSymbolResponse::Nested(out))
}

fn symbol_for(stmt: &Statement, text: &str, rope: &Rope) -> Option<DocumentSymbol> {
    let range = to_lsp_range(rope, stmt.range);
    match &stmt.kind {
        StatementKind::CreateTable(ct) => {
            let children: Vec<DocumentSymbol> = ct
                .columns
                .iter()
                .map(|c| {
                    #[allow(deprecated)]
                    DocumentSymbol {
                        name: c.name.clone(),
                        detail: Some(c.type_name.clone()),
                        kind: SymbolKind::FIELD,
                        tags: None,
                        deprecated: None,
                        range,
                        selection_range: range,
                        children: None,
                    }
                })
                .collect();
            Some(make_symbol(
                &ct.table.name,
                Some("table".into()),
                SymbolKind::CLASS,
                range,
                Some(children),
            ))
        }
        StatementKind::AlterTable(at) => Some(make_symbol(
            &at.table.name,
            Some("alter".into()),
            SymbolKind::CLASS,
            range,
            None,
        )),
        StatementKind::DropTable(d) => Some(make_symbol(
            &d.tables.first().map(|t| t.name.clone()).unwrap_or_else(|| "drop table".into()),
            Some("drop".into()),
            SymbolKind::CLASS,
            range,
            None,
        )),
        StatementKind::Insert(i) => Some(make_symbol(
            &format!("INSERT INTO {}", i.table.name),
            None,
            SymbolKind::EVENT,
            range,
            None,
        )),
        StatementKind::Update(u) => Some(make_symbol(
            &format!("UPDATE {}", u.table.name),
            None,
            SymbolKind::EVENT,
            range,
            None,
        )),
        StatementKind::Delete(d) => Some(make_symbol(
            &format!("DELETE FROM {}", d.table.name),
            None,
            SymbolKind::EVENT,
            range,
            None,
        )),
        StatementKind::Select(s) => {
            let from = s.from.first().map(|t| t.name.clone()).unwrap_or_else(|| "?".into());
            Some(make_symbol(
                &format!("SELECT ... FROM {from}"),
                None,
                SymbolKind::FUNCTION,
                range,
                None,
            ))
        }
        StatementKind::Unknown { text: t } => {
            // Detect CREATE FUNCTION / CREATE TRIGGER textually so they
            // still show up in the outline.
            let upper = t.to_ascii_uppercase();
            if upper.starts_with("CREATE OR REPLACE FUNCTION") || upper.starts_with("CREATE FUNCTION") {
                let name = extract_function_name(t);
                return Some(make_symbol(&name, Some("function".into()), SymbolKind::FUNCTION, range, None));
            }
            if upper.starts_with("CREATE TRIGGER") {
                let name = extract_trigger_name(t);
                return Some(make_symbol(&name, Some("trigger".into()), SymbolKind::EVENT, range, None));
            }
            let _ = text; // unused for now
            None
        }
    }
}

fn make_symbol(
    name: &str,
    detail: Option<String>,
    kind: SymbolKind,
    range: Range,
    children: Option<Vec<DocumentSymbol>>,
) -> DocumentSymbol {
    #[allow(deprecated)]
    DocumentSymbol {
        name: name.to_string(),
        detail,
        kind,
        tags: None,
        deprecated: None,
        range,
        selection_range: range,
        children,
    }
}

fn to_lsp_range(rope: &Rope, r: TextRange) -> Range {
    let s: u32 = r.start().into();
    let e: u32 = r.end().into();
    Range {
        start: byte_to_position(rope, s as usize),
        end: byte_to_position(rope, (e as usize).min(rope.len_bytes())),
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

fn extract_function_name(t: &str) -> String {
    let upper = t.to_ascii_uppercase();
    let idx = upper.find("FUNCTION").map(|i| i + "FUNCTION".len()).unwrap_or(0);
    t[idx..]
        .trim_start()
        .split(|c: char| c.is_whitespace() || c == '(')
        .find(|s| !s.is_empty())
        .unwrap_or("<function>")
        .to_string()
}

fn extract_trigger_name(t: &str) -> String {
    let upper = t.to_ascii_uppercase();
    let idx = upper.find("TRIGGER").map(|i| i + "TRIGGER".len()).unwrap_or(0);
    t[idx..]
        .trim_start()
        .split(|c: char| c.is_whitespace())
        .find(|s| !s.is_empty())
        .unwrap_or("<trigger>")
        .to_string()
}

// Unused; placeholder for future per-statement position math.
#[allow(dead_code)]
fn _via_position(rope: &Rope, byte: usize) -> Position {
    position::to_offset(rope, Position { line: 0, character: byte as u32 });
    Position::default()
}
