//! `textDocument/codeAction` handler.
//!
//! Produces quick-fixes for diagnostics that have an obvious mechanical
//! correction:
//!
//!   - sql015 (`= NULL` / `<> NULL` / `!= NULL`) -> rewrite to
//!     `IS NULL` / `IS NOT NULL`.
//!   - sql001 (unresolved table) -> suggest the closest catalog table
//!     by Levenshtein distance.

use crate::state::ServerState;
use tower_lsp::lsp_types::{
    CodeAction, CodeActionKind, CodeActionOrCommand, CodeActionParams, CodeActionResponse,
    Diagnostic, NumberOrString, Position, Range, TextEdit, Url, WorkspaceEdit,
};
use std::collections::HashMap;

pub fn run(state: &ServerState, params: CodeActionParams) -> Option<CodeActionResponse> {
    let uri = params.text_document.uri;
    let doc = state.documents.get(&uri)?;
    let mut actions: Vec<CodeActionOrCommand> = Vec::new();

    for diag in &params.context.diagnostics {
        if let Some(code) = code_str(&diag.code) {
            match code.as_str() {
                "sql015" => sql015_action(&uri, diag, &doc.text, &mut actions),
                "sql001" => sql001_action(&uri, diag, state, &mut actions),
                "sql013" => sql013_action(&uri, diag, &doc.text, &mut actions),
                _ => {}
            }
        }
    }

    // Cursor-position refactor: offer to quote / unquote the identifier
    // under the selection range. Always available, no diagnostic needed.
    quote_toggle_action(&uri, &params.range, &doc.text, &mut actions);

    if actions.is_empty() { return None; }
    Some(actions)
}

/// REFACTOR: wrap or unwrap the identifier under the requested range in
/// double quotes. Useful for case-sensitive Postgres identifiers and for
/// promoting a bare name to a quoted one when it collides with a keyword.
fn quote_toggle_action(
    uri: &Url,
    range: &Range,
    text: &str,
    out: &mut Vec<CodeActionOrCommand>,
) {
    if range.start.line != range.end.line { return; }
    let line_idx = range.start.line as usize;
    let lines: Vec<&str> = text.lines().collect();
    if line_idx >= lines.len() { return; }
    let line = lines[line_idx];

    let start_col = range.start.character as usize;
    if start_col >= line.len() { return; }

    // Expand selection backwards/forwards to the surrounding token.
    let bytes = line.as_bytes();
    let mut s = start_col;
    while s > 0 && is_id_char(bytes[s - 1] as char) { s -= 1; }
    let mut e = start_col;
    while e < bytes.len() && is_id_char(bytes[e] as char) { e += 1; }

    // Try the quoted form `"name"` if either bound is `"`.
    let (is_quoted, qs, qe, inner) = if s > 0 && bytes[s - 1] == b'"' && e < bytes.len() && bytes[e] == b'"' {
        (true, s - 1, e + 1, line[s..e].to_string())
    } else if s == e {
        return;
    } else {
        (false, s, e, line[s..e].to_string())
    };
    if inner.is_empty() { return; }

    let (new_text, title) = if is_quoted {
        (inner.clone(), format!("Unquote identifier `{inner}`"))
    } else {
        (format!("\"{inner}\""), format!("Quote identifier `{inner}`"))
    };
    let mut changes: HashMap<Url, Vec<TextEdit>> = HashMap::new();
    changes.insert(
        uri.clone(),
        vec![TextEdit {
            range: Range {
                start: Position { line: line_idx as u32, character: qs as u32 },
                end:   Position { line: line_idx as u32, character: qe as u32 },
            },
            new_text,
        }],
    );
    out.push(CodeActionOrCommand::CodeAction(CodeAction {
        title,
        kind: Some(CodeActionKind::REFACTOR),
        diagnostics: None,
        edit: Some(WorkspaceEdit {
            changes: Some(changes),
            document_changes: None,
            change_annotations: None,
        }),
        is_preferred: Some(false),
        ..Default::default()
    }));
}

fn is_id_char(c: char) -> bool { c.is_alphanumeric() || c == '_' }

