//! Unit tests for the per-handler logic without spinning up the LSP wire.

use dsl_server::{
  documents::DocumentStore,
  handlers::{
    code_action, completion, definition, document_highlight, document_symbol, folding_range, hover,
    inlay_hints, linked_editing, on_type_formatting, references, rename, selection_range,
    semantic_tokens, signature_help, type_definition, workspace_symbol,
  },
  state::ServerState,
};
use tower_lsp::lsp_types::{
  CompletionParams, CompletionResponse, HoverParams, PartialResultParams, Position, TextDocumentIdentifier,
  TextDocumentPositionParams, Url, WorkDoneProgressParams,
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
fn linked_editing_returns_all_in_statement_occurrences() {
  use tower_lsp::lsp_types::{
    LinkedEditingRangeParams, TextDocumentIdentifier, TextDocumentPositionParams,
    WorkDoneProgressParams,
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
    GotoDefinitionParams, GotoDefinitionResponse, TextDocumentIdentifier, TextDocumentPositionParams,
    WorkDoneProgressParams, PartialResultParams,
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
  ).expect("def");
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
    GotoDefinitionParams, GotoDefinitionResponse, TextDocumentIdentifier, TextDocumentPositionParams,
    WorkDoneProgressParams, PartialResultParams,
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
  ).expect("def");
  let loc = match resp {
    GotoDefinitionResponse::Scalar(l) => l,
    _ => panic!("expected scalar"),
  };
  assert_eq!(loc.uri, schema, "jump should land in the schema buffer");
}

#[test]
fn definition_jumps_to_cte_binding() {
  use tower_lsp::lsp_types::{
    GotoDefinitionParams, GotoDefinitionResponse, TextDocumentIdentifier, TextDocumentPositionParams,
    WorkDoneProgressParams, PartialResultParams,
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
  ).expect("def");
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
    TextDocumentIdentifier, TextDocumentPositionParams, WorkDoneProgressParams, PartialResultParams,
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
    TextDocumentIdentifier, TextDocumentPositionParams, WorkDoneProgressParams, PartialResultParams,
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
  let user_id_chip = hints.iter().find(|h| {
    matches!(&h.label, tower_lsp::lsp_types::InlayHintLabel::String(s) if s == "user_id")
  }).unwrap();
  // The chip's character should sit at the column where `'id_1'` starts.
  let line1 = "INSERT INTO user_roles (user_id, role) VALUES ('id_1', 'admin');";
  let expected_col = line1.find("'id_1'").unwrap() as u32;
  assert_eq!(user_id_chip.position.character, expected_col,
             "chip should sit at start of literal, got {} expected {}",
             user_id_chip.position.character, expected_col);
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
  let doc = it.documentation.as_ref().expect("snippet doc set");
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
