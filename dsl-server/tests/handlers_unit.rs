//! Unit tests for the per-handler logic without spinning up the LSP wire.

use dsl_server::{
    documents::DocumentStore,
    handlers::{completion, hover, inlay_hints, references, rename, selection_range, signature_help, workspace_symbol},
    state::ServerState,
};
use tower_lsp::lsp_types::{
    CompletionParams, CompletionResponse, HoverParams, PartialResultParams, Position,
    TextDocumentIdentifier, TextDocumentPositionParams, Url, WorkDoneProgressParams,
};

fn state_with(uri: &str, text: &str) -> (ServerState, Url) {
    let state = ServerState::new();
    let url: Url = uri.parse().unwrap();
    state.documents.open(url.clone(), text.into(), 1);
    (state, url)
}

#[test]
fn completion_returns_keywords_for_prefix() {
    let (state, url) = state_with("file:///t.sql", "SEL");
    let resp = completion::run(&state, CompletionParams {
        text_document_position: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri: url },
            position: Position { line: 0, character: 3 },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
        context: None,
    }).expect("completion result");
    let items = match resp {
        CompletionResponse::Array(v) => v,
        CompletionResponse::List(l) => l.items,
    };
    // Phase::Start emits only statement-starter keywords; FROM is a
    // mid-statement clause and should NOT appear at the buffer start.
    assert!(items.iter().any(|i| i.label == "SELECT"));
    assert!(items.iter().any(|i| i.label == "INSERT INTO"));
    assert!(!items.iter().any(|i| i.label == "FROM"));
}

#[test]
fn completion_handles_dot_context() {
    let (state, url) = state_with("file:///t.sql", "SELECT u. FROM users u");
    let resp = completion::run(&state, CompletionParams {
        text_document_position: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri: url },
            position: Position { line: 0, character: 9 }, // after the dot
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
        context: None,
    }).expect("completion result");
    let items = match resp {
        CompletionResponse::Array(v) => v,
        CompletionResponse::List(l) => l.items,
    };
    // Empty catalog -> 0 column items; assert no keywords leak through.
    assert!(items.is_empty(), "expected only columns of `u` (empty cat), got {:?}",
            items.iter().map(|i| &i.label).collect::<Vec<_>>());
}

#[test]
fn hover_returns_none_outside_known_tokens() {
    let (state, url) = state_with("file:///t.sql", "frobnicate_xyz");
    let resp = hover::run(&state, HoverParams {
        text_document_position_params: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri: url },
            position: Position { line: 0, character: 1 },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
    });
    assert!(resp.is_none());
}

#[test]
fn hover_returns_keyword_docs() {
    let (state, url) = state_with("file:///t.sql", "SELECT 1");
    let resp = hover::run(&state, HoverParams {
        text_document_position_params: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri: url },
            position: Position { line: 0, character: 3 },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
    });
    let h = resp.expect("hover result");
    // Hover now ships as MarkedString[] when the content has fenced
    // SQL; falls back to Markup for plain markdown. Either way the
    // serialised text should contain the keyword doc.
    let text = match h.contents {
        tower_lsp::lsp_types::HoverContents::Markup(m) => m.value,
        tower_lsp::lsp_types::HoverContents::Array(parts) => parts.into_iter().map(|p| match p {
            tower_lsp::lsp_types::MarkedString::String(s) => s,
            tower_lsp::lsp_types::MarkedString::LanguageString(ls) => ls.value,
        }).collect::<Vec<_>>().join("\n"),
        tower_lsp::lsp_types::HoverContents::Scalar(p) => match p {
            tower_lsp::lsp_types::MarkedString::String(s) => s,
            tower_lsp::lsp_types::MarkedString::LanguageString(ls) => ls.value,
        },
    };
    assert!(text.contains("Retrieve"), "got: {text}");
}

#[test]
fn references_skips_strings_and_comments() {
    let src = "SELECT id FROM products -- products in comment\n\
               WHERE name = 'products' AND id IN (SELECT id FROM products);";
    let hits = references::find_word_occurrences(src, "products");
    assert_eq!(hits.len(), 2, "expected 2 real refs, comment+string excluded");
}

#[test]
fn references_skips_dollar_quoted_bodies() {
    let src = "CREATE FUNCTION f() AS $$ products $$ LANGUAGE sql;\n\
               SELECT * FROM products;";
    let hits = references::find_word_occurrences(src, "products");
    assert_eq!(hits.len(), 1, "dollar-quoted body should be excluded");
}

#[test]
fn references_matches_quoted_identifier_case_insensitively() {
    let src = "CREATE TABLE \"Products\" (id INT);\nSELECT * FROM products;";
    let hits = references::find_word_occurrences(src, "products");
    assert_eq!(hits.len(), 2);
}

#[test]
fn rename_returns_workspace_edit() {
    let (state, url) = state_with(
        "file:///r.sql",
        "CREATE TABLE products (id INT);\nSELECT * FROM products;",
    );
    use tower_lsp::lsp_types::{
        RenameParams, TextDocumentIdentifier, TextDocumentPositionParams,
    };
    let edits = rename::run(&state, RenameParams {
        text_document_position: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri: url.clone() },
            position: Position { line: 0, character: 14 },
        },
        new_name: "items".into(),
        work_done_progress_params: WorkDoneProgressParams::default(),
    }).expect("rename result");
    let changes = edits.changes.expect("changes map");
    assert_eq!(changes.get(&url).unwrap().len(), 2);
}

