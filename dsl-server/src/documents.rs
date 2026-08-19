//! In-memory snapshot of every open document.
//!
//! Backed by `ropey::Rope`, which lets `didChange` apply the editor's
//! delta in place instead of re-sending and re-ingesting the whole
//! buffer on every keystroke. See [`DocumentStore::apply_changes`].

use dashmap::DashMap;
use dsl_catalog::Catalog;
use dsl_parse::{Dialect, ParsedFile};
use dsl_resolve::Scope;
use ropey::Rope;
use std::sync::{Arc, OnceLock};
use tower_lsp::lsp_types::{TextDocumentContentChangeEvent, Url};

/// Cap on the document size we are willing to parse / analyse. Beyond
/// this, heavy handlers (completion, hover, semantic tokens, etc.)
/// short-circuit so the LSP never blocks the editor on a multi-MB dump.
/// 4 MiB covers any realistic hand-edited schema/migration file.
pub const MAX_DOC_BYTES: usize = 4 * 1024 * 1024;

#[derive(Clone, Default)]
pub struct DocumentStore {
  docs: Arc<DashMap<Url, Document>>,
}

#[derive(Clone)]
pub struct Document {
  pub uri: Url,
  pub text: String,
  pub version: i32,
  pub rope: Rope,
  /// Dialect to parse this buffer against. Server pushes this from
  /// the resolved [`DuckSqllspConfig::effective_dialect`] at didOpen /
  /// configuration-change time; defaults to Postgres when unknown.
  pub dialect: Dialect,
  /// Lazily-populated parse + scope cache. Cleared on every update --
  /// the first heavy handler after didChange pays the parse cost, the
  /// rest reuse it. Wrapped in `Arc` so clones from `DashMap::get`
  /// don't re-run the parser.
  parse_cache: Arc<OnceLock<Arc<ParseCache>>>,
  /// Lazily-populated buffer-derived catalog -- see
  /// `dsl_completion::source_tables::from_source`. A separate
  /// `OnceLock` from `parse_cache` so `document_symbol` / `code_lens`
  /// (which call `parsed()` but never need this) don't pay for a
  /// derivation they don't use. Cleared alongside `parse_cache` on
  /// every text/dialect change.
  derived_cache: Arc<OnceLock<Arc<Catalog>>>,
}

pub struct ParseCache {
  pub file: ParsedFile,
  pub scopes: Vec<Scope>,
  /// `Document.version` that the cache was built from. Handlers can
  /// compare this against the current `DocumentStore` snapshot to
  /// detect mid-flight cancellation -- if a newer `didChange` came
  /// in, drop the in-flight result instead of shipping stale data.
  pub version: i32,
}

impl Document {
  pub fn new(uri: Url, text: String, version: i32) -> Self {
    Self::with_dialect(uri, text, version, Dialect::Postgres)
  }

  pub fn with_dialect(uri: Url, text: String, version: i32, dialect: Dialect) -> Self {
    let rope = Rope::from_str(&text);
    Self {
      uri,
      text,
      version,
      rope,
      dialect,
      parse_cache: Arc::new(OnceLock::new()),
      derived_cache: Arc::new(OnceLock::new()),
    }
  }

  /// True when the document exceeds [`MAX_DOC_BYTES`] -- heavy handlers
  /// should bail early in that case.
  pub fn too_large(&self) -> bool {
    self.text.len() > MAX_DOC_BYTES
  }

  /// Parsed AST + per-statement scopes for this document. First call
  /// runs the parser/resolver; subsequent calls return the cached
  /// value. Cleared on every `DocumentStore::update`.
  pub fn parsed(&self) -> Arc<ParseCache> {
    let version = self.version;
    let dialect = self.dialect;
    self
      .parse_cache
      .get_or_init(|| {
        let file = dsl_parse::parse(&self.text, dialect);
        let scopes = dsl_resolve::resolve_with_source(&file.statements, &self.text);
        Arc::new(ParseCache { file, scopes, version })
      })
      .clone()
  }