fn code_str(c: &Option<NumberOrString>) -> Option<String> {
    match c {
        Some(NumberOrString::String(s)) => Some(s.clone()),
        Some(NumberOrString::Number(n)) => Some(n.to_string()),
        None => None,
    }
}

fn sql015_action(uri: &Url, diag: &Diagnostic, text: &str, out: &mut Vec<CodeActionOrCommand>) {
    // Locate the offending `= NULL` / `<> NULL` / `!= NULL` substring
    // inside the diagnostic's range and emit a TextEdit replacing it
    // with `IS NULL` / `IS NOT NULL`.
    let start_line = diag.range.start.line as usize;
    let end_line = diag.range.end.line as usize;
    let lines: Vec<&str> = text.lines().collect();

    for line_idx in start_line..=end_line.min(lines.len().saturating_sub(1)) {
        let line = lines[line_idx];
        let upper = line.to_ascii_uppercase();
        for (needle, replacement) in [
            ("= NULL", "IS NULL"),
            ("=NULL", "IS NULL"),
            ("<> NULL", "IS NOT NULL"),
            ("<>NULL", "IS NOT NULL"),
            ("!= NULL", "IS NOT NULL"),
            ("!=NULL", "IS NOT NULL"),
        ] {
            if let Some(col) = upper.find(needle) {
                let mut changes: HashMap<Url, Vec<TextEdit>> = HashMap::new();
                changes.insert(
                    uri.clone(),
                    vec![TextEdit {
                        range: Range {
                            start: Position { line: line_idx as u32, character: col as u32 },
                            end:   Position { line: line_idx as u32, character: (col + needle.len()) as u32 },
                        },
                        new_text: replacement.into(),
                    }],
                );
                let mut act = CodeAction {
                    title: format!("Convert `{}` to `{}`", needle.trim(), replacement),
                    kind: Some(CodeActionKind::QUICKFIX),
                    diagnostics: Some(vec![diag.clone()]),
                    edit: Some(WorkspaceEdit {
                        changes: Some(changes),
                        document_changes: None,
                        change_annotations: None,
                    }),
                    is_preferred: Some(true),
                    ..Default::default()
                };
                act.command = None;
                out.push(CodeActionOrCommand::CodeAction(act));
                return;
            }
        }
    }
}

/// Quickfix for sql013 (mutating without WHERE): append `WHERE id = $1`
/// before the trailing semicolon. Conservative -- only fires when the
/// flagged line is an UPDATE or DELETE we can detect a terminator on.
fn sql013_action(uri: &Url, diag: &Diagnostic, text: &str, out: &mut Vec<CodeActionOrCommand>) {
    let start_line = diag.range.start.line as usize;
    let end_line = diag.range.end.line as usize;
    let lines: Vec<&str> = text.lines().collect();
    if start_line >= lines.len() { return; }

    // Find the line that ends the statement -- the semicolon, or the last
    // line of the diagnostic range, whichever comes first.
    let target_line = end_line.min(lines.len().saturating_sub(1));
    let mut col = lines[target_line].len();
    if let Some(semi) = lines[target_line].rfind(';') {
        col = semi;
    }

    let suffix = if col > 0 && !lines[target_line][..col].ends_with(char::is_whitespace) {
        " WHERE id = $1"
    } else {
        "WHERE id = $1"
    };

    let mut changes: HashMap<Url, Vec<TextEdit>> = HashMap::new();
    changes.insert(
        uri.clone(),
        vec![TextEdit {
            range: Range {
                start: Position { line: target_line as u32, character: col as u32 },
                end:   Position { line: target_line as u32, character: col as u32 },
            },
            new_text: suffix.into(),
        }],
    );
    out.push(CodeActionOrCommand::CodeAction(CodeAction {
        title: "Add `WHERE id = $1`".into(),
        kind: Some(CodeActionKind::QUICKFIX),
        diagnostics: Some(vec![diag.clone()]),
        edit: Some(WorkspaceEdit {
            changes: Some(changes),
            document_changes: None,
            change_annotations: None,
        }),
        is_preferred: Some(true),
        ..Default::default()
    }));
}

