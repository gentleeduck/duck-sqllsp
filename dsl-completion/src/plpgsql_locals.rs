//! Extract PL/pgSQL function parameters and DECLARE'd locals from the
//! buffer text so completion can surface them inside a function body.
//!
//! Approach: walk backwards from `offset` to the most recent
//! `CREATE [OR REPLACE] FUNCTION|PROCEDURE <name> (...)` header. Parse
//! its parameter list, then continue forward to find any `DECLARE ... BEGIN`
//! blocks before the cursor. Both contribute completion items.

use crate::item::{Item, ItemKind};

#[derive(Debug, Default, Clone)]
pub struct Locals {
    pub params: Vec<(String, String)>,    // (name, type)
    pub decls:  Vec<(String, String)>,    // (name, type)
}

/// Scan the buffer for the function header that encloses `offset`. When
/// the cursor sits outside any function the returned struct is empty.
pub fn extract(source: &str, offset: usize) -> Locals {
    let bytes = source.as_bytes();
    let cursor = offset.min(bytes.len());
    let mut out = Locals::default();

    // Find the most recent CREATE FUNCTION / PROCEDURE header at or
    // before the cursor. Case-insensitive, whole-word.
    let upper = source.to_ascii_uppercase();
    let mut header_start: Option<usize> = None;
    for needle in ["CREATE OR REPLACE FUNCTION ", "CREATE OR REPLACE PROCEDURE ",
                   "CREATE FUNCTION ", "CREATE PROCEDURE "] {
        let mut from = 0usize;
        while let Some(rel) = upper[from..].find(needle) {
            let pos = from + rel;
            if pos > cursor { break; }
            // Boundary check on preceding char.
            let prev_ok = pos == 0 || !(bytes[pos - 1].is_ascii_alphanumeric() || bytes[pos - 1] == b'_');
            if prev_ok {
                header_start = Some(pos);
            }
            from = pos + needle.len();
        }
    }
    let Some(start) = header_start else { return out };

    // The header parameter list lives between the first `(` after start
    // and its matching `)`. Read it.
    if let Some(open) = source[start..].find('(') {
        let open_abs = start + open;
        if let Some(close_abs) = match_paren(bytes, open_abs) {
            let params_text = &source[open_abs + 1..close_abs];
            out.params = parse_param_list(params_text);
            // Now look for DECLARE ... BEGIN between close_abs and cursor.
            out.decls = scan_decls(&source[close_abs..cursor.max(close_abs)]);
        }
    }
    out
}

/// Match the closing `)` for the `(` at `open`. Returns None on mismatch.
fn match_paren(bytes: &[u8], open: usize) -> Option<usize> {
    if open >= bytes.len() || bytes[open] != b'(' { return None; }
    let mut depth = 0i32;
    let mut i = open;
    let n = bytes.len();
    while i < n {
        match bytes[i] {
            b'(' => depth += 1,
            b')' => { depth -= 1; if depth == 0 { return Some(i); } }
            b'\'' => {
                i += 1;
                while i < n {
                    if bytes[i] == b'\'' { i += 1; break; }
                    i += 1;
                }
                continue;
            }
            _ => {}
        }
        i += 1;
    }
    None
}

/// Parse a parameter list like `p_user_id UUID, p_email text DEFAULT ''`.
/// Skips arg modes (IN / OUT / INOUT / VARIADIC).
fn parse_param_list(s: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for part in split_top_level(s, ',') {
        let trimmed = part.trim();
        if trimmed.is_empty() { continue; }
        // Strip any trailing `DEFAULT <expr>` / `= <expr>` so we keep just
        // the name + type.
        let head = trimmed
            .split_once(" DEFAULT ")
            .map(|(h, _)| h)
            .unwrap_or(trimmed);
        let head = head.split_once(" default ").map(|(h, _)| h).unwrap_or(head);
        let head = head.split_once(" = ").map(|(h, _)| h).unwrap_or(head);
        let mut tokens = head.split_whitespace().peekable();
        // Skip arg mode prefix
        if let Some(first) = tokens.peek() {
            let up = first.to_ascii_uppercase();
            if matches!(up.as_str(), "IN" | "OUT" | "INOUT" | "VARIADIC") {
                tokens.next();
            }
        }
        let Some(name) = tokens.next() else { continue; };
        let ty: String = tokens.collect::<Vec<_>>().join(" ");
        if !ty.is_empty() {
            out.push((name.to_string(), ty));
        }
    }
    out
}

