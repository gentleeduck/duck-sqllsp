//! Unit tests for the per-handler logic without spinning up the LSP wire.

use dsl_server::{
    documents::DocumentStore,
    handlers::{code_action, completion, document_symbol, hover, inlay_hints, references, rename, selection_range, signature_help, workspace_symbol},
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
fn no_op_update_keeps_parse_cache() {
    // didChange with byte-identical text (common: format-on-save with
    // nothing to format) should not invalidate the parse cache.
    let store = DocumentStore::default();
    let url: Url = "file:///x.sql".parse().unwrap();
    let src = "SELECT 1; SELECT 2;".to_string();
    store.open(url.clone(), src.clone(), 1);
    let before = store.get(&url).unwrap().parsed();
    store.update(&url, src.clone(), 2);
    let after = store.get(&url).unwrap().parsed();
    assert!(std::sync::Arc::ptr_eq(&before, &after), "Arc should be reused");
    assert_eq!(store.get(&url).unwrap().version, 2, "version bumps even on no-op");
}

#[test]
fn changed_update_invalidates_parse_cache() {
    let store = DocumentStore::default();
    let url: Url = "file:///x.sql".parse().unwrap();
    store.open(url.clone(), "SELECT 1;".into(), 1);
    let before = store.get(&url).unwrap().parsed();
    store.update(&url, "SELECT 2;".into(), 2);
    let after = store.get(&url).unwrap().parsed();
    assert!(!std::sync::Arc::ptr_eq(&before, &after), "real edit should reparse");
}

#[test]
fn references_walks_every_open_buffer() {
    use tower_lsp::lsp_types::{
        PartialResultParams, ReferenceContext, ReferenceParams, TextDocumentIdentifier,
        TextDocumentPositionParams, WorkDoneProgressParams,
    };
    let state = ServerState::new();
    let schema: Url = "file:///migrations/001_schema.sql".parse().unwrap();
    let seed: Url   = "file:///seeds/products.sql".parse().unwrap();
    let query: Url  = "file:///queries/list.sql".parse().unwrap();
    state.documents.open(schema.clone(), "CREATE TABLE products (id INT);".into(), 1);
    state.documents.open(seed.clone(),   "INSERT INTO products (id) VALUES (1);".into(), 1);
    state.documents.open(query.clone(),  "SELECT * FROM products WHERE products.id = 1;".into(), 1);

    let locs = references::run(&state, ReferenceParams {
        text_document_position: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri: schema.clone() },
            position: Position { line: 0, character: 16 }, // inside "products" in CREATE TABLE
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
        context: ReferenceContext { include_declaration: true },
    }).expect("ref result");

    let by_uri: std::collections::HashMap<_, usize> = locs.iter().fold(
        std::collections::HashMap::new(),
        |mut acc, l| { *acc.entry(l.uri.clone()).or_default() += 1; acc },
    );
    assert_eq!(by_uri.get(&schema).copied(), Some(1), "1 hit in schema (CREATE TABLE)");
    assert_eq!(by_uri.get(&seed).copied(),   Some(1), "1 hit in seed (INSERT INTO)");
    assert_eq!(by_uri.get(&query).copied(),  Some(2), "2 hits in query (FROM + WHERE qualifier)");
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
fn completion_snippet_item_has_expands_to_preview() {
    use tower_lsp::lsp_types::{
        CompletionParams, PartialResultParams, TextDocumentIdentifier,
        TextDocumentPositionParams, WorkDoneProgressParams, CompletionResponse,
        Documentation,
    };
    let (state, url) = state_with("file:///snip.sql", "");
    let resp = completion::run(&state, CompletionParams {
        text_document_position: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri: url },
            position: Position { line: 0, character: 0 },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
        context: None,
    }).expect("completion result");
    let items = match resp {
        CompletionResponse::Array(a) => a,
        CompletionResponse::List(l) => l.items,
    };
    let it = items.iter().find(|i| i.label.eq_ignore_ascii_case("ctable"))
        .expect("ctable snippet");
    let doc = it.documentation.as_ref().expect("snippet doc set");
    let text = match doc {
        Documentation::MarkupContent(m) => m.value.clone(),
        Documentation::String(s) => s.clone(),
    };
    assert!(text.contains("Expands to"),
        "snippet doc should preview the expansion; got: {text}");
    assert!(text.to_lowercase().contains("create table name"),
        "preview should show placeholder labels stripped of ${{}}; got: {text}");
}

