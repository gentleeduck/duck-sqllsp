//! `textDocument/prepareCallHierarchy` + `callHierarchy/incomingCalls` +
//! `callHierarchy/outgoingCalls`.
//!
//! Resolves the function / procedure under the cursor and answers:
//!
//!   * Incoming -- every CREATE FUNCTION / PROCEDURE body in any open
//!     buffer whose body text mentions the target function.
//!   * Outgoing -- the function bodies (in the cursor's buffer) whose
//!     own text mentions other catalog / buffer-defined functions.
//!
//! Text-scan based since pg_query doesn't expose function-call sites
//! through our internal AST. Good enough for navigation: false
//! positives only fire when a string literal contains the function
//! name without the call parens, and we filter for `name(` to avoid
//! that.

use crate::handlers::{perf, position};
use crate::state::ServerState;
use ropey::Rope;
use tower_lsp::lsp_types::{
  CallHierarchyIncomingCall, CallHierarchyIncomingCallsParams, CallHierarchyItem, CallHierarchyOutgoingCall,
  CallHierarchyOutgoingCallsParams, CallHierarchyPrepareParams, Position, Range, SymbolKind, Url,
};

pub fn prepare(state: &ServerState, params: CallHierarchyPrepareParams) -> Option<Vec<CallHierarchyItem>> {
  let uri = &params.text_document_position_params.text_document.uri;
  let _g = perf::Guard::with_uri("call_hierarchy_prepare", uri);
  let doc = state.documents.get(uri)?;
  let offset = position::to_offset(&doc.rope, params.text_document_position_params.position);
  let pos: usize = u32::from(offset) as usize;
  let name = token_at(&doc.text, pos)?;
  if name.is_empty() {
    return None;
  }
  // Locate the function definition. Workspace-wide search.
  let mut items: Vec<CallHierarchyItem> = Vec::new();
  for (other_uri, other_doc) in state.documents.snapshot() {
    if let Some((s, e, line)) = find_create_function_name(&other_doc.text, &name) {
      items.push(item_for(&other_uri, &other_doc.rope, &name, s, e, line));
    }
  }
  if items.is_empty() {
    // Synthetic item so the user can still see incoming/outgoing.
    items.push(item_for(uri, &doc.rope, &name, pos, pos + name.len(), 0));
  }
  Some(items)
}

pub fn incoming(
  state: &ServerState,
  params: CallHierarchyIncomingCallsParams,
) -> Option<Vec<CallHierarchyIncomingCall>> {
  let _g = perf::Guard::new("call_hierarchy_incoming");
  let target = params.item.name.clone();
  let mut out: Vec<CallHierarchyIncomingCall> = Vec::new();
  for (uri, doc) in state.documents.snapshot() {
    for caller in find_function_definitions(&doc.text) {
      // Find every call site of `target` inside this caller's body.
      let body_start = caller.body_start;
      let body_end = caller.body_end;
      let body = &doc.text[body_start..body_end];
      let mut ranges: Vec<Range> = Vec::new();
      for site in find_call_sites(body, &target) {
        let abs_s = body_start + site.0;
        let abs_e = body_start + site.1;
        ranges.push(Range { start: byte_to_position(&doc.rope, abs_s), end: byte_to_position(&doc.rope, abs_e) });
      }
      if ranges.is_empty() {
        continue;
      }
      out.push(CallHierarchyIncomingCall {
        from: item_for(&uri, &doc.rope, &caller.name, caller.name_start, caller.name_end, 0),
        from_ranges: ranges,
      });
    }
  }
  if out.is_empty() { None } else { Some(out) }
}