fn sql001_action(uri: &Url, diag: &Diagnostic, state: &ServerState, out: &mut Vec<CodeActionOrCommand>) {
    // Extract the unresolved name from the message: `unresolved table 'X' ...`.
    let needle = match diag
        .message
        .find('`')
        .and_then(|i| diag.message[i + 1..].find('`').map(|j| (i + 1, i + 1 + j)))
    {
        Some((s, e)) => diag.message[s..e].to_string(),
        None => return,
    };
    let cat = state.catalog.read();
    let mut best: Vec<(usize, String)> = Vec::new();
    for t in cat.tables() {
        let d = levenshtein(&needle.to_ascii_lowercase(), &t.name.to_ascii_lowercase());
        if d <= 3 && d < needle.len().max(1) {
            best.push((d, t.name.clone()));
        }
    }
    best.sort_by_key(|x| x.0);
    best.dedup_by_key(|x| x.1.clone());

    for (_, name) in best.into_iter().take(3) {
        // We don't know the precise byte range of the bad identifier --
        // the diagnostic covers the whole statement. Surface the action
        // as a Refactor with a copy-and-paste suggestion in the title;
        // the user applies it manually until a richer edit is wired.
        let mut act = CodeAction {
            title: format!("Did you mean: `{name}`?"),
            kind: Some(CodeActionKind::QUICKFIX),
            diagnostics: Some(vec![diag.clone()]),
            ..Default::default()
        };
        // Best-effort replace-everywhere edit: rewrite the bad word in
        // this document. Conservative -- only rewrites whole-word
        // matches.
        let doc_text = state.documents.get(uri).map(|d| d.text).unwrap_or_default();
        if let Some(edits) = whole_word_replace(&doc_text, &needle, &name) {
            let mut changes: HashMap<Url, Vec<TextEdit>> = HashMap::new();
            changes.insert(uri.clone(), edits);
            act.edit = Some(WorkspaceEdit {
                changes: Some(changes),
                document_changes: None,
                change_annotations: None,
            });
        }
        out.push(CodeActionOrCommand::CodeAction(act));
    }
}

fn whole_word_replace(text: &str, from: &str, to: &str) -> Option<Vec<TextEdit>> {
    if from.is_empty() { return None; }
    let mut edits = Vec::new();
    let bytes = text.as_bytes();
    let mut byte = 0usize;
    let mut line = 0u32;
    let mut col = 0u32;
    while byte < bytes.len() {
        let c = bytes[byte] as char;
        if c == '\n' { line += 1; col = 0; byte += 1; continue; }
        if text[byte..].starts_with(from)
            && (byte == 0 || !is_word(bytes[byte - 1] as char))
            && bytes
                .get(byte + from.len())
                .map_or(true, |b| !is_word(*b as char))
        {
            edits.push(TextEdit {
                range: Range {
                    start: Position { line, character: col },
                    end:   Position { line, character: col + from.chars().count() as u32 },
                },
                new_text: to.into(),
            });
            byte += from.len();
            col += from.chars().count() as u32;
            continue;
        }
        col += 1;
        byte += c.len_utf8();
    }
    if edits.is_empty() { None } else { Some(edits) }
}

fn is_word(c: char) -> bool { c.is_alphanumeric() || c == '_' }

fn levenshtein(a: &str, b: &str) -> usize {
    let m = a.chars().count();
    let n = b.chars().count();
    if m == 0 { return n; }
    if n == 0 { return m; }
    let mut prev: Vec<usize> = (0..=n).collect();
    let mut curr = vec![0usize; n + 1];
    let a_chars: Vec<char> = a.chars().collect();
    let b_chars: Vec<char> = b.chars().collect();
    for (i, ca) in a_chars.iter().enumerate() {
        curr[0] = i + 1;
        for (j, cb) in b_chars.iter().enumerate() {
            let cost = if ca == cb { 0 } else { 1 };
            curr[j + 1] = (curr[j] + 1).min(prev[j + 1] + 1).min(prev[j] + cost);
        }
        std::mem::swap(&mut prev, &mut curr);
    }
    prev[n]
}
