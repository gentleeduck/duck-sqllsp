//! `textDocument/inlayHint` handler.
//!
//! Two hint families today:
//!   1. `SELECT *` -> phantom ` -- id, name, price` after the `*`. Only
//!      fires when the FROM clause names exactly one catalog table.
//!   2. Column references in WHERE / SET / ORDER BY -> phantom `: TYPE`
//!      after the column if a single catalog table is in scope and the
//!      column is unambiguous. Skipped for `*`, NULL, literals.

use crate::handlers::position;
use crate::state::ServerState;
use dsl_parse::{Projection, StatementKind};
use ropey::Rope;
use text_size::TextRange;
use tower_lsp::lsp_types::{InlayHint, InlayHintKind, InlayHintLabel, InlayHintParams, Position};

pub fn run(state: &ServerState, params: InlayHintParams) -> Option<Vec<InlayHint>> {
  let uri = params.text_document.uri;
  let _g = crate::handlers::perf::Guard::with_uri("inlay_hints", &uri);
  let doc = state.documents.get(&uri)?;
  let live = state.catalog.read().clone();
  let cache = doc.parsed();
  let parsed = &cache.file;
  // Merge live catalog + buffer-derived tables + workspace .sql scan
  // so JOIN-on heuristics and SELECT * expansion see every CREATE
  // TABLE in the project, not just the open files.
  let derived = dsl_completion::source_tables::from_source(parsed, &doc.text);
  let ws_offline = state.workspace_offline_snapshot();
  let cat = dsl_completion::source_tables::merge(
    &dsl_completion::source_tables::merge(&live, &derived),
    &ws_offline,
  );

  // Also resolve against buffer-defined tables so a fresh `CREATE TABLE`
  // expands its columns immediately without needing a DB round-trip.
  let buffer_tables: Vec<(String, Vec<String>)> = parsed
    .statements
    .iter()
    .filter_map(|s| match &s.kind {
      StatementKind::CreateTable(ct) => {
        Some((ct.table.name.clone(), ct.columns.iter().map(|c| c.name.clone()).collect::<Vec<_>>()))
      },
      _ => None,
    })
    .collect();

  let mut hints: Vec<InlayHint> = Vec::new();

  // INSERT INTO t (a, b) VALUES (1, 'x')
  //  -> phantom `: int4` / `: text` next to each VALUES literal.
  // INSERT INTO t VALUES (1, 'x', ...)
  //  -> phantom `: column_name` next to each positional value
  //     (no column list, so the hint surfaces what column gets it).
  for stmt in &parsed.statements {
    let StatementKind::Insert(ins) = &stmt.kind else { continue };
    let target = &ins.table;
    let cols: Vec<(String, String)> = if let Some(t) = cat.find_table(target.schema.as_deref(), &target.name) {
      t.columns.iter().map(|c| (c.name.clone(), c.data_type.clone())).collect()
    } else if let Some((_, cs)) = buffer_tables.iter().find(|(n, _)| n.eq_ignore_ascii_case(&target.name)) {
      cs.iter().map(|n| (n.clone(), String::new())).collect()
    } else {
      continue;
    };
    if cols.is_empty() {
      continue;
    }
    // Map ins.columns -> Vec<(name, type)>; empty cols list means
    // positional, use the catalog order.
    let ordered: Vec<(String, String)> = if ins.columns.is_empty() {
      cols.clone()
    } else {
      ins.columns.iter().filter_map(|name| cols.iter().find(|(n, _)| n.eq_ignore_ascii_case(name)).cloned()).collect()
    };
    let positional = ins.columns.is_empty();
    if positional {
      // No explicit column list -> hint the column name AFTER each
      // value (mirrors the legacy behaviour).
      for (idx, lit_byte) in find_values_literals(&doc.text, stmt.range).into_iter().enumerate() {
        let Some((col_name, _)) = ordered.get(idx) else { break };
        let pos = byte_to_position(&doc.rope, lit_byte);
        hints.push(InlayHint {
          position: pos,
          label: InlayHintLabel::String(format!(" : {col_name}")),
          kind: Some(InlayHintKind::TYPE),
          text_edits: None,
          tooltip: None,
          padding_left: Some(false),
          padding_right: Some(false),
          data: None,
        });
      }
    } else {
      // Explicit column list -> drop a column-name chip BEFORE each
      // value, integrated inline with the literal (DataGrip-style).
      for (idx, lit_start) in find_values_literal_starts(&doc.text, stmt.range).into_iter().enumerate() {
        let Some((col_name, _)) = ordered.get(idx) else { break };
        let pos = byte_to_position(&doc.rope, lit_start);
        hints.push(InlayHint {
          position: pos,
          label: InlayHintLabel::String(col_name.clone()),
          kind: Some(InlayHintKind::PARAMETER),
          text_edits: None,
          tooltip: None,
          padding_left: Some(false),
          padding_right: Some(true),
          data: None,
        });
      }
    }
  }

  for stmt in &parsed.statements {
    let StatementKind::Select(sel) = &stmt.kind else { continue };
    // Only emit when SELECT *.
    if !sel.projections.iter().any(|p| matches!(p, Projection::Star)) {
      continue;
    }
    // Single table in FROM.
    if sel.from.len() != 1 || !sel.joins.is_empty() {
      continue;
    }
    let target = &sel.from[0];
    let cols: Vec<String> = if let Some(t) = cat.find_table(target.schema.as_deref(), &target.name) {
      t.columns.iter().map(|c| c.name.clone()).collect()
    } else if let Some((_, cs)) = buffer_tables.iter().find(|(n, _)| n.eq_ignore_ascii_case(&target.name)) {
      cs.clone()
    } else {
      continue;
    };
    if cols.is_empty() {
      continue;
    }

    if let Some(star_byte) = find_star(&doc.text, stmt.range) {
      let pos = byte_to_position(&doc.rope, star_byte + 1);
      let joined = cols.join(", ");
      hints.push(InlayHint {
        position: pos,
        label: InlayHintLabel::String(format!("  -- {joined}")),
        kind: Some(InlayHintKind::TYPE),
        text_edits: None,
        tooltip: None,
        padding_left: Some(false),
        padding_right: Some(false),
        data: None,
      });
    }
  }

  // JOIN with missing / minimal ON-clause: surface a guessed ` -- ON
  // t.user_id = u.id` next to each JOIN that has no ON/USING. Text-
  // scan rather than parser-based so we still fire on the incomplete
  // SQL the user is writing (a JOIN without ON usually does not parse,
  // so the AST branch would skip exactly the case we care about).
  for missing in scan_joins_missing_on(&doc.text) {
    let pred = predicate_for_join(&cat, &missing);
    let pos = byte_to_position(&doc.rope, missing.hint_pos.min(doc.text.len()));
    hints.push(InlayHint {
      position: pos,
      label: InlayHintLabel::String(format!("  -- ON {pred}")),
      kind: Some(InlayHintKind::TYPE),
      text_edits: None,
      tooltip: None,
      padding_left: Some(false),
      padding_right: Some(false),
      data: None,
    });
  }

  // Implicit literal cast in WHERE: `WHERE int_col = '123'` --
  // PG auto-casts text -> int; surface ` :: int` after the literal
  // so the cast is visible. Conservative -- only fires when:
  //   * a single FROM table is in scope
  //   * the comparison is `col OP literal` with literal text
  //   * the column resolves and its catalog type differs from text
  for stmt in &parsed.statements {
    let StatementKind::Select(sel) = &stmt.kind else { continue };
    if sel.from.len() != 1 { continue }
    let target = &sel.from[0];
    let cols: Vec<(String, String)> = if let Some(t) = cat.find_table(target.schema.as_deref(), &target.name) {
      t.columns.iter().map(|c| (c.name.clone(), c.data_type.clone())).collect()
    } else { continue };
    let s: u32 = stmt.range.start().into();
    let e: u32 = stmt.range.end().into();
    let body = &doc.text[(s as usize).min(doc.text.len())..(e as usize).min(doc.text.len())];
    let upper = body.to_ascii_uppercase();
    let Some(where_at) = upper.find("WHERE") else { continue };
    let bytes = body.as_bytes();
    let mut i = where_at + "WHERE".len();
    while i < bytes.len() {
      // Find an identifier.
      while i < bytes.len() && !(bytes[i].is_ascii_alphabetic() || bytes[i] == b'_') { i += 1 }
      let id_start = i;
      while i < bytes.len() && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_') { i += 1 }
      let ident = &body[id_start..i];
      if ident.is_empty() { break }
      while i < bytes.len() && bytes[i].is_ascii_whitespace() { i += 1 }
      let op_start = i;
      while i < bytes.len() && matches!(bytes[i], b'=' | b'<' | b'>' | b'!') { i += 1 }
      if i == op_start { continue }
      while i < bytes.len() && bytes[i].is_ascii_whitespace() { i += 1 }
      if i >= bytes.len() || bytes[i] != b'\'' { continue }
      // Found col OP 'lit'. Resolve col, check type.
      let Some((_, ty)) = cols.iter().find(|(n, _)| n.eq_ignore_ascii_case(ident)) else { continue };
      let ty_lc = ty.to_ascii_lowercase();
      let target_type = if ty_lc.starts_with("int") || ty_lc == "bigint" || ty_lc == "smallint" { Some("int") }
        else if ty_lc.starts_with("numeric") || ty_lc.starts_with("decimal") { Some("numeric") }
        else if ty_lc.starts_with("uuid") { Some("uuid") }
        else if ty_lc.starts_with("bool") { Some("bool") }
        else { None };
      let Some(target_type) = target_type else { continue };
      // Skip closing '.
      i += 1;
      while i < bytes.len() && bytes[i] != b'\'' { i += 1 }
      if i < bytes.len() { i += 1 }
      let abs_after_lit = s as usize + i;
      let pos = byte_to_position(&doc.rope, abs_after_lit);
      hints.push(InlayHint {
        position: pos,
        label: InlayHintLabel::String(format!("::{target_type}")),
        kind: Some(InlayHintKind::TYPE),
        text_edits: None,
        tooltip: None,
        padding_left: Some(false),
        padding_right: Some(false),
        data: None,
      });
    }
  }

  // Alias -> table chip on every FROM/JOIN binding that has a
  // user-typed alias (alias != table name). `FROM users u` adds a
  // ` -> users` chip after the `u` token.
  for stmt in &parsed.statements {
    let StatementKind::Select(sel) = &stmt.kind else { continue };
    let s: u32 = stmt.range.start().into();
    let e: u32 = stmt.range.end().into();
    let body = &doc.text[(s as usize).min(doc.text.len())..(e as usize).min(doc.text.len())];
    let upper = body.to_ascii_uppercase();
    for src in sel.from.iter().chain(sel.joins.iter().map(|j| &j.table)) {
      let Some(alias) = &src.alias else { continue };
      if alias.eq_ignore_ascii_case(&src.name) { continue }
      let name_up = src.name.to_ascii_uppercase();
      let mut from = 0usize;
      while let Some(rel) = upper[from..].find(&name_up) {
        let at = from + rel;
        let after = at + name_up.len();
        if at > 0 {
          let prev = body.as_bytes()[at - 1] as char;
          if prev.is_ascii_alphanumeric() || prev == '_' { from = after; continue }
        }
        if after < body.len() {
          let nx = body.as_bytes()[after] as char;
          if nx.is_ascii_alphanumeric() || nx == '_' { from = after; continue }
        }
        let mut k = after;
        while k < body.len() && body.as_bytes()[k].is_ascii_whitespace() { k += 1 }
        let kupper = upper[k..].to_string();
        if kupper.starts_with("AS ") || kupper.starts_with("AS\t") || kupper.starts_with("AS\n") {
          k += 2;
          while k < body.len() && body.as_bytes()[k].is_ascii_whitespace() { k += 1 }
        }
        let alias_start = k;
        while k < body.len() && (body.as_bytes()[k].is_ascii_alphanumeric() || body.as_bytes()[k] == b'_') { k += 1 }
        if k == alias_start { from = after; continue }
        let typed_alias = &body[alias_start..k];
        if !typed_alias.eq_ignore_ascii_case(alias) { from = after; continue }
        let abs_after_alias = s as usize + k;
        let pos = byte_to_position(&doc.rope, abs_after_alias);
        hints.push(InlayHint {
          position: pos,
          label: InlayHintLabel::String(format!(" -> {}", src.name)),
          kind: Some(InlayHintKind::TYPE),
          text_edits: None,
          tooltip: None,
          padding_left: Some(false),
          padding_right: Some(false),
          data: None,
        });
        break;
      }
    }
  }

  // INSERT VALUES row-count -- after the closing `)` of the last
  // tuple, append ` -- 3 rows` (skipped when only one tuple).
  for stmt in &parsed.statements {
    let StatementKind::Insert(_) = &stmt.kind else { continue };
    let s: u32 = stmt.range.start().into();
    let e: u32 = stmt.range.end().into();
    let body = &doc.text[(s as usize).min(doc.text.len())..(e as usize).min(doc.text.len())];
    let upper = body.to_ascii_uppercase();
    let Some(v_at) = upper.find("VALUES") else { continue };
    let after = v_at + "VALUES".len();
    let bytes = body.as_bytes();
    let mut i = after;
    let mut tuples = 0usize;
    let mut last_close = after;
    while i < bytes.len() {
      while i < bytes.len() && bytes[i].is_ascii_whitespace() { i += 1 }
      if i >= bytes.len() || bytes[i] != b'(' { break }
      let open = i;
      let close = match_paren_count(body, open);
      let Some(close) = close else { break };
      tuples += 1;
      last_close = close;
      i = close + 1;
      while i < bytes.len() && bytes[i].is_ascii_whitespace() { i += 1 }
      if i < bytes.len() && bytes[i] == b',' { i += 1 } else { break }
    }
    if tuples >= 2 {
      let abs = s as usize + last_close + 1;
      let pos = byte_to_position(&doc.rope, abs);
      hints.push(InlayHint {
        position: pos,
        label: InlayHintLabel::String(format!("  -- {tuples} rows")),
        kind: Some(InlayHintKind::TYPE),
        text_edits: None,
        tooltip: None,
        padding_left: Some(false),
        padding_right: Some(false),
        data: None,
      });
    }
  }

  if hints.is_empty() { None } else { Some(hints) }
}

