//! In-memory snapshot of every open document.
//!
//! Backed by `ropey::Rope` so future incremental edits stay cheap, but for
//! v0.1 we treat each didChange as a full re-sync.

use dashmap::DashMap;
use ropey::Rope;
use std::sync::Arc;
use tower_lsp::lsp_types::Url;

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
}

impl Document {
    pub fn new(uri: Url, text: String, version: i32) -> Self {
        let rope = Rope::from_str(&text);
        Self { uri, text, version, rope }
    }

    /// True when the document exceeds [`MAX_DOC_BYTES`] -- heavy handlers
    /// should bail early in that case.
    pub fn too_large(&self) -> bool {
        self.text.len() > MAX_DOC_BYTES
    }
}

impl DocumentStore {
    pub fn open(&self, uri: Url, text: String, version: i32) {
        self.docs.insert(uri.clone(), Document::new(uri, text, version));
    }

    pub fn update(&self, uri: &Url, text: String, version: i32) {
        if let Some(mut d) = self.docs.get_mut(uri) {
            d.text = text;
            d.rope = Rope::from_str(&d.text);
            d.version = version;
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
        self.docs
            .iter()
            .map(|r| (r.key().clone(), r.value().clone()))
            .collect()
    }
}
