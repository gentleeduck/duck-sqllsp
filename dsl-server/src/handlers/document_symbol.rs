//! `textDocument/documentSymbol` handler.
//!
//! Walks the parsed statements and surfaces an outline entry per
//! recognised top-level statement. For CREATE TABLE we nest each
//! defined column as a child symbol so the editor's outline panel
//! shows the full schema at a glance.

use crate::handlers::position;
use crate::state::ServerState;
use dsl_parse::{Statement, StatementKind};
use ropey::Rope;
use text_size::TextRange;
use tower_lsp::lsp_types::{DocumentSymbol, DocumentSymbolParams, DocumentSymbolResponse, Position, Range, SymbolKind};

pub fn run(state: &ServerState, params: DocumentSymbolParams) -> Option<DocumentSymbolResponse> {
  let _g = crate::handlers::perf::Guard::with_uri("document_symbol", &params.text_document.uri);
  let doc = state.documents.get(&params.text_document.uri)?;
  let cache = doc.parsed();
  let mut out = Vec::new();
  for s in &cache.file.statements {
    if let Some(sym) = symbol_for(s, &doc.text, &doc.rope) {
      out.push(sym);
    }
  }
  Some(DocumentSymbolResponse::Nested(out))
}

fn symbol_for(stmt: &Statement, text: &str, rope: &Rope) -> Option<DocumentSymbol> {
  let range = to_lsp_range(rope, stmt.range);
  match &stmt.kind {
    StatementKind::CreateTable(ct) => {
      let mut children: Vec<DocumentSymbol> = ct
        .columns
        .iter()
        .map(|c| {
          // Prefer the column's own range over the whole-stmt
          // range so the editor can jump straight to that
          // column definition.
          let col_range =
            if c.range.len() > text_size::TextSize::from(0) { to_lsp_range(rope, c.range) } else { range };
          // Compose a single-line detail like
          // `uuid NOT NULL DEFAULT gen_random_uuid()`.
          let mut detail = c.type_name.clone();
          if !c.nullable {
            detail.push_str(" NOT NULL");
          }
          if let Some(d) = &c.default {
            detail.push_str(&format!(" DEFAULT {d}"));
          }
          #[allow(deprecated)]
          DocumentSymbol {
            name: c.name.clone(),
            detail: Some(detail),
            kind: SymbolKind::FIELD,
            tags: None,
            deprecated: None,
            range: col_range,
            selection_range: col_range,
            children: None,
          }
        })
        .collect();
      // Scan for table-level constraints (named + anonymous) and
      // surface each one under the table.
      children.extend(scan_table_constraints(text, stmt.range, rope));
      Some(make_symbol(&ct.table.name, Some("table".into()), SymbolKind::CLASS, range, Some(children)))
    },
    StatementKind::AlterTable(at) => {
      Some(make_symbol(&at.table.name, Some("alter".into()), SymbolKind::CLASS, range, None))
    },
    StatementKind::DropTable(d) => Some(make_symbol(
      &d.tables.first().map(|t| t.name.clone()).unwrap_or_else(|| "drop table".into()),
      Some("drop".into()),
      SymbolKind::CLASS,
      range,
      None,
    )),
    StatementKind::Insert(i) => {
      Some(make_symbol(&format!("INSERT INTO {}", i.table.name), None, SymbolKind::EVENT, range, None))
    },
    StatementKind::Update(u) => {
      Some(make_symbol(&format!("UPDATE {}", u.table.name), None, SymbolKind::EVENT, range, None))
    },
    StatementKind::Delete(d) => {
      Some(make_symbol(&format!("DELETE FROM {}", d.table.name), None, SymbolKind::EVENT, range, None))
    },
    StatementKind::Select(s) => {
      let from = s.from.first().map(|t| t.name.clone()).unwrap_or_else(|| "?".into());
      Some(make_symbol(&format!("SELECT ... FROM {from}"), None, SymbolKind::FUNCTION, range, None))
    },
    StatementKind::Unknown { text: t } => {
      // Detect CREATE FUNCTION / CREATE TRIGGER textually so they
      // still show up in the outline.
      let upper = t.to_ascii_uppercase();
      if upper.starts_with("CREATE OR REPLACE FUNCTION") || upper.starts_with("CREATE FUNCTION") {
        let name = extract_function_name(t);
        return Some(make_symbol(&name, Some("function".into()), SymbolKind::FUNCTION, range, None));
      }
      if upper.starts_with("CREATE TRIGGER") {
        let name = extract_trigger_name(t);
        return Some(make_symbol(&name, Some("trigger".into()), SymbolKind::EVENT, range, None));
      }
      let _ = text; // unused for now
      None
    },
  }
}

