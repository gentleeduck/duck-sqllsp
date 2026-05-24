//! `textDocument/signatureHelp` handler.
//!
//! When the cursor is inside `fn(arg1, arg2|)`, we want to display the
//! function's formal signature with the active parameter highlighted.
//! Approach:
//!   1. Walk backwards from the cursor looking for the unmatched `(`.
//!   2. Read the identifier immediately before that `(` -- that's the
//!      function name. Bail if not an identifier (e.g. tuple paren).
//!   3. Count commas between the open paren and the cursor (skipping
//!      nested parens / strings) -> active parameter index.
//!   4. Look up the signature in dsl-knowledge.functions, then fall back
//!      to the catalog functions returned from the live DB.

use crate::handlers::position;
use crate::state::ServerState;
use tower_lsp::lsp_types::{
  ParameterInformation, ParameterLabel, SignatureHelp, SignatureHelpParams, SignatureInformation,
};

pub fn run(state: &ServerState, params: SignatureHelpParams) -> Option<SignatureHelp> {
  let uri = params.text_document_position_params.text_document.uri;
  let _g = crate::handlers::perf::Guard::with_uri("signature_help", &uri);
  let doc = state.documents.get(&uri)?;
  let offset = position::to_offset(&doc.rope, params.text_document_position_params.position);
  let pos: usize = offset.into();
  let pos = pos.min(doc.text.len());

  let (open_paren, arg_index) = find_enclosing_call(&doc.text, pos)?;

  // UPDATE ... SET (a, b, c) = (|...) -- the enclosing `(` is the
  // rhs tuple; no identifier precedes it (`=` does). Check this
  // BEFORE the function-name lookup, which would bail.
  if let Some(sig) = update_set_tuple_signature(&doc.text, &state.catalog.read(), open_paren, arg_index) {
    return Some(sig);
  }

  let name = identifier_before(&doc.text, open_paren)?;

  // INSERT INTO t (a, b, c) VALUES (|...) -- when the enclosing `(`
  // belongs to a VALUES tuple, treat each declared column as a
  // signature parameter. Active index = comma count from VALUES `(`.
  if name.eq_ignore_ascii_case("VALUES") {
    if let Some(sig) = insert_values_signature(&doc.text, &state.catalog.read(), open_paren, arg_index) {
      return Some(sig);
    }
  }

  // Knowledge base first.
  let kb = dsl_knowledge::functions();
  if let Some(entry) = kb.get(name.to_ascii_lowercase().as_str()) {
    if let Some(sig) = entry.signature {
      return Some(build(sig, &name, entry.doc, arg_index));
    }
  }
  // Catalog (live DB) fallback.
  let cat = state.catalog.read();
  if let Some(f) = cat.functions.iter().find(|f| f.name.eq_ignore_ascii_case(&name)) {
    let args = f
      .arguments
      .iter()
      .map(|a| match &a.name {
        Some(n) => format!("{n} {}", a.data_type),
        None => a.data_type.clone(),
      })
      .collect::<Vec<_>>()
      .join(", ");
    let sig = format!("{}({args}) -> {}", f.name, f.return_type);
    let doc = f.comment.clone().unwrap_or_default();
    return Some(build_owned(sig, &name, doc, arg_index));
  }
  None
}

fn build(sig: &str, _name: &str, doc: &str, active: usize) -> SignatureHelp {
  let params = parse_params(sig);
  SignatureHelp {
    signatures: vec![SignatureInformation {
      label: sig.to_string(),
      documentation: Some(tower_lsp::lsp_types::Documentation::String(doc.to_string())),
      parameters: Some(params),
      active_parameter: Some(active as u32),
    }],
    active_signature: Some(0),
    active_parameter: Some(active as u32),
  }
}

fn build_owned(sig: String, _name: &str, doc: String, active: usize) -> SignatureHelp {
  let params = parse_params(&sig);
  SignatureHelp {
    signatures: vec![SignatureInformation {
      label: sig,
      documentation: Some(tower_lsp::lsp_types::Documentation::String(doc)),
      parameters: Some(params),
      active_parameter: Some(active as u32),
    }],
    active_signature: Some(0),
    active_parameter: Some(active as u32),
  }
}

