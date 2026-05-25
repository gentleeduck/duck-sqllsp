//! `workspace/symbol` handler.
//!
//! Surfaces every catalog table, every column, and every user-defined
//! function as a flat list. The editor (Telescope / Trouble / fzf-lua)
//! does the fuzzy filtering against the `query` field, so we send all
//! symbols and let the client narrow.
//!
//! Symbols don't have a stable on-disk location -- a Postgres table lives
//! in the database, not a file. We point each symbol at a synthetic URI
//! so editors that try to "go to" the symbol fall back to hover/preview
//! rather than landing on a wrong file. CREATE TABLE definitions found
//! in opened buffers get a real Location.

use crate::state::ServerState;
use dsl_parse::StatementKind;
use ropey::Rope;
use text_size::TextRange;
use tower_lsp::lsp_types::{Location, Position, Range, SymbolInformation, SymbolKind, Url, WorkspaceSymbolParams};

pub fn run(state: &ServerState, params: WorkspaceSymbolParams) -> Option<Vec<SymbolInformation>> {
  let _g = crate::handlers::perf::Guard::new("workspace_symbol");
  let query = params.query.to_ascii_lowercase();
  // Merge live catalog with workspace-wide buffer-derived catalog so
  // sequences / extensions / types / roles defined only in open files
  // show up in workspace symbol search too.
  let live = state.catalog.read().clone();
  let open_merged = state.documents.snapshot().into_iter().fold(live, |acc, (_, doc)| {
    let cache = doc.parsed();
    let derived = dsl_completion::source_tables::from_source(&cache.file, &doc.text);
    dsl_completion::source_tables::merge(&acc, &derived)
  });
  // Also fold in the on-disk workspace scan so symbols hit objects
  // defined in files the user hasn't opened yet.
  let ws_offline = state.workspace_offline_snapshot();
  let cat = dsl_completion::source_tables::merge(&open_merged, &ws_offline);
  // Score every candidate so the best match floats to the top.
  // (score, SymbolInformation) -- sort descending by score.
  let mut scored: Vec<(i32, SymbolInformation)> = Vec::new();
  let mut out: Vec<SymbolInformation> = Vec::new();

  let synthetic: Url = "duck-sqllsp://catalog".parse().ok()?;
  let blank = Range { start: Position { line: 0, character: 0 }, end: Position { line: 0, character: 0 } };

  // Locate any CREATE TABLE / CREATE FUNCTION definitions in opened
  // buffers so symbols can jump to a real spot when available.
  let table_locs = collect_table_locations(state);
  let function_locs = collect_function_locations(state);

  for t in cat.tables() {
    let Some(score) = score_match(&t.name, &query) else { continue };
    let loc = table_locs
      .iter()
      .find(|(name, _)| name.eq_ignore_ascii_case(&t.name))
      .map(|(_, l)| l.clone())
      .unwrap_or(Location { uri: synthetic.clone(), range: blank });

    #[allow(deprecated)]
    out.push(SymbolInformation {
      name: format!("{}.{}", t.schema, t.name),
      kind: SymbolKind::CLASS,
      tags: None,
      deprecated: None,
      location: loc,
      container_name: Some(t.schema.clone()),
    });

    for c in &t.columns {
      if !query.is_empty() && !c.name.to_ascii_lowercase().contains(&query) {
        continue;
      }
      #[allow(deprecated)]
      out.push(SymbolInformation {
        name: format!("{}.{}.{}", t.schema, t.name, c.name),
        kind: SymbolKind::FIELD,
        tags: None,
        deprecated: None,
        location: Location { uri: synthetic.clone(), range: blank },
        container_name: Some(format!("{}.{}", t.schema, t.name)),
      });
    }
  }

  // Buffer-defined tables (CREATE TABLE in open documents) that aren't
  // already in the catalog -- this keeps symbols useful before the DB
  // refresh completes or for ad-hoc schema files.
  for (name, loc) in &table_locs {
    if cat.tables().any(|t| t.name.eq_ignore_ascii_case(name)) {
      continue;
    }
    if !query.is_empty() && !name.to_ascii_lowercase().contains(&query) {
      continue;
    }
    #[allow(deprecated)]
    out.push(SymbolInformation {
      name: name.clone(),
      kind: SymbolKind::CLASS,
      tags: None,
      deprecated: None,
      location: loc.clone(),
      container_name: Some("(buffer)".into()),
    });
  }

  for f in &cat.functions {
    if !query.is_empty() && !f.name.to_ascii_lowercase().contains(&query) {
      continue;
    }
    let loc = function_locs
      .iter()
      .find(|(name, _)| name.eq_ignore_ascii_case(&f.name))
      .map(|(_, l)| l.clone())
      .unwrap_or(Location { uri: synthetic.clone(), range: blank });

    #[allow(deprecated)]
    out.push(SymbolInformation {
      name: format!("{}.{}", f.schema, f.name),
      kind: SymbolKind::FUNCTION,
      tags: None,
      deprecated: None,
      location: loc,
      container_name: Some(f.schema.clone()),
    });
  }

  // Sequences -- merged catalog includes both live (pg_sequences) and
  // buffer-derived (CREATE SEQUENCE in any open file).
  for s in cat.sequences() {
    if !query.is_empty() && !s.name.to_ascii_lowercase().contains(&query) {
      continue;
    }
    #[allow(deprecated)]
    out.push(SymbolInformation {
      name: format!("{}.{}", s.schema, s.name),
      kind: SymbolKind::EVENT, // closest SymbolKind for an integer generator
      tags: None,
      deprecated: None,
      location: Location { uri: synthetic.clone(), range: blank },
      container_name: Some(s.schema.clone()),
    });
  }

  // Types (enums / domains / composites).
  for t in cat.types() {
    if !query.is_empty() && !t.name.to_ascii_lowercase().contains(&query) {
      continue;
    }
    #[allow(deprecated)]
    out.push(SymbolInformation {
      name: format!("{}.{}", t.schema, t.name),
      kind: SymbolKind::ENUM,
      tags: None,
      deprecated: None,
      location: Location { uri: synthetic.clone(), range: blank },
      container_name: Some(t.schema.clone()),
    });
  }

  // Extensions.
  for e in cat.extensions() {
    if !query.is_empty() && !e.name.to_ascii_lowercase().contains(&query) {
      continue;
    }
    #[allow(deprecated)]
    out.push(SymbolInformation {
      name: e.name.clone(),
      kind: SymbolKind::PACKAGE,
      tags: None,
      deprecated: None,
      location: Location { uri: synthetic.clone(), range: blank },
      container_name: Some(e.schema.clone()),
    });
  }

  // Policies + triggers + indexes (per-table collections in catalog).
  for t in cat.tables() {
    for p in &t.policies {
      if !query.is_empty() && !p.name.to_ascii_lowercase().contains(&query) {
        continue;
      }
      #[allow(deprecated)]
      out.push(SymbolInformation {
        name: format!("{}.{}.{}", t.schema, t.name, p.name),
        kind: SymbolKind::KEY, // RLS policy -- KEY is the closest visual cue
        tags: None,
        deprecated: None,
        location: Location { uri: synthetic.clone(), range: blank },
        container_name: Some(format!("{}.{}", t.schema, t.name)),
      });
    }
    for tr in &t.triggers {
      if !query.is_empty() && !tr.name.to_ascii_lowercase().contains(&query) {
        continue;
      }
      #[allow(deprecated)]
      out.push(SymbolInformation {
        name: format!("{}.{}.{}", t.schema, t.name, tr.name),
        kind: SymbolKind::EVENT,
        tags: None,
        deprecated: None,
        location: Location { uri: synthetic.clone(), range: blank },
        container_name: Some(format!("{}.{}", t.schema, t.name)),
      });
    }
    for i in &t.indexes {
      if !query.is_empty() && !i.name.to_ascii_lowercase().contains(&query) {
        continue;
      }
      #[allow(deprecated)]
      out.push(SymbolInformation {
        name: format!("{}.{}.{}", t.schema, t.name, i.name),
        kind: SymbolKind::FIELD, // indexes act like indexed projections of fields
        tags: None,
        deprecated: None,
        location: Location { uri: synthetic.clone(), range: blank },
        container_name: Some(format!("{}.{}", t.schema, t.name)),
      });
    }
    for c in &t.constraints {
      if !query.is_empty() && !c.name.to_ascii_lowercase().contains(&query) {
        continue;
      }
      #[allow(deprecated)]
      out.push(SymbolInformation {
        name: format!("{}.{}.{}", t.schema, t.name, c.name),
        kind: SymbolKind::INTERFACE, // constraint -- closest LSP-symbol cue
        tags: None,
        deprecated: None,
        location: Location { uri: synthetic.clone(), range: blank },
        container_name: Some(format!("{}.{}", t.schema, t.name)),
      });
    }
  }

  if out.is_empty() { None } else { Some(out) }
}

