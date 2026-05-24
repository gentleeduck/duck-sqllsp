//! `textDocument/selectionRange` handler.
//!
//! Returns nested ranges for "smart expand". Order grows outward:
//!   1. identifier under cursor
//!   2. enclosing parenthesised group (`(...)`)
//!   3. enclosing clause (SELECT / FROM / WHERE / ORDER BY / ... separator)
//!   4. enclosing statement (text from start of stmt to next `;`)
//!   5. whole document
//!
//! Each step builds on the previous so editors can chain `expand` /
//! `shrink` keystrokes.

use crate::handlers::position;
use crate::state::ServerState;
use ropey::Rope;
use tower_lsp::lsp_types::{
    Position, Range, SelectionRange, SelectionRangeParams,
};

pub fn run(state: &ServerState, params: SelectionRangeParams) -> Option<Vec<SelectionRange>> {
    let uri = params.text_document.uri;
    let doc = state.documents.get(&uri)?;

    let mut out = Vec::with_capacity(params.positions.len());
    for pos in params.positions {
        let offset_ts = position::to_offset(&doc.rope, pos);
        let offset: usize = u32::from(offset_ts) as usize;
        out.push(build_chain(&doc.text, &doc.rope, offset));
    }
    Some(out)
}

fn build_chain(text: &str, rope: &Rope, offset: usize) -> SelectionRange {
    let bytes = text.as_bytes();
    let n = bytes.len();
    let offset = offset.min(n);

    let mut layers: Vec<(usize, usize)> = Vec::new();
    if let Some(r) = identifier_range(bytes, offset) { layers.push(r); }
    if let Some(r) = paren_group(bytes, offset)    { layers.push(r); }
    if let Some(r) = clause_range(bytes, offset)   { layers.push(r); }
    if let Some(r) = statement_range(bytes, offset){ layers.push(r); }
    layers.push((0, n));

    // Build outer-first so each new wrapping becomes the innermost node,
    // pointing outward via `parent`. Final `chain` is the innermost
    // (identifier or smallest layer) -- that's what the client expects
    // as the SelectionRange root.
    let mut chain: Option<SelectionRange> = None;
    for (s, e) in layers.into_iter().rev() {
        let r = Range {
            start: byte_to_position(rope, s),
            end:   byte_to_position(rope, e),
        };
        chain = Some(SelectionRange { range: r, parent: chain.map(Box::new) });
    }
    chain.unwrap_or(SelectionRange {
        range: Range {
            start: Position { line: 0, character: 0 },
            end:   Position { line: 0, character: 0 },
        },
        parent: None,
    })
}

fn identifier_range(bytes: &[u8], offset: usize) -> Option<(usize, usize)> {
    let mut s = offset;
    while s > 0 && is_word(bytes[s - 1] as char) { s -= 1; }
    let mut e = offset;
    while e < bytes.len() && is_word(bytes[e] as char) { e += 1; }
    if s == e { return None; }
    Some((s, e))
}

fn paren_group(bytes: &[u8], offset: usize) -> Option<(usize, usize)> {
    // Walk back to find the unmatched `(`.
    let mut depth = 0i32;
    let mut i = offset;
    let open;
    loop {
        if i == 0 { return None; }
        i -= 1;
        match bytes[i] {
            b')' => depth += 1,
            b'(' => {
                if depth == 0 { open = i; break; }
                depth -= 1;
            }
            _ => {}
        }
    }
    // Walk forward to matching `)`.
    let mut depth = 1i32;
    let mut j = open + 1;
    while j < bytes.len() {
        match bytes[j] {
            b'(' => depth += 1,
            b')' => { depth -= 1; if depth == 0 { return Some((open, j + 1)); } }
            _ => {}
        }
        j += 1;
    }
    None
}

fn clause_range(bytes: &[u8], offset: usize) -> Option<(usize, usize)> {
    const CLAUSE_KW: &[&[u8]] = &[
        b"SELECT", b"FROM", b"WHERE", b"GROUP BY", b"ORDER BY", b"HAVING",
        b"LIMIT", b"OFFSET", b"VALUES", b"SET", b"USING", b"RETURNING",
        b"WITH", b"UNION", b"INTERSECT", b"EXCEPT", b"JOIN", b"ON",
    ];
    // Walk backwards to find the latest clause keyword start.
    for i in (0..offset).rev() {
        for kw in CLAUSE_KW {
            if i + kw.len() <= bytes.len()
                && bytes[i..i + kw.len()].eq_ignore_ascii_case(kw)
                && (i == 0 || !is_word(bytes[i - 1] as char))
                && (i + kw.len() >= bytes.len() || !is_word(bytes[i + kw.len()] as char))
            {
                let end = next_clause_or_semicolon(bytes, i + kw.len(), CLAUSE_KW);
                if end > i { return Some((i, end)); }
            }
        }
    }
    None
}

fn next_clause_or_semicolon(bytes: &[u8], from: usize, kws: &[&[u8]]) -> usize {
    let mut i = from;
    let mut depth = 0i32;
    while i < bytes.len() {
        match bytes[i] {
            b'(' => { depth += 1; i += 1; continue; }
            b')' => { depth -= 1; i += 1; continue; }
            b';' if depth == 0 => return i,
            _ => {}
        }
        if depth == 0 {
            for kw in kws {
                if i + kw.len() <= bytes.len()
                    && bytes[i..i + kw.len()].eq_ignore_ascii_case(kw)
                    && (i == 0 || !is_word(bytes[i - 1] as char))
                    && (i + kw.len() >= bytes.len() || !is_word(bytes[i + kw.len()] as char))
                {
                    return i;
                }
            }
        }
        i += 1;
    }
    bytes.len()
}

fn statement_range(bytes: &[u8], offset: usize) -> Option<(usize, usize)> {
    // Walk back to the previous `;` or start of buffer.
    let mut s = offset;
    while s > 0 {
        if bytes[s - 1] == b';' { break; }
        s -= 1;
    }
    while s < bytes.len() && (bytes[s] as char).is_whitespace() { s += 1; }
    // Walk forward to the next `;` or end of buffer.
    let mut e = offset;
    while e < bytes.len() && bytes[e] != b';' { e += 1; }
    if e < bytes.len() { e += 1; } // include the `;`
    if e <= s { return None; }
    Some((s, e))
}

fn is_word(c: char) -> bool { c.is_alphanumeric() || c == '_' }

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
