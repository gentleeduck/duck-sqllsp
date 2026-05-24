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

    // INSERT INTO t (a, b) VALUES (1, 'x')
    //  -> phantom `: int4` / `: text` next to each VALUES literal.
    // INSERT INTO t VALUES (1, 'x', ...)
    //  -> phantom `: column_name` next to each positional value
    //     (no column list, so the hint surfaces what column gets it).
    for stmt in &parsed.statements {
        let StatementKind::Insert(ins) = &stmt.kind else { continue };
        let target = &ins.table;
        let cols: Vec<(String, String)> = if let Some(t) = cat.find_table(target.schema.as_deref(), &target.name) {
            t.columns.iter().map(|c| (c.name.clone(), c.data_type.clone())).collect()
        } else if let Some((_, cs)) = buffer_tables.iter().find(|(n, _)| n.eq_ignore_ascii_case(&target.name)) {
            cs.iter().map(|n| (n.clone(), String::new())).collect()
        } else {
            continue;
        };
        if cols.is_empty() { continue; }
        // Map ins.columns -> Vec<(name, type)>; empty cols list means
        // positional, use the catalog order.
        let ordered: Vec<(String, String)> = if ins.columns.is_empty() {
            cols.clone()
        } else {
            ins.columns.iter().filter_map(|name| {
                cols.iter().find(|(n, _)| n.eq_ignore_ascii_case(name)).cloned()
            }).collect()
        };
        let positional = ins.columns.is_empty();
        for (idx, lit_byte) in find_values_literals(&doc.text, stmt.range).into_iter().enumerate() {
            let Some((col_name, col_type)) = ordered.get(idx) else { break };
            let label = if positional {
                format!(" : {col_name}")
            } else if col_type.is_empty() {
                continue
            } else {
                format!(" : {col_type}")
            };
            let pos = byte_to_position(&doc.rope, lit_byte);
            hints.push(InlayHint {
                position: pos,
                label: InlayHintLabel::String(label),
                kind: Some(InlayHintKind::TYPE),
                text_edits: None,
                tooltip: None,
                padding_left: Some(false),
                padding_right: Some(false),
                data: None,
            });
        }
    }

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

    // JOIN with missing / minimal ON-clause: when an FK relates the
    // two tables, surface ` -- t.user_id = u.id` next to the JOIN
    // keyword as a guess of the missing predicate.
    for stmt in &parsed.statements {
        let StatementKind::Select(sel) = &stmt.kind else { continue };
        if sel.joins.is_empty() { continue; }
        if sel.from.is_empty() { continue; }
        // Build alias -> table lookup for the FROM table.
        let from = &sel.from[0];
        let from_table = cat.find_table(from.schema.as_deref(), &from.name);
        let from_alias = from.alias.clone().unwrap_or_else(|| from.name.clone());
        for j in &sel.joins {
            // Skip joins that already have a non-trivial ON.
            if j.on.is_some() { continue; }
            let join_table = cat.find_table(j.table.schema.as_deref(), &j.table.name);
            let join_alias = j.table.alias.clone().unwrap_or_else(|| j.table.name.clone());
            // Walk both directions: from->join, join->from. Look for a
            // single-column FK linking the two.
            let predicate = find_fk_predicate(from_table, &from_alias, join_table, &join_alias)
                .or_else(|| find_fk_predicate(join_table, &join_alias, from_table, &from_alias));
            let Some(pred) = predicate else { continue };
            // Place the hint after the JOIN's table name.
            let stmt_start: u32 = stmt.range.start().into();
            let stmt_end: u32 = stmt.range.end().into();
            let body = &doc.text[stmt_start as usize..(stmt_end as usize).min(doc.text.len())];
            // First "JOIN" keyword after FROM is good enough as an anchor.
            let join_idx = body.to_ascii_uppercase().find(" JOIN ");
            if let Some(idx) = join_idx {
                let pos_byte = stmt_start as usize + idx + 6 + j.table.name.len() + 1;
                let pos = byte_to_position(&doc.rope, pos_byte.min(doc.text.len()));
                hints.push(InlayHint {
                    position: pos,
                    label: InlayHintLabel::String(format!("  -- ON {pred}")),
                    kind: Some(InlayHintKind::TYPE),
                    text_edits: None,
                    tooltip: None,
                    padding_left: Some(false),
                    padding_right: Some(false),
                    data: None,
                });
            }
        }
    }

    if hints.is_empty() { None } else { Some(hints) }
}

fn find_fk_predicate(
    src_table: Option<&dsl_catalog::Table>,
    src_alias: &str,
    target_table: Option<&dsl_catalog::Table>,
    target_alias: &str,
) -> Option<String> {
    let src = src_table?;
    let target = target_table?;
    for c in &src.constraints {
        if !matches!(c.kind, dsl_catalog::ConstraintKind::ForeignKey) { continue; }
        let Some(refs) = &c.references else { continue };
        if !refs.table.eq_ignore_ascii_case(&target.name) { continue; }
        if c.columns.len() != 1 || refs.columns.len() != 1 { continue; }
        return Some(format!(
            "{src_alias}.{} = {target_alias}.{}",
            c.columns[0], refs.columns[0]
        ));
    }
    None
}

/// Locate the byte position right *after* each top-level literal in the
/// first VALUES tuple of an INSERT statement. Skips nested parens and
/// quoted strings.
fn find_values_literals(source: &str, range: TextRange) -> Vec<usize> {
    let s: u32 = range.start().into();
    let e: u32 = range.end().into();
    let start = s as usize;
    let end = (e as usize).min(source.len());
    let slice = &source[start..end];
    let upper = slice.to_ascii_uppercase();
    let Some(values_at) = upper.find("VALUES") else { return Vec::new() };
    let bytes = slice.as_bytes();
    let n = bytes.len();
    let mut k = values_at + 6;
    while k < n && bytes[k].is_ascii_whitespace() { k += 1; }
    if k >= n || bytes[k] != b'(' { return Vec::new(); }
    let mut out = Vec::new();
    let mut depth = 1i32;
    let mut item_end = k + 1; // running end-of-current-item byte position
    let mut had_content = false;
    let mut i = k + 1;
    while i < n && depth > 0 {
        match bytes[i] {
            b'(' => { depth += 1; had_content = true; }
            b')' => {
                depth -= 1;
                if depth == 0 {
                    if had_content { out.push(start + item_end); }
                    break;
                }
            }
            b'\'' => {
                i += 1;
                while i < n && bytes[i] != b'\'' { i += 1; }
                had_content = true;
                if i < n {
                    i += 1;
                    item_end = i;
                    continue;
                }
            }
            b',' if depth == 1 => {
                if had_content { out.push(start + item_end); }
                had_content = false;
                i += 1;
                continue;
            }
            c if c.is_ascii_whitespace() => {}
            _ => { had_content = true; item_end = i + 1; }
        }
        if !bytes[i].is_ascii_whitespace() { item_end = i + 1; }
        i += 1;
    }
    out
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