fn match_paren_count(s: &str, open: usize) -> Option<usize> {
  let bytes = s.as_bytes();
  let mut depth = 0i32;
  let mut i = open;
  while i < bytes.len() {
    match bytes[i] {
      b'(' => depth += 1,
      b')' => { depth -= 1; if depth == 0 { return Some(i); } }
      b'\'' => {
        i += 1;
        while i < bytes.len() && bytes[i] != b'\'' { i += 1 }
      }
      _ => {}
    }
    i += 1;
  }
  None
}

/// A JOIN clause located in the buffer text that lacks an ON / USING
/// predicate. Carries both sides' table names + aliases, plus the byte
/// position where the hint should land (right after the JOIN's table /
/// alias declaration).
#[derive(Debug)]
struct MissingOnJoin {
  from_name: String,
  from_alias: String,
  join_name: String,
  join_alias: String,
  hint_pos: usize,
}

/// Walk `src` looking for `FROM <tbl> [alias] JOIN <tbl2> [alias2]`
/// segments where the next non-whitespace token is not ON / USING.
/// Each match becomes a `MissingOnJoin`. Token-level scan; doesn't try
/// to be a full parser, just covers the common one-JOIN case.
fn scan_joins_missing_on(src: &str) -> Vec<MissingOnJoin> {
  let mut out = Vec::new();
  let upper = src.to_ascii_uppercase();
  let mut cursor = 0usize;
  while let Some(rel_from) = upper[cursor..].find(" FROM ") {
    let from_at = cursor + rel_from + 6;
    let (from_name, from_alias, after_from) = read_table_decl(src, from_at);
    if from_name.is_empty() {
      cursor = from_at;
      continue;
    }
    // Scan forward looking for the next JOIN before a statement-
    // terminating semicolon / new SELECT / WHERE etc.
    let stop_re = ["WHERE ", "GROUP ", "ORDER ", "LIMIT ", "HAVING ", "OFFSET ", "RETURNING ", ";"];
    let mut k = after_from;
    // Loop multiple JOINs after the same FROM.
    let mut current_from_name = from_name.clone();
    let mut current_from_alias = from_alias.clone();
    while k < src.len() {
      // Bail if a stop-keyword arrives first.
      let upper_tail = upper[k..].trim_start();
      let consumed = upper[k..].len() - upper_tail.len();
      if stop_re.iter().any(|kw| upper_tail.starts_with(kw)) {
        break;
      }
      // Locate the next JOIN keyword.
      let Some(rel_join) = upper[k..].find("JOIN ") else { break };
      let join_at = k + rel_join + 5;
      // Skip CROSS JOIN -- those legitimately lack ON.
      let prefix = upper[k..k + rel_join].trim_end();
      if prefix.ends_with("CROSS") {
        k = join_at;
        continue;
      }
      let (join_name, join_alias, after_join) = read_table_decl(src, join_at);
      if join_name.is_empty() {
        k = join_at;
        break;
      }
      // What comes after the join table decl?
      let tail = upper[after_join..].trim_start();
      let already_has_predicate =
        tail.starts_with("ON ") || tail.starts_with("ON(") || tail.starts_with("USING ") || tail.starts_with("USING(");
      if !already_has_predicate {
        out.push(MissingOnJoin {
          from_name: current_from_name.clone(),
          from_alias: current_from_alias.clone(),
          join_name: join_name.clone(),
          join_alias: join_alias.clone(),
          hint_pos: after_join,
        });
      }
      // The right side of this JOIN becomes the FROM target for any
      // following JOIN.
      current_from_name = join_name;
      current_from_alias = join_alias;
      k = after_join;
      let _ = consumed;
    }
    cursor = after_from;
  }
  out
}