/// From a signature like `to_char(value, format)`, return the comma-separated
/// parameter spans as inline labels (LSP highlights them by index).
fn parse_params(sig: &str) -> Vec<ParameterInformation> {
  let open = match sig.find('(') {
    Some(i) => i + 1,
    None => return Vec::new(),
  };
  let close = match sig.rfind(')') {
    Some(i) => i,
    None => sig.len(),
  };
  if close <= open {
    return Vec::new();
  }
  let body = &sig[open..close];
  let mut out = Vec::new();
  let mut depth = 0i32;
  let mut last = 0usize;
  let bytes = body.as_bytes();
  for (i, b) in bytes.iter().enumerate() {
    match b {
      b'(' => depth += 1,
      b')' => depth -= 1,
      b',' if depth == 0 => {
        let span = body[last..i].trim();
        if !span.is_empty() {
          out.push(ParameterInformation { label: ParameterLabel::Simple(span.to_string()), documentation: None });
        }
        last = i + 1;
      },
      _ => {},
    }
  }
  let tail = body[last..].trim();
  if !tail.is_empty() {
    out.push(ParameterInformation { label: ParameterLabel::Simple(tail.to_string()), documentation: None });
  }
  out
}

/// Walk back from `cursor`, find the first un-closed `(`. Returns its
/// byte position and the comma index of the cursor inside the call.
fn find_enclosing_call(src: &str, cursor: usize) -> Option<(usize, usize)> {
  let bytes = src.as_bytes();
  let mut i = cursor;
  let mut depth = 0i32;
  let mut commas = 0usize;
  let mut in_string: Option<u8> = None;
  while i > 0 {
    i -= 1;
    let b = bytes[i];
    if let Some(q) = in_string {
      if b == q {
        in_string = None;
      }
      continue;
    }
    match b {
      b')' => depth += 1,
      b'(' => {
        if depth == 0 {
          return Some((i, commas));
        }
        depth -= 1;
      },
      b',' if depth == 0 => commas += 1,
      b'\'' => in_string = Some(b'\''),
      _ => {},
    }
  }
  None
}

/// Build a signature for `INSERT INTO <table> (...) VALUES (|...)` by
/// reading the explicit column list (when present) OR the table's
/// catalog column order (positional INSERT). Returns None when we
/// can't pin down a table.
fn insert_values_signature(
  src: &str,
  catalog: &dsl_catalog::Catalog,
  values_paren: usize,
  arg_index: usize,
) -> Option<SignatureHelp> {
  // Walk back from `VALUES` to find `INSERT INTO <table> [(cols)]`.
  let before = &src[..values_paren];
  let upper = before.to_ascii_uppercase();
  let insert_at = upper.rfind("INSERT INTO")?;
  let after_kw = &src[insert_at + 11..values_paren];
  let after_trim = after_kw.trim_start();
  let after_offset = after_kw.len() - after_trim.len();
  // Table name = first identifier token after INSERT INTO.
  let table: String = after_trim.chars().take_while(|c| c.is_alphanumeric() || *c == '_' || *c == '.').collect();
  if table.is_empty() {
    return None;
  }
  let bare_table = table.rsplit('.').next().unwrap_or(&table).to_string();
  // Optional explicit column list `(col1, col2, ...)`.
  let after_table_start = after_offset + table.len();
  let rest = &after_kw[after_table_start..];
  let rest_trim = rest.trim_start();
  let explicit_cols: Option<Vec<String>> = if rest_trim.starts_with('(') {
    rest_trim[1..].find(')').map(|close_rel| {
      rest_trim[1..1 + close_rel]
        .split(',')
        .map(|c| c.trim().trim_matches('"').to_string())
        .filter(|c| !c.is_empty())
        .collect()
    })
  } else {
    None
  };
  // Source of param names + types: explicit col list (use catalog
  // for types when available) OR catalog order for positional.
  let t = catalog.find_table(None, &bare_table);
  let params: Vec<(String, String)> = if let Some(cols) = explicit_cols {
    cols
      .into_iter()
      .map(|name| {
        let ty = t
          .and_then(|t| t.columns.iter().find(|c| c.name.eq_ignore_ascii_case(&name)).map(|c| c.data_type.clone()))
          .unwrap_or_default();
        (name, ty)
      })
      .collect()
  } else if let Some(t) = t {
    t.columns.iter().map(|c| (c.name.clone(), c.data_type.clone())).collect()
  } else {
    return None;
  };
  if params.is_empty() {
    return None;
  }
  // Render the signature label as a synthetic call:
  //   VALUES(col1 type1, col2 type2, ...)
  let parts: Vec<String> =
    params.iter().map(|(n, t)| if t.is_empty() { n.clone() } else { format!("{n} {t}") }).collect();
  let label = format!("VALUES({})", parts.join(", "));
  let parameters = parts
    .iter()
    .map(|p| ParameterInformation { label: ParameterLabel::Simple(p.clone()), documentation: None })
    .collect();
  let active = arg_index.min(params.len().saturating_sub(1));
  Some(SignatureHelp {
    signatures: vec![SignatureInformation {
      label,
      documentation: Some(tower_lsp::lsp_types::Documentation::String(format!(
        "Positional VALUES for `{bare_table}` -- column slot {active}."
      ))),
      parameters: Some(parameters),
      active_parameter: Some(active as u32),
    }],
    active_signature: Some(0),
    active_parameter: Some(active as u32),
  })
}

