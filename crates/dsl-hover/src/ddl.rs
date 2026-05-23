//! Lookups against the *current buffer's* CREATE TABLE statements.
//!
//! When the cursor sits inside a column declaration of a CREATE TABLE in
//! the buffer being edited, we want a hover card that explains that
//! specific declaration -- type, NOT NULL, DEFAULT -- even when the live
//! catalog doesn't know about the table yet (because the user is in the
//! middle of writing it). dsl-parse gives us the parsed shape; we use it
//! here to map the cursor to a column def.

use crate::render;
use dsl_parse::{ParsedFile, StatementKind};
use text_size::TextSize;

/// Try to render a column-declaration hover for the cursor position.
/// Returns None when the cursor isn't inside a column declaration.
pub fn column_decl_at(file: &ParsedFile, source: &str, offset: TextSize) -> Option<String> {
    let pos: u32 = offset.into();
    for stmt in &file.statements {
        let StatementKind::CreateTable(ct) = &stmt.kind else { continue; };
        if !contains(stmt.range, pos) { continue; }

        // Locate the parenthesised body inside the statement so we can
        // map the cursor to a column position.
        let stmt_start: u32 = stmt.range.start().into();
        let stmt_end: u32 = stmt.range.end().into();
        let slice = &source[stmt_start as usize..stmt_end as usize];

        let body = match column_body_range(slice) {
            Some(r) => r,
            None => continue,
        };

        // Position relative to the body's interior.
        if pos < stmt_start + body.0 || pos >= stmt_start + body.1 {
            continue;
        }
        let body_text = &slice[body.0 as usize..body.1 as usize];

        // Walk the body splitting on top-level commas. The split
        // honours nested parens (e.g. NUMERIC(10,2)) so a comma inside
        // a type spec does not begin a new column.
        let mut piece_start = 0usize;
        let mut depth: i32 = 0;
        let mut entries: Vec<(usize, usize)> = Vec::new();
        for (i, ch) in body_text.char_indices() {
            match ch {
                '(' => depth += 1,
                ')' => depth -= 1,
                ',' if depth == 0 => {
                    entries.push((piece_start, i));
                    piece_start = i + 1;
                }
                _ => {}
            }
        }
        if piece_start < body_text.len() {
            entries.push((piece_start, body_text.len()));
        }

        // Map the cursor to one of the entries.
        let cursor_in_body = (pos - (stmt_start + body.0)) as usize;
        let entry = entries
            .into_iter()
            .find(|(s, e)| cursor_in_body >= *s && cursor_in_body <= *e)?;

        let piece = body_text[entry.0..entry.1].trim();
        if piece.is_empty() { continue; }

        // Skip constraint-only lines; those are resolved by the
        // constraint-identifier resolver instead.
        let upper = piece.to_ascii_uppercase();
        if upper.starts_with("CONSTRAINT")
            || upper.starts_with("PRIMARY KEY")
            || upper.starts_with("FOREIGN KEY")
            || upper.starts_with("UNIQUE ")
            || upper.starts_with("CHECK ")
        {
            continue;
        }

        // Tighten: only fire when the cursor sits ON the column-name
        // token itself. The type, NOT NULL, DEFAULT, and expression
        // tokens each have their own hover (keyword / type / function),
        // and overriding them here would mask the richer card.
        let token = crate::token::token_at(source, offset)?;
        let ident = piece
            .split(|c: char| c.is_whitespace() || c == '(' || c == ',')
            .find(|s| !s.is_empty())?
            .trim_matches('"');
        if !token.eq_ignore_ascii_case(ident) { return None; }

        let col = ct.columns.iter().find(|c| c.name == ident)?;
        let implicit = crate::implicit::derive(body_text, col);
        return Some(render::column_decl_with_implicit(&ct.table.name, col, &implicit));
    }
    None
}

fn contains(range: text_size::TextRange, pos: u32) -> bool {
    let s: u32 = range.start().into();
    let e: u32 = range.end().into();
    pos >= s && pos <= e
}

/// Find the open and close paren of the column body within a CREATE
/// TABLE statement slice. Returns offsets relative to the slice.
fn column_body_range(slice: &str) -> Option<(u32, u32)> {
    let open = slice.find('(')?;
    let mut depth: i32 = 1;
    let mut i = open + 1;
    let bytes = slice.as_bytes();
    while i < slice.len() {
        match bytes[i] as char {
            '(' => depth += 1,
            ')' => { depth -= 1; if depth == 0 { return Some(((open + 1) as u32, i as u32)); } }
            _ => {}
        }
        i += 1;
    }
    None
}