/// Walk every opened buffer, returning (table_name, Location) for each
/// CREATE TABLE statement found.
fn collect_table_locations(state: &ServerState) -> Vec<(String, Location)> {
  let mut out = Vec::new();
  for (uri, doc) in state.documents.snapshot() {
    let cache = doc.parsed();
    for s in &cache.file.statements {
      if let StatementKind::CreateTable(ct) = &s.kind {
        out.push((ct.table.name.clone(), Location { uri: uri.clone(), range: to_lsp_range(&doc.rope, s.range) }));
      }
    }
  }
  out
}

/// Walk every opened buffer for `CREATE [OR REPLACE] FUNCTION <name>`.
fn collect_function_locations(state: &ServerState) -> Vec<(String, Location)> {
  let mut out = Vec::new();
  for (uri, doc) in state.documents.snapshot() {
    let upper = doc.text.to_ascii_uppercase();
    for needle in ["CREATE OR REPLACE FUNCTION ", "CREATE FUNCTION "] {
      let mut from = 0usize;
      while let Some(rel) = upper[from..].find(needle) {
        let after = from + rel + needle.len();
        let rest = &doc.text[after..];
        let trimmed_lead = rest.len() - rest.trim_start().len();
        let body = &rest[trimmed_lead..];
        let name: String = body.chars().take_while(|c| c.is_alphanumeric() || *c == '_' || *c == '.').collect();
        if !name.is_empty() {
          let name = name.rsplit('.').next().unwrap_or(&name).to_string();
          let s = after + trimmed_lead;
          let e = s + name.len();
          out.push((
            name,
            Location {
              uri: uri.clone(),
              range: to_lsp_range(&doc.rope, TextRange::new((s as u32).into(), (e as u32).into())),
            },
          ));
        }
        from = after;
      }
    }
  }
  out
}

