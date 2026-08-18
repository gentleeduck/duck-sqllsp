//! Unit tests for the per-handler logic without spinning up the LSP wire.

use dsl_server::{
  documents::DocumentStore,
  handlers::{
    call_hierarchy, code_action, code_lens, completion, completion_resolve, definition, diagnostic, document_highlight,
    document_link, document_symbol, folding_range, formatting, hover, inlay_hints, linked_editing, on_type_formatting,
    range_formatting, references, rename, selection_range, semantic_tokens, signature_help, type_definition,
    workspace_symbol,
  },
  state::ServerState,
};
use tower_lsp::lsp_types::{
  CompletionParams, CompletionResponse, HoverParams, InlayHintParams, PartialResultParams, Position, Range,
  TextDocumentIdentifier, TextDocumentPositionParams, Url, WorkDoneProgressParams, WorkspaceSymbolParams,
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
  let resp = completion::run(
    &state,
    CompletionParams {
      text_document_position: TextDocumentPositionParams {
        text_document: TextDocumentIdentifier { uri: url },
        position: Position { line: 0, character: 3 },
      },
      work_done_progress_params: WorkDoneProgressParams::default(),
      partial_result_params: PartialResultParams::default(),
      context: None,
    },
  )
  .expect("completion result");
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
  let resp = completion::run(
    &state,
    CompletionParams {
      text_document_position: TextDocumentPositionParams {
        text_document: TextDocumentIdentifier { uri: url },
        position: Position { line: 0, character: 9 }, // after the dot
      },
      work_done_progress_params: WorkDoneProgressParams::default(),
      partial_result_params: PartialResultParams::default(),
      context: None,
    },
  )
  .expect("completion result");
  let items = match resp {
    CompletionResponse::Array(v) => v,
    CompletionResponse::List(l) => l.items,
  };
  // Empty catalog -> 0 column items; assert no keywords leak through.
  assert!(
    items.is_empty(),
    "expected only columns of `u` (empty cat), got {:?}",
    items.iter().map(|i| &i.label).collect::<Vec<_>>()
  );
}

#[test]
fn hover_returns_none_outside_known_tokens() {
  let (state, url) = state_with("file:///t.sql", "frobnicate_xyz");
  let resp = hover::run(
    &state,
    HoverParams {
      text_document_position_params: TextDocumentPositionParams {
        text_document: TextDocumentIdentifier { uri: url },
        position: Position { line: 0, character: 1 },
      },
      work_done_progress_params: WorkDoneProgressParams::default(),
    },
  );
  assert!(resp.is_none());
}