fn make_symbol(
  name: &str,
  detail: Option<String>,
  kind: SymbolKind,
  range: Range,
  children: Option<Vec<DocumentSymbol>>,
) -> DocumentSymbol {
  #[allow(deprecated)]
  DocumentSymbol {
    name: name.to_string(),
    detail,
    kind,
    tags: None,
    deprecated: None,
    range,
    selection_range: range,
    children,
  }
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

/// Walk the body of a CREATE TABLE statement and yield a DocumentSymbol
/// per table-level constraint. Recognises both named (`CONSTRAINT x
/// CHECK (...)`) and anonymous forms (`PRIMARY KEY (...)`, `UNIQUE (...)`,
/// `FOREIGN KEY (...)`, `CHECK (...)`).
fn scan_table_constraints(source: &str, stmt_range: TextRange, rope: &Rope) -> Vec<DocumentSymbol> {
  let mut out = Vec::new();
  let s: u32 = stmt_range.start().into();
  let e: u32 = stmt_range.end().into();
  let start = s as usize;
  let end = (e as usize).min(source.len());
  let body_full = &source[start..end];
  let upper = body_full.to_ascii_uppercase();
  let Some(open) = upper.find('(') else { return out };
  let bytes = body_full.as_bytes();
  let n = bytes.len();
  // Walk top-level comma-separated entries.
  let mut d = 1i32;
  let mut item_start = open + 1;
  let mut i = open + 1;
  while i < n && d > 0 {
    match bytes[i] {
      b'(' => d += 1,
      b')' => {
        d -= 1;
        if d == 0 {
          if let Some(sym) = classify_entry(body_full, item_start, i, start, rope) {
            out.push(sym);
          }
          break;
        }
      },
      b'\'' => {
        i += 1;
        while i < n && bytes[i] != b'\'' {
          i += 1;
        }
      },
      b',' if d == 1 => {
        if let Some(sym) = classify_entry(body_full, item_start, i, start, rope) {
          out.push(sym);
        }
        item_start = i + 1;
      },
      _ => {},
    }
    i += 1;
  }
  out
}

fn classify_entry(
  body: &str,
  entry_start: usize,
  entry_end: usize,
  abs_offset: usize,
  rope: &Rope,
) -> Option<DocumentSymbol> {
  let raw = &body[entry_start..entry_end];
  let trimmed = raw.trim();
  if trimmed.is_empty() {
    return None;
  }
  let upper = trimmed.to_ascii_uppercase();
  let (name, detail) = if upper.starts_with("CONSTRAINT") {
    // `CONSTRAINT <name> <kind> (...)`
    let rest = trimmed[10..].trim_start();
    let cname: String = rest.chars().take_while(|c| c.is_alphanumeric() || *c == '_' || *c == '"').collect();
    let cname = cname.trim_matches('"').to_string();
    if cname.is_empty() {
      return None;
    }
    let after_name = rest[cname.len()..].trim_start();
    let kind = constraint_kind_label(after_name);
    (cname, kind)
  } else if upper.starts_with("PRIMARY KEY") {
    ("PRIMARY KEY".to_string(), "primary key".to_string())
  } else if upper.starts_with("UNIQUE") {
    ("UNIQUE".to_string(), "unique".to_string())
  } else if upper.starts_with("FOREIGN KEY") {
    ("FOREIGN KEY".to_string(), "foreign key".to_string())
  } else if upper.starts_with("CHECK") {
    ("CHECK".to_string(), "check".to_string())
  } else if upper.starts_with("EXCLUDE") {
    ("EXCLUDE".to_string(), "exclude".to_string())
  } else {
    return None;
  };
  let leading_ws = raw.len() - raw.trim_start().len();
  let abs_start = abs_offset + entry_start + leading_ws;
  let abs_end = abs_offset + entry_start + leading_ws + trimmed.len();
  let r = TextRange::new((abs_start as u32).into(), (abs_end as u32).into());
  let lsp_r = to_lsp_range(rope, r);
  #[allow(deprecated)]
  Some(DocumentSymbol {
    name,
    detail: Some(detail),
    kind: SymbolKind::PROPERTY,
    tags: None,
    deprecated: None,
    range: lsp_r,
    selection_range: lsp_r,
    children: None,
  })
}

fn constraint_kind_label(after_name: &str) -> String {
  let upper = after_name.to_ascii_uppercase();
  if upper.starts_with("PRIMARY KEY") {
    "primary key".into()
  } else if upper.starts_with("FOREIGN KEY") {
    "foreign key".into()
  } else if upper.starts_with("UNIQUE") {
    "unique".into()
  } else if upper.starts_with("CHECK") {
    "check".into()
  } else if upper.starts_with("EXCLUDE") {
    "exclude".into()
  } else {
    "constraint".into()
  }
}

fn extract_function_name(t: &str) -> String {
  let upper = t.to_ascii_uppercase();
  let idx = upper.find("FUNCTION").map(|i| i + "FUNCTION".len()).unwrap_or(0);
  t[idx..]
    .trim_start()
    .split(|c: char| c.is_whitespace() || c == '(')
    .find(|s| !s.is_empty())
    .unwrap_or("<function>")
    .to_string()
}

fn extract_trigger_name(t: &str) -> String {
  let upper = t.to_ascii_uppercase();
  let idx = upper.find("TRIGGER").map(|i| i + "TRIGGER".len()).unwrap_or(0);
  t[idx..].trim_start().split(|c: char| c.is_whitespace()).find(|s| !s.is_empty()).unwrap_or("<trigger>").to_string()
}

// Unused; placeholder for future per-statement position math.
#[allow(dead_code)]
fn _via_position(rope: &Rope, byte: usize) -> Position {
  position::to_offset(rope, Position { line: 0, character: byte as u32 });
  Position::default()
}