/// Walk forward from `body` (the text between the closing param `)` and
/// the cursor) collecting DECLARE'd variables. Multiple DECLARE blocks
/// inside nested BEGIN/END are accumulated.
fn scan_decls(body: &str) -> Vec<(String, String)> {
    let bytes = body.as_bytes();
    let n = bytes.len();
    let upper = body.to_ascii_uppercase();
    let mut out = Vec::new();
    let mut from = 0usize;
    while let Some(rel) = upper[from..].find("DECLARE") {
        let s = from + rel;
        // Boundary check.
        let prev_ok = s == 0 || !(bytes[s - 1].is_ascii_alphanumeric() || bytes[s - 1] == b'_');
        let after = s + "DECLARE".len();
        let next_ok = after >= n || !(bytes[after].is_ascii_alphanumeric() || bytes[after] == b'_');
        if !prev_ok || !next_ok { from = after; continue; }
        // Find the matching BEGIN that ends this DECLARE block (case-insensitive).
        let block_end = upper[after..].find("BEGIN").map(|p| after + p).unwrap_or(n);
        let decl_body = &body[after..block_end];
        out.extend(parse_decl_body(decl_body));
        from = block_end;
    }
    out
}

/// Each declaration ends with `;`. Pattern: `<name> [CONSTANT] <type> [DEFAULT|:= expr]`.
fn parse_decl_body(s: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for stmt in s.split(';') {
        let trimmed = stmt.trim();
        if trimmed.is_empty() { continue; }
        // Strip default expression: anything after `:=` or `DEFAULT`.
        let head = trimmed
            .split_once(":=").map(|(h, _)| h)
            .or_else(|| trimmed.split_once(" DEFAULT ").map(|(h, _)| h))
            .or_else(|| trimmed.split_once(" default ").map(|(h, _)| h))
            .unwrap_or(trimmed);
        let mut tokens = head.split_whitespace().peekable();
        let Some(name) = tokens.next() else { continue; };
        // Skip CONSTANT keyword.
        if let Some(t) = tokens.peek() {
            if t.eq_ignore_ascii_case("CONSTANT") { tokens.next(); }
        }
        let ty: String = tokens.collect::<Vec<_>>().join(" ");
        if !ty.is_empty() {
            out.push((name.to_string(), ty));
        }
    }
    out
}

/// Split `s` on `sep` at parenthesis depth 0, respecting single-quoted
/// strings. Used by `parse_param_list`.
fn split_top_level(s: &str, sep: char) -> Vec<String> {
    let bytes = s.as_bytes();
    let n = bytes.len();
    let mut out = Vec::new();
    let mut depth = 0i32;
    let mut start = 0usize;
    let mut i = 0usize;
    while i < n {
        let c = bytes[i] as char;
        match c {
            '(' => depth += 1,
            ')' => depth -= 1,
            '\'' => {
                i += 1;
                while i < n {
                    if bytes[i] == b'\'' { i += 1; break; }
                    i += 1;
                }
                continue;
            }
            _ if c == sep && depth == 0 => {
                out.push(s[start..i].to_string());
                start = i + 1;
            }
            _ => {}
        }
        i += 1;
    }
    out.push(s[start..].to_string());
    out
}

/// Look up the declared type of a PL/pgSQL local by name. Searches
/// parameters first, then DECLARE'd locals. Case-insensitive match.
/// Returns the raw type string (`users`, `INT`, `promo_codes%ROWTYPE`,
/// etc.) so callers can decide what to do with it.
pub fn type_of(locals: &Locals, name: &str) -> Option<String> {
    for (n, t) in locals.params.iter().chain(locals.decls.iter()) {
        if n.eq_ignore_ascii_case(name) {
            return Some(t.clone());
        }
    }
    None
}

/// Push `Locals` as completion items.
pub fn push_items(locals: &Locals, out: &mut Vec<Item>) {
    for (name, ty) in &locals.params {
        out.push(Item {
            label: name.clone(),
            kind: ItemKind::Parameter,
            detail: Some(ty.clone()),
            description: Some("parameter".into()),
            documentation_md: Some(format!("**{name}**\n\nfunction parameter — `{ty}`")),
            insert_text: name.clone(),
            is_snippet: false,
            sort_priority: 5,
        });
    }
    for (name, ty) in &locals.decls {
        out.push(Item {
            label: name.clone(),
            kind: ItemKind::Variable,
            detail: Some(ty.clone()),
            description: Some("local".into()),
            documentation_md: Some(format!("**{name}**\n\nlocal — `{ty}` (declared in this function)")),
            insert_text: name.clone(),
            is_snippet: false,
            sort_priority: 5,
        });
    }
}