#[test]
fn hover_returns_keyword_docs() {
  let (state, url) = state_with("file:///t.sql", "SELECT 1");
  let resp = hover::run(
    &state,
    HoverParams {
      text_document_position_params: TextDocumentPositionParams {
        text_document: TextDocumentIdentifier { uri: url },
        position: Position { line: 0, character: 3 },
      },
      work_done_progress_params: WorkDoneProgressParams::default(),
    },
  );
  let h = resp.expect("hover result");
  // Hover now ships as MarkedString[] when the content has fenced
  // SQL; falls back to Markup for plain markdown. Either way the
  // serialised text should contain the keyword doc.
  let text = match h.contents {
    tower_lsp::lsp_types::HoverContents::Markup(m) => m.value,
    tower_lsp::lsp_types::HoverContents::Array(parts) => parts
      .into_iter()
      .map(|p| match p {
        tower_lsp::lsp_types::MarkedString::String(s) => s,
        tower_lsp::lsp_types::MarkedString::LanguageString(ls) => ls.value,
      })
      .collect::<Vec<_>>()
      .join("\n"),
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
fn on_type_formatting_indents_after_open_paren() {
  use tower_lsp::lsp_types::{
    DocumentOnTypeFormattingParams, FormattingOptions, TextDocumentIdentifier, TextDocumentPositionParams,
  };
  // After `CREATE TABLE foo (\n` the new line should pick up two
  // spaces of indent (default tab_size=2, insert_spaces=true).
  let src = "CREATE TABLE foo (\n";
  let (state, url) = state_with("file:///ot.sql", src);
  let edits = on_type_formatting::run(
    &state,
    DocumentOnTypeFormattingParams {
      text_document_position: TextDocumentPositionParams {
        text_document: TextDocumentIdentifier { uri: url },
        position: Position { line: 1, character: 0 },
      },
      ch: "\n".into(),
      options: FormattingOptions { tab_size: 2, insert_spaces: true, ..Default::default() },
    },
  )
  .expect("edit");
  assert_eq!(edits.len(), 1);
  assert_eq!(edits[0].new_text, "  ", "expected 2 spaces after `(`");
}

#[test]
fn on_type_formatting_indents_after_begin_keyword() {
  use tower_lsp::lsp_types::{
    DocumentOnTypeFormattingParams, FormattingOptions, TextDocumentIdentifier, TextDocumentPositionParams,
  };
  let src = "DO $$ BEGIN\n";
  let (state, url) = state_with("file:///oi.sql", src);
  let edits = on_type_formatting::run(
    &state,
    DocumentOnTypeFormattingParams {
      text_document_position: TextDocumentPositionParams {
        text_document: TextDocumentIdentifier { uri: url },
        position: Position { line: 1, character: 0 },
      },
      ch: "\n".into(),
      options: FormattingOptions { tab_size: 4, insert_spaces: true, ..Default::default() },
    },
  )
  .expect("edit");
  assert_eq!(edits[0].new_text, "    ", "BEGIN keyword should indent +1 unit");
}

#[test]
fn on_type_formatting_keeps_indent_on_plain_wrap() {
  use tower_lsp::lsp_types::{
    DocumentOnTypeFormattingParams, FormattingOptions, TextDocumentIdentifier, TextDocumentPositionParams,
  };
  // Inside an already-indented body, plain text + newline keeps
  // the current indentation rather than adding more.
  let src = "    SELECT id\n";
  let (state, url) = state_with("file:///op.sql", src);
  let edits = on_type_formatting::run(
    &state,
    DocumentOnTypeFormattingParams {
      text_document_position: TextDocumentPositionParams {
        text_document: TextDocumentIdentifier { uri: url },
        position: Position { line: 1, character: 0 },
      },
      ch: "\n".into(),
      options: FormattingOptions { tab_size: 4, insert_spaces: true, ..Default::default() },
    },
  )
  .expect("edit");
  assert_eq!(edits[0].new_text, "    ", "plain wrap preserves existing indent");
}

#[test]
fn folding_range_collapses_create_table_body_and_plpgsql_block() {
  use tower_lsp::lsp_types::{FoldingRangeParams, PartialResultParams, TextDocumentIdentifier, WorkDoneProgressParams};
  let src = "\
CREATE TABLE users (
  id INT,
  email TEXT
);
CREATE OR REPLACE FUNCTION f() RETURNS VOID LANGUAGE plpgsql AS $$
BEGIN
  RAISE NOTICE 'hi';
END
$$;
";
  let (state, url) = state_with("file:///fr.sql", src);
  let r = folding_range::run(
    &state,
    FoldingRangeParams {
      text_document: TextDocumentIdentifier { uri: url },
      work_done_progress_params: WorkDoneProgressParams::default(),
      partial_result_params: PartialResultParams::default(),
    },
  )
  .expect("folds");
  // Paren fold for `( ... )` of CREATE TABLE.
  let any_paren = r.iter().any(|f| f.start_line == 0 && f.end_line >= 1 && f.end_line <= 3);
  assert!(any_paren, "expected CREATE TABLE paren fold; got: {r:?}");
  // BEGIN..END fold inside dollar-quoted body.
  let any_begin = r.iter().any(|f| {
    // BEGIN is on line 5; END on line 7. Fold should be [5, 6].
    f.start_line == 5 && f.end_line >= 6
  });
  assert!(any_begin, "expected BEGIN..END fold; got: {r:?}");
}

#[test]
fn folding_range_emits_block_comment_fold() {
  use tower_lsp::lsp_types::{
    FoldingRangeKind, FoldingRangeParams, PartialResultParams, TextDocumentIdentifier, WorkDoneProgressParams,
  };
  let src = "/* multi\n   line\n   comment */ SELECT 1;";
  let (state, url) = state_with("file:///fc.sql", src);
  let r = folding_range::run(
    &state,
    FoldingRangeParams {
      text_document: TextDocumentIdentifier { uri: url },
      work_done_progress_params: WorkDoneProgressParams::default(),
      partial_result_params: PartialResultParams::default(),
    },
  )
  .expect("folds");
  assert!(
    r.iter().any(|f| f.kind == Some(FoldingRangeKind::Comment) && f.start_line == 0 && f.end_line == 2),
    "missing comment fold: {r:?}"
  );
}

#[test]
fn call_hierarchy_finds_incoming_caller() {
  use tower_lsp::lsp_types::{
    CallHierarchyIncomingCallsParams, CallHierarchyItem, CallHierarchyPrepareParams, PartialResultParams, Range,
    SymbolKind, TextDocumentIdentifier, TextDocumentPositionParams, WorkDoneProgressParams,
  };
  let src = "\
CREATE OR REPLACE FUNCTION audit_log() RETURNS void LANGUAGE plpgsql AS $$
BEGIN
  RAISE NOTICE 'log';
END;
$$;
CREATE OR REPLACE FUNCTION on_update() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
  PERFORM audit_log();
  RETURN NEW;
END;
$$;
";
  let (state, url) = state_with("file:///ch.sql", src);
  // Prepare on `audit_log` (in the on_update body).
  let cur = src.rfind("audit_log()").unwrap() + 3;
  let items = call_hierarchy::prepare(
    &state,
    CallHierarchyPrepareParams {
      text_document_position_params: TextDocumentPositionParams {
        text_document: TextDocumentIdentifier { uri: url.clone() },
        position: Position {
          line: 7,
          character: (cur - src.lines().take(7).map(|l| l.len() + 1).sum::<usize>()) as u32,
        },
      },
      work_done_progress_params: WorkDoneProgressParams::default(),
    },
  );
  let item = items.expect("prepare returns").into_iter().next().expect("at least one item");
  let incoming = call_hierarchy::incoming(
    &state,
    CallHierarchyIncomingCallsParams {
      item: CallHierarchyItem {
        name: "audit_log".into(),
        kind: SymbolKind::FUNCTION,
        tags: None,
        detail: None,
        uri: url.clone(),
        range: Range { start: Position { line: 0, character: 0 }, end: Position { line: 0, character: 0 } },
        selection_range: Range { start: Position { line: 0, character: 0 }, end: Position { line: 0, character: 0 } },
        data: None,
      },
      work_done_progress_params: WorkDoneProgressParams::default(),
      partial_result_params: PartialResultParams::default(),
    },
  )
  .expect("incoming");
  assert_eq!(incoming.len(), 1, "expected one caller (on_update)");
  assert_eq!(incoming[0].from.name, "on_update");
  let _ = item;
}

#[test]
fn linked_editing_returns_all_in_statement_occurrences() {
  use tower_lsp::lsp_types::{
    LinkedEditingRangeParams, TextDocumentIdentifier, TextDocumentPositionParams, WorkDoneProgressParams,
  };
  let src = "SELECT u.id FROM users u WHERE u.id = 1;";
  let (state, url) = state_with("file:///le.sql", src);
  let cur = src.find("FROM users u").unwrap() + 5; // inside `users`
  let r = linked_editing::run(
    &state,
    LinkedEditingRangeParams {
      text_document_position_params: TextDocumentPositionParams {
        text_document: TextDocumentIdentifier { uri: url.clone() },
        position: Position { line: 0, character: cur as u32 },
      },
      work_done_progress_params: WorkDoneProgressParams::default(),
    },
  );
  // `users` appears only once -> < 2, returns None.
  assert!(r.is_none(), "single occurrence should not produce linked ranges");

  let src2 = "SELECT u.id FROM users u JOIN orders o ON u.id = o.user_id WHERE u.email = 'x';";
  let (state2, url2) = state_with("file:///le2.sql", src2);
  // Cursor on alias `u` (first occurrence in `u.id`).
  let cur2 = src2.find("u.id").unwrap();
  let r2 = linked_editing::run(
    &state2,
    LinkedEditingRangeParams {
      text_document_position_params: TextDocumentPositionParams {
        text_document: TextDocumentIdentifier { uri: url2 },
        position: Position { line: 0, character: cur2 as u32 },
      },
      work_done_progress_params: WorkDoneProgressParams::default(),
    },
  )
  .expect("linked ranges");
  assert!(r2.ranges.len() >= 3, "alias `u` repeats 3+ times; got {} ranges", r2.ranges.len());
}

#[test]
fn definition_jumps_to_create_role() {
  use tower_lsp::lsp_types::{
    GotoDefinitionParams, GotoDefinitionResponse, PartialResultParams, TextDocumentIdentifier,
    TextDocumentPositionParams, WorkDoneProgressParams,
  };
  let src = "\
CREATE ROLE app_owner;
ALTER TABLE users OWNER TO app_owner;
";
  let (state, url) = state_with("file:///gd.sql", src);
  let line1 = "ALTER TABLE users OWNER TO app_owner;";
  let cur = line1.find("app_owner").unwrap() + 3;
  let resp = definition::run(
    &state,
    GotoDefinitionParams {
      text_document_position_params: TextDocumentPositionParams {
        text_document: TextDocumentIdentifier { uri: url.clone() },
        position: Position { line: 1, character: cur as u32 },
      },
      work_done_progress_params: WorkDoneProgressParams::default(),
      partial_result_params: PartialResultParams::default(),
    },
  )
  .expect("def");
  let loc = match resp {
    GotoDefinitionResponse::Scalar(l) => l,
    _ => panic!("expected scalar"),
  };
  assert_eq!(loc.uri, url);
  assert_eq!(loc.range.start.line, 0, "should land on CREATE ROLE line");
}

#[test]
fn definition_jumps_across_open_buffers() {
  use tower_lsp::lsp_types::{
    GotoDefinitionParams, GotoDefinitionResponse, PartialResultParams, TextDocumentIdentifier,
    TextDocumentPositionParams, WorkDoneProgressParams,
  };
  let state = ServerState::new();
  let schema: Url = "file:///migrations/001_schema.sql".parse().unwrap();
  let query: Url = "file:///queries/list.sql".parse().unwrap();
  state.documents.open(schema.clone(), "CREATE TABLE products (id INT);".into(), 1);
  state.documents.open(query.clone(), "SELECT * FROM products;".into(), 1);
  let q_src = "SELECT * FROM products;";
  let cur = q_src.find("products").unwrap() + 3;
  let resp = definition::run(
    &state,
    GotoDefinitionParams {
      text_document_position_params: TextDocumentPositionParams {
        text_document: TextDocumentIdentifier { uri: query.clone() },
        position: Position { line: 0, character: cur as u32 },
      },
      work_done_progress_params: WorkDoneProgressParams::default(),
      partial_result_params: PartialResultParams::default(),
    },
  )
  .expect("def");
  let loc = match resp {
    GotoDefinitionResponse::Scalar(l) => l,
    _ => panic!("expected scalar"),
  };
  assert_eq!(loc.uri, schema, "jump should land in the schema buffer");
}

#[test]
fn definition_jumps_to_cte_binding() {
  use tower_lsp::lsp_types::{
    GotoDefinitionParams, GotoDefinitionResponse, PartialResultParams, TextDocumentIdentifier,
    TextDocumentPositionParams, WorkDoneProgressParams,
  };
  let src = "WITH active AS (SELECT 1) SELECT * FROM active;";
  let (state, url) = state_with("file:///cte.sql", src);
  let cur = src.rfind("active").unwrap() + 3;
  let resp = definition::run(
    &state,
    GotoDefinitionParams {
      text_document_position_params: TextDocumentPositionParams {
        text_document: TextDocumentIdentifier { uri: url },
        position: Position { line: 0, character: cur as u32 },
      },
      work_done_progress_params: WorkDoneProgressParams::default(),
      partial_result_params: PartialResultParams::default(),
    },
  )
  .expect("def");
  let loc = match resp {
    GotoDefinitionResponse::Scalar(l) => l,
    _ => panic!("expected scalar"),
  };
  let expected_col = src.find("active").unwrap() as u32;
  assert_eq!(loc.range.start.character, expected_col, "should land on CTE binding's `active`");
}

#[test]
fn type_definition_jumps_to_create_type_for_cast_target() {
  use tower_lsp::lsp_types::request::{GotoTypeDefinitionParams, GotoTypeDefinitionResponse};
  use tower_lsp::lsp_types::{
    PartialResultParams, TextDocumentIdentifier, TextDocumentPositionParams, WorkDoneProgressParams,
  };
  let src = "\
CREATE TYPE mood AS ENUM ('happy', 'sad');
SELECT 'happy'::mood;
";
  let (state, url) = state_with("file:///td.sql", src);
  // Cursor on `mood` after `::` on line 1 (0-based).
  let line1 = "SELECT 'happy'::mood;";
  let cur_in_line = line1.find("::mood").unwrap() + 4;
  let resp = type_definition::run(
    &state,
    GotoTypeDefinitionParams {
      text_document_position_params: TextDocumentPositionParams {
        text_document: TextDocumentIdentifier { uri: url.clone() },
        position: Position { line: 1, character: cur_in_line as u32 },
      },
      work_done_progress_params: WorkDoneProgressParams::default(),
      partial_result_params: PartialResultParams::default(),
    },
  )
  .expect("type-def");
  let loc = match resp {
    GotoTypeDefinitionResponse::Scalar(l) => l,
    _ => panic!("expected scalar location"),
  };
  assert_eq!(loc.uri, url);
  assert_eq!(loc.range.start.line, 0, "should jump to CREATE TYPE on line 0");
}

#[test]
fn type_definition_returns_none_for_builtin_type() {
  use tower_lsp::lsp_types::request::GotoTypeDefinitionParams;
  use tower_lsp::lsp_types::{
    PartialResultParams, TextDocumentIdentifier, TextDocumentPositionParams, WorkDoneProgressParams,
  };
  let src = "SELECT '1'::INT;";
  let (state, url) = state_with("file:///tdb.sql", src);
  let cur = src.find("::INT").unwrap() + 3;
  let resp = type_definition::run(
    &state,
    GotoTypeDefinitionParams {
      text_document_position_params: TextDocumentPositionParams {
        text_document: TextDocumentIdentifier { uri: url },
        position: Position { line: 0, character: cur as u32 },
      },
      work_done_progress_params: WorkDoneProgressParams::default(),
      partial_result_params: PartialResultParams::default(),
    },
  );
  assert!(resp.is_none(), "no CREATE TYPE INT exists -> None");
}

#[test]
fn document_highlight_marks_every_occurrence_in_buffer() {
  use tower_lsp::lsp_types::{
    DocumentHighlightParams, PartialResultParams, TextDocumentIdentifier, TextDocumentPositionParams,
    WorkDoneProgressParams,
  };
  let src = "SELECT id FROM users WHERE users.id = 1;";
  let (state, url) = state_with("file:///dh.sql", src);
  let cur = src.find("users").unwrap() + 2; // inside the first `users`
  let hl = document_highlight::run(
    &state,
    DocumentHighlightParams {
      text_document_position_params: TextDocumentPositionParams {
        text_document: TextDocumentIdentifier { uri: url },
        position: Position { line: 0, character: cur as u32 },
      },
      work_done_progress_params: WorkDoneProgressParams::default(),
      partial_result_params: PartialResultParams::default(),
    },
  )
  .expect("highlights");
  assert_eq!(hl.len(), 2, "expected both `users` occurrences highlighted");
  for h in &hl {
    assert_eq!(h.kind, Some(tower_lsp::lsp_types::DocumentHighlightKind::TEXT));
  }
}

#[test]
fn document_highlight_excludes_string_literal_match() {
  // Identifier `users` in a string literal must NOT be highlighted
  // -- same scanner as references / rename.
  use tower_lsp::lsp_types::{
    DocumentHighlightParams, PartialResultParams, TextDocumentIdentifier, TextDocumentPositionParams,
    WorkDoneProgressParams,
  };
  let src = "SELECT 'users' FROM users;";
  let (state, url) = state_with("file:///dh2.sql", src);
  let cur = src.find("FROM users").unwrap() + 5;
  let hl = document_highlight::run(
    &state,
    DocumentHighlightParams {
      text_document_position_params: TextDocumentPositionParams {
        text_document: TextDocumentIdentifier { uri: url },
        position: Position { line: 0, character: cur as u32 },
      },
      work_done_progress_params: WorkDoneProgressParams::default(),
      partial_result_params: PartialResultParams::default(),
    },
  )
  .expect("highlights");
  assert_eq!(hl.len(), 1, "string literal `'users'` should be excluded");
}

#[test]
fn rename_rewrites_every_open_buffer() {
  use tower_lsp::lsp_types::{
    RenameParams, TextDocumentIdentifier, TextDocumentPositionParams, WorkDoneProgressParams,
  };
  let state = ServerState::new();
  let schema: Url = "file:///s.sql".parse().unwrap();
  let query: Url = "file:///q.sql".parse().unwrap();
  state.documents.open(schema.clone(), "CREATE TABLE products (id INT);".into(), 1);
  state.documents.open(query.clone(), "SELECT * FROM products WHERE products.id = 1;".into(), 1);

  let edit = rename::run(
    &state,
    RenameParams {
      text_document_position: TextDocumentPositionParams {
        text_document: TextDocumentIdentifier { uri: schema.clone() },
        position: Position { line: 0, character: 16 },
      },
      new_name: "items".into(),
      work_done_progress_params: WorkDoneProgressParams::default(),
    },
  )
  .expect("workspace edit");
  let changes = edit.changes.expect("changes");
  assert!(changes.contains_key(&schema), "schema buffer should be edited");
  assert!(changes.contains_key(&query), "query buffer should be edited");
  assert_eq!(changes[&schema].len(), 1);
  assert_eq!(changes[&query].len(), 2, "FROM + WHERE qualifier");
  for e in &changes[&query] {
    assert_eq!(e.new_text, "items");
  }
}

#[test]
fn references_walks_every_open_buffer() {
  use tower_lsp::lsp_types::{
    PartialResultParams, ReferenceContext, ReferenceParams, TextDocumentIdentifier, TextDocumentPositionParams,
    WorkDoneProgressParams,
  };
  let state = ServerState::new();
  let schema: Url = "file:///migrations/001_schema.sql".parse().unwrap();
  let seed: Url = "file:///seeds/products.sql".parse().unwrap();
  let query: Url = "file:///queries/list.sql".parse().unwrap();
  state.documents.open(schema.clone(), "CREATE TABLE products (id INT);".into(), 1);
  state.documents.open(seed.clone(), "INSERT INTO products (id) VALUES (1);".into(), 1);
  state.documents.open(query.clone(), "SELECT * FROM products WHERE products.id = 1;".into(), 1);

  let locs = references::run(
    &state,
    ReferenceParams {
      text_document_position: TextDocumentPositionParams {
        text_document: TextDocumentIdentifier { uri: schema.clone() },
        position: Position { line: 0, character: 16 }, // inside "products" in CREATE TABLE
      },
      work_done_progress_params: WorkDoneProgressParams::default(),
      partial_result_params: PartialResultParams::default(),
      context: ReferenceContext { include_declaration: true },
    },
  )
  .expect("ref result");

  let by_uri: std::collections::HashMap<_, usize> = locs.iter().fold(std::collections::HashMap::new(), |mut acc, l| {
    *acc.entry(l.uri.clone()).or_default() += 1;
    acc
  });
  assert_eq!(by_uri.get(&schema).copied(), Some(1), "1 hit in schema (CREATE TABLE)");
  assert_eq!(by_uri.get(&seed).copied(), Some(1), "1 hit in seed (INSERT INTO)");
  assert_eq!(by_uri.get(&query).copied(), Some(2), "2 hits in query (FROM + WHERE qualifier)");
}

#[test]
fn rename_returns_workspace_edit() {
  let (state, url) = state_with("file:///r.sql", "CREATE TABLE products (id INT);\nSELECT * FROM products;");
  use tower_lsp::lsp_types::{RenameParams, TextDocumentIdentifier, TextDocumentPositionParams};
  let edits = rename::run(
    &state,
    RenameParams {
      text_document_position: TextDocumentPositionParams {
        text_document: TextDocumentIdentifier { uri: url.clone() },
        position: Position { line: 0, character: 14 },
      },
      new_name: "items".into(),
      work_done_progress_params: WorkDoneProgressParams::default(),
    },
  )
  .expect("rename result");
  let changes = edits.changes.expect("changes map");
  assert_eq!(changes.get(&url).unwrap().len(), 2);
}

#[test]
fn rename_rejects_invalid_identifier() {
  let (state, url) = state_with("file:///r.sql", "CREATE TABLE products (id INT);");
  use tower_lsp::lsp_types::{RenameParams, TextDocumentIdentifier, TextDocumentPositionParams};
  let edits = rename::run(
    &state,
    RenameParams {
      text_document_position: TextDocumentPositionParams {
        text_document: TextDocumentIdentifier { uri: url },
        position: Position { line: 0, character: 14 },
      },
      new_name: "1bad".into(), // starts with digit
      work_done_progress_params: WorkDoneProgressParams::default(),
    },
  );
  assert!(edits.is_none(), "must reject identifier starting with digit");
}

#[test]
fn inlay_emits_inline_column_chip_for_insert_with_explicit_columns() {
  // INSERT INTO t (a, b) VALUES (1, 'x') -- a chip with the column
  // name should land BEFORE each literal, not after.
  use tower_lsp::lsp_types::{InlayHintKind, InlayHintParams, Range, TextDocumentIdentifier, WorkDoneProgressParams};
  let src = "\
CREATE TABLE user_roles (user_id INT, role TEXT);
INSERT INTO user_roles (user_id, role) VALUES ('id_1', 'admin');
";
  let (state, url) = state_with("file:///iv.sql", src);
  let hints = inlay_hints::run(
    &state,
    InlayHintParams {
      text_document: TextDocumentIdentifier { uri: url },
      range: Range { start: Position { line: 0, character: 0 }, end: Position { line: 10, character: 0 } },
      work_done_progress_params: WorkDoneProgressParams::default(),
    },
  )
  .expect("hints");
  let chips: Vec<&str> = hints
    .iter()
    .filter(|h| h.kind == Some(InlayHintKind::PARAMETER))
    .filter_map(|h| match &h.label {
      tower_lsp::lsp_types::InlayHintLabel::String(s) => Some(s.as_str()),
      _ => None,
    })
    .collect();
  assert!(chips.contains(&"user_id"), "expected `user_id` chip; got: {chips:?}");
  assert!(chips.contains(&"role"), "expected `role` chip; got: {chips:?}");
  // Chip lands at the start of the value, not at end.
  let user_id_chip = hints
    .iter()
    .find(|h| matches!(&h.label, tower_lsp::lsp_types::InlayHintLabel::String(s) if s == "user_id"))
    .unwrap();
  // The chip's character should sit at the column where `'id_1'` starts.
  let line1 = "INSERT INTO user_roles (user_id, role) VALUES ('id_1', 'admin');";
  let expected_col = line1.find("'id_1'").unwrap() as u32;
  assert_eq!(
    user_id_chip.position.character, expected_col,
    "chip should sit at start of literal, got {} expected {}",
    user_id_chip.position.character, expected_col
  );
}

#[test]
fn inlay_guesses_join_predicate_without_fk() {
  // No live catalog, no parsed CREATE TABLE constraints -> source_tables
  // derives the schema but with zero FKs. Inlay must still surface a
  // heuristic ON for a JOIN whose schema follows the `*_id` convention.
  use tower_lsp::lsp_types::{InlayHintParams, Range, TextDocumentIdentifier, WorkDoneProgressParams};
  let src = "\
CREATE TABLE users  (id INT);
CREATE TABLE orders (id INT, user_id INT);
SELECT * FROM orders o JOIN users u;
";
  let (state, url) = state_with("file:///j.sql", src);
  let hints = inlay_hints::run(
    &state,
    InlayHintParams {
      text_document: TextDocumentIdentifier { uri: url },
      range: Range { start: Position { line: 0, character: 0 }, end: Position { line: 10, character: 0 } },
      work_done_progress_params: WorkDoneProgressParams::default(),
    },
  )
  .expect("inlay");
  let any_join_hint = hints.iter().any(|h| match &h.label {
    tower_lsp::lsp_types::InlayHintLabel::String(s) => s.contains("user_id") && s.contains("id") && s.contains("?"),
    _ => false,
  });
  assert!(any_join_hint, "expected heuristic JOIN ON hint, got: {hints:?}");
}

#[test]
fn inlay_falls_back_to_question_marks_when_no_overlap() {
  // Two tables with no column overlap and no convention match. The
  // hint should still surface as `???  -- missing ON` so the user is
  // nudged about the JOIN that lacks a predicate.
  use tower_lsp::lsp_types::{InlayHintParams, Range, TextDocumentIdentifier, WorkDoneProgressParams};
  let src = "\
CREATE TABLE alpha (x INT);
CREATE TABLE beta  (y INT);
SELECT * FROM alpha a JOIN beta b;
";
  let (state, url) = state_with("file:///jx.sql", src);
  let hints = inlay_hints::run(
    &state,
    InlayHintParams {
      text_document: TextDocumentIdentifier { uri: url },
      range: Range { start: Position { line: 0, character: 0 }, end: Position { line: 10, character: 0 } },
      work_done_progress_params: WorkDoneProgressParams::default(),
    },
  )
  .expect("inlay");
  let any_missing_on = hints.iter().any(|h| match &h.label {
    tower_lsp::lsp_types::InlayHintLabel::String(s) => s.contains("missing ON"),
    _ => false,
  });
  assert!(any_missing_on, "expected `missing ON` hint when no overlap, got: {hints:?}");
}

#[test]
fn semantic_tokens_classify_cast_and_brackets_and_range_type() {
  use tower_lsp::lsp_types::{
    PartialResultParams, SemanticTokensParams, SemanticTokensResult, TextDocumentIdentifier, WorkDoneProgressParams,
  };
  // Cast to a built-in: `'1'::INT` should emit Operator + Type tokens.
  // Cast to a non-built-in (custom_enum): promote_cast_targets should
  // still tag it as Type. Brackets around the subscript should be
  // Operators. Range type `tstzrange` should be classified as Type.
  let src = "SELECT '1'::INT, x::custom_enum, arr[0:5], v::tstzrange FROM t;";
  let (state, url) = state_with("file:///st.sql", src);
  let r = semantic_tokens::run(
    &state,
    SemanticTokensParams {
      text_document: TextDocumentIdentifier { uri: url },
      work_done_progress_params: WorkDoneProgressParams::default(),
      partial_result_params: PartialResultParams::default(),
    },
  )
  .expect("tokens");
  let SemanticTokensResult::Tokens(toks) = r else { panic!("expected tokens variant") };

  // Reconstruct (line, char, len, type) from delta encoding so we can
  // ask "what kind is the token at byte X?" without rebuilding the
  // whole encoder logic.
  let mut line = 0u32;
  let mut col = 0u32;
  // Map of (line, col) -> token_type for assertion.
  let mut by_pos: std::collections::HashMap<(u32, u32), (u32, u32)> = std::collections::HashMap::new();
  for t in &toks.data {
    if t.delta_line != 0 {
      line += t.delta_line;
      col = t.delta_start;
    } else {
      col += t.delta_start;
    }
    by_pos.insert((line, col), (t.length, t.token_type));
  }

  // Resolve the byte offset of each thing we want to assert on.
  let find_col = |needle: &str| -> u32 { src.find(needle).expect(needle) as u32 };
  // Constants from the Tok enum:
  const TOK_TYPE: u32 = 1;
  const TOK_OPERATOR: u32 = 10;

  let cast1 = find_col("::INT");
  let (_, ty) = by_pos.get(&(0, cast1)).expect("`::` operator should be a token");
  assert_eq!(*ty, TOK_OPERATOR, "`::` should be Operator");
  let int_at = find_col("INT,");
  let (_, ty) = by_pos.get(&(0, int_at)).expect("`INT` type token");
  assert_eq!(*ty, TOK_TYPE, "INT should be Type");

  let custom_at = find_col("custom_enum");
  let (_, ty) = by_pos.get(&(0, custom_at)).expect("`custom_enum` token after `::` should be promoted");
  assert_eq!(*ty, TOK_TYPE, "user-defined cast target should be promoted to Type");

  let open_bracket = find_col("[0:5]");
  let (len, ty) = by_pos.get(&(0, open_bracket)).expect("`[` token");
  assert_eq!(*ty, TOK_OPERATOR, "`[` should be Operator");
  assert_eq!(*len, 1);

  let tstz_at = find_col("tstzrange");
  let (_, ty) = by_pos.get(&(0, tstz_at)).expect("tstzrange token");
  assert_eq!(*ty, TOK_TYPE, "tstzrange should be Type");
}

#[test]
fn inlay_expands_select_star_against_buffer_table() {
  use tower_lsp::lsp_types::{InlayHintParams, Range, TextDocumentIdentifier, WorkDoneProgressParams};
  let src = "CREATE TABLE t (a INT, b INT);\nSELECT * FROM t;";
  let (state, url) = state_with("file:///i.sql", src);
  let hints = inlay_hints::run(
    &state,
    InlayHintParams {
      text_document: TextDocumentIdentifier { uri: url },
      range: Range { start: Position { line: 0, character: 0 }, end: Position { line: 5, character: 0 } },
      work_done_progress_params: WorkDoneProgressParams::default(),
    },
  )
  .expect("inlay");
  assert_eq!(hints.len(), 1);
  match &hints[0].label {
    tower_lsp::lsp_types::InlayHintLabel::String(s) => assert!(s.contains("a") && s.contains("b")),
    _ => panic!("expected string label"),
  }
}

#[test]
fn selection_range_emits_innermost_first() {
  use tower_lsp::lsp_types::{
    PartialResultParams, SelectionRangeParams, TextDocumentIdentifier, WorkDoneProgressParams,
  };
  let src = "SELECT id FROM users WHERE id = 1;";
  let (state, url) = state_with("file:///sr.sql", src);
  let r = selection_range::run(
    &state,
    SelectionRangeParams {
      text_document: TextDocumentIdentifier { uri: url },
      positions: vec![Position { line: 0, character: 8 }],
      work_done_progress_params: WorkDoneProgressParams::default(),
      partial_result_params: PartialResultParams::default(),
    },
  )
  .expect("selection range");
  assert_eq!(r.len(), 1);
  let inner = &r[0];
  let inner_text_len = inner.range.end.character - inner.range.start.character;
  let parent = inner.parent.as_ref().expect("has parent");
  let parent_text_len = parent.range.end.character - parent.range.start.character;
  assert!(parent_text_len >= inner_text_len, "parent must be at least as wide as inner");
}

#[test]
fn workspace_symbol_surfaces_buffer_table() {
  use tower_lsp::lsp_types::{PartialResultParams, WorkDoneProgressParams, WorkspaceSymbolParams};
  let (state, _url) = state_with("file:///ws.sql", "CREATE TABLE accounts (id UUID, balance NUMERIC);");
  let syms = workspace_symbol::run(
    &state,
    WorkspaceSymbolParams {
      query: "accounts".into(),
      partial_result_params: PartialResultParams::default(),
      work_done_progress_params: WorkDoneProgressParams::default(),
    },
  )
  .expect("symbols");
  // Catalog merge can present the table as either bare or fully
  // qualified depending on which branch surfaces it first; both are
  // valid for the purpose of "user typed accounts and the editor
  // showed something useful".
  assert!(
    syms.iter().any(|s| s.name == "accounts" || s.name.ends_with(".accounts")),
    "expected `accounts` symbol; got: {:?}",
    syms.iter().map(|s| s.name.as_str()).collect::<Vec<_>>()
  );
}

#[test]
fn workspace_symbol_surfaces_buffer_sequence_type_extension() {
  use tower_lsp::lsp_types::{PartialResultParams, WorkDoneProgressParams, WorkspaceSymbolParams};
  let src = "\
CREATE SEQUENCE my_seq;
CREATE TYPE mood AS ENUM ('happy', 'sad');
CREATE EXTENSION pgcrypto;
";
  let (state, _url) = state_with("file:///wsx.sql", src);
  let syms = workspace_symbol::run(
    &state,
    WorkspaceSymbolParams {
      query: "".into(),
      partial_result_params: PartialResultParams::default(),
      work_done_progress_params: WorkDoneProgressParams::default(),
    },
  )
  .expect("symbols");
  let names: Vec<&str> = syms.iter().map(|s| s.name.as_str()).collect();
  assert!(names.iter().any(|n| n.ends_with("my_seq")), "missing sequence: {names:?}");
  assert!(names.iter().any(|n| n.ends_with("mood")), "missing type: {names:?}");
  assert!(names.iter().any(|n| n == &"pgcrypto"), "missing extension: {names:?}");
}

#[test]
fn signature_help_picks_active_param() {
  use tower_lsp::lsp_types::{
    SignatureHelpParams, TextDocumentIdentifier, TextDocumentPositionParams, WorkDoneProgressParams,
  };
  let src = "SELECT coalesce(name, 'unknown') FROM users;";
  let (state, url) = state_with("file:///sh.sql", src);
  // Right after the comma -> active index 1
  let r = signature_help::run(
    &state,
    SignatureHelpParams {
      text_document_position_params: TextDocumentPositionParams {
        text_document: TextDocumentIdentifier { uri: url },
        position: Position { line: 0, character: 22 },
      },
      work_done_progress_params: WorkDoneProgressParams::default(),
      context: None,
    },
  )
  .expect("signature");
  assert_eq!(r.active_parameter, Some(1));
}

#[test]
fn signature_help_for_length_renders_signature() {
  use tower_lsp::lsp_types::{
    SignatureHelpParams, TextDocumentIdentifier, TextDocumentPositionParams, WorkDoneProgressParams,
  };
  let src = "SELECT length() FROM users;";
  let (state, url) = state_with("file:///sh-len.sql", src);
  // Cursor inside the `(`.
  let r = signature_help::run(
    &state,
    SignatureHelpParams {
      text_document_position_params: TextDocumentPositionParams {
        text_document: TextDocumentIdentifier { uri: url },
        position: Position { line: 0, character: 14 },
      },
      work_done_progress_params: WorkDoneProgressParams::default(),
      context: None,
    },
  )
  .expect("length signature");
  let sig = &r.signatures[0];
  assert!(sig.label.to_ascii_lowercase().contains("length"), "label should contain `length`; got: {}", sig.label);
  assert!(sig.label.to_ascii_lowercase().contains("text"), "label should mention text arg; got: {}", sig.label);
}

#[test]
fn completion_snippet_item_has_expands_to_preview() {
  use tower_lsp::lsp_types::{
    CompletionParams, CompletionResponse, Documentation, PartialResultParams, TextDocumentIdentifier,
    TextDocumentPositionParams, WorkDoneProgressParams,
  };
  let (state, url) = state_with("file:///snip.sql", "");
  let resp = completion::run(
    &state,
    CompletionParams {
      text_document_position: TextDocumentPositionParams {
        text_document: TextDocumentIdentifier { uri: url },
        position: Position { line: 0, character: 0 },
      },
      work_done_progress_params: WorkDoneProgressParams::default(),
      partial_result_params: PartialResultParams::default(),
      context: None,
    },
  )
  .expect("completion result");
  let items = match resp {
    CompletionResponse::Array(a) => a,
    CompletionResponse::List(l) => l.items,
  };
  let it = items.iter().find(|i| i.label.eq_ignore_ascii_case("ctable")).expect("ctable snippet");
  // The list response defers documentation -- it must carry the resolve
  // payload instead, and resolving it must produce the same preview the
  // eager path used to inline.
  assert!(it.documentation.is_none(), "snippet docs should be deferred to completionItem/resolve");
  assert!(it.data.is_some(), "a deferred item must carry resolve data");
  let resolved = completion_resolve::run(it.clone());
  let doc = resolved.documentation.as_ref().expect("snippet doc set after resolve");
  let text = match doc {
    Documentation::MarkupContent(m) => m.value.clone(),
    Documentation::String(s) => s.clone(),
  };
  assert!(text.contains("Expands to"), "snippet doc should preview the expansion; got: {text}");
  assert!(
    text.to_lowercase().contains("create table name"),
    "preview should show placeholder labels stripped of ${{}}; got: {text}"
  );
}

#[test]
fn code_action_exists_to_lateral() {
  use tower_lsp::lsp_types::{
    CodeActionContext, CodeActionParams, PartialResultParams, Range, TextDocumentIdentifier, WorkDoneProgressParams,
  };
  let src = "SELECT u.id FROM users u WHERE EXISTS (SELECT 1 FROM orders o WHERE o.user_id = u.id);";
  let (state, url) = state_with("file:///el.sql", src);
  // Cursor inside the EXISTS subquery body.
  let cur = src.find("SELECT 1 FROM").unwrap() + 5;
  let line_col = Position { line: 0, character: cur as u32 };
  let r = code_action::run(
    &state,
    CodeActionParams {
      text_document: TextDocumentIdentifier { uri: url.clone() },
      range: Range { start: line_col, end: line_col },
      context: CodeActionContext { diagnostics: vec![], only: None, trigger_kind: None },
      work_done_progress_params: WorkDoneProgressParams::default(),
      partial_result_params: PartialResultParams::default(),
    },
  )
  .expect("actions");
  let lateral = r
    .iter()
    .find_map(|a| match a {
      tower_lsp::lsp_types::CodeActionOrCommand::CodeAction(ca) if ca.title.contains("LATERAL") => ca.edit.clone(),
      _ => None,
    })
    .expect("expected LATERAL action");
  let edits = lateral.changes.unwrap().remove(&url).unwrap();
  assert_eq!(edits.len(), 2, "expected 2 edits (EXISTS->TRUE + JOIN insert)");
  let new_texts: Vec<&str> = edits.iter().map(|e| e.new_text.as_str()).collect();
  assert!(new_texts.iter().any(|t| t.contains("TRUE")), "missing TRUE edit");
  assert!(
    new_texts.iter().any(|t| t.contains("CROSS JOIN LATERAL") && t.contains("SELECT 1 FROM orders o")),
    "missing LATERAL join edit; got: {new_texts:?}"
  );
}

#[test]
fn code_action_explain_analyze_wrap() {
  use tower_lsp::lsp_types::{
    CodeActionContext, CodeActionParams, CodeActionResponse, PartialResultParams, Range, TextDocumentIdentifier,
    WorkDoneProgressParams,
  };
  let src = "SELECT id FROM users WHERE id = '1';";
  let (state, url) = state_with("file:///ea.sql", src);
  let r = code_action::run(
    &state,
    CodeActionParams {
      text_document: TextDocumentIdentifier { uri: url },
      range: Range { start: Position { line: 0, character: 5 }, end: Position { line: 0, character: 5 } },
      context: CodeActionContext { diagnostics: vec![], only: None, trigger_kind: None },
      work_done_progress_params: WorkDoneProgressParams::default(),
      partial_result_params: PartialResultParams::default(),
    },
  )
  .expect("actions");
  let titles: Vec<String> = r
    .iter()
    .filter_map(|a| match a {
      tower_lsp::lsp_types::CodeActionOrCommand::CodeAction(ca) => Some(ca.title.clone()),
      _ => None,
    })
    .collect();
  assert!(
    titles.iter().any(|t| t.contains("EXPLAIN ANALYZE")),
    "expected EXPLAIN ANALYZE wrap action; got: {titles:?}"
  );
  let _: CodeActionResponse = r;
}

#[test]
fn signature_help_for_update_set_tuple() {
  use tower_lsp::lsp_types::{
    SignatureHelpParams, TextDocumentIdentifier, TextDocumentPositionParams, WorkDoneProgressParams,
  };
  let src = "UPDATE users SET (id, email) = ();";
  let (state, url) = state_with("file:///us.sql", src);
  let cur = src.find(") = (").unwrap() + 5;
  let r = signature_help::run(
    &state,
    SignatureHelpParams {
      text_document_position_params: TextDocumentPositionParams {
        text_document: TextDocumentIdentifier { uri: url },
        position: Position { line: 0, character: cur as u32 },
      },
      work_done_progress_params: WorkDoneProgressParams::default(),
      context: None,
    },
  )
  .expect("UPDATE SET sig");
  let sig = &r.signatures[0];
  assert!(sig.label.contains("SET"), "label: {}", sig.label);
  assert!(sig.label.contains("id"));
  assert!(sig.label.contains("email"));
}

#[test]
fn signature_help_for_insert_values_explicit_columns() {
  use tower_lsp::lsp_types::{
    SignatureHelpParams, TextDocumentIdentifier, TextDocumentPositionParams, WorkDoneProgressParams,
  };
  let src = "INSERT INTO users (id, email) VALUES ();";
  let (state, url) = state_with("file:///iv.sql", src);
  // Cursor right after the opening `(` of VALUES.
  let cur = src.find("VALUES (").unwrap() + 8;
  let r = signature_help::run(
    &state,
    SignatureHelpParams {
      text_document_position_params: TextDocumentPositionParams {
        text_document: TextDocumentIdentifier { uri: url },
        position: Position { line: 0, character: cur as u32 },
      },
      work_done_progress_params: WorkDoneProgressParams::default(),
      context: None,
    },
  )
  .expect("INSERT VALUES sig");
  let sig = &r.signatures[0];
  assert!(sig.label.contains("VALUES"), "label: {}", sig.label);
  assert!(sig.label.contains("id"), "label should list id: {}", sig.label);
  assert!(sig.label.contains("email"), "label should list email: {}", sig.label);
}

#[test]
fn signature_help_for_char_length_renders_signature() {
  use tower_lsp::lsp_types::{
    SignatureHelpParams, TextDocumentIdentifier, TextDocumentPositionParams, WorkDoneProgressParams,
  };
  let src = "SELECT char_length() FROM users;";
  let (state, url) = state_with("file:///sh-cl.sql", src);
  let r = signature_help::run(
    &state,
    SignatureHelpParams {
      text_document_position_params: TextDocumentPositionParams {
        text_document: TextDocumentIdentifier { uri: url },
        position: Position { line: 0, character: 19 },
      },
      work_done_progress_params: WorkDoneProgressParams::default(),
      context: None,
    },
  )
  .expect("char_length signature");
  let sig = &r.signatures[0];
  assert!(sig.label.to_ascii_lowercase().contains("char_length"));
}

#[test]
fn document_symbol_nests_columns_and_constraints_under_table() {
  use tower_lsp::lsp_types::{
    DocumentSymbolParams, DocumentSymbolResponse, PartialResultParams, TextDocumentIdentifier, WorkDoneProgressParams,
  };
  let src = "CREATE TABLE t (\n  id uuid NOT NULL PRIMARY KEY,\n  email text NOT NULL,\n  CONSTRAINT uq_t_email UNIQUE (email),\n  CHECK (length(email) > 3)\n);";
  let (state, url) = state_with("file:///ds.sql", src);
  let resp = document_symbol::run(
    &state,
    DocumentSymbolParams {
      text_document: TextDocumentIdentifier { uri: url },
      work_done_progress_params: WorkDoneProgressParams::default(),
      partial_result_params: PartialResultParams::default(),
    },
  )
  .expect("document symbol");
  let symbols = match resp {
    DocumentSymbolResponse::Nested(n) => n,
    _ => panic!("expected nested"),
  };
  let table = symbols.iter().find(|s| s.name == "t").expect("table symbol");
  let children = table.children.as_ref().expect("children");
  let names: Vec<&str> = children.iter().map(|c| c.name.as_str()).collect();
  assert!(names.contains(&"id"), "expected `id` column child; got: {names:?}");
  assert!(names.contains(&"email"), "expected `email` column child");
  assert!(names.contains(&"uq_t_email"), "expected named UNIQUE constraint child; got: {names:?}");
  assert!(names.contains(&"CHECK"), "expected anonymous CHECK constraint child");
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

#[test]
fn r2_166_completion_at_eof_no_panic() {
  let (state, url) = state_with("file:///t.sql", "SELECT * FROM users");
  let resp = completion::run(
    &state,
    CompletionParams {
      text_document_position: TextDocumentPositionParams {
        text_document: TextDocumentIdentifier { uri: url },
        // Cursor at line 0 char beyond the buffer length.
        position: Position { line: 0, character: 500 },
      },
      work_done_progress_params: WorkDoneProgressParams::default(),
      partial_result_params: PartialResultParams::default(),
      context: None,
    },
  );
  let _ = resp;
}

#[test]
fn r2_166_completion_empty_doc_no_panic() {
  let (state, url) = state_with("file:///empty.sql", "");
  let resp = completion::run(
    &state,
    CompletionParams {
      text_document_position: TextDocumentPositionParams {
        text_document: TextDocumentIdentifier { uri: url },
        position: Position { line: 0, character: 0 },
      },
      work_done_progress_params: WorkDoneProgressParams::default(),
      partial_result_params: PartialResultParams::default(),
      context: None,
    },
  );
  let _ = resp;
}

#[test]
fn r2_166_hover_in_string_literal_no_panic() {
  let (state, url) = state_with("file:///t.sql", "SELECT 'hello world' FROM users");
  let resp = hover::run(
    &state,
    HoverParams {
      text_document_position_params: TextDocumentPositionParams {
        text_document: TextDocumentIdentifier { uri: url },
        position: Position { line: 0, character: 11 }, // inside 'hello'
      },
      work_done_progress_params: WorkDoneProgressParams::default(),
    },
  );
  let _ = resp;
}

#[test]
fn r2_166_completion_past_line_end_clamps() {
  let (state, url) = state_with("file:///t.sql", "SELECT id FROM users\nSELECT *");
  let resp = completion::run(
    &state,
    CompletionParams {
      text_document_position: TextDocumentPositionParams {
        text_document: TextDocumentIdentifier { uri: url },
        // Line 1, char past the end of "SELECT *".
        position: Position { line: 1, character: 999 },
      },
      work_done_progress_params: WorkDoneProgressParams::default(),
      partial_result_params: PartialResultParams::default(),
      context: None,
    },
  );
  let _ = resp;
}

#[test]
fn r2_166_completion_for_unknown_doc_no_panic() {
  let state = ServerState::new();
  let url: Url = "file:///never-opened.sql".parse().unwrap();
  let resp = completion::run(
    &state,
    CompletionParams {
      text_document_position: TextDocumentPositionParams {
        text_document: TextDocumentIdentifier { uri: url },
        position: Position { line: 0, character: 0 },
      },
      work_done_progress_params: WorkDoneProgressParams::default(),
      partial_result_params: PartialResultParams::default(),
      context: None,
    },
  );
  let _ = resp;
}

#[test]
fn r2_167_definition_on_broken_sql_no_panic() {
  use tower_lsp::lsp_types::GotoDefinitionParams;
  let (state, url) = state_with("file:///t.sql", "SELECT * FROM (((id =");
  let resp = definition::run(
    &state,
    GotoDefinitionParams {
      text_document_position_params: TextDocumentPositionParams {
        text_document: TextDocumentIdentifier { uri: url },
        position: Position { line: 0, character: 5 },
      },
      work_done_progress_params: WorkDoneProgressParams::default(),
      partial_result_params: PartialResultParams::default(),
    },
  );
  let _ = resp;
}

#[test]
fn r2_167_document_symbol_empty_no_panic() {
  use tower_lsp::lsp_types::DocumentSymbolParams;
  let (state, url) = state_with("file:///empty.sql", "");
  let resp = document_symbol::run(
    &state,
    DocumentSymbolParams {
      text_document: TextDocumentIdentifier { uri: url },
      work_done_progress_params: WorkDoneProgressParams::default(),
      partial_result_params: PartialResultParams::default(),
    },
  );
  let _ = resp;
}

#[test]
fn r2_167_document_symbol_broken_sql_no_panic() {
  use tower_lsp::lsp_types::DocumentSymbolParams;
  let (state, url) = state_with("file:///broken.sql", "CREATE TABLE (((");
  let resp = document_symbol::run(
    &state,
    DocumentSymbolParams {
      text_document: TextDocumentIdentifier { uri: url },
      work_done_progress_params: WorkDoneProgressParams::default(),
      partial_result_params: PartialResultParams::default(),
    },
  );
  let _ = resp;
}

#[test]
fn r2_167_code_lens_empty_no_panic() {
  use tower_lsp::lsp_types::CodeLensParams;
  let (state, url) = state_with("file:///empty.sql", "");
  let resp = code_lens::run(
    &state,
    CodeLensParams {
      text_document: TextDocumentIdentifier { uri: url },
      work_done_progress_params: WorkDoneProgressParams::default(),
      partial_result_params: PartialResultParams::default(),
    },
  );
  let _ = resp;
}

#[test]
fn r2_167_code_lens_large_doc_no_panic() {
  use tower_lsp::lsp_types::CodeLensParams;
  // 500 simple statements -- code lens scans each. Verify no panic.
  let mut text = String::new();
  for i in 0..500 {
    text.push_str(&format!("SELECT id FROM users WHERE id = {i};\n"));
  }
  let (state, url) = state_with("file:///large.sql", &text);
  let resp = code_lens::run(
    &state,
    CodeLensParams {
      text_document: TextDocumentIdentifier { uri: url },
      work_done_progress_params: WorkDoneProgressParams::default(),
      partial_result_params: PartialResultParams::default(),
    },
  );
  let _ = resp;
}

#[test]
fn r2_167_folding_range_broken_sql_no_panic() {
  use tower_lsp::lsp_types::FoldingRangeParams;
  let (state, url) = state_with("file:///fold.sql", "BEGIN; (((( SELECT 1");
  let resp = folding_range::run(
    &state,
    FoldingRangeParams {
      text_document: TextDocumentIdentifier { uri: url },
      work_done_progress_params: WorkDoneProgressParams::default(),
      partial_result_params: PartialResultParams::default(),
    },
  );
  let _ = resp;
}

#[test]
fn r2_167_selection_range_empty_no_panic() {
  use tower_lsp::lsp_types::SelectionRangeParams;
  let (state, url) = state_with("file:///empty.sql", "");
  let resp = selection_range::run(
    &state,
    SelectionRangeParams {
      text_document: TextDocumentIdentifier { uri: url },
      positions: vec![Position { line: 0, character: 0 }],
      work_done_progress_params: WorkDoneProgressParams::default(),
      partial_result_params: PartialResultParams::default(),
    },
  );
  let _ = resp;
}

#[test]
fn r2_167_inlay_hints_broken_sql_no_panic() {
  use tower_lsp::lsp_types::{InlayHintParams, Range};
  let (state, url) = state_with("file:///hint.sql", "INSERT INTO ((");
  let resp = inlay_hints::run(
    &state,
    InlayHintParams {
      text_document: TextDocumentIdentifier { uri: url },
      range: Range {
        start: Position { line: 0, character: 0 },
        end: Position { line: 100, character: 100 },
      },
      work_done_progress_params: WorkDoneProgressParams::default(),
    },
  );
  let _ = resp;
}

#[test]
fn r2_168_references_broken_sql_no_panic() {
  use tower_lsp::lsp_types::{ReferenceContext, ReferenceParams};
  let (state, url) = state_with("file:///t.sql", "SELECT u.id FROM ((( WHERE");
  let resp = references::run(
    &state,
    ReferenceParams {
      text_document_position: TextDocumentPositionParams {
        text_document: TextDocumentIdentifier { uri: url },
        position: Position { line: 0, character: 7 },
      },
      work_done_progress_params: WorkDoneProgressParams::default(),
      partial_result_params: PartialResultParams::default(),
      context: ReferenceContext { include_declaration: true },
    },
  );
  let _ = resp;
}

#[test]
fn r2_168_references_empty_doc_no_panic() {
  use tower_lsp::lsp_types::{ReferenceContext, ReferenceParams};
  let (state, url) = state_with("file:///empty.sql", "");
  let resp = references::run(
    &state,
    ReferenceParams {
      text_document_position: TextDocumentPositionParams {
        text_document: TextDocumentIdentifier { uri: url },
        position: Position { line: 0, character: 0 },
      },
      work_done_progress_params: WorkDoneProgressParams::default(),
      partial_result_params: PartialResultParams::default(),
      context: ReferenceContext { include_declaration: false },
    },
  );
  let _ = resp;
}

#[test]
fn r2_168_rename_on_broken_sql_no_panic() {
  use tower_lsp::lsp_types::RenameParams;
  let (state, url) = state_with("file:///t.sql", "SELECT u.id FROM ((((");
  let resp = rename::run(
    &state,
    RenameParams {
      text_document_position: TextDocumentPositionParams {
        text_document: TextDocumentIdentifier { uri: url },
        position: Position { line: 0, character: 7 },
      },
      new_name: "newname".into(),
      work_done_progress_params: WorkDoneProgressParams::default(),
    },
  );
  let _ = resp;
}

#[test]
fn r2_168_rename_empty_new_name_no_panic() {
  use tower_lsp::lsp_types::RenameParams;
  let (state, url) = state_with("file:///t.sql", "SELECT u.id FROM users u");
  let resp = rename::run(
    &state,
    RenameParams {
      text_document_position: TextDocumentPositionParams {
        text_document: TextDocumentIdentifier { uri: url },
        position: Position { line: 0, character: 7 },
      },
      new_name: String::new(),
      work_done_progress_params: WorkDoneProgressParams::default(),
    },
  );
  let _ = resp;
}

#[test]
fn r2_168_semantic_tokens_broken_sql_no_panic() {
  use tower_lsp::lsp_types::SemanticTokensParams;
  let (state, url) = state_with("file:///tok.sql", "((( ); SELECT 1; DROP TABLE");
  let resp = semantic_tokens::run(
    &state,
    SemanticTokensParams {
      text_document: TextDocumentIdentifier { uri: url },
      work_done_progress_params: WorkDoneProgressParams::default(),
      partial_result_params: PartialResultParams::default(),
    },
  );
  let _ = resp;
}

#[test]
fn r2_168_semantic_tokens_large_doc_no_panic() {
  use tower_lsp::lsp_types::SemanticTokensParams;
  let mut text = String::new();
  for i in 0..500 {
    text.push_str(&format!("SELECT id FROM users WHERE id = {i};\n"));
  }
  let (state, url) = state_with("file:///big.sql", &text);
  let resp = semantic_tokens::run(
    &state,
    SemanticTokensParams {
      text_document: TextDocumentIdentifier { uri: url },
      work_done_progress_params: WorkDoneProgressParams::default(),
      partial_result_params: PartialResultParams::default(),
    },
  );
  let _ = resp;
}

#[test]
fn r2_168_workspace_symbol_empty_query_no_panic() {
  use tower_lsp::lsp_types::WorkspaceSymbolParams;
  let state = ServerState::new();
  let resp = workspace_symbol::run(
    &state,
    WorkspaceSymbolParams {
      query: String::new(),
      work_done_progress_params: WorkDoneProgressParams::default(),
      partial_result_params: PartialResultParams::default(),
    },
  );
  let _ = resp;
}

#[test]
fn r2_168_signature_help_in_function_call() {
  use tower_lsp::lsp_types::SignatureHelpParams;
  let (state, url) = state_with("file:///sig.sql", "SELECT generate_series(1, ");
  let resp = signature_help::run(
    &state,
    SignatureHelpParams {
      text_document_position_params: TextDocumentPositionParams {
        text_document: TextDocumentIdentifier { uri: url },
        position: Position { line: 0, character: 26 },
      },
      work_done_progress_params: WorkDoneProgressParams::default(),
      context: None,
    },
  );
  let _ = resp;
}

#[test]
fn r2_168_signature_help_broken_sql_no_panic() {
  use tower_lsp::lsp_types::SignatureHelpParams;
  let (state, url) = state_with("file:///sig.sql", "(((");
  let resp = signature_help::run(
    &state,
    SignatureHelpParams {
      text_document_position_params: TextDocumentPositionParams {
        text_document: TextDocumentIdentifier { uri: url },
        position: Position { line: 0, character: 2 },
      },
      work_done_progress_params: WorkDoneProgressParams::default(),
      context: None,
    },
  );
  let _ = resp;
}

#[test]
fn r2_169_call_hierarchy_prepare_on_broken_sql_no_panic() {
  use tower_lsp::lsp_types::CallHierarchyPrepareParams;
  let (state, url) = state_with("file:///t.sql", "CREATE FUNCTION ((((");
  let resp = call_hierarchy::prepare(
    &state,
    CallHierarchyPrepareParams {
      text_document_position_params: TextDocumentPositionParams {
        text_document: TextDocumentIdentifier { uri: url },
        position: Position { line: 0, character: 16 },
      },
      work_done_progress_params: WorkDoneProgressParams::default(),
    },
  );
  let _ = resp;
}

#[test]
fn r2_169_call_hierarchy_prepare_empty_no_panic() {
  use tower_lsp::lsp_types::CallHierarchyPrepareParams;
  let (state, url) = state_with("file:///empty.sql", "");
  let resp = call_hierarchy::prepare(
    &state,
    CallHierarchyPrepareParams {
      text_document_position_params: TextDocumentPositionParams {
        text_document: TextDocumentIdentifier { uri: url },
        position: Position { line: 0, character: 0 },
      },
      work_done_progress_params: WorkDoneProgressParams::default(),
    },
  );
  let _ = resp;
}

#[test]
fn r2_169_code_action_broken_sql_no_panic() {
  use tower_lsp::lsp_types::{CodeActionContext, CodeActionParams, Range};
  let (state, url) = state_with("file:///t.sql", "SELECT (((");
  let resp = code_action::run(
    &state,
    CodeActionParams {
      text_document: TextDocumentIdentifier { uri: url },
      range: Range {
        start: Position { line: 0, character: 0 },
        end: Position { line: 0, character: 10 },
      },
      context: CodeActionContext { diagnostics: Vec::new(), only: None, trigger_kind: None },
      work_done_progress_params: WorkDoneProgressParams::default(),
      partial_result_params: PartialResultParams::default(),
    },
  );
  let _ = resp;
}

#[test]
fn r2_169_document_highlight_broken_sql_no_panic() {
  use tower_lsp::lsp_types::DocumentHighlightParams;
  let (state, url) = state_with("file:///hl.sql", "SELECT (((( WHERE");
  let resp = document_highlight::run(
    &state,
    DocumentHighlightParams {
      text_document_position_params: TextDocumentPositionParams {
        text_document: TextDocumentIdentifier { uri: url },
        position: Position { line: 0, character: 7 },
      },
      work_done_progress_params: WorkDoneProgressParams::default(),
      partial_result_params: PartialResultParams::default(),
    },
  );
  let _ = resp;
}

#[test]
fn r2_169_type_definition_broken_sql_no_panic() {
  use tower_lsp::lsp_types::request::GotoTypeDefinitionParams;
  let (state, url) = state_with("file:///td.sql", "SELECT id FROM (((");
  let resp = type_definition::run(
    &state,
    GotoTypeDefinitionParams {
      text_document_position_params: TextDocumentPositionParams {
        text_document: TextDocumentIdentifier { uri: url },
        position: Position { line: 0, character: 7 },
      },
      work_done_progress_params: WorkDoneProgressParams::default(),
      partial_result_params: PartialResultParams::default(),
    },
  );
  let _ = resp;
}

#[test]
fn r2_169_linked_editing_empty_no_panic() {
  use tower_lsp::lsp_types::LinkedEditingRangeParams;
  let (state, url) = state_with("file:///empty.sql", "");
  let resp = linked_editing::run(
    &state,
    LinkedEditingRangeParams {
      text_document_position_params: TextDocumentPositionParams {
        text_document: TextDocumentIdentifier { uri: url },
        position: Position { line: 0, character: 0 },
      },
      work_done_progress_params: WorkDoneProgressParams::default(),
    },
  );
  let _ = resp;
}

#[test]
fn r2_169_on_type_formatting_broken_sql_no_panic() {
  use tower_lsp::lsp_types::{DocumentOnTypeFormattingParams, FormattingOptions};
  let (state, url) = state_with("file:///fmt.sql", "SELECT (((");
  let resp = on_type_formatting::run(
    &state,
    DocumentOnTypeFormattingParams {
      text_document_position: TextDocumentPositionParams {
        text_document: TextDocumentIdentifier { uri: url },
        position: Position { line: 0, character: 5 },
      },
      ch: ";".into(),
      options: FormattingOptions::default(),
    },
  );
  let _ = resp;
}

#[test]
fn r2_169_formatting_broken_sql_no_panic() {
  use tower_lsp::lsp_types::{DocumentFormattingParams, FormattingOptions};
  let (state, url) = state_with("file:///fmt.sql", "SELECT (((");
  let resp = formatting::run(
    &state,
    DocumentFormattingParams {
      text_document: TextDocumentIdentifier { uri: url },
      options: FormattingOptions::default(),
      work_done_progress_params: WorkDoneProgressParams::default(),
    },
  );
  let _ = resp;
}

#[test]
fn r2_169_formatting_empty_no_panic() {
  use tower_lsp::lsp_types::{DocumentFormattingParams, FormattingOptions};
  let (state, url) = state_with("file:///empty.sql", "");
  let resp = formatting::run(
    &state,
    DocumentFormattingParams {
      text_document: TextDocumentIdentifier { uri: url },
      options: FormattingOptions::default(),
      work_done_progress_params: WorkDoneProgressParams::default(),
    },
  );
  let _ = resp;
}

#[test]
fn r2_170_did_open_then_close_lifecycle() {
  let state = ServerState::new();
  let url: Url = "file:///life.sql".parse().unwrap();
  assert!(state.documents.get(&url).is_none(), "doc should not exist before open");
  state.documents.open(url.clone(), "SELECT 1;".into(), 1);
  assert!(state.documents.get(&url).is_some(), "doc should exist after open");
  state.documents.close(&url);
  assert!(state.documents.get(&url).is_none(), "doc should be gone after close");
}

#[test]
fn r2_170_did_open_then_update_increments_version() {
  let state = ServerState::new();
  let url: Url = "file:///up.sql".parse().unwrap();
  state.documents.open(url.clone(), "SELECT 1;".into(), 1);
  state.documents.update(&url, "SELECT 2;".into(), 2);
  let doc = state.documents.get(&url).expect("doc");
  assert_eq!(doc.version, 2);
}

#[test]
fn r2_170_update_with_older_version_no_panic() {
  let state = ServerState::new();
  let url: Url = "file:///stale.sql".parse().unwrap();
  state.documents.open(url.clone(), "SELECT 1;".into(), 5);
  // Out-of-order: update with version 3 after version 5.
  state.documents.update(&url, "SELECT 2;".into(), 3);
  let _ = state.documents.get(&url);
}

#[test]
fn r2_170_multi_doc_state_isolation() {
  let state = ServerState::new();
  let a: Url = "file:///a.sql".parse().unwrap();
  let b: Url = "file:///b.sql".parse().unwrap();
  state.documents.open(a.clone(), "SELECT 1;".into(), 1);
  state.documents.open(b.clone(), "SELECT 2;".into(), 1);
  assert_eq!(state.documents.get(&a).unwrap().text, "SELECT 1;");
  assert_eq!(state.documents.get(&b).unwrap().text, "SELECT 2;");
  state.documents.close(&a);
  assert!(state.documents.get(&a).is_none());
  assert!(state.documents.get(&b).is_some(), "b unaffected by a's close");
}

#[test]
fn r2_170_update_unopened_doc_no_panic() {
  let state = ServerState::new();
  let url: Url = "file:///ghost.sql".parse().unwrap();
  // No open first. Update should not panic.
  state.documents.update(&url, "SELECT 1;".into(), 1);
}

#[test]
fn r2_170_close_unopened_doc_no_panic() {
  let state = ServerState::new();
  let url: Url = "file:///ghost.sql".parse().unwrap();
  state.documents.close(&url);
}

#[test]
fn r2_170_open_same_uri_twice_replaces() {
  let state = ServerState::new();
  let url: Url = "file:///twice.sql".parse().unwrap();
  state.documents.open(url.clone(), "v1".into(), 1);
  state.documents.open(url.clone(), "v2".into(), 2);
  let doc = state.documents.get(&url).expect("doc");
  assert_eq!(doc.text, "v2");
  assert_eq!(doc.version, 2);
}

#[test]
fn r2_170_open_then_completion_then_close_lifecycle() {
  let state = ServerState::new();
  let url: Url = "file:///cyc.sql".parse().unwrap();
  state.documents.open(url.clone(), "SELECT em FROM users".into(), 1);
  let resp = completion::run(
    &state,
    CompletionParams {
      text_document_position: TextDocumentPositionParams {
        text_document: TextDocumentIdentifier { uri: url.clone() },
        position: Position { line: 0, character: 9 },
      },
      work_done_progress_params: WorkDoneProgressParams::default(),
      partial_result_params: PartialResultParams::default(),
      context: None,
    },
  );
  let _ = resp;
  state.documents.close(&url);
}

#[test]
fn r3_286_rapid_update_no_panic() {
  let state = ServerState::new();
  let url: Url = "file:///rapid.sql".parse().unwrap();
  state.documents.open(url.clone(), "SELECT".into(), 1);
  for i in 2..=50 {
    state.documents.update(&url, format!("SELECT {i}"), i);
  }
  let doc = state.documents.get(&url).expect("doc");
  assert_eq!(doc.version, 50);
  state.documents.close(&url);
}

#[test]
fn r3_287_many_concurrent_docs_no_panic() {
  let state = ServerState::new();
  for i in 0..100 {
    let url: Url = format!("file:///doc{i}.sql").parse().unwrap();
    state.documents.open(url.clone(), format!("SELECT {i}"), 1);
  }
  for i in 0..100 {
    let url: Url = format!("file:///doc{i}.sql").parse().unwrap();
    assert!(state.documents.get(&url).is_some());
  }
  for i in 0..100 {
    let url: Url = format!("file:///doc{i}.sql").parse().unwrap();
    state.documents.close(&url);
  }
}

#[test]
fn r3_288_update_with_huge_text() {
  let state = ServerState::new();
  let url: Url = "file:///huge.sql".parse().unwrap();
  let mut text = String::new();
  for i in 0..1000 {
    text.push_str(&format!("SELECT {i};\n"));
  }
  state.documents.open(url.clone(), text, 1);
  let doc = state.documents.get(&url).expect("doc");
  assert!(doc.text.len() > 5000);
  state.documents.close(&url);
}

#[test]
fn r3_289_open_with_multibyte() {
  let state = ServerState::new();
  let url: Url = "file:///utf8.sql".parse().unwrap();
  state.documents.open(url.clone(), "SELECT '日本語🎉';".into(), 1);
  let doc = state.documents.get(&url).expect("doc");
  assert!(doc.text.contains("日本語"));
}

#[test]
fn r3_290_completion_on_empty_doc_no_panic() {
  let state = ServerState::new();
  let url: Url = "file:///empty.sql".parse().unwrap();
  state.documents.open(url.clone(), "".into(), 1);
  let resp = completion::run(
    &state,
    CompletionParams {
      text_document_position: TextDocumentPositionParams {
        text_document: TextDocumentIdentifier { uri: url.clone() },
        position: Position { line: 0, character: 0 },
      },
      work_done_progress_params: WorkDoneProgressParams::default(),
      partial_result_params: PartialResultParams::default(),
      context: None,
    },
  );
  let _ = resp;
}

#[test]
fn r4_196_hover_unopened_doc_no_panic() {
  use tower_lsp::lsp_types::{HoverParams, TextDocumentPositionParams, Position, TextDocumentIdentifier};
  let state = ServerState::new();
  let url: Url = "file:///ghost.sql".parse().unwrap();
  let resp = hover::run(&state, HoverParams {
    text_document_position_params: TextDocumentPositionParams {
      text_document: TextDocumentIdentifier { uri: url.clone() },
      position: Position { line: 0, character: 0 },
    },
    work_done_progress_params: Default::default(),
  });
  let _ = resp;
}

#[test]
fn r4_197_completion_at_huge_position_no_panic() {
  let state = ServerState::new();
  let url: Url = "file:///hp.sql".parse().unwrap();
  state.documents.open(url.clone(), "SELECT".into(), 1);
  let resp = completion::run(
    &state,
    CompletionParams {
      text_document_position: TextDocumentPositionParams {
        text_document: TextDocumentIdentifier { uri: url.clone() },
        position: Position { line: 100, character: 9999 },
      },
      work_done_progress_params: WorkDoneProgressParams::default(),
      partial_result_params: PartialResultParams::default(),
      context: None,
    },
  );
  let _ = resp;
}

#[test]
fn r4_198_open_doc_with_only_dollar_quote() {
  let state = ServerState::new();
  let url: Url = "file:///dq.sql".parse().unwrap();
  state.documents.open(url.clone(), "$$ $$".into(), 1);
  let doc = state.documents.get(&url).expect("doc");
  assert_eq!(doc.text, "$$ $$");
}

#[test]
fn r4_199_update_with_crlf_lineending() {
  let state = ServerState::new();
  let url: Url = "file:///crlf.sql".parse().unwrap();
  state.documents.open(url.clone(), "SELECT 1;\r\nSELECT 2;".into(), 1);
  let doc = state.documents.get(&url).expect("doc");
  assert!(doc.text.contains("\r\n"));
}

#[test]
fn r4_200_close_then_reopen_keeps_separate_versions() {
  let state = ServerState::new();
  let url: Url = "file:///rv.sql".parse().unwrap();
  state.documents.open(url.clone(), "v1".into(), 1);
  state.documents.close(&url);
  state.documents.open(url.clone(), "v2".into(), 2);
  let doc = state.documents.get(&url).expect("doc");
  assert_eq!(doc.version, 2);
  assert_eq!(doc.text, "v2");
}

/// Phase D re-measurement of the Phase A finding (`complete()` at
/// ~25ms/call at 10k statements, 5x over the < 5ms p50 target -- see
/// `dsl-completion/tests/perf_bench.rs`). Opens one large document and
/// fires completion + hover + inlay_hints + workspace_symbol `rounds`
/// times each against it (one version, no edits), cursor at end-of-
/// statement to match `perf_bench.rs`'s cursor convention.
///
/// Two fixes landed: `Document::derived_catalog` caches
/// `source_tables::from_source` per document version instead of every
/// handler re-deriving it (small win, ~1.6ms/call at n=10,000); the
/// dominant fix is `engine::current_statement_span`, which scopes
/// `fallback::{scope_from_text, cte_names_from_text,
/// cte_columns_from_text}` to the current statement instead of the
/// whole buffer -- those used to scan the entire file for every
/// FROM/JOIN regardless of cursor position, which was the real O(buffer
/// size) cost (details + numbers in the design doc's "Phase D
/// findings" section and in `perf_bench.rs`'s doc comments).
///
/// At n=10,000: completion cold (round 0, first call after open) ~74ms
/// one-time; warm (every cache hit after) ~1.6ms/call, under target.
/// Not a hard-threshold assertion (machine-dependent) -- prints the
/// numbers so a regression is visible by eye.
///
/// `hover::run` was NOT fixed by this and used to dominate the
/// per-handler breakdown by 1-2 orders of magnitude -- a pre-existing
/// issue in `dsl-hover` (re-parsed the whole buffer on every call,
/// ignoring `ParseCache`), left as out of scope here. Fixed in the
/// body-context-completion-hover project's Task 5 -- see
/// `r5_203_perf_hover_reuses_cached_parse` below and the design doc.
#[test]
#[ignore]
fn r5_201_perf_derived_catalog_cache_avoids_redundant_rescans() {
  let n = 10_000;
  let mut text = String::with_capacity(n * 50);
  for i in 0..n {
    text.push_str(&format!("SELECT id FROM users WHERE id = {i};\n"));
  }
  let (state, url) = state_with("file:///perf_cache.sql", &text);
  let rounds = 20u32;

  // "Cached": completion + hover + inlay_hints fired `rounds` times each
  // against the SAME open document (one version, zero edits between
  // calls) -- derived_catalog() should pay the buffer scan once, not
  // 3 * rounds times. Timed per-handler-type to isolate which one
  // dominates rather than only seeing a combined total.
  let mut t_completion = std::time::Duration::ZERO;
  let mut t_completion_first = std::time::Duration::ZERO;
  let mut t_hover = std::time::Duration::ZERO;
  let mut t_inlay = std::time::Duration::ZERO;
  let mut t_wsym = std::time::Duration::ZERO;
  // Cursor lands right after each line's `WHERE id = <i>` (before the
  // trailing `;`) -- end-of-statement, matching the cursor placement
  // `dsl-completion/tests/perf_bench.rs`'s `build_buffer` uses. A
  // mid-keyword position (e.g. character 5, inside "SELECT") hits a
  // much cheaper, earlier phase-detection short-circuit and would
  // *not* be a fair comparison against Phase A's original numbers.
  let prefix_len = "SELECT id FROM users WHERE id = ".len() as u32;
  let t0 = std::time::Instant::now();
  for i in 0..rounds {
    let line = i.min(n as u32 - 1);
    let character = prefix_len + line.to_string().len() as u32;
    let pos = Position { line, character };
    let s = std::time::Instant::now();
    let _ = completion::run(
      &state,
      CompletionParams {
        text_document_position: TextDocumentPositionParams {
          text_document: TextDocumentIdentifier { uri: url.clone() },
          position: pos,
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
        context: None,
      },
    );
    let e = s.elapsed();
    t_completion += e;
    if i == 0 {
      t_completion_first = e;
    }
    let s = std::time::Instant::now();
    let _ = hover::run(
      &state,
      HoverParams {
        text_document_position_params: TextDocumentPositionParams {
          text_document: TextDocumentIdentifier { uri: url.clone() },
          position: pos,
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
      },
    );
    t_hover += s.elapsed();
    let s = std::time::Instant::now();
    let _ = inlay_hints::run(
      &state,
      InlayHintParams {
        text_document: TextDocumentIdentifier { uri: url.clone() },
        range: Range { start: Position { line: 0, character: 0 }, end: Position { line: i + 1, character: 0 } },
        work_done_progress_params: WorkDoneProgressParams::default(),
      },
    );
    t_inlay += s.elapsed();
    let s = std::time::Instant::now();
    let _ = workspace_symbol::run(
      &state,
      WorkspaceSymbolParams {
        query: String::new(),
        partial_result_params: PartialResultParams::default(),
        work_done_progress_params: WorkDoneProgressParams::default(),
      },
    );
    t_wsym += s.elapsed();
  }
  let _ = t0.elapsed(); // combined wall time isn't meaningful here -- hover's unrelated cost (see doc comment) would dominate and mislead; per-handler numbers below are the honest comparison.
  let rest = rounds - 1;
  eprintln!(
    "per-handler totals over {rounds} rounds (n_stmts={n}) -- completion: {t_completion:?} ({:?}/call)  hover: {t_hover:?} ({:?}/call)  inlay_hints: {t_inlay:?} ({:?}/call)  workspace_symbol: {t_wsym:?} ({:?}/call)",
    t_completion / rounds,
    t_hover / rounds,
    t_inlay / rounds,
    t_wsym / rounds
  );
  eprintln!(
    "completion cold-vs-warm split -- round 0 (cold derived_catalog cache): {t_completion_first:?}  rounds 1..{rounds} avg (warm cache): {:?}",
    (t_completion - t_completion_first) / rest
  );

  // Reference point: a single standalone `from_source` call at this
  // buffer size, for scale against the cold-cache number above (round 0
  // pays this once, plus parse + resolve + complete_with_derived's own
  // work; warm rounds pay none of it).
  let doc = state.documents.get(&url).unwrap();
  let cache = doc.parsed();
  let t1 = std::time::Instant::now();
  let _ = dsl_completion::source_tables::from_source(&cache.file, &doc.text);
  eprintln!("standalone from_source call at n_stmts={n}: {:?}", t1.elapsed());
}

/// Task 5 (body-context-completion-hover project) fix for the gap
/// `r5_201` above flagged and left untouched: `dsl_hover::hover_with`
/// re-parsed the whole buffer on every single call, ignoring
/// `Document::parsed()`'s cache. Fixed by splitting it into a thin
/// `hover_with` wrapper (parses fresh, delegates -- mirrors
/// `complete()` / `complete_with_derived()`) plus `hover_with_parsed`,
/// which takes an already-parsed `ParsedFile`; `hover::run` now calls
/// `hover_with_parsed` with `doc.parsed()`'s cached file.
///
/// Because `hover_with` still parses fresh by design (it's the public,
/// cache-less entry point for callers with no parse cache of their
/// own), calling it directly reproduces exactly what `hover::run` used
/// to pay per request -- used below as a live "before" proxy, so this
/// benchmark compares two code paths that both still exist today
/// rather than needing to resurrect deleted code.
#[test]
#[ignore]
fn r5_203_perf_hover_reuses_cached_parse() {
  let n = 10_000;
  let mut text = String::with_capacity(n * 50);
  let mut line_starts: Vec<u32> = Vec::with_capacity(n);
  for i in 0..n {
    line_starts.push(text.len() as u32);
    text.push_str(&format!("SELECT id FROM users WHERE id = {i};\n"));
  }
  let (state, url) = state_with("file:///perf_hover.sql", &text);
  let rounds = 20u32;
  // Cursor lands right after each line's `WHERE id = <i>` (before the
  // trailing `;`) -- same end-of-statement convention as r5_201.
  let prefix_len = "SELECT id FROM users WHERE id = ".len() as u32;

  // Warm the parse cache and grab a catalog once, mirroring normal LSP
  // traffic (some earlier handler has always parsed the doc by the
  // time hover fires) -- both loops below measure steady-state calls,
  // not the unavoidable one-time first parse.
  let doc = state.documents.get(&url).unwrap();
  let cat = doc.derived_catalog();
  let _ = doc.parsed();
  drop(doc);

  // "Before" proxy: hover_with reparses `text` on every call.
  let mut t_before = std::time::Duration::ZERO;
  for i in 0..rounds {
    let line = i.min(n as u32 - 1) as usize;
    let character = prefix_len + line.to_string().len() as u32;
    let offset = text_size::TextSize::from(line_starts[line] + character);
    let s = std::time::Instant::now();
    let _ = dsl_hover::hover_with(&text, offset, &cat, dsl_hover::KeywordCase::Upper);
    t_before += s.elapsed();
  }

  // "After": hover::run, now threading doc.parsed()'s cached file
  // through hover_with_parsed.
  let mut t_after = std::time::Duration::ZERO;
  for i in 0..rounds {
    let line = i.min(n as u32 - 1);
    let character = prefix_len + line.to_string().len() as u32;
    let s = std::time::Instant::now();
    let _ = hover::run(
      &state,
      HoverParams {
        text_document_position_params: TextDocumentPositionParams {
          text_document: TextDocumentIdentifier { uri: url.clone() },
          position: Position { line, character },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
      },
    );
    t_after += s.elapsed();
  }

  eprintln!(
    "hover before (hover_with, reparse-per-call) vs after (hover::run, cached parse) over {rounds} rounds (n_stmts={n}) -- before: {t_before:?} ({:?}/call)  after: {t_after:?} ({:?}/call)  speedup: {:.1}x",
    t_before / rounds,
    t_after / rounds,
    t_before.as_secs_f64() / t_after.as_secs_f64().max(1e-9),
  );
}

#[test]
fn r5_202_completion_sees_table_defined_in_another_open_document() {
  // A CREATE TABLE in one open (unsaved) buffer should be completable
  // from another open buffer immediately -- the same cross-document
  // visibility `workspace/symbol` already has -- rather than only
  // appearing after the file is saved and the workspace rescanned.
  // Regression coverage for a gap found during the Phase C cross-file
  // intelligence probe: `completion::run` used to merge in only the
  // *current* document's derived catalog.
  let state = ServerState::new();
  let url_a: Url = "file:///a.sql".parse().unwrap();
  let url_b: Url = "file:///b.sql".parse().unwrap();
  state.documents.open(url_a, "CREATE TABLE widgets (id int, name text);".into(), 1);
  state.documents.open(url_b.clone(), "SELECT * FROM widg".into(), 1);
  let resp = completion::run(
    &state,
    CompletionParams {
      text_document_position: TextDocumentPositionParams {
        text_document: TextDocumentIdentifier { uri: url_b },
        position: Position { line: 0, character: 18 },
      },
      work_done_progress_params: WorkDoneProgressParams::default(),
      partial_result_params: PartialResultParams::default(),
      context: None,
    },
  )
  .expect("completion result");
  let items = match resp {
    CompletionResponse::Array(v) => v,
    CompletionResponse::List(l) => l.items,
  };
  let labels: Vec<String> = items.iter().map(|i| i.label.clone()).collect();
  assert!(labels.contains(&"widgets".to_string()), "expected `widgets` from the other open document; got {labels:?}");
}

// ---------------------------------------------------------------------
// textDocument/rangeFormatting -- format-selection support.
// The selection snaps outward to whole top-level statements, so the
// deterministic contract these tests pin down is the *edit range*, not
// the formatter output (which varies with whether the sql-formatter CLI
// is installed on the machine running the suite).
// ---------------------------------------------------------------------

fn range_fmt(state: &ServerState, url: &Url, range: Range) -> Option<Vec<tower_lsp::lsp_types::TextEdit>> {
  use tower_lsp::lsp_types::{DocumentRangeFormattingParams, FormattingOptions};
  range_formatting::run(
    state,
    DocumentRangeFormattingParams {
      text_document: TextDocumentIdentifier { uri: url.clone() },
      range,
      options: FormattingOptions::default(),
      work_done_progress_params: WorkDoneProgressParams::default(),
    },
  )
}

#[test]
fn range_formatting_snaps_selection_to_the_touched_statement() {
  let src = "SELECT 1;\n\nCREATE TABLE t (\nid int,\nname text\n);\n";
  let (state, url) = state_with("file:///rf.sql", src);
  // Caret parked on the `id int,` line -- well inside statement two.
  let edits = range_fmt(
    &state,
    &url,
    Range { start: Position { line: 3, character: 2 }, end: Position { line: 3, character: 2 } },
  )
  .expect("an edit for the CREATE TABLE");
  assert_eq!(edits.len(), 1);
  let e = &edits[0];
  assert_eq!(e.range.start, Position { line: 2, character: 0 }, "should start at CREATE, not at SELECT 1");
  assert_eq!(e.range.end, Position { line: 5, character: 2 }, "should end just past the terminating `;`");
  assert!(!e.new_text.contains("SELECT 1"), "must not rewrite the untouched neighbour: {:?}", e.new_text);
}

#[test]
fn range_formatting_spanning_two_statements_covers_both() {
  let src = "CREATE TABLE a (\nid int\n);\nCREATE TABLE b (\nid int\n);\n";
  let (state, url) = state_with("file:///rf2.sql", src);
  let edits = range_fmt(
    &state,
    &url,
    Range { start: Position { line: 1, character: 0 }, end: Position { line: 4, character: 0 } },
  )
  .expect("an edit covering both statements");
  let e = &edits[0];
  assert_eq!(e.range.start, Position { line: 0, character: 0 });
  assert_eq!(e.range.end, Position { line: 5, character: 2 });
}

#[test]
fn range_formatting_whitespace_only_selection_is_a_no_op() {
  let src = "SELECT 1;\n\n\nSELECT 2;\n";
  let (state, url) = state_with("file:///rf3.sql", src);
  let edits = range_fmt(
    &state,
    &url,
    Range { start: Position { line: 1, character: 0 }, end: Position { line: 2, character: 0 } },
  );
  assert!(edits.is_none(), "selecting the gap between statements must not emit an edit: {edits:?}");
}

#[test]
fn range_formatting_out_of_bounds_range_does_not_panic() {
  let (state, url) = state_with("file:///rf4.sql", "SELECT 1;");
  let _ = range_fmt(
    &state,
    &url,
    Range { start: Position { line: 9_999, character: 9_999 }, end: Position { line: 10_000, character: 0 } },
  );
}

#[test]
fn range_formatting_reversed_range_is_normalised() {
  let src = "CREATE TABLE t (\nid int\n);\n";
  let (state, url) = state_with("file:///rf5.sql", src);
  // end before start -- some clients send visual selections backwards.
  let edits = range_fmt(
    &state,
    &url,
    Range { start: Position { line: 2, character: 0 }, end: Position { line: 0, character: 0 } },
  );
  if let Some(edits) = edits {
    assert_eq!(edits[0].range.start, Position { line: 0, character: 0 });
  }
}

#[test]
fn range_formatting_on_broken_sql_does_not_panic() {
  let (state, url) = state_with("file:///rf6.sql", "SELECT ((( ;\nCREATE TABLE");
  let _ = range_fmt(
    &state,
    &url,
    Range { start: Position { line: 0, character: 0 }, end: Position { line: 1, character: 12 } },
  );
}

#[test]
fn range_formatting_empty_document_is_a_no_op() {
  let (state, url) = state_with("file:///rf7.sql", "");
  let edits = range_fmt(
    &state,
    &url,
    Range { start: Position { line: 0, character: 0 }, end: Position { line: 0, character: 0 } },
  );
  assert!(edits.is_none());
}

// ---------------------------------------------------------------------
// textDocument/diagnostic -- LSP 3.17 pull diagnostics.
// ---------------------------------------------------------------------

fn pull_diagnostics(
  state: &ServerState,
  url: &Url,
  previous: Option<String>,
) -> tower_lsp::lsp_types::DocumentDiagnosticReportResult {
  use tower_lsp::lsp_types::DocumentDiagnosticParams;
  diagnostic::run(
    state,
    DocumentDiagnosticParams {
      text_document: TextDocumentIdentifier { uri: url.clone() },
      identifier: Some("duck-sqllsp".into()),
      previous_result_id: previous,
      work_done_progress_params: WorkDoneProgressParams::default(),
      partial_result_params: PartialResultParams::default(),
    },
  )
}

/// `(result_id, item_count)` from a Full report; panics on Unchanged.
fn expect_full(report: &tower_lsp::lsp_types::DocumentDiagnosticReportResult) -> (Option<String>, usize) {
  use tower_lsp::lsp_types::{DocumentDiagnosticReport, DocumentDiagnosticReportResult};
  match report {
    DocumentDiagnosticReportResult::Report(DocumentDiagnosticReport::Full(r)) => {
      (r.full_document_diagnostic_report.result_id.clone(), r.full_document_diagnostic_report.items.len())
    },
    other => panic!("expected a Full report, got {other:?}"),
  }
}

fn is_unchanged(report: &tower_lsp::lsp_types::DocumentDiagnosticReportResult) -> bool {
  use tower_lsp::lsp_types::{DocumentDiagnosticReport, DocumentDiagnosticReportResult};
  matches!(report, DocumentDiagnosticReportResult::Report(DocumentDiagnosticReport::Unchanged(_)))
}

#[test]
fn pull_diagnostics_returns_a_full_report_with_a_result_id() {
  let (state, url) = state_with("file:///pull.sql", "SELECT a FROM t WHERE x = NULL;");
  let report = pull_diagnostics(&state, &url, None);
  let (id, count) = expect_full(&report);
  assert!(id.is_some(), "a full report must carry a result id or the client can never cache");
  assert!(count > 0, "`= NULL` (sql015) should produce at least one diagnostic");
}

#[test]
fn pull_diagnostics_agrees_with_the_push_path() {
  // Both channels run the same `compute`, so an inconsistency here
  // means a user would see different results depending on their client.
  let (state, url) = state_with("file:///pull2.sql", "SELECT a FROM t WHERE x = NULL;");
  let (_, pulled) = expect_full(&pull_diagnostics(&state, &url, None));
  let pushed = dsl_server::diagnostics::compute(&state, &url).expect("computed").0.len();
  assert_eq!(pulled, pushed);
}

#[test]
fn pull_diagnostics_second_call_with_same_result_id_is_unchanged() {
  let (state, url) = state_with("file:///pull3.sql", "SELECT a FROM t WHERE x = NULL;");
  let (id, _) = expect_full(&pull_diagnostics(&state, &url, None));
  let again = pull_diagnostics(&state, &url, id);
  assert!(is_unchanged(&again), "unchanged buffer + unchanged catalog must short-circuit: {again:?}");
}

#[test]
fn pull_diagnostics_result_id_changes_when_the_buffer_changes() {
  let (state, url) = state_with("file:///pull4.sql", "SELECT 1;");
  let (first, _) = expect_full(&pull_diagnostics(&state, &url, None));
  state.documents.update(&url, "SELECT 2;".into(), 2);
  let second = pull_diagnostics(&state, &url, first.clone());
  let (second_id, _) = expect_full(&second);
  assert_ne!(first, second_id, "edited buffer must invalidate the client's cached report");
}

#[test]
fn pull_diagnostics_result_id_changes_when_analysis_inputs_move() {
  // A catalog swap / config reload can flip sql001 without the buffer
  // changing a byte. The generation counter is what makes that visible.
  let (state, url) = state_with("file:///pull5.sql", "SELECT 1;");
  let (first, _) = expect_full(&pull_diagnostics(&state, &url, None));
  state.bump_analysis_generation();
  let (second_id, _) = expect_full(&pull_diagnostics(&state, &url, first.clone()));
  assert_ne!(first, second_id, "catalog/config change must invalidate the cached report");
}

#[test]
fn pull_diagnostics_for_an_unopened_document_is_an_empty_full_report() {
  let state = ServerState::new();
  let url: Url = "file:///never-opened.sql".parse().unwrap();
  let (id, count) = expect_full(&pull_diagnostics(&state, &url, None));
  assert_eq!(count, 0);
  assert!(id.is_none(), "no document, no cacheable id");
}

#[test]
fn pull_diagnostics_honours_rule_severity_overrides() {
  use dsl_server::config::DuckSqllspConfig;
  let (state, url) = state_with("file:///pull6.sql", "SELECT a FROM t WHERE x = NULL;");
  let (_, before) = expect_full(&pull_diagnostics(&state, &url, None));
  assert!(before > 0);
  let mut cfg = DuckSqllspConfig::default();
  cfg.rules.insert("sql015".to_string(), "off".to_string());
  state.set_config(cfg);
  let (_, after) = expect_full(&pull_diagnostics(&state, &url, None));
  assert!(after < before, "silencing sql015 should drop findings: {before} -> {after}");
}

#[test]
fn push_is_the_default_and_pull_mode_is_opt_in() {
  let state = ServerState::new();
  assert!(!state.client_pulls_diagnostics(), "clients that never advertise pull must keep getting pushes");
  state.set_client_pull_diagnostics(true);
  assert!(state.client_pulls_diagnostics());
}

// ---------------------------------------------------------------------
// semanticTokens/range + token modifiers.
// ---------------------------------------------------------------------

/// Undo the LSP delta encoding so two token streams produced from
/// different starting points can be compared directly.
fn absolute_tokens(data: &[tower_lsp::lsp_types::SemanticToken]) -> Vec<(u32, u32, u32, u32, u32)> {
  let (mut line, mut ch) = (0u32, 0u32);
  let mut out = Vec::with_capacity(data.len());
  for t in data {
    if t.delta_line == 0 {
      ch += t.delta_start;
    } else {
      line += t.delta_line;
      ch = t.delta_start;
    }
    out.push((line, ch, t.length, t.token_type, t.token_modifiers_bitset));
  }
  out
}

fn full_tokens(state: &ServerState, url: &Url) -> Vec<(u32, u32, u32, u32, u32)> {
  use tower_lsp::lsp_types::{SemanticTokensParams, SemanticTokensResult};
  match semantic_tokens::run(
    state,
    SemanticTokensParams {
      text_document: TextDocumentIdentifier { uri: url.clone() },
      work_done_progress_params: WorkDoneProgressParams::default(),
      partial_result_params: PartialResultParams::default(),
    },
  ) {
    Some(SemanticTokensResult::Tokens(t)) => absolute_tokens(&t.data),
    other => panic!("expected full tokens, got {other:?}"),
  }
}

fn range_tokens(state: &ServerState, url: &Url, range: Range) -> Vec<(u32, u32, u32, u32, u32)> {
  use tower_lsp::lsp_types::{SemanticTokensRangeParams, SemanticTokensRangeResult};
  match semantic_tokens::run_range(
    state,
    SemanticTokensRangeParams {
      text_document: TextDocumentIdentifier { uri: url.clone() },
      range,
      work_done_progress_params: WorkDoneProgressParams::default(),
      partial_result_params: PartialResultParams::default(),
    },
  ) {
    Some(SemanticTokensRangeResult::Tokens(t)) => absolute_tokens(&t.data),
    other => panic!("expected range tokens, got {other:?}"),
  }
}

#[test]
fn semantic_tokens_range_matches_the_full_pass_for_the_same_lines() {
  let src = "SELECT 1;\nCREATE TABLE t (id int, email text);\nSELECT now();\n";
  let (state, url) = state_with("file:///st_range.sql", src);
  let full = full_tokens(&state, &url);
  let ranged = range_tokens(
    &state,
    &url,
    Range { start: Position { line: 1, character: 0 }, end: Position { line: 2, character: 0 } },
  );
  let expected: Vec<_> = full.into_iter().filter(|(line, ..)| *line == 1).collect();
  assert!(!expected.is_empty(), "line 1 should have tokens at all");
  assert_eq!(ranged, expected, "range must agree with the full pass, byte for byte");
}

#[test]
fn semantic_tokens_range_over_a_dollar_quoted_body_still_lexes_from_the_top() {
  // The body is only known to be a string because of context above the
  // range -- a range request that started lexing at the range would
  // classify these bytes as identifiers.
  let src = "CREATE FUNCTION f() RETURNS int AS $$\n  SELECT FROM WHERE;\n$$ LANGUAGE sql;\n";
  let (state, url) = state_with("file:///st_dollar.sql", src);
  let ranged = range_tokens(
    &state,
    &url,
    Range { start: Position { line: 1, character: 0 }, end: Position { line: 2, character: 0 } },
  );
  // Everything overlapping line 1 belongs to the one big string token
  // that starts on line 0.
  assert!(ranged.iter().all(|(_, _, _, ty, _)| *ty == 7), "expected only STRING tokens, got {ranged:?}");
}

#[test]
fn semantic_tokens_range_outside_the_document_is_empty() {
  let (state, url) = state_with("file:///st_empty_range.sql", "SELECT 1;\n");
  let ranged = range_tokens(
    &state,
    &url,
    Range { start: Position { line: 50, character: 0 }, end: Position { line: 60, character: 0 } },
  );
  assert!(ranged.is_empty(), "got {ranged:?}");
}

#[test]
fn semantic_tokens_emit_declaration_modifiers_for_created_names() {
  let src = "CREATE TABLE users (id int);";
  let (state, url) = state_with("file:///st_mods.sql", src);
  let full = full_tokens(&state, &url);
  // `users` at line 0, char 13, type 3 (class), declaration|definition.
  assert!(
    full.iter().any(|(l, c, _, ty, m)| *l == 0 && *c == 13 && *ty == 3 && *m == 0b011),
    "expected a declared+defined class token for `users`, got {full:?}"
  );
  // `id` -- property with the declaration bit only.
  assert!(
    full.iter().any(|(_, _, _, ty, m)| *ty == 4 && *m == 0b001),
    "expected a declared property token for `id`, got {full:?}"
  );
}

#[test]
fn every_emitted_modifier_bit_is_covered_by_the_advertised_legend() {
  // A bit the client's legend does not name is silently dropped (or
  // worse, mapped to the wrong modifier), so the widest bit we ever
  // emit must fit inside SEMANTIC_MODIFIERS.
  use dsl_server::capabilities::SEMANTIC_MODIFIERS;
  let src = "CREATE TABLE users (id int);\nCREATE FUNCTION f(a int) RETURNS int AS $$ SELECT 1 $$ LANGUAGE sql;\nSELECT count(a)::int FROM users;";
  let (state, url) = state_with("file:///st_legend.sql", src);
  let max_bit = full_tokens(&state, &url).iter().map(|(.., m)| *m).fold(0u32, |a, b| a | b);
  let allowed = if SEMANTIC_MODIFIERS.is_empty() { 0 } else { (1u32 << SEMANTIC_MODIFIERS.len()) - 1 };
  assert_eq!(max_bit & !allowed, 0, "emitted modifier bits {max_bit:#b} exceed legend of {}", SEMANTIC_MODIFIERS.len());
  assert!(max_bit > 0, "the fixture should exercise at least one modifier");
}

#[test]
fn every_emitted_token_type_is_covered_by_the_advertised_legend() {
  use dsl_server::capabilities::SEMANTIC_LEGEND;
  let src = "-- doc\nCREATE FUNCTION f(a int) RETURNS int AS $$ SELECT 1 $$ LANGUAGE sql;\nSELECT count(a)::int, 'x', 1 FROM users WHERE a > 1;";
  let (state, url) = state_with("file:///st_types.sql", src);
  for (.., ty, _) in full_tokens(&state, &url) {
    assert!((ty as usize) < SEMANTIC_LEGEND.len(), "token type {ty} outside legend of {}", SEMANTIC_LEGEND.len());
  }
}

// ---------------------------------------------------------------------
// completionItem/resolve -- deferred documentation.
// ---------------------------------------------------------------------

fn complete_at(state: &ServerState, url: &Url, line: u32, character: u32) -> Vec<tower_lsp::lsp_types::CompletionItem> {
  match completion::run(
    state,
    CompletionParams {
      text_document_position: TextDocumentPositionParams {
        text_document: TextDocumentIdentifier { uri: url.clone() },
        position: Position { line, character },
      },
      work_done_progress_params: WorkDoneProgressParams::default(),
      partial_result_params: PartialResultParams::default(),
      context: None,
    },
  )
  .expect("completion result")
  {
    CompletionResponse::Array(a) => a,
    CompletionResponse::List(l) => l.items,
  }
}

#[test]
fn completion_list_ships_no_builtin_documentation() {
  // The whole point of resolve: the per-keystroke response must not
  // carry ~2700 rendered markdown blobs.
  let (state, url) = state_with("file:///res1.sql", "SEL");
  let items = complete_at(&state, &url, 0, 3);
  assert!(!items.is_empty());
  let with_docs = items.iter().filter(|i| i.documentation.is_some()).count();
  assert_eq!(with_docs, 0, "{with_docs} items shipped eager documentation");
}

#[test]
fn keyword_item_resolves_to_real_documentation() {
  let (state, url) = state_with("file:///res2.sql", "SEL");
  let items = complete_at(&state, &url, 0, 3);
  let select = items.iter().find(|i| i.label.eq_ignore_ascii_case("SELECT")).expect("SELECT item");
  assert!(select.data.is_some(), "deferred item must carry resolve data");
  let resolved = completion_resolve::run(select.clone());
  let doc = match resolved.documentation.expect("resolved documentation") {
    tower_lsp::lsp_types::Documentation::MarkupContent(m) => m.value,
    tower_lsp::lsp_types::Documentation::String(s) => s,
  };
  assert!(doc.contains("Retrieve"), "expected the knowledge-base blurb, got {doc:?}");
}

#[test]
fn resolve_is_idempotent() {
  // Clients re-send already-resolved items; resolving twice must not
  // stack a second copy of the docs.
  let (state, url) = state_with("file:///res3.sql", "SEL");
  let items = complete_at(&state, &url, 0, 3);
  let select = items.iter().find(|i| i.label.eq_ignore_ascii_case("SELECT")).expect("SELECT item");
  let once = completion_resolve::run(select.clone());
  let twice = completion_resolve::run(once.clone());
  assert_eq!(format!("{:?}", once.documentation), format!("{:?}", twice.documentation));
}

#[test]
fn resolve_survives_the_style_config_recasing_labels() {
  // `style.keyword = lower` rewrites the label we send out; resolve
  // must still find the uppercase-keyed knowledge-base entry.
  use dsl_server::config::DuckSqllspConfig;
  let (state, url) = state_with("file:///res4.sql", "sel");
  let mut cfg = DuckSqllspConfig::default();
  cfg.style.keyword = dsl_server::config::Case::Lower;
  state.set_config(cfg);
  let items = complete_at(&state, &url, 0, 3);
  let select = items.iter().find(|i| i.label.eq_ignore_ascii_case("SELECT")).expect("SELECT item");
  let resolved = completion_resolve::run(select.clone());
  assert!(resolved.documentation.is_some(), "recased label must still resolve");
}

#[test]
fn every_deferred_item_carries_resolvable_data() {
  // An item with no documentation and no `data` is a dead end -- the
  // user highlights it and gets a blank doc panel forever.
  let (state, url) = state_with("file:///res5.sql", "SEL");
  for it in complete_at(&state, &url, 0, 3) {
    if it.documentation.is_none() {
      assert!(it.data.is_some(), "item {:?} has neither documentation nor resolve data", it.label);
    }
  }
}

// ---------------------------------------------------------------------
// textDocument/documentLink -- psql includes, COPY paths, URLs.
// File links are only emitted for paths that actually resolve, so these
// tests write real files into a temp dir.
// ---------------------------------------------------------------------

fn doc_links(state: &ServerState, url: &Url) -> Vec<tower_lsp::lsp_types::DocumentLink> {
  use tower_lsp::lsp_types::DocumentLinkParams;
  document_link::run(
    state,
    DocumentLinkParams {
      text_document: TextDocumentIdentifier { uri: url.clone() },
      work_done_progress_params: WorkDoneProgressParams::default(),
      partial_result_params: PartialResultParams::default(),
    },
  )
  .unwrap_or_default()
}

/// Unique temp dir under the OS temp root, created fresh per test.
fn temp_dir(tag: &str) -> std::path::PathBuf {
  let dir = std::env::temp_dir().join(format!("duck-sqllsp-doclink-{tag}-{}", std::process::id()));
  let _ = std::fs::remove_dir_all(&dir);
  std::fs::create_dir_all(&dir).expect("temp dir");
  dir
}

#[test]
fn document_link_resolves_an_existing_include_relative_to_the_file() {
  let dir = temp_dir("inc");
  std::fs::write(dir.join("schema.sql"), "CREATE TABLE t (id int);").unwrap();
  let main = dir.join("main.sql");
  std::fs::write(&main, "\\ir schema.sql\n").unwrap();

  let state = ServerState::new();
  let url = Url::from_file_path(&main).unwrap();
  state.documents.open(url.clone(), "\\ir schema.sql\n".into(), 1);

  let links = doc_links(&state, &url);
  assert_eq!(links.len(), 1, "expected one include link, got {links:?}");
  let target = links[0].target.as_ref().expect("target");
  assert!(target.path().ends_with("schema.sql"), "got {target}");
  let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn document_link_skips_includes_that_do_not_exist() {
  // A link that always fails to open is worse than no link.
  let dir = temp_dir("missing");
  let main = dir.join("main.sql");
  let text = "\\ir nope_not_here.sql\n";
  std::fs::write(&main, text).unwrap();

  let state = ServerState::new();
  let url = Url::from_file_path(&main).unwrap();
  state.documents.open(url.clone(), text.into(), 1);

  assert!(doc_links(&state, &url).is_empty());
  let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn document_link_resolves_a_copy_path_that_exists() {
  let dir = temp_dir("copy");
  std::fs::write(dir.join("data.csv"), "a,b\n1,2\n").unwrap();
  let main = dir.join("load.sql");
  let text = "COPY t FROM 'data.csv' WITH (FORMAT csv);";
  std::fs::write(&main, text).unwrap();

  let state = ServerState::new();
  let url = Url::from_file_path(&main).unwrap();
  state.documents.open(url.clone(), text.into(), 1);

  let links = doc_links(&state, &url);
  assert_eq!(links.len(), 1, "got {links:?}");
  assert!(links[0].target.as_ref().unwrap().path().ends_with("data.csv"));
  assert!(links[0].tooltip.as_deref().unwrap_or("").contains("COPY"));
  let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn document_link_emits_urls_without_touching_the_filesystem() {
  // URLs are unverifiable, so they are always linked -- and the
  // document need not exist on disk at all.
  let (state, url) = state_with("file:///virtual.sql", "-- docs: https://example.com/guide\nSELECT 1;");
  let links = doc_links(&state, &url);
  assert_eq!(links.len(), 1, "got {links:?}");
  assert_eq!(links[0].target.as_ref().unwrap().as_str(), "https://example.com/guide");
}

#[test]
fn document_link_range_covers_exactly_the_url() {
  let src = "-- see https://example.com/x now";
  let (state, url) = state_with("file:///rng.sql", src);
  let links = doc_links(&state, &url);
  let r = links[0].range;
  assert_eq!(r.start.line, 0);
  assert_eq!(r.start.character as usize, src.find("https://").unwrap());
  assert_eq!(r.end.character as usize, src.find(" now").unwrap());
}

#[test]
fn document_link_on_a_document_with_nothing_linkable_returns_none() {
  let (state, url) = state_with("file:///plain.sql", "SELECT a FROM t WHERE b = 1;");
  assert!(doc_links(&state, &url).is_empty());
}

// ---------------------------------------------------------------------
// Resource hygiene: closing a document must not leak state, and
// oversized buffers must not be analysed.
// ---------------------------------------------------------------------

/// A buffer just over `MAX_DOC_BYTES`.
fn oversized_sql() -> String {
  let unit = "SELECT id FROM users WHERE id = 1;\n";
  let reps = (dsl_server::documents::MAX_DOC_BYTES / unit.len()) + 2;
  unit.repeat(reps)
}

#[test]
fn oversized_documents_short_circuit_every_heavy_handler() {
  use tower_lsp::lsp_types::{
    CodeLensParams, DocumentHighlightParams, DocumentSymbolParams, FoldingRangeParams, InlayHintParams,
    LinkedEditingRangeParams, SelectionRangeParams, SemanticTokensParams, SemanticTokensRangeParams,
  };
  let text = oversized_sql();
  assert!(text.len() > dsl_server::documents::MAX_DOC_BYTES);
  let (state, url) = state_with("file:///huge.sql", &text);
  let td = || TextDocumentIdentifier { uri: url.clone() };
  let origin = Position { line: 0, character: 8 };
  let range = Range { start: Position { line: 0, character: 0 }, end: Position { line: 1, character: 0 } };

  assert!(
    semantic_tokens::run(
      &state,
      SemanticTokensParams {
        text_document: td(),
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
      }
    )
    .is_none(),
    "semantic_tokens"
  );
  assert!(
    semantic_tokens::run_range(
      &state,
      SemanticTokensRangeParams {
        text_document: td(),
        range,
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
      }
    )
    .is_none(),
    "semantic_tokens_range"
  );
  assert!(
    document_symbol::run(
      &state,
      DocumentSymbolParams {
        text_document: td(),
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
      }
    )
    .is_none(),
    "document_symbol"
  );
  assert!(
    folding_range::run(
      &state,
      FoldingRangeParams {
        text_document: td(),
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
      }
    )
    .is_none(),
    "folding_range"
  );
  assert!(
    code_lens::run(
      &state,
      CodeLensParams {
        text_document: td(),
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
      }
    )
    .is_none(),
    "code_lens"
  );
  assert!(
    inlay_hints::run(
      &state,
      InlayHintParams { text_document: td(), range, work_done_progress_params: WorkDoneProgressParams::default() }
    )
    .is_none(),
    "inlay_hints"
  );
  assert!(
    document_highlight::run(
      &state,
      DocumentHighlightParams {
        text_document_position_params: TextDocumentPositionParams { text_document: td(), position: origin },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
      }
    )
    .is_none(),
    "document_highlight"
  );
  assert!(
    linked_editing::run(
      &state,
      LinkedEditingRangeParams {
        text_document_position_params: TextDocumentPositionParams { text_document: td(), position: origin },
        work_done_progress_params: WorkDoneProgressParams::default(),
      }
    )
    .is_none(),
    "linked_editing"
  );
  assert!(
    selection_range::run(
      &state,
      SelectionRangeParams {
        text_document: td(),
        positions: vec![origin],
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
      }
    )
    .is_none(),
    "selection_range"
  );
}

#[test]
fn normal_sized_documents_are_still_served_by_those_handlers() {
  // Guard against a guard that is always on.
  use tower_lsp::lsp_types::DocumentSymbolParams;
  let (state, url) = state_with("file:///normal.sql", "CREATE TABLE t (id int);");
  assert!(
    document_symbol::run(
      &state,
      DocumentSymbolParams {
        text_document: TextDocumentIdentifier { uri: url },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
      }
    )
    .is_some(),
    "a normal document must still produce symbols"
  );
}

#[test]
fn closing_a_document_evicts_its_format_cache_entry() {
  use tower_lsp::lsp_types::{DocumentFormattingParams, FormattingOptions};
  let (state, url) = state_with("file:///fmtcache.sql", "select   1;");
  let _ = formatting::run(
    &state,
    DocumentFormattingParams {
      text_document: TextDocumentIdentifier { uri: url.clone() },
      options: FormattingOptions::default(),
      work_done_progress_params: WorkDoneProgressParams::default(),
    },
  );
  assert!(state.format_cache.read().contains_key(&url.to_string()), "format should populate the cache");
  state.documents.close(&url);
  state.forget_document(&url);
  assert!(
    !state.format_cache.read().contains_key(&url.to_string()),
    "closing must drop the cached copy of the buffer"
  );
}