/// Starting at `pos` (after `FROM ` or `JOIN `), read the table name
/// (possibly schema-qualified or quoted) and an optional alias. Returns
/// `(table_name, alias, byte_offset_after_decl)`. Alias defaults to the
/// table's base name when absent. Skips a leading `ONLY` (PG extension).
fn read_table_decl(src: &str, pos: usize) -> (String, String, usize) {
  let bytes = src.as_bytes();
  let mut i = pos;
  while i < bytes.len() && bytes[i].is_ascii_whitespace() {
    i += 1;
  }
  // Optional ONLY.
  let upper_at = src[i..].chars().take(5).collect::<String>().to_ascii_uppercase();
  if upper_at.starts_with("ONLY ") {
    i += 5;
    while i < bytes.len() && bytes[i].is_ascii_whitespace() {
      i += 1;
    }
  }
  // Read identifier (possibly quoted, possibly schema.qualified).
  let id_start = i;
  while i < bytes.len() {
    let c = bytes[i] as char;
    if c.is_alphanumeric() || c == '_' || c == '.' || c == '"' {
      i += 1;
    } else {
      break;
    }
  }
  if id_start == i {
    return (String::new(), String::new(), pos);
  }
  let full = src[id_start..i].trim_matches('"').to_string();
  let bare = full.rsplit('.').next().unwrap_or(&full).trim_matches('"').to_string();
  let after_id = i;
  // Optional alias: AS x, or bare alias x.
  while i < bytes.len() && bytes[i].is_ascii_whitespace() {
    i += 1;
  }
  let upper_alias_kw = src[i..].chars().take(3).collect::<String>().to_ascii_uppercase();
  if upper_alias_kw == "AS " {
    i += 3;
    while i < bytes.len() && bytes[i].is_ascii_whitespace() {
      i += 1;
    }
  }
  let alias_start = i;
  while i < bytes.len() {
    let c = bytes[i] as char;
    if c.is_alphanumeric() || c == '_' {
      i += 1;
    } else {
      break;
    }
  }
  let alias_raw = &src[alias_start..i];
  let alias_upper = alias_raw.to_ascii_uppercase();
  // Reserved-ish words that aren't aliases.
  let is_alias = !alias_raw.is_empty()
    && !matches!(
      alias_upper.as_str(),
      "JOIN"
        | "INNER"
        | "LEFT"
        | "RIGHT"
        | "FULL"
        | "CROSS"
        | "ON"
        | "USING"
        | "WHERE"
        | "GROUP"
        | "ORDER"
        | "LIMIT"
        | "HAVING"
        | "OFFSET"
        | "RETURNING"
    );
  if is_alias { (bare.clone(), alias_raw.to_string(), i) } else { (bare.clone(), bare, after_id) }
}