fn to_lsp_range(rope: &Rope, r: TextRange) -> Range {
  let s: u32 = r.start().into();
  let e: u32 = r.end().into();
  Range { start: byte_to_position(rope, s as usize), end: byte_to_position(rope, (e as usize).min(rope.len_bytes())) }
}

fn byte_to_position(rope: &Rope, byte: usize) -> Position {
  let byte = byte.min(rope.len_bytes());
  let line = rope.byte_to_line(byte);
  let line_start_byte = rope.line_to_byte(line);
  let line_slice = rope.line(line);
  let mut utf16 = 0u32;
  let mut bytes_seen = 0usize;
  let bytes_in_line = byte.saturating_sub(line_start_byte);
  for c in line_slice.chars() {
    if bytes_seen >= bytes_in_line {
      break;
    }
    utf16 += c.len_utf16() as u32;
    bytes_seen += c.len_utf8();
  }
  Position { line: line as u32, character: utf16 }
}

/// Score a candidate symbol name against the user's lowercased query.
/// Returns None for no-match, higher = better. Heuristics:
///   100 -- exact match
///    90 -- prefix match
///    80 -- snake_case initials prefix (`ur` matches `user_roles`)
///    70 -- camelCase initials prefix (`uR` matches `userRoles`)
///    50 -- substring match
///    20 -- subsequence match (chars in order)
fn score_match(name: &str, query: &str) -> Option<i32> {
  if query.is_empty() {
    return Some(0);
  }
  let lower = name.to_ascii_lowercase();
  if lower == query {
    return Some(100);
  }
  if lower.starts_with(query) {
    return Some(90);
  }
  let snake_initials: String = lower.split('_').filter_map(|seg| seg.chars().next()).collect();
  if snake_initials.starts_with(query) {
    return Some(80);
  }
  let camel_initials: String = name
    .chars()
    .enumerate()
    .filter_map(|(i, c)| {
      if i == 0 {
        Some(c.to_ascii_lowercase())
      } else if c.is_ascii_uppercase() {
        Some(c.to_ascii_lowercase())
      } else {
        None
      }
    })
    .collect();
  if camel_initials.starts_with(query) {
    return Some(70);
  }
  if lower.contains(query) {
    return Some(50);
  }
  if is_subsequence(query, &lower) {
    return Some(20);
  }
  None
}

fn is_subsequence(needle: &str, haystack: &str) -> bool {
  let mut hit = haystack.chars();
  needle.chars().all(|c| hit.any(|h| h == c))
}
