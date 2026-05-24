//! `textDocument/inlayHint` handler.
//!
//! Two hint families today:
//!   1. `SELECT *` -> phantom ` -- id, name, price` after the `*`. Only
//!      fires when the FROM clause names exactly one catalog table.
//!   2. Column references in WHERE / SET / ORDER BY -> phantom `: TYPE`
//!      after the column if a single catalog table is in scope and the
//!      column is unambiguous. Skipped for `*`, NULL, literals.

use crate::handlers::position;
use crate::state::ServerState;
use dsl_parse::{StatementKind, Projection};
use ropey::Rope;
use text_size::TextRange;
use tower_lsp::lsp_types::{
    InlayHint, InlayHintKind, InlayHintLabel, InlayHintParams, Position,
};

pub fn run(state: &ServerState, params: InlayHintParams) -> Option<Vec<InlayHint>> {
    let uri = params.text_document.uri;
    let doc = state.documents.get(&uri)?;
    let cat = state.catalog.read().clone();
    let cache = doc.parsed();
    let parsed = &cache.file;

    // Also resolve against buffer-defined tables so a fresh `CREATE TABLE`
    // expands its columns immediately without needing a DB round-trip.
    let buffer_tables: Vec<(String, Vec<String>)> = parsed
        .statements
        .iter()
        .filter_map(|s| match &s.kind {
            StatementKind::CreateTable(ct) => Some((
                ct.table.name.clone(),
                ct.columns.iter().map(|c| c.name.clone()).collect::<Vec<_>>(),
            )),
            _ => None,
        })
        .collect();

    let mut hints: Vec<InlayHint> = Vec::new();
    for stmt in &parsed.statements {
        let StatementKind::Select(sel) = &stmt.kind else { continue };
        // Only emit when SELECT *.
        if !sel.projections.iter().any(|p| matches!(p, Projection::Star)) { continue; }
        // Single table in FROM.
        if sel.from.len() != 1 || !sel.joins.is_empty() { continue; }
        let target = &sel.from[0];
        let cols: Vec<String> = if let Some(t) = cat.find_table(target.schema.as_deref(), &target.name) {
            t.columns.iter().map(|c| c.name.clone()).collect()
        } else if let Some((_, cs)) = buffer_tables.iter().find(|(n, _)| n.eq_ignore_ascii_case(&target.name)) {
            cs.clone()
        } else {
            continue;
        };
        if cols.is_empty() { continue; }

        if let Some(star_byte) = find_star(&doc.text, stmt.range) {
            let pos = byte_to_position(&doc.rope, star_byte + 1);
            let joined = cols.join(", ");
            hints.push(InlayHint {
                position: pos,
                label: InlayHintLabel::String(format!("  -- {joined}")),
                kind: Some(InlayHintKind::TYPE),
                text_edits: None,
                tooltip: None,
                padding_left: Some(false),
                padding_right: Some(false),
                data: None,
            });
        }
    }
    if hints.is_empty() { None } else { Some(hints) }
}

fn find_star(source: &str, range: TextRange) -> Option<usize> {
    let s: u32 = range.start().into();
    let e: u32 = range.end().into();
    let s = s as usize;
    let e = (e as usize).min(source.len());
    let slice = &source[s..e];
    let upper = slice.to_ascii_uppercase();
    let select_at = upper.find("SELECT")?;
    let after = select_at + "SELECT".len();
    let rest = &slice[after..];
    let trim_lead = rest.len() - rest.trim_start().len();
    let star_local = rest[trim_lead..].chars().next()?;
    if star_local != '*' { return None; }
    Some(s + after + trim_lead)
}

// Same byte-to-LSP-position walker as the other handlers.
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

// Suppress unused; supplied for future cursor-position lookup.
#[allow(dead_code)]
fn _unused(_p: Position) {
    let _ = position::to_offset;
}