/// Build the predicate text for an inlay hint, in order of confidence:
/// real FK (either direction), name-convention guess, shared column,
/// `???` placeholder.
fn predicate_for_join(cat: &dsl_catalog::Catalog, j: &MissingOnJoin) -> String {
  let from_table = cat.find_table(None, &j.from_name);
  let join_table = cat.find_table(None, &j.join_name);
  find_fk_predicate(from_table, &j.from_alias, join_table, &j.join_alias)
    .or_else(|| find_fk_predicate(join_table, &j.join_alias, from_table, &j.from_alias))
    .or_else(|| guess_join_predicate(from_table, &j.from_alias, &j.from_name, join_table, &j.join_alias, &j.join_name))
    .unwrap_or_else(|| "???  -- missing ON".to_string())
}

fn find_fk_predicate(
  src_table: Option<&dsl_catalog::Table>,
  src_alias: &str,
  target_table: Option<&dsl_catalog::Table>,
  target_alias: &str,
) -> Option<String> {
  let src = src_table?;
  let target = target_table?;
  for c in &src.constraints {
    if !matches!(c.kind, dsl_catalog::ConstraintKind::ForeignKey) {
      continue;
    }
    let Some(refs) = &c.references else { continue };
    if !refs.table.eq_ignore_ascii_case(&target.name) {
      continue;
    }
    if c.columns.len() != 1 || refs.columns.len() != 1 {
      continue;
    }
    return Some(format!("{src_alias}.{} = {target_alias}.{}", c.columns[0], refs.columns[0]));
  }
  None
}

