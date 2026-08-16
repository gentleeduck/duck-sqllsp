//! Completion engine.
//!
//! Routing runs through two ordered detector registries, each an
//! array of `(source, offset, file, scopes, catalog) -> Option<Vec<Item>>`
//! functions walked in order -- first `Some` wins, `None` falls
//! through to the next entry. This is the *only* place precedence
//! order lives; every entry's doc comment explains why it needs to
//! run where it does relative to its neighbors.
//!
//!   1. [`PRE_PHASE_DETECTORS`], run by [`complete_with_derived`]
//!      before the cursor's phase is even determined: fresh-name-slot
//!      suppression, JSON-path key slot, inert-span bailout, dot
//!      context (`<alias>.<cursor>`), grouping-sets-inner-paren, and
//!      `contexts::detect` (index/trigger/policy/etc special cases
//!      the phase state machine doesn't model).
//!   2. Falling through all of those, the phase is determined --
//!      `create_index::detect` / `create_table::detect` narrow it
//!      when applicable, otherwise [`phase::Phase`] via
//!      [`phase::detect`] -- and handed to `route_phase`, which runs
//!      [`POST_PHASE_DETECTORS`] (slot-keyword shortcuts too specific
//!      for the phase match: `FILTER (`, `WITH ORDINALITY`, window
//!      frame bounds, etc), then falls through to the `Phase`-driven
//!      `match` for the "ordinary, no special case" menu per phase.
//!
//! This two-registry split (rather than one flat list) mirrors the
//! algorithm's actual shape: stage 1 runs pre-phase, stage 2 runs
//! post-phase, and unifying them into one list would mean reordering
//! stage-1 checks relative to the phase-determination step in between
//! -- a real behavior change, not just reorganization.
//!
//! This is what makes the menu context-aware: after `SELECT *` we
//! surface `FROM`, not more SELECT keywords; after a table we surface
//! `JOIN` / `WHERE` / `GROUP BY` / etc; inside ORDER BY we surface
//! columns + ASC/DESC; after WHERE we surface columns + expression
//! keywords; after a semicolon we surface top-level statement starters
//! only.

use crate::create_index;
use crate::create_table;
// Wildcard: `detectors` is currently one large "everything not yet
// thematically split" module (189 functions, deliberately not
// subdivided in this Stage-1 pure-motion batch -- see its module doc).
// Explicit per-item imports would mean ~150+ names here; a future
// thematic split of detectors.rs would naturally restore explicit
// `module::fn()` call sites the way merge/json_path already have.
use crate::detectors::*;
use crate::fallback;
use crate::item::Item;
use crate::json_path;
use crate::merge;
use crate::phase::{self, Phase};
use crate::source_tables;
use crate::sources;
use dsl_catalog::Catalog;
use dsl_parse::ParsedFile;
use dsl_resolve::Scope;
use text_size::TextSize;

/// Extract the current SQL statement up to `offset` as (slice, upper).
/// Statement boundary = last `;` before pos. `slice` is `source[stmt_start..pos]`
/// verbatim; `upper` is the uppercase of `slice` with byte offsets preserved
/// (no trim). Callers that need `upper.starts_with("CREATE …")` should do
/// `upper.trim_start().starts_with(...)` themselves.
pub(crate) fn stmt_slice_upper(source: &str, offset: TextSize) -> (String, String) {
  let pos: usize = (u32::from(offset) as usize).min(source.len());
  let stmt_start = source[..pos].rfind(';').map(|p| p + 1).unwrap_or(0);
  let slice = source[stmt_start..pos].to_string();
  let upper = slice.to_ascii_uppercase();
  (slice, upper)
}

/// Byte-slice of the current statement, semicolon-delimited, covering
/// *both* sides of the cursor (unlike `stmt_slice_upper`, which stops
/// at `offset`). Naive `;`-scan -- doesn't account for a semicolon
/// inside a string literal or a dollar-quoted PL/pgSQL body, same
/// known limitation `stmt_slice_upper`'s start-boundary already has.
///
/// Scopes text-fallback scans (`fallback::scope_from_text` and
/// friends) to the statement under the cursor instead of the whole
/// buffer -- `iter_table_bindings` scans its input start-to-end for
/// every FROM/JOIN, so an unscoped whole-buffer call is O(buffer size)
/// regardless of cursor position. This was the dominant cost behind
/// the Phase A perf finding (`dsl-completion/tests/perf_bench.rs`,
/// ~25ms/call at 10k statements); see the design doc's "Phase D
/// findings" for the full writeup.
pub(crate) fn current_statement_span(source: &str, offset: TextSize) -> &str {
  let pos: usize = (u32::from(offset) as usize).min(source.len());
  let start = source[..pos].rfind(';').map(|p| p + 1).unwrap_or(0);
  let end = source[pos..].find(';').map(|p| pos + p + 1).unwrap_or(source.len());
  &source[start..end]
}

/// Return true when `offset` does NOT sit at a whitespace (or EOF)
/// boundary. Phase detectors short-circuit on this so they don't yank
/// the menu open while the user is typing a token. Matches the legacy
/// guard `pos < bytes.len() && !bytes[pos].is_ascii_whitespace()`.
pub(crate) fn cursor_not_at_ws_boundary(source: &str, offset: TextSize) -> bool {
  let pos: usize = (u32::from(offset) as usize).min(source.len());
  let bytes = source.as_bytes();
  pos < bytes.len() && !bytes[pos].is_ascii_whitespace()
}

/// Append every `(label, detail)` pair as a fresh `Keyword` item with
/// default sort priority. Used by every per-statement phase emitter
/// (CREATE TABLE / TRIGGER / TYPE / ALTER ROLE / ...).
pub(crate) fn push_keyword_kvs(out: &mut Vec<Item>, kws: &[(&'static str, &'static str)]) {
  for (kw, doc) in kws {
    out.push(Item {
      label: (*kw).into(),
      kind: crate::item::ItemKind::Keyword,
      detail: Some((*doc).into()),
      insert_text: (*kw).into(),
      sort_priority: 0,
      ..Default::default()
    });
  }
}

/// Convenience wrapper for callers with no pre-computed buffer-derived
/// catalog (tests, one-shot tools, `dsl-cli`). Derives it fresh from
/// `source` via `source_tables::from_source` on every call. Callers
/// that invoke completion repeatedly against the *same* buffer (an LSP
/// server servicing one document across many requests) should derive
/// this catalog once and call [`complete_with_derived`] directly
/// instead -- see `Document::derived_catalog` in dsl-server.
pub fn complete(source: &str, file: &ParsedFile, scopes: &[Scope], catalog: &Catalog, offset: TextSize) -> Vec<Item> {
  let derived = source_tables::from_source(file, source);
  complete_with_derived(source, file, scopes, catalog, &derived, offset)
}

/// Same as [`complete`] but takes the buffer-derived catalog
/// (normally `source_tables::from_source(file, source)`) as an
/// explicit parameter instead of computing it internally. `catalog`
/// and `derived` are merged with `catalog` winning on collisions
/// (matches [`source_tables::merge`]'s live-wins-over-derived
/// semantics).
pub fn complete_with_derived(
  source: &str,
  file: &ParsedFile,
  scopes: &[Scope],
  catalog: &Catalog,
  derived: &Catalog,
  offset: TextSize,
) -> Vec<Item> {
  // Normalise offset to the nearest valid UTF-8 char boundary so
  // downstream slicing can't panic on multi-byte characters.
  let raw_off: usize = offset.into();
  let off = floor_char_boundary(source, raw_off.min(source.len()));
  let offset = TextSize::from(off as u32);

  // Merge live catalog with the (caller-supplied) buffer-derived
  // catalog: tables from AST + sequences / types / extensions /
  // functions / roles harvested from buffer text + the default
  // offline roles. Live catalog wins on collisions. Computed up front
  // (cheap -- no more scan work than a shallow struct merge) so the
  // JSON-path key slot below can consult catalog-typed `json_keys`
  // hints, not just same-buffer literal examples.
  let cat = source_tables::merge(catalog, derived);

  // Every pre-phase check, in the exact order they used to run as a
  // chain of `if`/`if let` statements -- see `PRE_PHASE_DETECTORS`'s
  // doc comment for the full list and why it's shaped this way.
  for detector in PRE_PHASE_DETECTORS {
    if let Some(items) = detector(source, offset, file, scopes, &cat) {
      return items;
    }
  }

  // CREATE INDEX scoped context wins before CREATE TABLE / generic
  // phases. `CREATE INDEX <name> ON users (` should only ever surface
  // columns of `users`, never a global table or column dump.
  if let Some(ix_phase) = create_index::detect(source, offset) {
    return route_phase(ix_phase, file, scopes, source, &cat, offset);
  }
  // CREATE TABLE sub-phase trumps the generic state machine because the
  // narrower context (column name vs type vs constraint) is what the
  // user is in the middle of writing.
  if let Some(ct_phase) = create_table::detect(source, offset) {
    return route_phase(ct_phase, file, scopes, source, &cat, offset);
  }

  let ph = phase::detect(source, offset);
  route_phase(ph, file, scopes, source, &cat, offset)
}

/// Hard-suppress completion when the cursor sits at the "fresh name"
/// slot after a `CREATE [OR REPLACE] <KIND>` keyword. The user is
/// naming a brand-new object; no existing catalog symbol or keyword
/// is a sensible suggestion there, with several overrides for
/// contexts that only *look* like a fresh-name slot (PREPARE
/// TRANSACTION, FETCH/MOVE direction, CREATE TRANSFORM, [ALTER] USER
/// MAPPING, the SELECT-trailing-FETCH chain, PARTITION OF ... FOR
/// VALUES). Always claims the slot once the gate matches -- even the
/// "none of the overrides apply" case commits to an empty menu rather
/// than falling through, since guessing at a brand-new name is worse
/// than suggesting nothing.
fn detect_fresh_name_slot(
  source: &str,
  offset: TextSize,
  _file: &ParsedFile,
  _scopes: &[Scope],
  _cat: &Catalog,
) -> Option<Vec<Item>> {
  if !at_fresh_name_slot(source, offset) {
    return None;
  }
  if let Some(label) = fresh_name_slot_optional_keyword(source, offset) {
    return Some(vec![crate::item::Item {
      label: label.into(),
      kind: crate::item::ItemKind::Keyword,
      detail: Some("optional clarifier before the new object name".into()),
      insert_text: label.into(),
      sort_priority: 0,
      ..Default::default()
    }]);
  }
  // `PREPARE TRANSACTION` overrides the fresh-name-slot suppression
  // because TRANSACTION is a literal kw, not a fresh statement name.
  if let Some(kws) = txn_followup_next_keyword(source, offset) {
    let mut out = Vec::with_capacity(kws.len());
    push_keyword_kvs(&mut out, kws);
    return Some(out);
  }
  // FETCH / MOVE -- direction keyword set is more useful than the
  // fresh-name suppression (cursor name comes after FROM/IN).
  if let Some(kws) = fetch_move_direction_keyword(source, offset) {
    let mut out = Vec::with_capacity(kws.len());
    push_keyword_kvs(&mut out, kws);
    return Some(out);
  }
  // CREATE TRANSFORM -- post-keyword slot is FOR TYPE, not a name.
  if let Some(kws) = create_transform_next_keyword(source, offset) {
    let mut out = Vec::with_capacity(kws.len());
    push_keyword_kvs(&mut out, kws);
    return Some(out);
  }
  // CREATE/ALTER USER MAPPING -- post-keyword slot is FOR/IF NOT EXISTS,
  // not a brand-new identifier.
  if let Some(kws) = create_user_mapping_next_keyword(source, offset) {
    let mut out = Vec::with_capacity(kws.len());
    push_keyword_kvs(&mut out, kws);
    return Some(out);
  }
  if let Some(kws) = alter_user_mapping_next_keyword(source, offset) {
    let mut out = Vec::with_capacity(kws.len());
    push_keyword_kvs(&mut out, kws);
    return Some(out);
  }
  // `SELECT ... FETCH` / `... FIRST` / `... ROW(S)`: not a cursor
  // command, it's the SELECT trailing FETCH clause. Fresh-name guard
  // misfires because `FETCH` is in the cursor pattern list.
  if let Some(kws) = select_fetch_offset_next_keyword(source, offset) {
    let mut out = Vec::with_capacity(kws.len());
    push_keyword_kvs(&mut out, kws);
    return Some(out);
  }
  // `CREATE TABLE child PARTITION OF parent FOR VALUES ` -- the
  // trailing VALUES is part of the partition spec, not a top-level
  // VALUES (...) statement, so the partition menu (IN/FROM/WITH/
  // DEFAULT) wins over the fresh-name suppression.
  if let Some(kws) = partition_next_keyword(source, offset) {
    let mut out = Vec::with_capacity(kws.len());
    push_keyword_kvs(&mut out, kws);
    return Some(out);
  }
  Some(Vec::new())
}