#[test]
fn rename_rejects_invalid_identifier() {
    let (state, url) = state_with("file:///r.sql", "CREATE TABLE products (id INT);");
    use tower_lsp::lsp_types::{
        RenameParams, TextDocumentIdentifier, TextDocumentPositionParams,
    };
    let edits = rename::run(&state, RenameParams {
        text_document_position: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri: url },
            position: Position { line: 0, character: 14 },
        },
        new_name: "1bad".into(), // starts with digit
        work_done_progress_params: WorkDoneProgressParams::default(),
    });
    assert!(edits.is_none(), "must reject identifier starting with digit");
}

#[test]
fn inlay_expands_select_star_against_buffer_table() {
    use tower_lsp::lsp_types::{InlayHintParams, Range, TextDocumentIdentifier, WorkDoneProgressParams};
    let src = "CREATE TABLE t (a INT, b INT);\nSELECT * FROM t;";
    let (state, url) = state_with("file:///i.sql", src);
    let hints = inlay_hints::run(&state, InlayHintParams {
        text_document: TextDocumentIdentifier { uri: url },
        range: Range {
            start: Position { line: 0, character: 0 },
            end:   Position { line: 5, character: 0 },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
    }).expect("inlay");
    assert_eq!(hints.len(), 1);
    match &hints[0].label {
        tower_lsp::lsp_types::InlayHintLabel::String(s) => assert!(s.contains("a") && s.contains("b")),
        _ => panic!("expected string label"),
    }
}

#[test]
fn selection_range_emits_innermost_first() {
    use tower_lsp::lsp_types::{SelectionRangeParams, TextDocumentIdentifier, WorkDoneProgressParams, PartialResultParams};
    let src = "SELECT id FROM users WHERE id = 1;";
    let (state, url) = state_with("file:///sr.sql", src);
    let r = selection_range::run(&state, SelectionRangeParams {
        text_document: TextDocumentIdentifier { uri: url },
        positions: vec![Position { line: 0, character: 8 }],
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
    }).expect("selection range");
    assert_eq!(r.len(), 1);
    let inner = &r[0];
    let inner_text_len = inner.range.end.character - inner.range.start.character;
    let parent = inner.parent.as_ref().expect("has parent");
    let parent_text_len = parent.range.end.character - parent.range.start.character;
    assert!(parent_text_len >= inner_text_len, "parent must be at least as wide as inner");
}

#[test]
fn workspace_symbol_surfaces_buffer_table() {
    use tower_lsp::lsp_types::{WorkspaceSymbolParams, WorkDoneProgressParams, PartialResultParams};
    let (state, _url) = state_with(
        "file:///ws.sql",
        "CREATE TABLE accounts (id UUID, balance NUMERIC);",
    );
    let syms = workspace_symbol::run(&state, WorkspaceSymbolParams {
        query: "accounts".into(),
        partial_result_params: PartialResultParams::default(),
        work_done_progress_params: WorkDoneProgressParams::default(),
    }).expect("symbols");
    assert!(syms.iter().any(|s| s.name == "accounts"));
}

#[test]
fn signature_help_picks_active_param() {
    use tower_lsp::lsp_types::{SignatureHelpParams, TextDocumentIdentifier, TextDocumentPositionParams, WorkDoneProgressParams};
    let src = "SELECT coalesce(name, 'unknown') FROM users;";
    let (state, url) = state_with("file:///sh.sql", src);
    // Right after the comma -> active index 1
    let r = signature_help::run(&state, SignatureHelpParams {
        text_document_position_params: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri: url },
            position: Position { line: 0, character: 22 },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        context: None,
    }).expect("signature");
    assert_eq!(r.active_parameter, Some(1));
}

#[test]
fn signature_help_for_length_renders_signature() {
    use tower_lsp::lsp_types::{SignatureHelpParams, TextDocumentIdentifier, TextDocumentPositionParams, WorkDoneProgressParams};
    let src = "SELECT length() FROM users;";
    let (state, url) = state_with("file:///sh-len.sql", src);
    // Cursor inside the `(`.
    let r = signature_help::run(&state, SignatureHelpParams {
        text_document_position_params: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri: url },
            position: Position { line: 0, character: 14 },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        context: None,
    }).expect("length signature");
    let sig = &r.signatures[0];
    assert!(sig.label.to_ascii_lowercase().contains("length"),
        "label should contain `length`; got: {}", sig.label);
    assert!(sig.label.to_ascii_lowercase().contains("text"),
        "label should mention text arg; got: {}", sig.label);
}

#[test]
fn signature_help_for_char_length_renders_signature() {
    use tower_lsp::lsp_types::{SignatureHelpParams, TextDocumentIdentifier, TextDocumentPositionParams, WorkDoneProgressParams};
    let src = "SELECT char_length() FROM users;";
    let (state, url) = state_with("file:///sh-cl.sql", src);
    let r = signature_help::run(&state, SignatureHelpParams {
        text_document_position_params: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri: url },
            position: Position { line: 0, character: 19 },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        context: None,
    }).expect("char_length signature");
    let sig = &r.signatures[0];
    assert!(sig.label.to_ascii_lowercase().contains("char_length"));
}

#[test]
fn document_store_roundtrip() {
    let store = DocumentStore::default();
    let url: Url = "file:///x.sql".parse().unwrap();
    store.open(url.clone(), "hello".into(), 1);
    assert_eq!(store.get(&url).unwrap().text, "hello");
    store.update(&url, "world".into(), 2);
    assert_eq!(store.get(&url).unwrap().text, "world");
    store.close(&url);
    assert!(store.get(&url).is_none());
}