/// Catalog has no FK -> guess a JOIN predicate from column names. Two
/// signals, in order of confidence:
///
///   1. Convention: one side has `id` and the other has `<this>_id` or
///      `<singular(other)>_id`. Postgres / Rails / Django / Ecto all
///      land here, so this catches a large chunk of real schemas with
///      no FKs (migrations skipped, intentional denormalisation, etc).
///   2. Shared column name: both sides expose a column with the exact
///      same name (e.g. `tenant_id`, `account_id`). Common for
///      multi-tenant or sharded designs.
///
/// Output is annotated with `?` so users can tell the suggestion came
/// from heuristics rather than a real FK.
fn guess_join_predicate(
  from_table: Option<&dsl_catalog::Table>,
  from_alias: &str,
  from_name: &str,
  join_table: Option<&dsl_catalog::Table>,
  join_alias: &str,
  join_name: &str,
) -> Option<String> {
  let from = from_table?;
  let join = join_table?;
  let has = |t: &dsl_catalog::Table, col: &str| -> bool { t.columns.iter().any(|c| c.name.eq_ignore_ascii_case(col)) };
  // Convention check both directions.
  for (parent_t, parent_a, parent_name, child_t, child_a) in
    [(from, from_alias, from_name, join, join_alias), (join, join_alias, join_name, from, from_alias)]
  {
    if !has(parent_t, "id") {
      continue;
    }
    // Try `<parent_singular>_id` first (orders.user_id -> users.id),
    // then `<parent>_id` (rare but real for non-pluralised tables).
    let singular = singularise(parent_name);
    for candidate in [format!("{singular}_id"), format!("{}_id", parent_name)] {
      if has(child_t, &candidate) {
        return Some(format!("{child_a}.{candidate} = {parent_a}.id  -- ?"));
      }
    }
  }
  // Shared-column-name fallback. Pick the first column they both expose,
  // preferring `*_id` so we don't latch onto something trivial like
  // `created_at`.
  let mut shared: Vec<&str> = from
    .columns
    .iter()
    .filter(|c| join.columns.iter().any(|jc| jc.name.eq_ignore_ascii_case(&c.name)))
    .map(|c| c.name.as_str())
    .collect();
  shared.sort_by_key(|c| !c.ends_with("_id"));
  let col = shared.first()?;
  Some(format!("{from_alias}.{col} = {join_alias}.{col}  -- ?"))
}