/// JSON-path key slot: `data->'<cursor>` or `data->>'<cursor>`.
/// Surface keys observed in same-buffer jsonb literal defaults / CHECK
/// constraints, falling back to catalog-recorded `Column.json_keys`
/// when the buffer has no example literal to harvest. Highest
/// priority -- we don't want to drown the menu in catalog table names
/// when the user is clearly typing a JSON key. (Runs before the
/// inert-span bailout so JSON key completion still works while the
/// cursor sits inside the `'...'` literal.)
fn detect_json_path_key(
  source: &str,
  offset: TextSize,
  _file: &ParsedFile,
  _scopes: &[Scope],
  cat: &Catalog,
) -> Option<Vec<Item>> {
  let keys = json_path::json_path_keys_at_with_catalog(source, offset, cat)?;
  let mut out = Vec::with_capacity(keys.len());
  for k in keys {
    out.push(crate::item::Item {
      label: k.clone(),
      kind: crate::item::ItemKind::Variable,
      detail: Some("JSON key".into()),
      description: Some("known JSON key".into()),
      documentation_md: None,
      insert_text: k,
      is_snippet: false,
      sort_priority: 0,
    });
  }
  Some(out)
}

/// Cursor inside a string literal or comment? Suggesting keywords /
/// tables / columns there is just noise -- the user is typing string
/// content. Dollar-quoted bodies (PL/pgSQL) are NOT inert -- recurse
/// into them so completion still works inside function bodies. Claims
/// the position with an empty menu (`Some(Vec::new())`) rather than
/// declining (`None`) -- declining would let later detectors and the
/// phase match compute a real, wrong answer for a cursor sitting
/// inside a literal.
fn detect_inert_span(
  source: &str,
  offset: TextSize,
  _file: &ParsedFile,
  _scopes: &[Scope],
  _cat: &Catalog,
) -> Option<Vec<Item>> {
  if cursor_in_inert_span(source, u32::from(offset) as usize) { Some(Vec::new()) } else { None }
}

/// Dot context: `<alias>.<cursor>` -- highest priority once past the
/// fresh-name-slot / JSON-path / inert-span checks above, beats every
/// phase result. Resolution order for the alias's columns: NEW/OLD
/// virtual trigger-body aliases, `EXCLUDED` (ON CONFLICT DO UPDATE),
/// in-scope table/CTE binding (AST-resolved, falling back to a
/// text-scan when the resolver didn't run), a PL/pgSQL local typed as
/// a catalog table (row variable), a schema-qualified relation, and
/// finally a bare table name with no scope binding at all. Always
/// claims the slot once `dot_alias` matches -- even "found nothing"
/// is a deliberate empty menu, not a fall-through, since a global
/// column dump would be wrong once we know the user typed `x.`.
fn detect_dot_context(
  source: &str,
  offset: TextSize,
  file: &ParsedFile,
  scopes: &[Scope],
  cat: &Catalog,
) -> Option<Vec<Item>> {
  let alias = dot_alias(source, offset)?;
  let mut out = Vec::new();
  // NEW / OLD virtual aliases inside trigger-function bodies.
  // Resolution order:
  //   1. Look for `CREATE TRIGGER ... ON <table>` in the buffer.
  //   2. If the cursor sits inside a CREATE FUNCTION body, find
  //      the function name, then search the buffer + live catalog
  //      for `CREATE TRIGGER ... EXECUTE [FUNCTION|PROCEDURE]
  //      <fn>` and read the table off that trigger.
  // Return WITHOUT completion when we can't pin down a single
  // target -- guessing leads to broken hints.
  let alias_upper = alias.to_ascii_uppercase();
  if alias_upper == "NEW" || alias_upper == "OLD" {
    let pos: usize = u32::from(offset) as usize;
    let target = trigger_target_table(source).or_else(|| enclosing_fn_trigger_table(source, pos, cat));
    if let Some(t) = target {
      sources::columns_of_table(cat, None, &t, &mut out);
      return Some(out);
    }
    // No table known -- emit nothing so the user doesn't get a
    // misleading global column dump.
    return Some(out);
  }
  // `EXCLUDED.<col>` (inside INSERT ... ON CONFLICT DO UPDATE SET ...):
  // virtual row that mirrors the rejected INSERT row, so its column
  // shape matches the INSERT target table.
  if alias_upper == "EXCLUDED" {
    if let Some(t) = insert_target_table_name_only(source) {
      sources::columns_of_table(cat, None, &t, &mut out);
    }
    return Some(out);
  }
  let stmt_scope = scope_for_offset(file, scopes, offset);
  let count = stmt_scope.map(|s| sources::columns_of_alias(cat, s, &alias, &mut out)).unwrap_or(0);
  if count == 0
    && let Some(fb) = fallback::scope_from_text(current_statement_span(source, offset))
  {
    sources::columns_of_alias(cat, &fb, &alias, &mut out);
  }
  // CTE alias: surface columns the resolver extracted from the
  // CTE body projection. `cte_columns_of(alias)` returns
  // `Some(empty)` when the CTE is declared but the body was not
  // parsed -- in that case we have nothing useful to add.
  if out.is_empty()
    && let Some(s) = stmt_scope
    && let Some(cols) = s.cte_columns_of(&alias)
  {
    for col in cols {
      out.push(crate::item::Item {
        label: col.clone(),
        kind: crate::item::ItemKind::Column,
        detail: Some(format!("CTE {alias}")),
        description: None,
        documentation_md: None,
        insert_text: col.clone(),
        is_snippet: false,
        sort_priority: 0,
      });
    }
  }
  // Fallback: when pg_query refused the outer statement (typical
  // mid-typing `WITH t AS (...) SELECT t.`), the resolver never
  // ran -- so cte_columns_of returns None even though the CTE
  // is plainly declared. Text-scan the *current statement* for its
  // leading WITH and surface that CTE's projected columns. Must be
  // scoped to this statement: `cte_columns_from_text` only looks at
  // its argument's own prefix, so the whole buffer would check
  // statement #1 instead of the one under the cursor.
  if out.is_empty()
    && let Some(cols) = fallback::cte_columns_from_text(current_statement_span(source, offset), &alias)
  {
    for col in cols {
      out.push(crate::item::Item {
        label: col.clone(),
        kind: crate::item::ItemKind::Column,
        detail: Some(format!("CTE {alias}")),
        description: None,
        documentation_md: None,
        insert_text: col.clone(),
        is_snippet: false,
        sort_priority: 0,
      });
    }
  }
  // PL/pgSQL local typed as a catalog table (row variable).
  // `DECLARE r users; ... r.<TAB>` should list users' columns.
  if out.is_empty() {
    let pos: usize = u32::from(offset) as usize;
    let locals = crate::plpgsql_locals::extract(source, pos);
    if let Some(ty) = crate::plpgsql_locals::type_of(&locals, &alias) {
      // Strip `%ROWTYPE` suffix if present.
      let bare = ty.split('%').next().unwrap_or(&ty).trim().trim_end_matches(';').trim();
      if cat.find_table(None, bare).is_some() {
        sources::columns_of_table(cat, None, bare, &mut out);
      }
    }
  }
  // Schema-qualified relation slot: `FROM <schema>.<TAB>` /
  // `SELECT * FROM <schema>.|`. The alias here is the schema name,
  // not an in-scope alias; surface the tables/views that schema
  // exposes so the user can pick one. Emit nothing when the name
  // is neither a schema nor an alias -- a global dump would be wrong.
  if out.is_empty() {
    sources::tables_in_schema(cat, &alias, &mut out);
    // Also surface functions declared in this schema: `app.<TAB>`
    // should offer `app.current_user_id()`, `app.user_in_org(...)`,
    // etc., not just tables.
    sources::functions_in_schema(cat, &alias, &mut out);
  }
  // Last-resort: the alias names a real table in the live or derived
  // catalog (case-insensitive), even though it has no binding in the
  // current scope. Common when the user types `SELECT USERS.<cursor>`
  // before the FROM clause exists. pg_query rejects the prefix and
  // the fallback scope is empty, but the table is still resolvable.
  if out.is_empty() && cat.find_table(None, &alias).is_some() {
    sources::columns_of_table(cat, None, &alias, &mut out);
  }
  // Filter columns already used in the same clause -- even in dot
  // context, typing `SELECT u.id, u.|` should not re-offer `id`.
  let used = used_columns_in_clause(source, offset);
  if !used.is_empty() {
    out.retain(|it| !is_column_listed(it, &used));
  }
  Some(out)
}

/// GROUP BY GROUPING SETS ((<cursor>...)) -- inner tuple is a column
/// list slot. Must beat `contexts::detect` (which sees the inner
/// paren as a function-call expression context) and every Phase
/// variant.
fn detect_grouping_sets_inner_paren(
  source: &str,
  offset: TextSize,
  file: &ParsedFile,
  scopes: &[Scope],
  cat: &Catalog,
) -> Option<Vec<Item>> {
  if !grouping_sets_inner_paren_expects_column(source, offset) {
    return None;
  }
  let mut out = Vec::new();
  // Pull catalog columns directly off whatever the resolver or
  // text-fallback found in FROM. Skip the aliased-table hide rule
  // used by push_scope_columns -- inside GROUPING SETS the user
  // wants bare column names since each entry is part of a tuple,
  // not a free expression.
  let mut tables: Vec<(Option<String>, String)> = Vec::new();
  if let Some(scope) = scope_for_offset(file, scopes, offset) {
    for b in scope.tables() {
      tables.push((b.table.schema.clone(), b.table.name.clone()));
    }
  }
  if tables.is_empty()
    && let Some(fb) = fallback::scope_from_text(current_statement_span(source, offset))
  {
    for b in fb.tables() {
      tables.push((b.table.schema.clone(), b.table.name.clone()));
    }
  }
  let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
  for (schema, name) in tables {
    let key = format!("{}.{}", schema.as_deref().unwrap_or(""), name.to_ascii_lowercase());
    if !seen.insert(key) {
      continue;
    }
    sources::columns_of_table(cat, schema.as_deref(), &name, &mut out);
  }
  push_aliases(file, scopes, source, offset, &mut out);
  Some(out)
}

/// Special context completions (INDEX USING method, TRIGGER EXECUTE
/// FUNCTION, CALL procedure, CREATE POLICY FOR/TO, ALTER COLUMN TYPE,
/// index opclass slot, trigger event slot, trigger ON table). All run
/// *before* the index/table phases because they're more specific than
/// the column dump those phases would emit.
fn detect_contexts(source: &str, offset: TextSize, _file: &ParsedFile, _scopes: &[Scope], cat: &Catalog) -> Option<Vec<Item>> {
  crate::contexts::detect(source, offset, cat)
}

/// Every pre-phase check, in the exact precedence order they used to
/// run as a chain of `if`/`if let` statements at the top of
/// `complete_with_derived` -- this array is now the only place that
/// order lives. `complete_with_derived` runs them in sequence right
/// after normalising the offset and merging the catalog; the first
/// `Some` wins. Falling through every entry means none of these
/// higher-priority contexts apply, so control moves on to the
/// `create_index::detect` / `create_table::detect` phase overrides
/// and finally `phase::detect` + `route_phase`'s own
/// `POST_PHASE_DETECTORS` registry.
const PRE_PHASE_DETECTORS: &[Detector] = &[
  detect_fresh_name_slot,
  detect_json_path_key,
  detect_inert_span,
  detect_dot_context,
  detect_grouping_sets_inner_paren,
  detect_contexts,
];

/// A post-phase detector: given the same `(source, offset, file,
/// scopes, cat)` `route_phase` receives, either claims the slot
/// (`Some`, short-circuiting every remaining detector and the `Phase`
/// match) or declines (`None`, falls through to the next entry).
/// Unused parameters are intentional -- most detectors are pure text
/// checks and only need `source`/`offset`, but the type must be
/// uniform across every entry in [`POST_PHASE_DETECTORS`].
type Detector = fn(&str, TextSize, &ParsedFile, &[Scope], &Catalog) -> Option<Vec<Item>>;

/// UNION/INTERSECT/EXCEPT trailing slot expects ALL/DISTINCT/SELECT/
/// VALUES, never an expression-list dump.
fn detect_set_op_followup(
  source: &str,
  offset: TextSize,
  _file: &ParsedFile,
  _scopes: &[Scope],
  _cat: &Catalog,
) -> Option<Vec<Item>> {
  let kws = set_op_followup_next_keyword(source, offset)?;
  let mut out = Vec::new();
  push_keyword_kvs(&mut out, kws);
  Some(out)
}

/// JSON_TABLE(... COLUMNS (<cursor> at a fresh column-def slot -- a
/// brand-new name, not a catalog entity, so must beat the generic
/// table/column dump every phase would otherwise emit.
fn detect_json_table_fresh_column_slot(
  source: &str,
  offset: TextSize,
  _file: &ParsedFile,
  _scopes: &[Scope],
  _cat: &Catalog,
) -> Option<Vec<Item>> {
  if !json_table_fresh_column_slot(source, offset) {
    return None;
  }
  let mut out = Vec::new();
  push_keyword_kvs(&mut out, &[("FOR", "<name> FOR ORDINALITY -- 1-based row-number column")]);
  Some(out)
}