pub fn outgoing(
  state: &ServerState,
  params: CallHierarchyOutgoingCallsParams,
) -> Option<Vec<CallHierarchyOutgoingCall>> {
  let _g = perf::Guard::new("call_hierarchy_outgoing");
  let caller_name = params.item.name.clone();
  // Find the caller body in any open buffer.
  for (uri, doc) in state.documents.snapshot() {
    for caller in find_function_definitions(&doc.text) {
      if !caller.name.eq_ignore_ascii_case(&caller_name) {
        continue;
      }
      let body = &doc.text[caller.body_start..caller.body_end];
      // Group call sites by callee name.
      let mut by_callee: std::collections::BTreeMap<String, Vec<Range>> = Default::default();
      for callee in extract_called_names(body) {
        for site in find_call_sites(body, &callee) {
          let abs_s = caller.body_start + site.0;
          let abs_e = caller.body_start + site.1;
          by_callee
            .entry(callee.clone())
            .or_default()
            .push(Range { start: byte_to_position(&doc.rope, abs_s), end: byte_to_position(&doc.rope, abs_e) });
        }
      }
      if by_callee.is_empty() {
        return None;
      }
      let mut out = Vec::with_capacity(by_callee.len());
      for (callee, ranges) in by_callee {
        out.push(CallHierarchyOutgoingCall { to: item_for(&uri, &doc.rope, &callee, 0, 0, 0), from_ranges: ranges });
      }
      return Some(out);
    }
  }
  None
}

// -------- helpers -------------------------------------------------

struct FunctionDef {
  name: String,
  name_start: usize,
  name_end: usize,
  body_start: usize,
  body_end: usize,
}

/// Walk the text for CREATE [OR REPLACE] FUNCTION / PROCEDURE blocks
/// and return the name + body span (between matching dollar-quotes).
fn find_function_definitions(src: &str) -> Vec<FunctionDef> {
  let mut out = Vec::new();
  let upper = src.to_ascii_uppercase();
  for prefix in ["CREATE OR REPLACE FUNCTION ", "CREATE FUNCTION ", "CREATE OR REPLACE PROCEDURE ", "CREATE PROCEDURE "]
  {
    let mut from = 0usize;
    while let Some(rel) = upper[from..].find(prefix) {
      let after = from + rel + prefix.len();
      let bytes = src.as_bytes();
      let mut k = after;
      while k < bytes.len() && bytes[k].is_ascii_whitespace() {
        k += 1;
      }
      let id_start = k;
      while k < bytes.len() && (bytes[k].is_ascii_alphanumeric() || bytes[k] == b'_' || bytes[k] == b'.') {
        k += 1;
      }
      let id_end = k;
      if id_end > id_start {
        let raw = &src[id_start..id_end];
        let name = raw.rsplit('.').next().unwrap_or(raw).to_string();
        // Body: between `$$` and next `$$`.
        if let Some(body_start) = src[id_end..].find("$$").map(|i| id_end + i + 2)
          && let Some(rel_end) = src[body_start..].find("$$")
        {
          let body_end = body_start + rel_end;
          out.push(FunctionDef { name, name_start: id_start, name_end: id_end, body_start, body_end });
        }
      }
      from = after;
    }
  }
  out
}

/// Find the first `CREATE [OR REPLACE] FUNCTION/PROCEDURE <name>` in
/// `src` whose name matches. Returns (name_start, name_end, line) or
/// None.
fn find_create_function_name(src: &str, name: &str) -> Option<(usize, usize, u32)> {
  for f in find_function_definitions(src) {
    if f.name.eq_ignore_ascii_case(name) {
      return Some((f.name_start, f.name_end, 0));
    }
  }
  None
}