#[test]
fn code_action_explain_analyze_wrap() {
    use tower_lsp::lsp_types::{
        CodeActionContext, CodeActionParams, CodeActionResponse,
        PartialResultParams, Range, TextDocumentIdentifier, WorkDoneProgressParams,
    };
    let src = "SELECT id FROM users WHERE id = '1';";
    let (state, url) = state_with("file:///ea.sql", src);
    let r = code_action::run(&state, CodeActionParams {
        text_document: TextDocumentIdentifier { uri: url },
        range: Range {
            start: Position { line: 0, character: 5 },
            end: Position { line: 0, character: 5 },
        },
        context: CodeActionContext { diagnostics: vec![], only: None, trigger_kind: None },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
    }).expect("actions");
    let titles: Vec<String> = r.iter().filter_map(|a| match a {
        tower_lsp::lsp_types::CodeActionOrCommand::CodeAction(ca) => Some(ca.title.clone()),
        _ => None,
    }).collect();
    assert!(titles.iter().any(|t| t.contains("EXPLAIN ANALYZE")),
        "expected EXPLAIN ANALYZE wrap action; got: {titles:?}");
    let _: CodeActionResponse = r;
}

#[test]
fn signature_help_for_update_set_tuple() {
    use tower_lsp::lsp_types::{SignatureHelpParams, TextDocumentIdentifier, TextDocumentPositionParams, WorkDoneProgressParams};
    let src = "UPDATE users SET (id, email) = ();";
    let (state, url) = state_with("file:///us.sql", src);
    let cur = src.find(") = (").unwrap() + 5;
    let r = signature_help::run(&state, SignatureHelpParams {
        text_document_position_params: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri: url },
            position: Position { line: 0, character: cur as u32 },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        context: None,
    }).expect("UPDATE SET sig");
    let sig = &r.signatures[0];
    assert!(sig.label.contains("SET"), "label: {}", sig.label);
    assert!(sig.label.contains("id"));
    assert!(sig.label.contains("email"));
}

#[test]
fn signature_help_for_insert_values_explicit_columns() {
    use tower_lsp::lsp_types::{SignatureHelpParams, TextDocumentIdentifier, TextDocumentPositionParams, WorkDoneProgressParams};
    let src = "INSERT INTO users (id, email) VALUES ();";
    let (state, url) = state_with("file:///iv.sql", src);
    // Cursor right after the opening `(` of VALUES.
    let cur = src.find("VALUES (").unwrap() + 8;
    let r = signature_help::run(&state, SignatureHelpParams {
        text_document_position_params: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri: url },
            position: Position { line: 0, character: cur as u32 },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        context: None,
    }).expect("INSERT VALUES sig");
    let sig = &r.signatures[0];
    assert!(sig.label.contains("VALUES"), "label: {}", sig.label);
    assert!(sig.label.contains("id"), "label should list id: {}", sig.label);
    assert!(sig.label.contains("email"), "label should list email: {}", sig.label);
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
fn document_symbol_nests_columns_and_constraints_under_table() {
    use tower_lsp::lsp_types::{
        DocumentSymbolParams, DocumentSymbolResponse, PartialResultParams,
        TextDocumentIdentifier, WorkDoneProgressParams,
    };
    let src = "CREATE TABLE t (\n  id uuid NOT NULL PRIMARY KEY,\n  email text NOT NULL,\n  CONSTRAINT uq_t_email UNIQUE (email),\n  CHECK (length(email) > 3)\n);";
    let (state, url) = state_with("file:///ds.sql", src);
    let resp = document_symbol::run(&state, DocumentSymbolParams {
        text_document: TextDocumentIdentifier { uri: url },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
    }).expect("document symbol");
    let symbols = match resp {
        DocumentSymbolResponse::Nested(n) => n,
        _ => panic!("expected nested"),
    };
    let table = symbols.iter().find(|s| s.name == "t").expect("table symbol");
    let children = table.children.as_ref().expect("children");
    let names: Vec<&str> = children.iter().map(|c| c.name.as_str()).collect();
    assert!(names.contains(&"id"), "expected `id` column child; got: {names:?}");
    assert!(names.contains(&"email"), "expected `email` column child");
    assert!(names.contains(&"uq_t_email"),
        "expected named UNIQUE constraint child; got: {names:?}");
    assert!(names.contains(&"CHECK"),
        "expected anonymous CHECK constraint child");
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