  /// Buffer-derived catalog (tables from AST + sequences / types /
  /// extensions / functions / roles scanned from raw text). Expensive
  /// multi-pass text scan, so it's computed once per document version
  /// and cached here instead of every handler re-deriving it
  /// independently.
  pub fn derived_catalog(&self) -> Arc<Catalog> {
    let cache = self.parsed();
    self
      .derived_cache
      .get_or_init(|| Arc::new(dsl_completion::source_tables::from_source(&cache.file, &self.text)))
      .clone()
  }
}

impl DocumentStore {
  /// True when the current document version is newer than `version`.
  /// Heavy async handlers (diagnostics) compare the version their
  /// parse cache was built from against this; if newer, the request
  /// is effectively cancelled by a fresher didChange and we should
  /// drop the in-flight result.
  pub fn is_stale(&self, uri: &Url, version: i32) -> bool {
    self.docs.get(uri).map(|d| d.version > version).unwrap_or(true)
  }
}

impl DocumentStore {
  pub fn open(&self, uri: Url, text: String, version: i32) {
    self.docs.insert(uri.clone(), Document::new(uri, text, version));
  }

  pub fn open_with_dialect(&self, uri: Url, text: String, version: i32, dialect: Dialect) {
    self.docs.insert(uri.clone(), Document::with_dialect(uri, text, version, dialect));
  }

  /// Update the dialect for every open document. Called when the
  /// effective config dialect changes. Invalidates parse cache so
  /// the next handler re-parses with the new dialect.
  pub fn set_dialect_all(&self, dialect: Dialect) {
    for mut entry in self.docs.iter_mut() {
      if entry.dialect != dialect {
        entry.dialect = dialect;
        entry.parse_cache = Arc::new(OnceLock::new());
        entry.derived_cache = Arc::new(OnceLock::new());
      }
    }
  }

  /// Apply an incremental `didChange` batch.
  ///
  /// Each event is either a ranged delta or (when `range` is `None`) a
  /// whole-document replacement -- clients may mix both in one
  /// notification, and a server that advertises `INCREMENTAL` must
  /// still handle the full form.
  ///
  /// Order matters: per the LSP spec each change's range refers to the
  /// document *as of the previous change in the same batch*, so they
  /// are applied strictly in sequence and never sorted or merged.
  ///
  /// Positions are UTF-16 code units. `to_offset` handles that
  /// conversion, so a delta landing after an emoji or any other
  /// astral-plane character maps to the right byte -- the classic way
  /// incremental sync corrupts a buffer.
  pub fn apply_changes(&self, uri: &Url, changes: Vec<TextDocumentContentChangeEvent>, version: i32) {
    let Some(mut d) = self.docs.get_mut(uri) else { return };
    if changes.is_empty() {
      d.version = version;
      return;
    }
    let mut dirty = false;
    for change in changes {
      match change.range {
        None => {
          // Full replacement: cheaper to rebuild than to diff.
          if d.text == change.text {
            continue;
          }
          d.rope = Rope::from_str(&change.text);
          d.text = change.text;
          dirty = true;
        },
        Some(range) => {
          let start_byte = usize::from(crate::handlers::position::to_offset(&d.rope, range.start));
          let end_byte = usize::from(crate::handlers::position::to_offset(&d.rope, range.end));
          // A reversed range would panic inside ropey; normalise
          // rather than trusting the client.
          let (start_byte, end_byte) =
            if start_byte <= end_byte { (start_byte, end_byte) } else { (end_byte, start_byte) };
          let start_char = d.rope.byte_to_char(start_byte.min(d.rope.len_bytes()));
          let end_char = d.rope.byte_to_char(end_byte.min(d.rope.len_bytes()));
          if start_char == end_char && change.text.is_empty() {
            continue;
          }
          if start_char < end_char {
            d.rope.remove(start_char..end_char);
          }
          if !change.text.is_empty() {
            d.rope.insert(start_char, &change.text);
          }
          dirty = true;
        },
      }
    }
    d.version = version;
    if !dirty {
      // Editors re-send identical content on save/blur; skip the
      // string rebuild and keep every cached parse alive.
      return;
    }
    // `text` is the flat mirror every handler reads. Rebuild it once
    // per batch rather than once per change.
    d.text = d.rope.to_string();
    d.parse_cache = Arc::new(OnceLock::new());
    d.derived_cache = Arc::new(OnceLock::new());
  }