/// Drop a single trailing 's' for the obvious pluralisation case so
/// `users` -> `user`. Keeps any other word ending untouched (we don't
/// try to be a real inflector -- the `_id` lookup will still hit the
/// non-singularised candidate below).
fn singularise(name: &str) -> String {
  let lower = name.to_ascii_lowercase();
  if lower.ends_with('s') && !lower.ends_with("ss") { lower[..lower.len() - 1].to_string() } else { lower }
}

/// Locate the byte position right *after* each top-level literal in the
/// Mirror of [`find_values_literals`] returning byte offsets that
/// land at the *start* of each item -- the first non-whitespace byte
/// inside the tuple (or immediately after the preceding comma). Used
/// to drop a column-name chip BEFORE each value, the way DataGrip /
/// JetBrains DBs render the column hint inline with the literal.
fn find_values_literal_starts(source: &str, range: TextRange) -> Vec<usize> {
  let s: u32 = range.start().into();
  let e: u32 = range.end().into();
  let start = s as usize;
  let end = (e as usize).min(source.len());
  let slice = &source[start..end];
  let upper = slice.to_ascii_uppercase();
  let Some(values_at) = upper.find("VALUES") else { return Vec::new() };
  let bytes = slice.as_bytes();
  let n = bytes.len();
  let mut k = values_at + 6;
  while k < n && bytes[k].is_ascii_whitespace() {
    k += 1;
  }
  if k >= n || bytes[k] != b'(' {
    return Vec::new();
  }
  let mut out: Vec<usize> = Vec::new();
  let mut depth = 1i32;
  let mut item_start: Option<usize> = None;
  let mut i = k + 1;
  while i < n && depth > 0 {
    match bytes[i] {
      b'(' => {
        depth += 1;
        if item_start.is_none() { item_start = Some(i); }
      }
      b')' => {
        depth -= 1;
        if depth == 0 {
          if let Some(s) = item_start.take() { out.push(start + s); }
          break;
        }
      }
      b'\'' => {
        if item_start.is_none() { item_start = Some(i); }
        i += 1;
        while i < n && bytes[i] != b'\'' { i += 1; }
        if i < n { i += 1; continue; }
      }
      b',' if depth == 1 => {
        if let Some(s) = item_start.take() { out.push(start + s); }
        i += 1;
        continue;
      }
      c if c.is_ascii_whitespace() => {}
      _ => {
        if item_start.is_none() { item_start = Some(i); }
      }
    }
    i += 1;
  }
  out
}