/// `UPDATE t SET (a, b) = (|...)` -- when the enclosing paren follows
/// `= (` and a column-tuple list, surface each column as a parameter.
fn update_set_tuple_signature(
  src: &str,
  catalog: &dsl_catalog::Catalog,
  rhs_paren: usize,
  arg_index: usize,
) -> Option<SignatureHelp> {
  // Look back across whitespace, expect `=`, then whitespace, then `)`.
  let bytes = src.as_bytes();
  let mut k = rhs_paren;
  while k > 0 && bytes[k - 1].is_ascii_whitespace() {
    k -= 1;
  }
  if k == 0 || bytes[k - 1] != b'=' {
    return None;
  }
  k -= 1;
  while k > 0 && bytes[k - 1].is_ascii_whitespace() {
    k -= 1;
  }
  if k == 0 || bytes[k - 1] != b')' {
    return None;
  }
  let lhs_close = k - 1;
  // Walk back to the matching `(` of the lhs.
  let mut depth = 1i32;
  let mut j = lhs_close;
  while j > 0 && depth > 0 {
    j -= 1;
    match bytes[j] {
      b')' => depth += 1,
      b'(' => depth -= 1,
      _ => {},
    }
  }
  if depth != 0 {
    return None;
  }
  let lhs_open = j;
  let lhs_inner = &src[lhs_open + 1..lhs_close];
  let cols: Vec<String> =
    lhs_inner.split(',').map(|c| c.trim().trim_matches('"').to_string()).filter(|c| !c.is_empty()).collect();
  if cols.is_empty() {
    return None;
  }
  // Look back for `UPDATE <table>` to grab catalog types.
  let upper = src[..lhs_open].to_ascii_uppercase();
  let table = upper.rfind("UPDATE").and_then(|u| {
    let after = src[u + 6..lhs_open].trim_start();
    let name: String = after.chars().take_while(|c| c.is_alphanumeric() || *c == '_' || *c == '.').collect();
    if name.is_empty() { None } else { Some(name) }
  });
  let t = table.as_deref().and_then(|n| {
    let bare = n.rsplit('.').next().unwrap_or(n);
    catalog.find_table(None, bare)
  });
  let parts: Vec<String> = cols
    .iter()
    .map(|n| {
      let ty = t
        .and_then(|t| t.columns.iter().find(|c| c.name.eq_ignore_ascii_case(n)).map(|c| c.data_type.clone()))
        .unwrap_or_default();
      if ty.is_empty() { n.clone() } else { format!("{n} {ty}") }
    })
    .collect();
  let label = format!("SET ({})", parts.join(", "));
  let parameters = parts
    .iter()
    .map(|p| ParameterInformation { label: ParameterLabel::Simple(p.clone()), documentation: None })
    .collect();
  let active = arg_index.min(cols.len().saturating_sub(1));
  Some(SignatureHelp {
    signatures: vec![SignatureInformation {
      label,
      documentation: Some(tower_lsp::lsp_types::Documentation::String(format!(
        "Tuple-form UPDATE SET -- column slot {active}."
      ))),
      parameters: Some(parameters),
      active_parameter: Some(active as u32),
    }],
    active_signature: Some(0),
    active_parameter: Some(active as u32),
  })
}

fn identifier_before(src: &str, paren_pos: usize) -> Option<String> {
  let bytes = src.as_bytes();
  // Skip whitespace immediately before `(`.
  let mut end = paren_pos;
  while end > 0 && (bytes[end - 1] as char).is_whitespace() {
    end -= 1;
  }
  let mut start = end;
  while start > 0 {
    let c = bytes[start - 1] as char;
    if c.is_alphanumeric() || c == '_' || c == '.' {
      start -= 1;
    } else {
      break;
    }
  }
  if start == end {
    return None;
  }
  let raw = src[start..end].to_string();
  // Strip schema qualifier `public.now()` -> `now`
  Some(raw.rsplit('.').next().unwrap_or(&raw).to_string())
}