  pub fn update(&self, uri: &Url, text: String, version: i32) {
    if let Some(mut d) = self.docs.get_mut(uri) {
      // Incremental fast-path: editors regularly re-send an
      // identical full buffer (format-on-save with no diff, save
      // hooks, autosave-on-blur). Skip rope rebuild + parse
      // invalidation when bytes are unchanged -- only bump the
      // version. Any in-flight handler keyed to the old version
      // is correct for the new one too.
      if d.text == text {
        d.version = version;
        return;
      }
      d.text = text;
      d.rope = Rope::from_str(&d.text);
      d.version = version;
      d.parse_cache = Arc::new(OnceLock::new());
      d.derived_cache = Arc::new(OnceLock::new());
    }
  }

  pub fn close(&self, uri: &Url) {
    self.docs.remove(uri);
  }

  pub fn get(&self, uri: &Url) -> Option<Document> {
    self.docs.get(uri).map(|r| r.clone())
  }

  /// Snapshot of all open URIs paired with their documents. Used by
  /// workspace-scoped handlers (`workspace/symbol`, project-wide refs).
  pub fn snapshot(&self) -> Vec<(Url, Document)> {
    self.docs.iter().map(|r| (r.key().clone(), r.value().clone())).collect()
  }
}

#[cfg(test)]
mod incremental_tests {
  use super::*;
  use tower_lsp::lsp_types::{Position, Range};

  fn store_with(text: &str) -> (DocumentStore, Url) {
    let store = DocumentStore::default();
    let url: Url = "file:///inc.sql".parse().unwrap();
    store.open(url.clone(), text.into(), 1);
    (store, url)
  }

  fn delta(range: Range, text: &str) -> TextDocumentContentChangeEvent {
    TextDocumentContentChangeEvent { range: Some(range), range_length: None, text: text.into() }
  }

  fn at(line: u32, character: u32) -> Position {
    Position { line, character }
  }

  /// The rope and its flat `text` mirror must never disagree -- every
  /// handler reads one or the other.
  fn assert_consistent(store: &DocumentStore, url: &Url) -> String {
    let d = store.get(url).unwrap();
    assert_eq!(d.text, d.rope.to_string(), "text mirror drifted from the rope");
    d.text
  }

  #[test]
  fn single_insert_applies_at_the_right_offset() {
    let (store, url) = store_with("SELECT a FROM t;");
    store.apply_changes(&url, vec![delta(Range { start: at(0, 7), end: at(0, 8) }, "b")], 2);
    assert_eq!(assert_consistent(&store, &url), "SELECT b FROM t;");
  }

  #[test]
  fn pure_insertion_with_an_empty_range_shifts_the_tail() {
    let (store, url) = store_with("SELECT FROM t;");
    store.apply_changes(&url, vec![delta(Range { start: at(0, 7), end: at(0, 7) }, "x ")], 2);
    assert_eq!(assert_consistent(&store, &url), "SELECT x FROM t;");
  }

  #[test]
  fn pure_deletion_removes_the_range() {
    let (store, url) = store_with("SELECT a, b FROM t;");
    store.apply_changes(&url, vec![delta(Range { start: at(0, 8), end: at(0, 11) }, "")], 2);
    assert_eq!(assert_consistent(&store, &url), "SELECT a FROM t;");
  }

  #[test]
  fn multi_line_range_edit_spans_lines() {
    let (store, url) = store_with("SELECT a\nFROM t\nWHERE x;");
    // (0,7)..(2,7) spans from the `a` through `WHERE x`, leaving the `;`.
    store.apply_changes(&url, vec![delta(Range { start: at(0, 7), end: at(2, 7) }, "b FROM u WHERE y")], 2);
    assert_eq!(assert_consistent(&store, &url), "SELECT b FROM u WHERE y;");
  }

  #[test]
  fn batched_changes_apply_in_sequence_not_against_the_original() {
    // Each range is relative to the document *after* the previous
    // change. Applying them against the original text would misplace
    // the second edit.
    let (store, url) = store_with("SELECT a FROM t;");
    store.apply_changes(
      &url,
      vec![
        delta(Range { start: at(0, 7), end: at(0, 8) }, "col_one"),
        delta(Range { start: at(0, 14), end: at(0, 14) }, ", col_two"),
      ],
      2,
    );
    assert_eq!(assert_consistent(&store, &url), "SELECT col_one, col_two FROM t;");
  }