/// JSON_TABLE column-def grammar beyond the fresh-slot case above --
/// type slot, FOR ORDINALITY, PATH/FORMAT/EXISTS, FORMAT JSON, EXISTS
/// PATH. See `json_table_column_slot_items`'s doc comment for the
/// exact slot-by-slot breakdown; must beat the generic table/column
/// dump the same way the fresh-slot case does.
fn detect_json_table_column_slot(
  source: &str,
  offset: TextSize,
  _file: &ParsedFile,
  _scopes: &[Scope],
  _cat: &Catalog,
) -> Option<Vec<Item>> {
  json_table_column_slot_items(source, offset)
}

/// CREATE TRANSFORM ... otherwise gets swallowed by the Phase::Start
/// statement-keyword dump.
fn detect_create_transform(
  source: &str,
  offset: TextSize,
  _file: &ParsedFile,
  _scopes: &[Scope],
  _cat: &Catalog,
) -> Option<Vec<Item>> {
  let kws = create_transform_next_keyword(source, offset)?;
  let mut out = Vec::new();
  push_keyword_kvs(&mut out, kws);
  Some(out)
}

/// TABLESAMPLE REPEATABLE chain must beat the SELECT trailing-clause
/// menu (which would emit JOIN/WHERE/ORDER BY).
fn detect_tablesample_after_paren(
  source: &str,
  offset: TextSize,
  _file: &ParsedFile,
  _scopes: &[Scope],
  _cat: &Catalog,
) -> Option<Vec<Item>> {
  let kws = tablesample_after_paren_next_keyword(source, offset)?;
  let mut out = Vec::new();
  push_keyword_kvs(&mut out, kws);
  Some(out)
}

/// `agg(...) FILTER (<cursor>` -- only WHERE is legal, must beat the
/// expression/column dump every phase would otherwise emit.
fn detect_filter_clause(
  source: &str,
  offset: TextSize,
  _file: &ParsedFile,
  _scopes: &[Scope],
  _cat: &Catalog,
) -> Option<Vec<Item>> {
  let kws = filter_clause_next_keyword(source, offset)?;
  let mut out = Vec::new();
  push_keyword_kvs(&mut out, kws);
  Some(out)
}

/// `CREATE POLICY ... USING (⏵` / `... WITH CHECK (⏵` -- must beat
/// the generic phase's ~2300-item fallback dump the same way the
/// other slot-keyword shortcuts beat the generic menu for their
/// narrower contexts.
fn detect_policy_expr(
  source: &str,
  offset: TextSize,
  _file: &ParsedFile,
  _scopes: &[Scope],
  cat: &Catalog,
) -> Option<Vec<Item>> {
  policy_expr_items(source, offset, cat)
}

/// `<table-fn>(...) WITH <cursor>` in FROM/JOIN -- only ORDINALITY is
/// legal, must beat the JOIN/WHERE/ORDER BY clause-continuation menu
/// the FROM-item-just-finished phase would otherwise emit.
fn detect_table_function_with_ordinality(
  source: &str,
  offset: TextSize,
  _file: &ParsedFile,
  _scopes: &[Scope],
  _cat: &Catalog,
) -> Option<Vec<Item>> {
  let kws = table_function_with_ordinality_next_keyword(source, offset)?;
  let mut out = Vec::new();
  push_keyword_kvs(&mut out, kws);
  Some(out)
}

/// `SELECT ... FETCH FIRST/NEXT <n> ROW(S) <cursor>` -- only fires
/// when the last token before the cursor is actually part of that
/// chain (`select_fetch_offset_next_keyword` alone over-matches).
fn detect_select_fetch_offset(
  source: &str,
  offset: TextSize,
  _file: &ParsedFile,
  _scopes: &[Scope],
  _cat: &Catalog,
) -> Option<Vec<Item>> {
  let kws = select_fetch_offset_next_keyword(source, offset)?;
  let (_, upper) = stmt_slice_upper(source, offset);
  let words: Vec<&str> = upper.split_ascii_whitespace().collect();
  let last = words.last().copied();
  if !matches!(last, Some("FETCH") | Some("FIRST") | Some("NEXT") | Some("ROW") | Some("ROWS")) {
    return None;
  }
  let mut out = Vec::new();
  push_keyword_kvs(&mut out, kws);
  Some(out)
}

/// CREATE VIEW / MATERIALIZED VIEW trailing WITH clauses (CHECK
/// OPTION, WITH DATA) must beat the SELECT-body phase that otherwise
/// emits join/order kws after `WITH `.
fn detect_create_view_post_name(
  source: &str,
  offset: TextSize,
  _file: &ParsedFile,
  _scopes: &[Scope],
  _cat: &Catalog,
) -> Option<Vec<Item>> {
  let kws = create_view_post_name_next_keyword(source, offset)?;
  let (_, upper) = stmt_slice_upper(source, offset);
  let words: Vec<&str> = upper.split_ascii_whitespace().collect();
  let last = words.last().copied();
  if !matches!(last, Some("WITH") | Some("NO") | Some("CASCADED") | Some("LOCAL")) {
    return None;
  }
  let mut out = Vec::new();
  push_keyword_kvs(&mut out, kws);
  Some(out)
}

/// WINDOW frame-bound sub-chain only -- the fresh-slot and
/// PARTITION/ORDER BY column-list paths must keep flowing to the
/// catalog so existing tests stay green, so this only fires when the
/// last token before the cursor is a frame-family keyword.
fn detect_window_clause_frame_bound(
  source: &str,
  offset: TextSize,
  _file: &ParsedFile,
  _scopes: &[Scope],
  _cat: &Catalog,
) -> Option<Vec<Item>> {
  if window_clause_partition_or_order_by_expects_column(source, offset) {
    return None;
  }
  let kws = window_clause_as_paren_keyword(source, offset)?;
  let pos: usize = (u32::from(offset) as usize).min(source.len());
  let pre = &source[..pos];
  let stmt_start_idx = pre.rfind(';').map(|i| i + 1).unwrap_or(0);
  let words: Vec<&str> = pre[stmt_start_idx..].split_ascii_whitespace().collect();
  let last_up = words.last().map(|s| s.to_ascii_uppercase()).unwrap_or_default();
  if !matches!(
    last_up.as_str(),
    "RANGE" | "ROWS" | "GROUPS" | "BETWEEN" | "AND" | "PRECEDING" | "FOLLOWING" | "UNBOUNDED" | "CURRENT" | "EXCLUDE"
  ) {
    return None;
  }
  let mut out = Vec::new();
  push_keyword_kvs(&mut out, kws);
  Some(out)
}

/// Every post-phase shortcut, in the exact precedence order they used
/// to run as a chain of `if`/`if let` statements at the top of
/// `route_phase` -- this array is now the only place that order
/// lives. `route_phase` runs them in sequence; the first `Some` wins.
///
/// Dropped rather than migrated: a `grouping_sets_inner_paren_expects_
/// column` check used to sit here too, but it's dead code -- `complete_
/// with_derived` already checks the identical `(source, offset)` pair
/// earlier and returns unconditionally when true (see the dot-context
/// block above), and `source`/`offset` are unmodified between that
/// check and every call into `route_phase`, so the condition is
/// guaranteed false by the time any of these detectors run.
const POST_PHASE_DETECTORS: &[Detector] = &[
  detect_set_op_followup,
  detect_json_table_fresh_column_slot,
  detect_json_table_column_slot,
  detect_create_transform,
  detect_tablesample_after_paren,
  detect_filter_clause,
  detect_policy_expr,
  detect_table_function_with_ordinality,
  detect_select_fetch_offset,
  detect_create_view_post_name,
  detect_window_clause_frame_bound,
];