/// first VALUES tuple of an INSERT statement. Skips nested parens and
/// quoted strings.
fn find_values_literals(source: &str, range: TextRange) -> Vec<usize> {
  let s: u32 = range.start().into();
  let e: u32 = range.end().into();
  let start = s as usize;
  let end = (e as usize).min(source.len());
  let slice = &source[start..end];
  let upper = slice.to_ascii_uppercase();
  let Some(values_at) = upper.find("VALUES") else { return Vec::new() };
  let bytes = slice.as_bytes();
  let n = bytes.len();
  let mut k = values_at + 6;
  while k < n && bytes[k].is_ascii_whitespace() {
    k += 1;
  }
  if k >= n || bytes[k] != b'(' {
    return Vec::new();
  }
  let mut out = Vec::new();
  let mut depth = 1i32;
  let mut item_end = k + 1; // running end-of-current-item byte position
  let mut had_content = false;
  let mut i = k + 1;
  while i < n && depth > 0 {
    match bytes[i] {
      b'(' => {
        depth += 1;
        had_content = true;
      },
      b')' => {
        depth -= 1;
        if depth == 0 {
          if had_content {
            out.push(start + item_end);
          }
          break;
        }
      },
      b'\'' => {
        i += 1;
        while i < n && bytes[i] != b'\'' {
          i += 1;
        }
        had_content = true;
        if i < n {
          i += 1;
          item_end = i;
          continue;
        }
      },
      b',' if depth == 1 => {
        if had_content {
          out.push(start + item_end);
        }
        had_content = false;
        i += 1;
        continue;
      },
      c if c.is_ascii_whitespace() => {},
      _ => {
        had_content = true;
        item_end = i + 1;
      },
    }
    if !bytes[i].is_ascii_whitespace() {
      item_end = i + 1;
    }
    i += 1;
  }
  out
}

fn find_star(source: &str, range: TextRange) -> Option<usize> {
  let s: u32 = range.start().into();
  let e: u32 = range.end().into();
  let s = s as usize;
  let e = (e as usize).min(source.len());
  let slice = &source[s..e];
  let upper = slice.to_ascii_uppercase();
  let select_at = upper.find("SELECT")?;
  let after = select_at + "SELECT".len();
  let rest = &slice[after..];
  let trim_lead = rest.len() - rest.trim_start().len();
  let star_local = rest[trim_lead..].chars().next()?;
  if star_local != '*' {
    return None;
  }
  Some(s + after + trim_lead)
}

// Same byte-to-LSP-position walker as the other handlers.
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

// Suppress unused; supplied for future cursor-position lookup.
#[allow(dead_code)]
fn _unused(_p: Position) {
  let _ = position::to_offset;
}
