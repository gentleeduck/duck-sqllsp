//! `textDocument/signatureHelp` handler.
//!
//! When the cursor is inside `fn(arg1, arg2|)`, we want to display the
//! function's formal signature with the active parameter highlighted.
//! Approach:
//!   1. Walk backwards from the cursor looking for the unmatched `(`.
//!   2. Read the identifier immediately before that `(` -- that's the
//!      function name. Bail if not an identifier (e.g. tuple paren).
//!   3. Count commas between the open paren and the cursor (skipping
//!      nested parens / strings) -> active parameter index.
//!   4. Look up the signature in dsl-knowledge.functions, then fall back
//!      to the catalog functions returned from the live DB.

use crate::handlers::position;
use crate::state::ServerState;
use tower_lsp::lsp_types::{
    ParameterInformation, ParameterLabel, SignatureHelp, SignatureHelpParams, SignatureInformation,
};

pub fn run(state: &ServerState, params: SignatureHelpParams) -> Option<SignatureHelp> {
    let uri = params.text_document_position_params.text_document.uri;
    let doc = state.documents.get(&uri)?;
    let offset = position::to_offset(&doc.rope, params.text_document_position_params.position);
    let pos: usize = offset.into();
    let pos = pos.min(doc.text.len());

    let (open_paren, arg_index) = find_enclosing_call(&doc.text, pos)?;
    let name = identifier_before(&doc.text, open_paren)?;

    // Knowledge base first.
    let kb = dsl_knowledge::functions();
    if let Some(entry) = kb.get(name.to_ascii_lowercase().as_str()) {
        if let Some(sig) = entry.signature {
            return Some(build(sig, &name, entry.doc, arg_index));
        }
    }
    // Catalog (live DB) fallback.
    let cat = state.catalog.read();
    if let Some(f) = cat
        .functions
        .iter()
        .find(|f| f.name.eq_ignore_ascii_case(&name))
    {
        let args = f
            .arguments
            .iter()
            .map(|a| match &a.name {
                Some(n) => format!("{n} {}", a.data_type),
                None => a.data_type.clone(),
            })
            .collect::<Vec<_>>()
            .join(", ");
        let sig = format!("{}({args}) -> {}", f.name, f.return_type);
        let doc = f.comment.clone().unwrap_or_default();
        return Some(build_owned(sig, &name, doc, arg_index));
    }
    None
}

fn build(sig: &str, _name: &str, doc: &str, active: usize) -> SignatureHelp {
    let params = parse_params(sig);
    SignatureHelp {
        signatures: vec![SignatureInformation {
            label: sig.to_string(),
            documentation: Some(tower_lsp::lsp_types::Documentation::String(doc.to_string())),
            parameters: Some(params),
            active_parameter: Some(active as u32),
        }],
        active_signature: Some(0),
        active_parameter: Some(active as u32),
    }
}

fn build_owned(sig: String, _name: &str, doc: String, active: usize) -> SignatureHelp {
    let params = parse_params(&sig);
    SignatureHelp {
        signatures: vec![SignatureInformation {
            label: sig,
            documentation: Some(tower_lsp::lsp_types::Documentation::String(doc)),
            parameters: Some(params),
            active_parameter: Some(active as u32),
        }],
        active_signature: Some(0),
        active_parameter: Some(active as u32),
    }
}

/// From a signature like `to_char(value, format)`, return the comma-separated
/// parameter spans as inline labels (LSP highlights them by index).
fn parse_params(sig: &str) -> Vec<ParameterInformation> {
    let open = match sig.find('(') { Some(i) => i + 1, None => return Vec::new() };
    let close = match sig.rfind(')') { Some(i) => i, None => sig.len() };
    if close <= open { return Vec::new(); }
    let body = &sig[open..close];
    let mut out = Vec::new();
    let mut depth = 0i32;
    let mut last = 0usize;
    let bytes = body.as_bytes();
    for (i, b) in bytes.iter().enumerate() {
        match b {
            b'(' => depth += 1,
            b')' => depth -= 1,
            b',' if depth == 0 => {
                let span = body[last..i].trim();
                if !span.is_empty() {
                    out.push(ParameterInformation {
                        label: ParameterLabel::Simple(span.to_string()),
                        documentation: None,
                    });
                }
                last = i + 1;
            }
            _ => {}
        }
    }
    let tail = body[last..].trim();
    if !tail.is_empty() {
        out.push(ParameterInformation {
            label: ParameterLabel::Simple(tail.to_string()),
            documentation: None,
        });
    }
    out
}

/// Walk back from `cursor`, find the first un-closed `(`. Returns its
/// byte position and the comma index of the cursor inside the call.
fn find_enclosing_call(src: &str, cursor: usize) -> Option<(usize, usize)> {
    let bytes = src.as_bytes();
    let mut i = cursor;
    let mut depth = 0i32;
    let mut commas = 0usize;
    let mut in_string: Option<u8> = None;
    while i > 0 {
        i -= 1;
        let b = bytes[i];
        if let Some(q) = in_string {
            if b == q { in_string = None; }
            continue;
        }
        match b {
            b')' => depth += 1,
            b'(' => {
                if depth == 0 { return Some((i, commas)); }
                depth -= 1;
            }
            b',' if depth == 0 => commas += 1,
            b'\'' => in_string = Some(b'\''),
            _ => {}
        }
    }
    None
}

fn identifier_before(src: &str, paren_pos: usize) -> Option<String> {
    let bytes = src.as_bytes();
    // Skip whitespace immediately before `(`.
    let mut end = paren_pos;
    while end > 0 && (bytes[end - 1] as char).is_whitespace() { end -= 1; }
    let mut start = end;
    while start > 0 {
        let c = bytes[start - 1] as char;
        if c.is_alphanumeric() || c == '_' || c == '.' { start -= 1; } else { break; }
    }
    if start == end { return None; }
    let raw = src[start..end].to_string();
    // Strip schema qualifier `public.now()` -> `now`
    Some(raw.rsplit('.').next().unwrap_or(&raw).to_string())
}