  #[test]
  fn utf16_positions_after_an_astral_character_map_to_the_right_byte() {
    // The emoji is 4 UTF-8 bytes but 2 UTF-16 code units. An
    // implementation that treats LSP columns as bytes or as chars puts
    // this edit in the wrong place -- the classic incremental-sync
    // corruption.
    let (store, url) = store_with("SELECT '🦆' AS duck, a FROM t;");
    let prefix_utf16 = "SELECT '🦆' AS duck, ".encode_utf16().count() as u32;
    store.apply_changes(&url, vec![delta(Range { start: at(0, prefix_utf16), end: at(0, prefix_utf16 + 1) }, "b")], 2);
    assert_eq!(assert_consistent(&store, &url), "SELECT '🦆' AS duck, b FROM t;");
  }

  #[test]
  fn full_replacement_events_are_still_honoured() {
    // A client that advertises incremental sync may still send a
    // range-less event; the spec allows mixing.
    let (store, url) = store_with("SELECT a;");
    store.apply_changes(
      &url,
      vec![TextDocumentContentChangeEvent { range: None, range_length: None, text: "SELECT b;".into() }],
      2,
    );
    assert_eq!(assert_consistent(&store, &url), "SELECT b;");
  }

  #[test]
  fn mixed_full_then_ranged_batch_applies_in_order() {
    let (store, url) = store_with("old");
    store.apply_changes(
      &url,
      vec![
        TextDocumentContentChangeEvent { range: None, range_length: None, text: "SELECT a;".into() },
        delta(Range { start: at(0, 7), end: at(0, 8) }, "z"),
      ],
      2,
    );
    assert_eq!(assert_consistent(&store, &url), "SELECT z;");
  }

  #[test]
  fn a_change_invalidates_the_parse_cache() {
    let (store, url) = store_with("SELECT a FROM t;");
    let before = Arc::as_ptr(&store.get(&url).unwrap().parsed());
    store.apply_changes(&url, vec![delta(Range { start: at(0, 7), end: at(0, 8) }, "b")], 2);
    let after = Arc::as_ptr(&store.get(&url).unwrap().parsed());
    assert_ne!(before, after, "stale parse cache would outlive the edit");
  }

  #[test]
  fn a_no_op_change_keeps_the_parse_cache_alive() {
    let (store, url) = store_with("SELECT a FROM t;");
    let before = Arc::as_ptr(&store.get(&url).unwrap().parsed());
    store.apply_changes(&url, vec![delta(Range { start: at(0, 3), end: at(0, 3) }, "")], 2);
    let after = Arc::as_ptr(&store.get(&url).unwrap().parsed());
    assert_eq!(before, after, "an empty edit should not force a re-parse");
    assert_eq!(store.get(&url).unwrap().version, 2, "version must still advance");
  }

  #[test]
  fn out_of_bounds_and_reversed_ranges_do_not_panic() {
    let (store, url) = store_with("SELECT a;");
    store.apply_changes(&url, vec![delta(Range { start: at(99, 99), end: at(200, 0) }, "!")], 2);
    store.apply_changes(&url, vec![delta(Range { start: at(0, 6), end: at(0, 2) }, "")], 3);
    let _ = assert_consistent(&store, &url);
  }

  #[test]
  fn changes_for_an_unopened_document_are_ignored() {
    let store = DocumentStore::default();
    let url: Url = "file:///never.sql".parse().unwrap();
    store.apply_changes(&url, vec![delta(Range { start: at(0, 0), end: at(0, 0) }, "x")], 2);
    assert!(store.get(&url).is_none());
  }

  #[test]
  fn an_empty_change_batch_only_bumps_the_version() {
    let (store, url) = store_with("SELECT a;");
    store.apply_changes(&url, vec![], 7);
    let d = store.get(&url).unwrap();
    assert_eq!(d.text, "SELECT a;");
    assert_eq!(d.version, 7);
  }
}