/// Find every call site of `name(` inside `body` -- excludes string /
/// comment regions.
fn find_call_sites(body: &str, name: &str) -> Vec<(usize, usize)> {
  let bytes = body.as_bytes();
  let n = bytes.len();
  let needle_lower = name.to_ascii_lowercase();
  let nlen = name.len();
  let mut out = Vec::new();
  let mut i = 0usize;
  while i < n {
    let c = bytes[i] as char;
    if c == '\'' {
      i += 1;
      while i < n && bytes[i] != b'\'' {
        i += 1;
      }
      i = (i + 1).min(n);
      continue;
    }
    if c == '-' && i + 1 < n && bytes[i + 1] == b'-' {
      while i < n && bytes[i] != b'\n' {
        i += 1;
      }
      continue;
    }
    if c == '/' && i + 1 < n && bytes[i + 1] == b'*' {
      i += 2;
      while i + 1 < n && !(bytes[i] == b'*' && bytes[i + 1] == b'/') {
        i += 1;
      }
      i = (i + 2).min(n);
      continue;
    }
    if c.is_alphabetic() || c == '_' {
      let start = i;
      while i < n && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_') {
        i += 1;
      }
      if i - start == nlen && body[start..i].eq_ignore_ascii_case(&needle_lower) {
        // Next non-space char must be `(` for it to be a call.
        let mut k = i;
        while k < n && bytes[k].is_ascii_whitespace() {
          k += 1;
        }
        if k < n && bytes[k] == b'(' {
          out.push((start, i));
        }
      }
      continue;
    }
    i += 1;
  }
  out
}

/// Extract every distinct identifier in `body` that is immediately
/// followed by `(` -- i.e. every call target.
fn extract_called_names(body: &str) -> std::collections::BTreeSet<String> {
  let bytes = body.as_bytes();
  let n = bytes.len();
  let mut out = std::collections::BTreeSet::new();
  let mut i = 0usize;
  while i < n {
    let c = bytes[i] as char;
    if c == '\'' {
      i += 1;
      while i < n && bytes[i] != b'\'' {
        i += 1;
      }
      i = (i + 1).min(n);
      continue;
    }
    if c == '-' && i + 1 < n && bytes[i + 1] == b'-' {
      while i < n && bytes[i] != b'\n' {
        i += 1;
      }
      continue;
    }
    if c.is_alphabetic() || c == '_' {
      let start = i;
      while i < n && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_') {
        i += 1;
      }
      let mut k = i;
      while k < n && bytes[k].is_ascii_whitespace() {
        k += 1;
      }
      if k < n && bytes[k] == b'(' {
        let name = body[start..i].to_string();
        // Skip flow keywords that look like calls but aren't.
        if !is_flow_keyword(&name) {
          out.insert(name);
        }
      }
      continue;
    }
    i += 1;
  }
  out
}

fn is_flow_keyword(s: &str) -> bool {
  matches!(
    s.to_ascii_uppercase().as_str(),
    "IF"
      | "WHILE"
      | "FOR"
      | "CASE"
      | "WHEN"
      | "RETURN"
      | "RAISE"
      | "SELECT"
      | "VALUES"
      | "INSERT"
      | "UPDATE"
      | "DELETE"
      | "PERFORM"
      | "EXECUTE"
      | "DECLARE"
      | "BEGIN"
      | "EXCEPTION"
      | "LOOP"
      | "EXIT"
      | "CONTINUE"
      | "ALL"
      | "ANY"
      | "OR"
      | "AND"
      | "NOT"
      | "IN"
      | "NULLIF"
      | "COALESCE"
      | "GREATEST"
      | "LEAST"
      | "CAST"
  )
}

fn token_at(src: &str, pos: usize) -> Option<String> {
  let bytes = src.as_bytes();
  let pos = pos.min(src.len());
  let mut start = pos;
  while start > 0 && is_word(bytes[start - 1] as char) {
    start -= 1;
  }
  let mut end = pos;
  while end < bytes.len() && is_word(bytes[end] as char) {
    end += 1;
  }
  if start == end {
    return None;
  }
  Some(src[start..end].to_string())
}

fn is_word(c: char) -> bool {
  c.is_alphanumeric() || c == '_'
}

fn item_for(uri: &Url, rope: &Rope, name: &str, s: usize, e: usize, _line: u32) -> CallHierarchyItem {
  let range = Range { start: byte_to_position(rope, s), end: byte_to_position(rope, e.max(s + 1)) };
  CallHierarchyItem {
    name: name.to_string(),
    kind: SymbolKind::FUNCTION,
    tags: None,
    detail: None,
    uri: uri.clone(),
    range,
    selection_range: range,
    data: None,
  }
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