fn route_phase(
  ph: Phase,
  file: &ParsedFile,
  scopes: &[Scope],
  source: &str,
  cat: &Catalog,
  offset: TextSize,
) -> Vec<Item> {
  for detector in POST_PHASE_DETECTORS {
    if let Some(items) = detector(source, offset, file, scopes, cat) {
      return items;
    }
  }
  let mut out = Vec::new();
  match ph {
    Phase::Start => {
      // `CREATE VIEW v AS <cursor>` -- the body must start with
      // SELECT / WITH / VALUES / TABLE. The phase machine's anchor
      // routes us here, but the full statement-start menu (47 items)
      // includes DDL like CREATE TABLE / DROP / INSERT which PG won't
      // accept here. Narrow.
      if at_create_view_body_start(source, offset) {
        for (kw, doc) in [
          ("SELECT", "the projection-list query that defines the view"),
          ("WITH", "CTE list then SELECT"),
          ("VALUES", "VALUES (...) literal-row source"),
          ("TABLE", "TABLE <name> -- shorthand for SELECT * FROM <name>"),
        ] {
          out.push(crate::item::Item {
            label: kw.into(),
            kind: crate::item::ItemKind::Keyword,
            detail: Some(doc.into()),
            insert_text: kw.into(),
            sort_priority: 0,
            ..Default::default()
          });
        }
      } else {
        sources::statement_keywords(&mut out);
      }
    },

    Phase::SelectProjection | Phase::InProjection | Phase::NextProjection => {
      push_scope_columns_or_all(file, scopes, source, cat, offset, &mut out);
      push_aliases(file, scopes, source, offset, &mut out);
      push_all_functions(cat, &mut out);
      sources::expression_keywords(&mut out);
    },

    Phase::AfterStar | Phase::ProjectionAlias => {
      // CAST(<expr> AS <type>) is parsed as a projection-AS slot, but
      // the legal next token is a type, not the FROM/INTO menu. Detect
      // by walking back: the most recent unmatched `(` is preceded by
      // `CAST`.
      if cast_as_expects_type(source, offset) {
        sources::types_only(&mut out);
      } else {
        // Just typed `*` or `AS alias`. Next legal tokens: FROM
        // (continue the query) or `,` (more projection). Emit only
        // the small after-projection keyword set.
        sources::after_projection_keywords(&mut out);
      }
    },

    Phase::ExpectTable => {
      sources::tables(cat, &mut out);
      push_cte_names(file, scopes, source, offset, &mut out);
    },

    Phase::AfterTable | Phase::JoinModifier | Phase::JoinComplete => {
      // `WINDOW w AS (PARTITION BY <cursor>` or `(ORDER BY <cursor>`
      // -- inside a window-clause body, expects column references
      // from the FROM tables, not JOIN keywords.
      if window_clause_partition_or_order_by_expects_column(source, offset) {
        push_scope_columns_or_all(file, scopes, source, cat, offset, &mut out);
        push_aliases(file, scopes, source, offset, &mut out);
        return dedup_items(out);
      }
      // `WINDOW w AS (<cursor>` -- start of a window-clause body.
      // First sub-clause is PARTITION BY / ORDER BY / ROWS / RANGE /
      // GROUPS.
      if window_clause_paren_expects_subclause(source, offset) {
        for (kw, doc) in [
          ("PARTITION BY", "PARTITION BY <expr>[, ...] -- frame partitioning"),
          ("ORDER BY", "ORDER BY <expr>[, ...] -- frame ordering"),
          ("ROWS", "ROWS BETWEEN ... -- row-relative frame"),
          ("RANGE", "RANGE BETWEEN ... -- value-relative frame"),
          ("GROUPS", "GROUPS BETWEEN ... -- peer-group frame"),
        ] {
          out.push(crate::item::Item {
            label: kw.into(),
            kind: crate::item::ItemKind::Keyword,
            detail: Some(doc.into()),
            description: None,
            documentation_md: None,
            insert_text: kw.into(),
            is_snippet: false,
            sort_priority: 0,
          });
        }
        return dedup_items(out);
      }
      // `... LATERAL <cursor>` (after JOIN or FROM,) -- the only legal
      // followers are a set-returning function call or a parenthesized
      // subquery. The generic AfterTable handler would offer JOIN
      // keywords, WHERE, and the table list -- all wrong here.
      if lateral_target_expected(source, offset) {
        for (label, doc) in LATERAL_TARGETS {
          out.push(crate::item::Item {
            label: (*label).into(),
            kind: crate::item::ItemKind::Function,
            detail: Some((*doc).into()),
            description: None,
            documentation_md: None,
            insert_text: (*label).into(),
            is_snippet: false,
            sort_priority: 0,
          });
        }
        // Catalog tables are also legal: `LATERAL <table>` is the
        // sub-SELECT shortcut form. Surface them so the user can pick
        // a relation as the LATERAL source without a parenthesized
        // SELECT wrapper.
        sources::tables(cat, &mut out);
        return dedup_items(out);
      }
      // `SELECT ... <table> TABLESAMPLE <cursor>` -- sampling method
      // slot (BERNOULLI / SYSTEM). The generic AfterTable handler
      // would wrongly offer JOIN keywords.
      if tablesample_expects_method(source, offset) {
        push_keyword_kvs(&mut out, &[
          ("BERNOULLI", "TABLESAMPLE BERNOULLI (<percent>) -- row-level uniform sample"),
          ("SYSTEM", "TABLESAMPLE SYSTEM (<percent>) -- page-level random sample"),
        ]);
        return dedup_items(out);
      }
      // `SELECT ... FOR <cursor>` / `... FOR UPDATE|SHARE <cursor>`
      // -- locking clause. Narrow to those keywords instead of the
      // generic JOIN/WHERE/GROUP follow-up dump.
      if let Some(kws) = select_for_locking_keywords(source, offset) {
        for (kw, doc) in kws {
          out.push(crate::item::Item {
            label: (*kw).into(),
            kind: crate::item::ItemKind::Keyword,
            detail: Some((*doc).into()),
            description: None,
            documentation_md: None,
            insert_text: (*kw).into(),
            is_snippet: false,
            sort_priority: 0,
          });
        }
      } else {
        push_aliases(file, scopes, source, offset, &mut out);
        sources::after_table_keywords(&mut out);
      }
    },

    Phase::OnClause | Phase::WhereClause | Phase::InPredicate | Phase::HavingClause => {
      // `<col> IS <cursor>` and `<col> IS NOT <cursor>` are tightly-
      // scoped slots whose only legal next tokens are NULL / TRUE /
      // FALSE / UNKNOWN / DISTINCT FROM (NOT NULL only after `IS`).
      // Surface just those keywords instead of the full expression
      // menu (350+ functions).
      if let Some(kws) = is_predicate_continuation_keywords(source, offset) {
        for (kw, doc) in kws {
          out.push(crate::item::Item {
            label: (*kw).into(),
            kind: crate::item::ItemKind::Keyword,
            detail: Some((*doc).into()),
            description: None,
            documentation_md: None,
            insert_text: (*kw).into(),
            is_snippet: false,
            sort_priority: 0,
          });
        }
      } else {
        push_scope_columns_or_all(file, scopes, source, cat, offset, &mut out);
        push_aliases(file, scopes, source, offset, &mut out);
        push_all_functions(cat, &mut out);
        sources::expression_keywords(&mut out);
      }
    },

    Phase::UsingClause => {
      push_scope_columns_or_all(file, scopes, source, cat, offset, &mut out);
      push_aliases(file, scopes, source, offset, &mut out);
    },

    Phase::AfterGroup | Phase::AfterOrder => {
      // Just typed GROUP / ORDER, next is "BY".
      sources::after_table_keywords(&mut out);
    },
    Phase::GroupByList => {
      // PG-specific set-grouping prefixes -- offer alongside columns
      // so `GROUP BY <cursor>` surfaces GROUPING SETS / CUBE / ROLLUP.
      if let Some(kws) = group_by_set_op_next_keyword(source, offset) {
        push_keyword_kvs(&mut out, kws);
      }
      push_scope_columns_or_all(file, scopes, source, cat, offset, &mut out);
      push_aliases(file, scopes, source, offset, &mut out);
      push_all_functions(cat, &mut out);
    },
    Phase::OrderByList => {
      // `... ORDER BY <col> NULLS <cursor>` -- after NULLS the only
      // legal continuation is FIRST | LAST. Suppress the full column
      // + function dump for this tightly-scoped slot.
      if order_by_nulls_expects_first_last(source, offset) {
        push_keyword_kvs(&mut out, &[
          ("FIRST", "NULLS FIRST -- NULLs sort before non-NULL values"),
          ("LAST", "NULLS LAST -- NULLs sort after non-NULL values"),
        ]);
      } else {
        push_scope_columns_or_all(file, scopes, source, cat, offset, &mut out);
        push_aliases(file, scopes, source, offset, &mut out);
        push_all_functions(cat, &mut out);
        sources::order_modifiers(&mut out);
      }
    },

    Phase::LimitClause => {
      // LIMIT takes an integer literal. The only useful follow-up
      // keyword is OFFSET. Emitting after_table_keywords (which
      // contains the JOIN family + WHERE / GROUP / ORDER) is wrong
      // here -- those don't follow LIMIT.
      out.push(crate::item::Item {
        label: "OFFSET".into(),
        kind: crate::item::ItemKind::Keyword,
        detail: Some("OFFSET <n> -- skip the first n rows".into()),
        description: None,
        documentation_md: None,
        insert_text: "OFFSET".into(),
        is_snippet: false,
        sort_priority: 0,
      });
    },
    Phase::OffsetClause => {
      // OFFSET takes an integer literal. Nothing meaningful follows
      // until the user types a comma / semicolon. Emit nothing.
    },

    Phase::AfterInsert => {
      sources::after_projection_keywords(&mut out);
    },
    Phase::AfterInsertTable => {
      sources::tables(cat, &mut out);
    },
    Phase::InsertColumnList => {
      // OVERRIDING SYSTEM/USER mid-clause -- emit the dedicated VALUE
      // / SYSTEM VALUE / USER VALUE followup menu.
      if let Some(kws) = insert_overriding_next_keyword(source, offset) {
        push_keyword_kvs(&mut out, kws);
        return out;
      }
      // Closed paren list -> body-shape menu (VALUES/SELECT/...).
      // Phase machine stays InsertColumnList until VALUES/SELECT token
      // arrives, but the user is past the column list at `)` and wants
      // the next-keyword menu, not another column suggestion.
      if let Some(kws) = insert_into_next_keyword(source, offset) {
        push_keyword_kvs(&mut out, kws);
        return out;
      }
      // Strict: only columns of the INSERT target table. Falling back
      // to the global column dump showed every catalog table's columns
      // (huge menu) which is never what the user wants in a paren list
      // that PG strictly validates against the target.
      if let Some(target) = insert_target_table(source, offset) {
        sources::columns_of_table(cat, None, &target, &mut out);
        // Filter out columns the user already typed in this paren list.
        let used = used_columns_in_clause(source, offset);
        if !used.is_empty() {
          out.retain(|it| !is_column_listed(it, &used));
        }
      } else {
        push_scope_columns_or_all(file, scopes, source, cat, offset, &mut out);
        push_aliases(file, scopes, source, offset, &mut out);
      }
    },
    Phase::InsertExpectValues | Phase::InsertValuesList => {
      // ON CONFLICT (...) and ON CONFLICT DO UPDATE SET ... are
      // column-LHS slots scoped to the INSERT target table -- not a
      // free expression context like VALUES (...). Narrow to columns
      // of the target so the menu isn't 300+ functions.
      if let Some(kws) = on_conflict_next_keyword(source, offset) {
        push_keyword_kvs(&mut out, kws);
      } else if on_conflict_expects_target_column(source, offset) {
        if let Some(target) = dml_target_table(source, offset) {
          sources::columns_of_table(cat, None, &target, &mut out);
          let used = used_columns_in_clause(source, offset);
          if !used.is_empty() {
            out.retain(|it| !is_column_listed(it, &used));
          }
        }
      } else if insert_after_values_tuple(source, offset) {
        // After a closed `VALUES (...)` tuple at depth 0, the legal
        // continuations are `,` (another tuple), `RETURNING`,
        // `ON CONFLICT`, or `;`. Narrow the menu instead of dumping
        // 351 functions.
        for (kw, doc) in [
          ("RETURNING", "RETURNING <cols> -- return inserted rows"),
          ("ON CONFLICT", "ON CONFLICT (cols) DO ... -- upsert handling"),
        ] {
          out.push(crate::item::Item {
            label: kw.into(),
            kind: crate::item::ItemKind::Keyword,
            detail: Some(doc.into()),
            insert_text: kw.into(),
            sort_priority: 0,
            ..Default::default()
          });
        }
      } else {
        // DEFAULT is the most-useful VALUES-list token: it tells PG
        // "use the column default" and is far more relevant than any
        // catalog function here. Promote it to the top of the menu.
        out.push(
          crate::item::Item {
            label: "DEFAULT".into(),
            kind: crate::item::ItemKind::Keyword,
            detail: Some("use this column's DEFAULT value".into()),
            insert_text: "DEFAULT".into(),
            ..Default::default()
          }
          .with_sort(0),
        );
        push_aliases(file, scopes, source, offset, &mut out);
        push_all_functions(cat, &mut out);
        sources::expression_keywords(&mut out);
      }
    },

    Phase::AfterUpdate => {
      sources::tables(cat, &mut out);
    },
    Phase::AfterUpdateTable => {
      push_aliases(file, scopes, source, offset, &mut out);
      sources::after_table_keywords(&mut out);
    },
    Phase::UpdateAssignment => {
      // Two slots collapsed into one phase: the column LHS (before any
      // `=` since the last comma or SET) and the value expression RHS.
      // The LHS slot is narrow -- only the target table's columns,
      // minus any already named earlier in the SET list.
      if update_set_at_column_slot(source, offset) {
        if let Some(target) = dml_target_table(source, offset) {
          sources::columns_of_table(cat, None, &target, &mut out);
        } else {
          push_scope_columns_or_all(file, scopes, source, cat, offset, &mut out);
        }
        let used = used_columns_in_clause(source, offset);
        if !used.is_empty() {
          out.retain(|it| !used.contains(&it.label.to_ascii_lowercase()));
        }
      } else {
        push_scope_columns_or_all(file, scopes, source, cat, offset, &mut out);
        push_aliases(file, scopes, source, offset, &mut out);
        push_all_functions(cat, &mut out);
        sources::expression_keywords(&mut out);
      }
    },

    Phase::AfterDelete => {
      sources::after_projection_keywords(&mut out);
    },

    Phase::ReturningClause => {
      // `RETURNING <expr> AS <cursor>` -- alias-name slot. The next
      // token is a free-form identifier the user types. Don't dump
      // the catalog (was 1100+ items pre-fix); leave the menu empty
      // so the user just types the name.
      let (_, ret_upper) = stmt_slice_upper(source, offset);
      let ret_words: Vec<&str> = ret_upper.split_ascii_whitespace().collect();
      let last_returning_word = ret_words.last().copied();
      if last_returning_word == Some("AS") {
        return out;
      }
      // INSERT / UPDATE / DELETE ... RETURNING <cursor> -- expression
      // context (PG accepts any expression here, not just plain
      // column refs). Emit target-table columns first (highest sort
      // priority), then the full function library + expression
      // keywords so things like `left(id::text, 10)`, `count(*)`,
      // `now()`, `coalesce(...)`, etc. complete cleanly.
      if let Some(target) = dml_target_table(source, offset) {
        sources::columns_of_table(cat, None, &target, &mut out);
      } else {
        push_scope_columns_or_all(file, scopes, source, cat, offset, &mut out);
      }
      push_aliases(file, scopes, source, offset, &mut out);
      push_all_functions(cat, &mut out);
      sources::expression_keywords(&mut out);
      let used = used_columns_in_clause(source, offset);
      if !used.is_empty() {
        out.retain(|it| !is_column_listed(it, &used));
      }
    },

    // CREATE TABLE sub-phases ---------------------------------------
    Phase::CtlExpectTableName => {
      // Fresh name slot. The only sensible keyword suggestion here is
      // the optional IF NOT EXISTS qualifier the user may want to add
      // before typing the actual table name.
      out.push(crate::item::Item {
        label: "IF NOT EXISTS".into(),
        kind: crate::item::ItemKind::Keyword,
        detail: Some("CREATE TABLE IF NOT EXISTS <name> -- skip silently if already present".into()),
        insert_text: "IF NOT EXISTS".into(),
        sort_priority: 0,
        ..Default::default()
      });
    },
    Phase::CtlBodyStart => {
      // EXCLUDE constraint sub-chain wins over the generic body menu
      // when the user has typed EXCLUDE / EXCLUDE USING.
      if let Some(kws) = exclude_constraint_next_keyword(source, offset) {
        push_keyword_kvs(&mut out, kws);
      } else {
        // User could be starting a column declaration (no completion
        // for the name itself) or a table-level constraint line. Also
        // surface types so once they have a name typed and hit space,
        // the next keystroke (a type letter) keeps the dropdown open
        // -- the LSP client filters by prefix, so the type-name slot
        // shows only the matching types and the constraint-starters.
        sources::create_table_entry_starters(&mut out);
        sources::types_only(&mut out);
      }
    },
    Phase::CtlExpectType => {
      // EXCLUDE / EXCLUDE USING -- specialised constraint chain wins.
      if let Some(kws) = exclude_constraint_next_keyword(source, offset) {
        push_keyword_kvs(&mut out, kws);
      } else {
        sources::types_only(&mut out);
      }
    },
    Phase::CtlExpectColumnConstraint => {
      // `... AS IDENTITY ( <cursor>` -- sequence option-name slot.
      if let Some(kws) = identity_paren_next_keyword(source, offset) {
        push_keyword_kvs(&mut out, kws);
      // `... GENERATED <cursor>` / `... GENERATED ALWAYS AS <cursor>` /
      // `... AS (expr) <cursor>` -- specialised GENERATED chain.
      } else if let Some(kws) = column_generated_next_keyword(source, offset) {
        push_keyword_kvs(&mut out, kws);
      // `... DEFERRABLE <cursor>` / `... INITIALLY <cursor>` -- tail of
      // an inline constraint (PRIMARY KEY/UNIQUE/REFERENCES/FK ON ...).
      // Without this branch the generic column-constraint menu fires
      // and drowns the user in NOT NULL/CHECK/etc.
      } else if let Some(kws) = column_constraint_next_keyword(source, offset) {
        push_keyword_kvs(&mut out, kws);
      } else if ctl_column_constraint_after_default(source, offset) {
        for (kw, doc) in DEFAULT_EXPRESSION_SUGGESTIONS {
          out.push(crate::item::Item {
            label: (*kw).into(),
            kind: crate::item::ItemKind::Keyword,
            detail: Some((*doc).into()),
            insert_text: (*kw).into(),
            sort_priority: 0,
            ..Default::default()
          });
        }
        // Surface the full function library + expression keywords so a
        // user typing `DEFAULT length(...)` / `DEFAULT now() + INTERVAL ...`
        // gets matching completions instead of just the short menu.
        push_all_functions(cat, &mut out);
        sources::expression_keywords(&mut out);
      } else {
        sources::column_constraint_keywords(&mut out);
        // Constraint keywords like DEFAULT / CHECK introduce
        // expression contexts. Surface functions + expression
        // keywords here too so `col text DEFAULT now()` /
        // `col text CHECK (length(col) > 0)` autocompletes the
        // function names without forcing a new phase.
        push_all_functions(cat, &mut out);
        sources::expression_keywords(&mut out);
      }
    },
    Phase::CtlExpectConstraintName => {
      // Fresh constraint name; nothing useful.
    },
    Phase::CtlExpectConstraintKind => {
      sources::constraint_kinds(&mut out);
    },
    Phase::CtlExpectFkTable {} => {
      sources::tables(cat, &mut out);
    },
    Phase::CtlCheckExpr { ref table } => {
      if let Some(t) = table.as_ref() {
        sources::columns_of_table(cat, None, t, &mut out);
        if out.is_empty() {
          for name in crate::source_tables::buffer_column_names(source, t) {
            out.push(crate::item::Item {
              label: name.clone(),
              kind: crate::item::ItemKind::Column,
              detail: Some(format!("column of `{t}` (buffer)")),
              description: None,
              documentation_md: None,
              insert_text: name,
              is_snippet: false,
              sort_priority: 0,
            });
          }
        }
      }
      push_all_functions(cat, &mut out);
      sources::expression_keywords(&mut out);
    },
    Phase::CtlExpectFkColumn { ref table } => {
      sources::columns_of_table(cat, None, table, &mut out);
      // Fallback: the table being created may not have parsed
      // cleanly yet (cursor inside an unclosed body). Scan the
      // buffer for `CREATE TABLE <table>` and harvest column names
      // directly.
      if out.is_empty() {
        for name in crate::source_tables::buffer_column_names(source, table) {
          out.push(crate::item::Item {
            label: name.clone(),
            kind: crate::item::ItemKind::Column,
            detail: Some(format!("{}.<column>", table)),
            description: Some("buffer".into()),
            documentation_md: None,
            insert_text: name,
            is_snippet: false,
            sort_priority: 5,
          });
        }
      }
    },

    // PL/pgSQL body --------------------------------------------------
    Phase::PlpgsqlBody => {
      if let Some(items) = plpgsql_body_from_or_where_items(source, offset, file, scopes, cat) {
        out = items;
      } else {
        // Function parameters and DECLARE'd locals first so they
        // sort above the broader keyword / function lists.
        let locals = crate::plpgsql_locals::extract(source, u32::from(offset) as usize);
        crate::plpgsql_locals::push_items(&locals, &mut out);
        // PL/pgSQL flow keywords + standard built-ins + NEW / OLD
        // identifiers + any FROM/JOIN aliases inside the body.
        sources::plpgsql_keywords(&mut out);
        push_aliases(file, scopes, source, offset, &mut out);
        push_all_functions(cat, &mut out);
        sources::new_old_aliases(&mut out);
        sources::tables(cat, &mut out);
        sources::columns(cat, &mut out);
      }
    },
    // Right-hand side of an assignment -- expression only. Skip the
    // statement-starter keywords (SELECT / CREATE / DELETE / ...).
    // Skip the all-tables column dump too -- the user reaches for
    // NEW.col / OLD.col / a parameter, not a random column from
    // some unrelated table.
    Phase::PlpgsqlAssignRhs => {
      let locals = crate::plpgsql_locals::extract(source, u32::from(offset) as usize);
      crate::plpgsql_locals::push_items(&locals, &mut out);
      sources::new_old_aliases(&mut out);
      push_aliases(file, scopes, source, offset, &mut out);
      push_all_functions(cat, &mut out);
      sources::expression_keywords(&mut out);
    },

    // After PG `::` cast operator -- emit types only. Built-ins
    // first, then user-defined enums/domains/composites from the
    // live catalog.
    Phase::CastType => {
      sources::types(&mut out);
      sources::db_types(cat, &mut out);
    },

    Phase::AfterAlterTableExpectName => {
      sources::tables(cat, &mut out);
    },
    Phase::AfterAlterTableName => {
      // ALTER TABLE <t> DROP/RENAME/ALTER COLUMN <cursor> -- the user
      // is picking an EXISTING column of the target table, not an
      // action keyword. Detect that slot from the recent tokens and
      // surface columns instead of the action menu.
      if let Some(target) = alter_table_existing_column_target(source, offset) {
        sources::columns_of_table(cat, None, &target, &mut out);
      } else if alter_table_expects_type(source, offset) {
        // ALTER TABLE <t> ADD COLUMN <name> <cursor> -- after the fresh
        // column name, the next token is a type. Same shape as
        // CtlExpectType inside a CREATE TABLE body.
        sources::types_only(&mut out);
      } else if let Some(kws) = alter_column_set_value_keywords(source, offset) {
        push_keyword_kvs(&mut out, kws);
      } else if alter_column_after_set_subkeyword_expects_silence(source, offset) {
        // `ALTER COLUMN <name> SET DEFAULT|STATISTICS <cursor>` --
        // these slots take freeform expressions / integers that have
        // no useful catalog completion. SET STORAGE / SET COMPRESSION
        // are handled above (curated value menu).
      } else if let Some(kind) = alter_column_action_kind(source, offset) {
        // `ALTER COLUMN <name> SET <cursor>` / `... DROP <cursor>` --
        // the slot is a SET/DROP sub-keyword, not the top-level
        // ALTER TABLE action menu.
        let kws: &[(&str, &str)] = match kind {
          AlterColumnAction::Set => &[
            ("DEFAULT", "SET DEFAULT <expr>"),
            ("NOT NULL", "SET NOT NULL"),
            ("DATA TYPE", "SET DATA TYPE <type>"),
            ("STATISTICS", "SET STATISTICS <int>"),
            ("STORAGE", "SET STORAGE PLAIN|EXTERNAL|EXTENDED|MAIN"),
            ("COMPRESSION", "SET COMPRESSION pglz|lz4|default"),
          ],
          AlterColumnAction::Drop => &[
            ("DEFAULT", "DROP DEFAULT"),
            ("NOT NULL", "DROP NOT NULL"),
            ("IDENTITY", "DROP IDENTITY"),
            ("EXPRESSION", "DROP EXPRESSION"),
          ],
        };
        for (kw, doc) in kws {
          out.push(crate::item::Item {
            label: (*kw).into(),
            kind: crate::item::ItemKind::Keyword,
            detail: Some((*doc).into()),
            description: None,
            documentation_md: None,
            insert_text: (*kw).into(),
            is_snippet: false,
            sort_priority: 0,
          });
        }
      } else if let Some(kws) = column_constraint_next_keyword(source, offset) {
        // `ALTER TABLE ... REFERENCES other(col) ON DELETE <cursor>` /
        // `... DEFERRABLE <cursor>` etc -- FK action / deferrable slot.
        push_keyword_kvs(&mut out, kws);
      } else if let Some(kws) = column_generated_next_keyword(source, offset) {
        push_keyword_kvs(&mut out, kws);
      } else if let Some(kws) = check_constraint_no_inherit_next_keyword(source, offset) {
        push_keyword_kvs(&mut out, kws);
      } else if let Some(kws) = replica_identity_next_keyword(source, offset) {
        push_keyword_kvs(&mut out, kws);
      } else if alter_table_add_column_after_default(source, offset) {
        // `ALTER TABLE t ADD COLUMN c <type> DEFAULT <cursor>` --
        // expression slot. The action menu would suggest ADD COLUMN /
        // DROP COLUMN here which makes no sense. Emit a curated set of
        // common default expressions.
        for (kw, doc) in DEFAULT_EXPRESSION_SUGGESTIONS {
          out.push(crate::item::Item {
            label: (*kw).into(),
            kind: crate::item::ItemKind::Keyword,
            detail: Some((*doc).into()),
            insert_text: (*kw).into(),
            sort_priority: 0,
            ..Default::default()
          });
        }
      } else if alter_table_inherit_expects_parent(source, offset) {
        // `ALTER TABLE t INHERIT <cursor>` / `... NO INHERIT <cursor>`
        // -- the next token is a parent table name from the catalog.
        sources::tables(cat, &mut out);
      } else if let Some(kws) = alter_table_subaction_at(source, offset) {
        // `ALTER TABLE <t> ADD <cursor>` / `... DROP <cursor>` etc --
        // the user has already picked the top-level action; narrow
        // to the sub-keywords (COLUMN / CONSTRAINT / etc) instead
        // of re-listing the entire 18-item action menu.
        push_keyword_kvs(&mut out, kws);
      } else {
        sources::alter_table_actions(&mut out);
      }
    },

    Phase::AfterGrantOrRevoke => {
      // GRANT/REVOKE follow-up chain (WITH / GRANTED BY / ON menus)
      // takes priority over the privilege list when those slot tokens
      // are present.
      if let Some(kws) = grant_revoke_followup_next_keyword(source, offset) {
        push_keyword_kvs(&mut out, kws);
      } else {
        sources::grant_privileges(&mut out);
        // Also surface ON so the chain can continue after the priv list.
        out.push(crate::item::Item {
          label: "ON".into(),
          kind: crate::item::ItemKind::Keyword,
          detail: Some("ON <object_class> <name>".into()),
          insert_text: "ON".into(),
          sort_priority: 0,
          ..Default::default()
        });
        // `REVOKE [GRANT OPTION FOR] <priv>` -- the GRANT OPTION FOR
        // modifier only legal on REVOKE. Surface it at the bare-REVOKE
        // slot so users discover it without typing.
        let (_, upper) = stmt_slice_upper(source, offset);
        if upper.trim_start().starts_with("REVOKE") {
          out.push(crate::item::Item {
            label: "GRANT OPTION FOR".into(),
            kind: crate::item::ItemKind::Keyword,
            detail: Some("REVOKE GRANT OPTION FOR <priv> -- drop forwarding right, keep the priv".into()),
            insert_text: "GRANT OPTION FOR".into(),
            sort_priority: 0,
            ..Default::default()
          });
        }
      }
    },
    Phase::AfterGrantOn => {
      // Object class keywords (TABLE/SEQUENCE/FUNCTION/SCHEMA/...)
      // plus the actual catalog targets so the user can either pick
      // the explicit class keyword or jump straight to a name.
      sources::grant_object_classes(&mut out);
      sources::tables(cat, &mut out);
    },
    Phase::AfterGrantTo => {
      // SET ROLE / SET SESSION AUTHORIZATION sneak past the phase
      // detector because they're parsed similarly; emit the dedicated
      // keyword chain (NONE / DEFAULT / etc) before the role list.
      if let Some(kws) = set_role_auth_next_keyword(source, offset) {
        push_keyword_kvs(&mut out, kws);
      }
      sources::grant_roles(cat, &mut out);
      // Trailing clauses legal at the end of a GRANT / REVOKE -- emit
      // them so users can quickly add `WITH GRANT OPTION` etc.
      for (kw, doc) in [
        ("WITH GRANT OPTION", "GRANT ... WITH GRANT OPTION -- grantee can re-grant"),
        ("WITH ADMIN OPTION", "GRANT <role> ... WITH ADMIN OPTION -- grantee can add other members"),
        ("GRANTED BY", "GRANT ... GRANTED BY <role> -- record the grantor explicitly"),
        ("CASCADE", "REVOKE ... CASCADE -- also revoke dependent grants"),
        ("RESTRICT", "REVOKE ... RESTRICT -- fail if dependent grants exist (default)"),
      ] {
        out.push(crate::item::Item {
          label: kw.into(),
          kind: crate::item::ItemKind::Keyword,
          detail: Some(doc.into()),
          insert_text: kw.into(),
          sort_priority: 1,
          ..Default::default()
        });
      }
    },

    Phase::AfterCreate | Phase::AfterAlter | Phase::AfterDrop | Phase::Unknown => {
      // `DROP TABLE [IF EXISTS]` / `DROP VIEW [IF EXISTS]` /
      // `TRUNCATE [TABLE]` all expect a table-class name. Skip the
      // generic catch-all dump and emit only matching catalog targets.
      if let Some(kws) = copy_paren_options_keyword(source, offset) {
        // COPY ... WITH ( <cursor> ) -- option-name slot beats the generic
        // dml_drop_or_truncate_expects_table branch which would otherwise
        // emit the COPY target table list inside the option paren.
        push_keyword_kvs(&mut out, kws);
      } else if merge::merge_insert_col_list_slot(source, offset) {
        // MERGE ... WHEN NOT MATCHED THEN INSERT (<cursor>) -- column
        // list slot scoped to the MERGE target table. Surface its
        // columns instead of the generic table dump.
        let (tgt, _) = merge::merge_target_and_source(source);
        if let Some(t) = tgt.as_deref() {
          sources::columns_of_table(cat, None, t, &mut out);
        }
      } else if merge::merge_update_set_lhs_slot(source, offset) {
        // MERGE ... WHEN MATCHED THEN UPDATE SET <cursor> / SET c=v,
        // <cursor> -- LHS column slot. Must beat `dml_drop_or_truncate_
        // expects_table` which otherwise matches `MERGE INTO` and
        // dumps every catalog table (e.g. when the slice ends in a
        // trailing comma).
        let (tgt, _) = merge::merge_target_and_source(source);
        if let Some(t) = tgt.as_deref() {
          sources::columns_of_table(cat, None, t, &mut out);
        }
      } else if grouping_sets_inner_paren_expects_column(source, offset) {
        // GROUP BY GROUPING SETS ((<cursor>...)) -- inner tuple is a
        // column list, not an expression context. Phase machine sees
        // the double paren as a function call and would otherwise
        // dump the function library.
        push_scope_columns_or_all(file, scopes, source, cat, offset, &mut out);
        push_aliases(file, scopes, source, offset, &mut out);
      } else if vacuum_paren_expects_option(source, offset) {
        for (kw, doc) in [
          ("FULL", "FULL -- rewrite the table (locks it)"),
          ("FREEZE", "FREEZE -- mark tuples as committed eagerly"),
          ("VERBOSE", "VERBOSE -- per-relation progress"),
          ("ANALYZE", "ANALYZE -- update planner stats too"),
          ("SKIP_LOCKED", "SKIP_LOCKED -- don't wait for locks"),
          ("INDEX_CLEANUP", "INDEX_CLEANUP AUTO|ON|OFF"),
          ("PROCESS_TOAST", "PROCESS_TOAST [true|false]"),
          ("PROCESS_MAIN", "PROCESS_MAIN [true|false] -- PG16+"),
          ("TRUNCATE", "TRUNCATE [true|false] -- shrink the table file"),
          ("DISABLE_PAGE_SKIPPING", "DISABLE_PAGE_SKIPPING [true|false]"),
          ("BUFFER_USAGE_LIMIT", "BUFFER_USAGE_LIMIT '<size>' -- ring-buffer cap"),
          ("PARALLEL", "PARALLEL <int> -- workers for index vacuum (PG13+)"),
          ("SKIP_DATABASE_STATS", "SKIP_DATABASE_STATS [true|false] -- PG16+"),
          ("ONLY_DATABASE_STATS", "ONLY_DATABASE_STATS [true|false] -- PG16+"),
        ] {
          out.push(crate::item::Item {
            label: kw.into(),
            kind: crate::item::ItemKind::Keyword,
            detail: Some(doc.into()),
            description: None,
            documentation_md: None,
            insert_text: kw.into(),
            is_snippet: false,
            sort_priority: 0,
          });
        }
      } else if dml_drop_or_truncate_expects_table(source, offset) {
        // Surface `IF EXISTS` first when the user hasn't typed it yet,
        // so `DROP TABLE |` offers both the modifier and the target list.
        let (_, upper_drop) = stmt_slice_upper(source, offset);
        let words: Vec<&str> = upper_drop.split_ascii_whitespace().collect();
        let starts_drop = matches!(words.first().copied(), Some("DROP"));
        let has_if_exists = words.windows(2).any(|w| w[0] == "IF" && w[1] == "EXISTS");
        if starts_drop && !has_if_exists {
          out.push(crate::item::Item {
            label: "IF EXISTS".into(),
            kind: crate::item::ItemKind::Keyword,
            detail: Some("IF EXISTS -- skip silently when the target does not exist".into()),
            insert_text: "IF EXISTS".into(),
            sort_priority: 0,
            ..Default::default()
          });
        }
        sources::tables(cat, &mut out);
      } else if command_expects_role_name(source, offset) {
        // `ALTER ROLE | / DROP ROLE | / DROP USER | / REASSIGN OWNED
        // BY |` -- next token is an existing role from the catalog.
        sources::grant_roles(cat, &mut out);
      } else if reset_expects_subkeyword(source, offset) {
        // `RESET <cursor>` -> ALL | ROLE | <GUC name>. GUC names are
        // freeform so we only emit the two keyword candidates.
        for (kw, doc) in [
          ("ALL", "RESET ALL -- reset every GUC to its default"),
          ("ROLE", "RESET ROLE -- undo a SET ROLE"),
        ] {
          out.push(crate::item::Item {
            label: kw.into(),
            kind: crate::item::ItemKind::Keyword,
            detail: Some(doc.into()),
            description: None,
            documentation_md: None,
            insert_text: kw.into(),
            is_snippet: false,
            sort_priority: 0,
          });
        }
      } else if raise_expects_level_keyword(source, offset) {
        // `RAISE <cursor>` (PL/pgSQL) -> level keyword.
        for (kw, doc) in [
          ("DEBUG", "RAISE DEBUG '...' -- developer-visible diagnostic"),
          ("LOG", "RAISE LOG '...' -- to server log only"),
          ("INFO", "RAISE INFO '...' -- to client always"),
          ("NOTICE", "RAISE NOTICE '...' -- default level"),
          ("WARNING", "RAISE WARNING '...' -- always to client"),
          ("EXCEPTION", "RAISE EXCEPTION '...' -- abort transaction (default if no level)"),
        ] {
          out.push(crate::item::Item {
            label: kw.into(),
            kind: crate::item::ItemKind::Keyword,
            detail: Some(doc.into()),
            description: None,
            documentation_md: None,
            insert_text: kw.into(),
            is_snippet: false,
            sort_priority: 0,
          });
        }
      } else if discard_expects_subkeyword(source, offset) {
        // `DISCARD <cursor>` -> ALL | PLANS | SEQUENCES | TEMP | TEMPORARY.
        for (kw, doc) in [
          ("ALL", "DISCARD ALL -- session reset"),
          ("PLANS", "DISCARD PLANS -- drop cached plans"),
          ("SEQUENCES", "DISCARD SEQUENCES -- forget session sequence state"),
          ("TEMP", "DISCARD TEMP -- drop temporary tables"),
          ("TEMPORARY", "DISCARD TEMPORARY -- same as DISCARD TEMP"),
        ] {
          out.push(crate::item::Item {
            label: kw.into(),
            kind: crate::item::ItemKind::Keyword,
            detail: Some(doc.into()),
            description: None,
            documentation_md: None,
            insert_text: kw.into(),
            is_snippet: false,
            sort_priority: 0,
          });
        }
      } else if create_index_expects_on(source, offset) {
        // `CREATE INDEX <name> <cursor>` -> ON.
        out.push(crate::item::Item {
          label: "ON".into(),
          kind: crate::item::ItemKind::Keyword,
          detail: Some("ON <table> (<col> [, ...])".into()),
          description: None,
          documentation_md: None,
          insert_text: "ON".into(),
          is_snippet: false,
          sort_priority: 0,
        });
      } else if create_policy_expects_on(source, offset) {
        // `CREATE POLICY <name> <cursor>` -> ON.
        out.push(crate::item::Item {
          label: "ON".into(),
          kind: crate::item::ItemKind::Keyword,
          detail: Some("ON <table> -- attach the policy to a table".into()),
          description: None,
          documentation_md: None,
          insert_text: "ON".into(),
          is_snippet: false,
          sort_priority: 0,
        });
      } else if create_policy_expects_table(source, offset) {
        // `CREATE POLICY <name> ON <cursor>` -> tables.
        sources::tables(cat, &mut out);
      } else if create_trigger_expects_timing(source, offset) {
        // `CREATE [OR REPLACE] TRIGGER <name> <cursor>` -- next token
        // is the timing keyword.
        for (kw, doc) in [
          ("BEFORE", "BEFORE <event> ON <table>"),
          ("AFTER", "AFTER <event> ON <table>"),
          ("INSTEAD OF", "INSTEAD OF <event> ON <view>"),
        ] {
          out.push(crate::item::Item {
            label: kw.into(),
            kind: crate::item::ItemKind::Keyword,
            detail: Some(doc.into()),
            description: None,
            documentation_md: None,
            insert_text: kw.into(),
            is_snippet: false,
            sort_priority: 0,
          });
        }
      } else if create_function_expects_return_type(source, offset) {
        // `CREATE [OR REPLACE] FUNCTION/PROCEDURE ... RETURNS <cursor>`
        // -- return type slot. Types only.
        sources::types_only(&mut out);
      } else if declare_cursor_for_expects_statement(source, offset) {
        // `DECLARE <name> [...] CURSOR FOR <cursor>` -- expects a
        // SELECT statement.
        sources::statement_keywords(&mut out);
      } else if with_cte_after_as_expects_materialized(source, offset) {
        // `WITH cte AS <cursor>` -> MATERIALIZED | NOT MATERIALIZED | (
        for (kw, doc) in [
          ("MATERIALIZED", "AS MATERIALIZED (...) -- always materialize the CTE"),
          ("NOT MATERIALIZED", "AS NOT MATERIALIZED (...) -- inline when possible"),
        ] {
          out.push(crate::item::Item {
            label: kw.into(),
            kind: crate::item::ItemKind::Keyword,
            detail: Some(doc.into()),
            description: None,
            documentation_md: None,
            insert_text: kw.into(),
            is_snippet: false,
            sort_priority: 0,
          });
        }
      } else if do_expects_language_or_body(source, offset) {
        // `DO <cursor>` -> LANGUAGE (then plpgsql) or $$ body (no
        // completion for the dollar-quote opener).
        out.push(crate::item::Item {
          label: "LANGUAGE".into(),
          kind: crate::item::ItemKind::Keyword,
          detail: Some("LANGUAGE plpgsql -- explicit body language (default is plpgsql)".into()),
          description: None,
          documentation_md: None,
          insert_text: "LANGUAGE".into(),
          is_snippet: false,
          sort_priority: 0,
        });
      } else if create_sequence_expects_option(source, offset) {
        // `CREATE SEQUENCE <name> <cursor>` -> sequence options.
        for (kw, doc) in [
          ("AS", "AS <type> -- smallint / integer / bigint"),
          ("INCREMENT", "INCREMENT [BY] <n>"),
          ("MINVALUE", "MINVALUE <n> | NO MINVALUE"),
          ("MAXVALUE", "MAXVALUE <n> | NO MAXVALUE"),
          ("START", "START [WITH] <n>"),
          ("CACHE", "CACHE <n> -- preallocate n values per session"),
          ("CYCLE", "CYCLE -- wrap around at MAXVALUE/MINVALUE"),
          ("NO CYCLE", "NO CYCLE -- error at the limit (default)"),
          ("OWNED BY", "OWNED BY <table>.<column> -- auto-drop with the column"),
        ] {
          out.push(crate::item::Item {
            label: kw.into(),
            kind: crate::item::ItemKind::Keyword,
            detail: Some(doc.into()),
            description: None,
            documentation_md: None,
            insert_text: kw.into(),
            is_snippet: false,
            sort_priority: 0,
          });
        }
      } else if create_type_enum_or_range_body(source, offset) {
        // `CREATE TYPE foo AS ENUM (` / `RANGE (` -- body expects
        // string literals or option=value pairs; nothing useful from
        // the catalog. Stay silent rather than dump the keyword soup.
      } else if create_type_as_expects_kind(source, offset) {
        // `CREATE TYPE <name> AS <cursor>` -> ENUM | RANGE | ( (composite).
        for (kw, doc) in [
          ("ENUM", "CREATE TYPE t AS ENUM ('a', 'b', ...) -- discrete labels"),
          ("RANGE", "CREATE TYPE t AS RANGE (SUBTYPE = ...) -- value-range type"),
        ] {
          out.push(crate::item::Item {
            label: kw.into(),
            kind: crate::item::ItemKind::Keyword,
            detail: Some(doc.into()),
            description: None,
            documentation_md: None,
            insert_text: kw.into(),
            is_snippet: false,
            sort_priority: 0,
          });
        }
      } else if explain_paren_format_value(source, offset) {
        for (kw, doc) in [
          ("TEXT", "TEXT -- default human-readable"),
          ("JSON", "JSON -- machine-parseable"),
          ("XML", "XML"),
          ("YAML", "YAML"),
        ] {
          out.push(crate::item::Item {
            label: kw.into(),
            kind: crate::item::ItemKind::Keyword,
            detail: Some(doc.into()),
            insert_text: kw.into(),
            sort_priority: 0,
            ..Default::default()
          });
        }
      } else if explain_paren_serialize_value(source, offset) {
        for (kw, doc) in [
          ("none", "none -- skip output serialization"),
          ("text", "text -- text-protocol serialization (default)"),
          ("binary", "binary -- binary-protocol serialization"),
        ] {
          out.push(crate::item::Item {
            label: kw.into(),
            kind: crate::item::ItemKind::Keyword,
            detail: Some(doc.into()),
            insert_text: kw.into(),
            sort_priority: 0,
            ..Default::default()
          });
        }
      } else if explain_paren_expects_option(source, offset) {
        // `EXPLAIN ( <cursor>` -- inside the options paren, expects
        // option keywords (FORMAT/ANALYZE/VERBOSE/BUFFERS/COSTS/...).
        for (kw, doc) in [
          ("ANALYZE", "ANALYZE -- actually run and time the query"),
          ("VERBOSE", "VERBOSE -- include extra plan detail"),
          ("COSTS", "COSTS [true|false] -- show estimated start/total cost"),
          ("BUFFERS", "BUFFERS -- include buffer-use stats (requires ANALYZE)"),
          ("WAL", "WAL -- include WAL stats (requires ANALYZE)"),
          ("TIMING", "TIMING [true|false] -- include per-node timing"),
          ("SUMMARY", "SUMMARY [true|false] -- include planning/exec totals"),
          ("SETTINGS", "SETTINGS -- include any non-default GUCs"),
          ("FORMAT", "FORMAT TEXT|XML|JSON|YAML -- output format"),
          ("GENERIC_PLAN", "GENERIC_PLAN [true|false] -- show a parameter-free plan"),
          ("SERIALIZE", "SERIALIZE [text|binary|none] -- include output serialization cost (PG17+, requires ANALYZE)"),
          ("MEMORY", "MEMORY [true|false] -- per-node peak memory used (PG17+)"),
        ] {
          out.push(crate::item::Item {
            label: kw.into(),
            kind: crate::item::ItemKind::Keyword,
            detail: Some(doc.into()),
            description: None,
            documentation_md: None,
            insert_text: kw.into(),
            is_snippet: false,
            sort_priority: 0,
          });
        }
      } else if explain_expects_statement(source, offset) {
        // `EXPLAIN [(...)] [ANALYZE [VERBOSE]] <cursor>` -- the user
        // is starting a statement. Surface the top-level statement
        // keywords (SELECT / INSERT INTO / UPDATE / DELETE FROM /
        // ...) plus the ANALYZE / VERBOSE / `(` modifiers that legally
        // sit between EXPLAIN and the statement.
        sources::statement_keywords(&mut out);
        for (kw, doc) in [
          ("ANALYZE", "EXPLAIN ANALYZE -- actually run, report timing"),
          ("VERBOSE", "EXPLAIN VERBOSE -- include extra detail"),
          ("(", "EXPLAIN (FORMAT JSON, ANALYZE, ...) <stmt>"),
        ] {
          out.push(crate::item::Item {
            label: kw.into(),
            kind: crate::item::ItemKind::Keyword,
            detail: Some(doc.into()),
            insert_text: kw.into(),
            sort_priority: 0,
            ..Default::default()
          });
        }
      } else if let Some(kw_list) = set_statement_completion(source, offset) {
        // `SET <cursor>` -> LOCAL/SESSION scope modifiers; `SET LOCAL
        // <cursor>` / `SET SESSION <cursor>` -> GUC name slot (no
        // catalog-derived completion). Avoid the 638-item dump.
        for (kw, doc) in kw_list {
          out.push(crate::item::Item {
            label: (*kw).into(),
            kind: crate::item::ItemKind::Keyword,
            detail: Some((*doc).into()),
            description: None,
            documentation_md: None,
            insert_text: (*kw).into(),
            is_snippet: false,
            sort_priority: 0,
          });
        }
      } else if let Some(kws) = txn_followup_next_keyword(source, offset) {
        push_keyword_kvs(&mut out, kws);
      } else if let Some(kw_list) = transaction_control_completion(source, offset) {
        // BEGIN / START TRANSACTION / COMMIT / ROLLBACK / END / ABORT
        // / SAVEPOINT -- emit only the keywords that make sense in
        // each slot. SAVEPOINT and the COMMIT-family take fresh
        // identifiers / no completion, so kw_list may be empty.
        for (kw, doc) in kw_list {
          out.push(crate::item::Item {
            label: (*kw).into(),
            kind: crate::item::ItemKind::Keyword,
            detail: Some((*doc).into()),
            description: None,
            documentation_md: None,
            insert_text: (*kw).into(),
            is_snippet: false,
            sort_priority: 0,
          });
        }
      } else if comment_on_expects_class_keyword(source, offset) {
        // `COMMENT ON <cursor>` -- next token is the object class
        // (TABLE / COLUMN / SCHEMA / FUNCTION / ROLE / ...).
        for (kw, doc) in COMMENT_ON_CLASSES {
          out.push(crate::item::Item {
            label: (*kw).into(),
            kind: crate::item::ItemKind::Keyword,
            detail: Some((*doc).into()),
            description: None,
            documentation_md: None,
            insert_text: (*kw).into(),
            is_snippet: false,
            sort_priority: 0,
          });
        }
      } else if let Some(kws) = set_role_auth_next_keyword(source, offset) {
        push_keyword_kvs(&mut out, kws);
      } else if let Some(kws) = show_or_set_guc_names(source, offset) {
        push_keyword_kvs(&mut out, kws);
      } else if let Some(kws) = comment_on_is_value_next_keyword(source, offset) {
        push_keyword_kvs(&mut out, kws);
      } else if let Some(kws) = security_label_next_keyword(source, offset) {
        push_keyword_kvs(&mut out, kws);
      } else if let Some(kws) = vacuum_paren_value_keyword(source, offset) {
        push_keyword_kvs(&mut out, kws);
      } else if let Some(kws) = tablesample_after_paren_next_keyword(source, offset) {
        push_keyword_kvs(&mut out, kws);
      } else if let Some(kws) = window_clause_as_paren_keyword(source, offset) {
        push_keyword_kvs(&mut out, kws);
      } else if let Some(kws) = alter_table_attach_detach_next_keyword(source, offset) {
        push_keyword_kvs(&mut out, kws);
      } else if let Some(kws) = partition_next_keyword(source, offset) {
        push_keyword_kvs(&mut out, kws);
      } else if let Some(kws) = ctas_with_data_next_keyword(source, offset) {
        push_keyword_kvs(&mut out, kws);
      } else if let Some(kws) = insert_into_next_keyword(source, offset) {
        push_keyword_kvs(&mut out, kws);
      } else if let Some(kws) = update_from_set_next_keyword(source, offset) {
        push_keyword_kvs(&mut out, kws);
      } else if let Some(kws) = delete_using_next_keyword(source, offset) {
        push_keyword_kvs(&mut out, kws);
      } else if let Some(kws) = group_by_set_op_next_keyword(source, offset) {
        push_keyword_kvs(&mut out, kws);
      } else if let Some(kws) = select_fetch_offset_next_keyword(source, offset) {
        push_keyword_kvs(&mut out, kws);
      } else if let Some(kws) = insert_overriding_next_keyword(source, offset) {
        push_keyword_kvs(&mut out, kws);
      } else if let Some(kws) = on_conflict_next_keyword(source, offset) {
        push_keyword_kvs(&mut out, kws);
      } else if let Some(kws) = prepare_next_keyword(source, offset) {
        push_keyword_kvs(&mut out, kws);
      } else if let Some(kws) = declare_cursor_next_keyword(source, offset) {
        push_keyword_kvs(&mut out, kws);
      } else if let Some(kws) = truncate_next_keyword(source, offset) {
        push_keyword_kvs(&mut out, kws);
      } else if let Some(kws) = grant_revoke_followup_next_keyword(source, offset) {
        push_keyword_kvs(&mut out, kws);
      } else if let Some(kws) = set_op_followup_next_keyword(source, offset) {
        push_keyword_kvs(&mut out, kws);
      } else if let Some(kws) = with_cte_after_paren_close_next_keyword(source, offset) {
        push_keyword_kvs(&mut out, kws);
      } else if let Some(kws) = create_table_post_body_next_keyword(source, offset) {
        push_keyword_kvs(&mut out, kws);
      } else if let Some(kws) = create_database_next_keyword(source, offset) {
        push_keyword_kvs(&mut out, kws);
      } else if let Some(kws) = alter_database_next_keyword(source, offset) {
        push_keyword_kvs(&mut out, kws);
      } else if let Some(kws) = alter_tablespace_next_keyword(source, offset) {
        push_keyword_kvs(&mut out, kws);
      } else if let Some(kws) = create_user_mapping_next_keyword(source, offset) {
        push_keyword_kvs(&mut out, kws);
      } else if let Some(kws) = alter_user_mapping_next_keyword(source, offset) {
        push_keyword_kvs(&mut out, kws);
      } else if let Some(kws) = create_language_next_keyword(source, offset) {
        push_keyword_kvs(&mut out, kws);
      } else if let Some(kws) = alter_language_next_keyword(source, offset) {
        push_keyword_kvs(&mut out, kws);
      } else if let Some(kws) = alter_server_next_keyword(source, offset) {
        push_keyword_kvs(&mut out, kws);
      } else if let Some(kws) = alter_fdw_next_keyword(source, offset) {
        push_keyword_kvs(&mut out, kws);
      } else if let Some(kws) = drop_user_mapping_next_keyword(source, offset) {
        push_keyword_kvs(&mut out, kws);
      } else if let Some(kws) = exclude_constraint_next_keyword(source, offset) {
        push_keyword_kvs(&mut out, kws);
      } else if let Some(kws) = create_function_attribute_next_keyword(source, offset) {
        push_keyword_kvs(&mut out, kws);
      } else if let Some(kws) = set_role_auth_next_keyword(source, offset) {
        push_keyword_kvs(&mut out, kws);
      } else if let Some(kws) = fetch_move_direction_keyword(source, offset) {
        push_keyword_kvs(&mut out, kws);
      } else if let Some(kws) = lock_mode_next_keyword(source, offset) {
        push_keyword_kvs(&mut out, kws);
      } else if let Some(kws) = deallocate_next_keyword(source, offset) {
        push_keyword_kvs(&mut out, kws);
      } else if let Some(kws) = alter_default_privileges_next_keyword(source, offset) {
        push_keyword_kvs(&mut out, kws);
      } else if let Some(kws) = alter_extension_next_keyword(source, offset) {
        push_keyword_kvs(&mut out, kws);
      } else if let Some(kws) = create_transform_next_keyword(source, offset) {
        push_keyword_kvs(&mut out, kws);
      } else if let Some(kws) = alter_large_object_next_keyword(source, offset) {
        push_keyword_kvs(&mut out, kws);
      } else if let Some(kws) = alter_rule_next_keyword(source, offset) {
        push_keyword_kvs(&mut out, kws);
      } else if let Some(kws) = alter_trigger_next_keyword(source, offset) {
        push_keyword_kvs(&mut out, kws);
      } else if let Some(kws) = alter_access_method_next_keyword(source, offset) {
        push_keyword_kvs(&mut out, kws);
      } else if let Some(kws) = create_conversion_next_keyword(source, offset) {
        push_keyword_kvs(&mut out, kws);
      } else if let Some(kws) = alter_conversion_next_keyword(source, offset) {
        push_keyword_kvs(&mut out, kws);
      } else if let Some(kws) = create_operator_family_next_keyword(source, offset) {
        push_keyword_kvs(&mut out, kws);
      } else if let Some(kws) = create_operator_class_next_keyword(source, offset) {
        push_keyword_kvs(&mut out, kws);
      } else if let Some(kws) = alter_event_trigger_next_keyword(source, offset) {
        push_keyword_kvs(&mut out, kws);
      } else if let Some(kws) = alter_operator_class_family_next_keyword(source, offset) {
        push_keyword_kvs(&mut out, kws);
      } else if let Some(kws) = alter_text_search_next_keyword(source, offset) {
        push_keyword_kvs(&mut out, kws);
      } else if let Some(kws) = alter_statistics_next_keyword(source, offset) {
        push_keyword_kvs(&mut out, kws);
      } else if let Some(kws) = alter_index_next_keyword(source, offset) {
        push_keyword_kvs(&mut out, kws);
      } else if let Some(kws) = alter_sequence_next_keyword(source, offset) {
        push_keyword_kvs(&mut out, kws);
      } else if let Some(kws) = alter_policy_next_keyword(source, offset) {
        push_keyword_kvs(&mut out, kws);
      } else if let Some(kws) = alter_domain_next_keyword(source, offset) {
        push_keyword_kvs(&mut out, kws);
      } else if let Some(kws) = alter_collation_next_keyword(source, offset) {
        push_keyword_kvs(&mut out, kws);
      } else if let Some(kws) = create_access_method_next_keyword(source, offset) {
        push_keyword_kvs(&mut out, kws);
      } else if let Some(kws) = alter_operator_next_keyword(source, offset) {
        push_keyword_kvs(&mut out, kws);
      } else if let Some(kws) = alter_aggregate_next_keyword(source, offset) {
        push_keyword_kvs(&mut out, kws);
      } else if let Some(kws) = alter_publication_next_keyword(source, offset) {
        push_keyword_kvs(&mut out, kws);
      } else if let Some(kws) = alter_subscription_next_keyword(source, offset) {
        push_keyword_kvs(&mut out, kws);
      } else if let Some(kws) = alter_schema_next_keyword(source, offset) {
        push_keyword_kvs(&mut out, kws);
      } else if let Some(kws) = create_text_search_next_keyword(source, offset) {
        push_keyword_kvs(&mut out, kws);
      } else if let Some(kws) = create_extension_next_keyword(source, offset) {
        push_keyword_kvs(&mut out, kws);
      } else if let Some(kws) = alter_function_next_keyword(source, offset) {
        push_keyword_kvs(&mut out, kws);
      } else if let Some(kws) = alter_view_next_keyword(source, offset) {
        push_keyword_kvs(&mut out, kws);
      } else if let Some(kws) = create_tablespace_next_keyword(source, offset) {
        push_keyword_kvs(&mut out, kws);
      } else if let Some(kws) = create_aggregate_next_keyword(source, offset) {
        push_keyword_kvs(&mut out, kws);
      } else if let Some(kws) = create_cast_next_keyword(source, offset) {
        push_keyword_kvs(&mut out, kws);
      } else if let Some(kws) = create_rule_next_keyword(source, offset) {
        push_keyword_kvs(&mut out, kws);
      } else if let Some(kws) = create_statistics_next_keyword(source, offset) {
        push_keyword_kvs(&mut out, kws);
      } else if let Some(kws) = create_type_next_keyword(source, offset) {
        push_keyword_kvs(&mut out, kws);
      } else if let Some(kws) = create_event_trigger_next_keyword(source, offset) {
        push_keyword_kvs(&mut out, kws);
      } else if let Some(kws) = create_server_next_keyword(source, offset) {
        push_keyword_kvs(&mut out, kws);
      } else if let Some(kws) = create_operator_next_keyword(source, offset) {
        push_keyword_kvs(&mut out, kws);
      } else if let Some(kws) = create_domain_next_keyword(source, offset) {
        push_keyword_kvs(&mut out, kws);
      } else if let Some(kws) = create_collation_next_keyword(source, offset) {
        push_keyword_kvs(&mut out, kws);
      } else if let Some(kws) = alter_system_next_keyword(source, offset) {
        push_keyword_kvs(&mut out, kws);
      } else if let Some(kws) = cte_search_cycle_next_keyword(source, offset) {
        push_keyword_kvs(&mut out, kws);
      } else if let Some(kws) = column_constraint_next_keyword(source, offset) {
        push_keyword_kvs(&mut out, kws);
      } else if let Some(kws) = column_generated_next_keyword(source, offset) {
        push_keyword_kvs(&mut out, kws);
      } else if let Some(kws) = create_index_trailing_next_keyword(source, offset) {
        push_keyword_kvs(&mut out, kws);
      } else if let Some(kws) = check_constraint_no_inherit_next_keyword(source, offset) {
        push_keyword_kvs(&mut out, kws);
      } else if let Some(kws) = refresh_mv_next_keyword(source, offset) {
        push_keyword_kvs(&mut out, kws);
      } else if let Some(kws) = identity_paren_next_keyword(source, offset) {
        push_keyword_kvs(&mut out, kws);
      } else if let Some(kws) = replica_identity_next_keyword(source, offset) {
        push_keyword_kvs(&mut out, kws);
      } else if let Some(kws) = create_foreign_table_next_keyword(source, offset) {
        push_keyword_kvs(&mut out, kws);
      } else if let Some(kws) = create_subscription_next_keyword(source, offset) {
        push_keyword_kvs(&mut out, kws);
      } else if let Some(kws) = create_view_post_name_next_keyword(source, offset) {
        push_keyword_kvs(&mut out, kws);
      } else if let Some(kws) = alter_materialized_view_next_keyword(source, offset) {
        push_keyword_kvs(&mut out, kws);
      } else if let Some(kws) = create_publication_next_keyword(source, offset) {
        push_keyword_kvs(&mut out, kws);
      } else if let Some(kws) = cluster_next_keyword(source, offset) {
        push_keyword_kvs(&mut out, kws);
      } else if let Some(kws) = import_foreign_schema_next_keyword(source, offset) {
        push_keyword_kvs(&mut out, kws);
      } else if let Some(kws) = create_schema_next_keyword(source, offset) {
        push_keyword_kvs(&mut out, kws);
      } else if let Some(kws) = reindex_next_keyword(source, offset) {
        push_keyword_kvs(&mut out, kws);
      } else if let Some(kws) = set_transaction_next_keyword(source, offset) {
        push_keyword_kvs(&mut out, kws);
      } else if call_expects_procedure(source, offset) {
        // `CALL <cursor>` -- emit catalog procedures + buffer-derived
        // procedures + functions (PG treats them interchangeably enough
        // that surfacing both is friendlier than a strict split).
        for f in &cat.functions {
          out.push(crate::item::Item {
            label: f.name.clone(),
            kind: crate::item::ItemKind::Function,
            detail: Some("procedure / function".into()),
            insert_text: format!("{}(", f.name),
            sort_priority: 0,
            ..Default::default()
          });
        }
      } else if let Some(kws) = reassign_owned_next_keyword(source, offset) {
        push_keyword_kvs(&mut out, kws);
      } else if let Some(kws) = release_savepoint_next_keyword(source, offset) {
        push_keyword_kvs(&mut out, kws);
      } else if let Some(kws) = txn_followup_next_keyword(source, offset) {
        push_keyword_kvs(&mut out, kws);
      } else if let Some(kws) = copy_paren_options_keyword(source, offset) {
        push_keyword_kvs(&mut out, kws);
      } else if let Some(kws) = copy_next_keyword(source, offset) {
        push_keyword_kvs(&mut out, kws);
      } else if let Some(kws) = do_block_next_keyword(source, offset) {
        push_keyword_kvs(&mut out, kws);
      } else if let Some(kws) = role_next_keyword(source, offset) {
        push_keyword_kvs(&mut out, kws);
      } else if let Some(_tbl) = create_trigger_when_table(source, offset) {
        // Inside `CREATE TRIGGER ... ON <tbl> ... WHEN ( <cursor> )`.
        // Per user feedback: only the two row aliases `NEW` and `OLD`
        // belong here. Once the user types `NEW.` / `OLD.` the
        // dot-alias handler at the top of `complete()` resolves to
        // the trigger's target table and emits its columns. So this
        // slot's menu is intentionally just the two virtual rows
        // plus a small comparison-operator hint set.
        for (kw, doc) in [
          ("NEW", "NEW row alias (INSERT / UPDATE triggers) -- type `NEW.<col>` to access columns"),
          ("OLD", "OLD row alias (UPDATE / DELETE triggers) -- type `OLD.<col>` to access columns"),
        ] {
          out.push(crate::item::Item {
            label: kw.into(),
            kind: crate::item::ItemKind::Keyword,
            detail: Some(doc.into()),
            insert_text: kw.into(),
            sort_priority: 0,
            ..Default::default()
          });
        }
        for (kw, doc) in [
          ("IS DISTINCT FROM", "row-level distinctness comparison (NULL-safe)"),
          ("IS NOT DISTINCT FROM", "row-level equality comparison (NULL-safe)"),
          ("AND", "boolean AND"),
          ("OR", "boolean OR"),
          ("NOT", "boolean NOT"),
          ("IS NULL", "null-ness predicate"),
          ("IS NOT NULL", "null-ness predicate (negated)"),
        ] {
          out.push(crate::item::Item {
            label: kw.into(),
            kind: crate::item::ItemKind::Keyword,
            detail: Some(doc.into()),
            insert_text: kw.into(),
            sort_priority: 1,
            ..Default::default()
          });
        }
      } else if let Some(kws) = create_trigger_next_keyword(source, offset) {
        push_keyword_kvs(&mut out, kws);
      } else if let Some(kws) = create_index_next_keyword(source, offset) {
        push_keyword_kvs(&mut out, kws);
      } else if let Some(kws) = alter_type_next_keyword(source, offset) {
        push_keyword_kvs(&mut out, kws);
      } else if merge::merge_update_set_lhs_slot(source, offset) {
        // `MERGE ... THEN UPDATE SET <cursor>` / `... SET col=v, <cursor>`
        // -- LHS slot wants target table columns only.
        let (tgt, _) = merge::merge_target_and_source(source);
        if let Some(t) = tgt.as_deref() {
          sources::columns_of_table(cat, None, t, &mut out);
        }
      } else if merge::merge_when_matched_and_predicate_slot(source, offset)
        || merge::merge_update_set_rhs_expr_slot(source, offset)
      {
        // `MERGE ... WHEN [NOT] MATCHED AND <cursor>` -- expression
        // slot. Same shape for `... UPDATE SET <col> = <cursor>` RHS.
        // Surface columns from both the MERGE target and the USING
        // source, plus aliases, functions, expression kws.
        let (tgt, src_tbl) = merge::merge_target_and_source(source);
        if let Some(t) = tgt.as_deref() {
          sources::columns_of_table(cat, None, t, &mut out);
        }
        if let Some(s) = src_tbl.as_deref() {
          sources::columns_of_table(cat, None, s, &mut out);
        }
        for alias in merge::merge_aliases(source) {
          out.push(crate::item::Item {
            label: alias.clone(),
            kind: crate::item::ItemKind::Table,
            detail: Some("MERGE alias".into()),
            insert_text: alias,
            sort_priority: 0,
            ..Default::default()
          });
        }
        push_all_functions(cat, &mut out);
        sources::expression_keywords(&mut out);
      } else if let Some(kws) = merge::merge_next_keyword(source, offset) {
        push_keyword_kvs(&mut out, kws);
      } else if let Some(kws) = after_top_level_create_keyword(source, offset) {
        // `CREATE <cursor>` -- narrow to the object-type keywords PG
        // accepts after CREATE.
        push_keyword_kvs(&mut out, kws);
      } else if let Some(kws) = create_class_expects_if_not_exists(source, offset) {
        // `CREATE TABLE <cursor>` / `CREATE INDEX <cursor>` etc -- the
        // next legal optional tokens are IF NOT EXISTS / ONLY / etc.
        push_keyword_kvs(&mut out, kws);
      } else if let Some(kws) = after_top_level_alter_keyword(source, offset) {
        push_keyword_kvs(&mut out, kws);
      } else if let Some(kws) = after_top_level_drop_keyword(source, offset) {
        // `DROP <cursor>` -- narrow to the object-type keywords PG
        // accepts after DROP. The catch-all fallback below would dump
        // 642 keywords/tables/columns which is useless here.
        push_keyword_kvs(&mut out, kws);
      } else if drop_target_trailing_slot(source, offset) {
        // `DROP TABLE users <cursor>` -- the user finished the target
        // name; the next legal tokens are CASCADE / RESTRICT / `;`.
        // Without this guard the catch-all dumps 641 unrelated items.
        for (kw, doc) in [
          ("CASCADE", "DROP ... CASCADE -- drop dependent objects too"),
          ("RESTRICT", "DROP ... RESTRICT -- refuse if dependents exist (default)"),
        ] {
          out.push(crate::item::Item {
            label: kw.into(),
            kind: crate::item::ItemKind::Keyword,
            detail: Some(doc.into()),
            insert_text: kw.into(),
            sort_priority: 0,
            ..Default::default()
          });
        }
      } else {
        // Broad fallback: keywords + tables + columns + types + funcs.
        sources::keywords(&mut out);
        sources::types(&mut out);
        sources::functions(&mut out);
        sources::tables(cat, &mut out);
        sources::columns(cat, &mut out);
      }
    },
  }
  // Filter columns the user already typed in the same comma-list
  // clause. Applies to projection / SET / GROUP BY / ORDER BY /
  // INSERT (cols) / CREATE INDEX ON t (cols) / CONSTRAINT (cols) /
  // RETURNING. Keeps the menu honest -- typing `SELECT id, ` won't
  // re-offer `id`.
  if matches!(
    ph,
    Phase::SelectProjection
      | Phase::InProjection
      | Phase::NextProjection
      | Phase::GroupByList
      | Phase::OrderByList
      | Phase::InsertColumnList
      | Phase::UpdateAssignment
  ) {
    let used = used_columns_in_clause(source, offset);
    if !used.is_empty() {
      out.retain(|it| !is_column_listed(it, &used));
    }
  }
  dedup_items(out)
}

