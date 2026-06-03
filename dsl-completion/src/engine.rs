//! Completion engine.
//!
//! Two-phase routing:
//!   1. Look for an immediate "dot context" (`<alias>.<cursor>`); when
//!      present, emit only the columns of that alias.
//!   2. Otherwise classify the cursor by walking the current statement's
//!      tokens through a state machine ([`phase::detect`]) and emit the
//!      set of items appropriate to the resulting [`phase::Phase`].
//!
//! This is what makes the menu context-aware: after `SELECT *` we
//! surface `FROM`, not more SELECT keywords; after a table we surface
//! `JOIN` / `WHERE` / `GROUP BY` / etc; inside ORDER BY we surface
//! columns + ASC/DESC; after WHERE we surface columns + expression
//! keywords; after a semicolon we surface top-level statement starters
//! only.

use crate::create_index;
use crate::create_table;
use crate::fallback;
use crate::item::Item;
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
fn stmt_slice_upper(source: &str, offset: TextSize) -> (String, String) {
  let pos: usize = (u32::from(offset) as usize).min(source.len());
  let stmt_start = source[..pos].rfind(';').map(|p| p + 1).unwrap_or(0);
  let slice = source[stmt_start..pos].to_string();
  let upper = slice.to_ascii_uppercase();
  (slice, upper)
}

/// Return true when `offset` does NOT sit at a whitespace (or EOF)
/// boundary. Phase detectors short-circuit on this so they don't yank
/// the menu open while the user is typing a token. Matches the legacy
/// guard `pos < bytes.len() && !bytes[pos].is_ascii_whitespace()`.
fn cursor_not_at_ws_boundary(source: &str, offset: TextSize) -> bool {
  let pos: usize = (u32::from(offset) as usize).min(source.len());
  let bytes = source.as_bytes();
  pos < bytes.len() && !bytes[pos].is_ascii_whitespace()
}

/// Append every `(label, detail)` pair as a fresh `Keyword` item with
/// default sort priority. Used by every per-statement phase emitter
/// (CREATE TABLE / TRIGGER / TYPE / ALTER ROLE / ...).
fn push_keyword_kvs(out: &mut Vec<Item>, kws: &[(&'static str, &'static str)]) {
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

pub fn complete(source: &str, file: &ParsedFile, scopes: &[Scope], catalog: &Catalog, offset: TextSize) -> Vec<Item> {
  // Normalise offset to the nearest valid UTF-8 char boundary so
  // downstream slicing can't panic on multi-byte characters.
  let raw_off: usize = offset.into();
  let off = floor_char_boundary(source, raw_off.min(source.len()));
  let offset = TextSize::from(off as u32);

  // Hard-suppress completion when the cursor sits at the "fresh
  // name" slot after a `CREATE [OR REPLACE] <KIND>` keyword. The
  // user is naming a brand-new object; no existing catalog symbol or
  // keyword is a sensible suggestion there. Exception: when nothing
  // has been typed yet AND the class supports `IF NOT EXISTS`, emit
  // that single optional clarifier so the user can pick it before
  // typing the name.
  if at_fresh_name_slot(source, offset) {
    if let Some(label) = fresh_name_slot_optional_keyword(source, offset) {
      return vec![crate::item::Item {
        label: label.into(),
        kind: crate::item::ItemKind::Keyword,
        detail: Some("optional clarifier before the new object name".into()),
        insert_text: label.into(),
        sort_priority: 0,
        ..Default::default()
      }];
    }
    // `PREPARE TRANSACTION` overrides the fresh-name-slot suppression
    // because TRANSACTION is a literal kw, not a fresh statement name.
    if let Some(kws) = txn_followup_next_keyword(source, offset) {
      let mut out = Vec::with_capacity(kws.len());
      push_keyword_kvs(&mut out, kws);
      return out;
    }
    // FETCH / MOVE -- direction keyword set is more useful than the
    // fresh-name suppression (cursor name comes after FROM/IN).
    if let Some(kws) = fetch_move_direction_keyword(source, offset) {
      let mut out = Vec::with_capacity(kws.len());
      push_keyword_kvs(&mut out, kws);
      return out;
    }
    // CREATE TRANSFORM -- post-keyword slot is FOR TYPE, not a name.
    if let Some(kws) = create_transform_next_keyword(source, offset) {
      let mut out = Vec::with_capacity(kws.len());
      push_keyword_kvs(&mut out, kws);
      return out;
    }
    // CREATE/ALTER USER MAPPING -- post-keyword slot is FOR/IF NOT EXISTS,
    // not a brand-new identifier.
    if let Some(kws) = create_user_mapping_next_keyword(source, offset) {
      let mut out = Vec::with_capacity(kws.len());
      push_keyword_kvs(&mut out, kws);
      return out;
    }
    if let Some(kws) = alter_user_mapping_next_keyword(source, offset) {
      let mut out = Vec::with_capacity(kws.len());
      push_keyword_kvs(&mut out, kws);
      return out;
    }
    // `SELECT ... FETCH` / `... FIRST` / `... ROW(S)`: not a cursor
    // command, it's the SELECT trailing FETCH clause. Fresh-name guard
    // misfires because `FETCH` is in the cursor pattern list.
    if let Some(kws) = select_fetch_offset_next_keyword(source, offset) {
      let mut out = Vec::with_capacity(kws.len());
      push_keyword_kvs(&mut out, kws);
      return out;
    }
    // `CREATE TABLE child PARTITION OF parent FOR VALUES ` -- the
    // trailing VALUES is part of the partition spec, not a top-level
    // VALUES (...) statement, so the partition menu (IN/FROM/WITH/
    // DEFAULT) wins over the fresh-name suppression.
    if let Some(kws) = partition_next_keyword(source, offset) {
      let mut out = Vec::with_capacity(kws.len());
      push_keyword_kvs(&mut out, kws);
      return out;
    }
    return Vec::new();
  }

  // JSON-path key slot: `data->'<cursor>` or `data->>'<cursor>`.
  // Surface keys observed in same-buffer jsonb literal defaults / CHECK
  // constraints. Highest priority -- we don't want to drown the menu
  // in catalog table names when the user is clearly typing a JSON key.
  // (Runs BEFORE the inert-span bailout so JSON key completion still
  // works while the cursor sits inside the `'...'` literal.)
  if let Some(keys) = json_path_keys_at(source, offset) {
    let mut out = Vec::with_capacity(keys.len());
    for k in keys {
      out.push(crate::item::Item {
        label: k.clone(),
        kind: crate::item::ItemKind::Variable,
        detail: Some("JSON key".into()),
        description: Some("observed in this buffer".into()),
        documentation_md: None,
        insert_text: k,
        is_snippet: false,
        sort_priority: 0,
      });
    }
    return out;
  }

  // Cursor inside a string literal or comment? Suggesting keywords /
  // tables / columns there is just noise -- the user is typing string
  // content. Dollar-quoted bodies (PL/pgSQL) are NOT inert -- recurse
  // into them so completion still works inside function bodies.
  if cursor_in_inert_span(source, u32::from(offset) as usize) {
    return Vec::new();
  }

  // Merge live catalog with in-file CREATE TABLE definitions.
  // Offline-mode enrichment: tables from AST + sequences / types /
  // extensions / functions / roles harvested from buffer text + the
  // default offline roles. Live catalog wins on collisions.
  let derived = source_tables::from_source(file, source);
  let cat = source_tables::merge(catalog, &derived);

  // Dot context first: highest priority, beats any phase result.
  if let Some(alias) = dot_alias(source, offset) {
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
      let target = trigger_target_table(source).or_else(|| enclosing_fn_trigger_table(source, pos, &cat));
      if let Some(t) = target {
        sources::columns_of_table(&cat, None, &t, &mut out);
        return out;
      }
      // No table known -- emit nothing so the user doesn't get a
      // misleading global column dump.
      return out;
    }
    // `EXCLUDED.<col>` (inside INSERT ... ON CONFLICT DO UPDATE SET ...):
    // virtual row that mirrors the rejected INSERT row, so its column
    // shape matches the INSERT target table.
    if alias_upper == "EXCLUDED" {
      if let Some(t) = insert_target_table_name_only(source) {
        sources::columns_of_table(&cat, None, &t, &mut out);
      }
      return out;
    }
    let stmt_scope = scope_for_offset(file, scopes, offset);
    let count = stmt_scope.map(|s| sources::columns_of_alias(&cat, s, &alias, &mut out)).unwrap_or(0);
    if count == 0
      && let Some(fb) = fallback::scope_from_text(source)
    {
      sources::columns_of_alias(&cat, &fb, &alias, &mut out);
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
    // is plainly declared. Text-scan the buffer for the WITH prefix
    // and surface that CTE's projected columns.
    if out.is_empty()
      && let Some(cols) = fallback::cte_columns_from_text(source, &alias)
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
          sources::columns_of_table(&cat, None, bare, &mut out);
        }
      }
    }
    // Schema-qualified relation slot: `FROM <schema>.<TAB>` /
    // `SELECT * FROM <schema>.|`. The alias here is the schema name,
    // not an in-scope alias; surface the tables/views that schema
    // exposes so the user can pick one. Emit nothing when the name
    // is neither a schema nor an alias -- a global dump would be wrong.
    if out.is_empty() {
      sources::tables_in_schema(&cat, &alias, &mut out);
      // Also surface functions declared in this schema: `app.<TAB>`
      // should offer `app.current_user_id()`, `app.user_in_org(...)`,
      // etc., not just tables.
      sources::functions_in_schema(&cat, &alias, &mut out);
    }
    // Last-resort: the alias names a real table in the live or derived
    // catalog (case-insensitive), even though it has no binding in the
    // current scope. Common when the user types `SELECT USERS.<cursor>`
    // before the FROM clause exists. pg_query rejects the prefix and
    // the fallback scope is empty, but the table is still resolvable.
    if out.is_empty() && cat.find_table(None, &alias).is_some() {
      sources::columns_of_table(&cat, None, &alias, &mut out);
    }
    // Filter columns already used in the same clause -- even in dot
    // context, typing `SELECT u.id, u.|` should not re-offer `id`.
    let used = used_columns_in_clause(source, offset);
    if !used.is_empty() {
      out.retain(|it| !is_column_listed(it, &used));
    }
    return out;
  }

  // GROUP BY GROUPING SETS ((<cursor>...)) -- inner tuple is a column
  // list slot. Must beat contexts::detect (which sees the inner paren
  // as a function-call expression context) and every Phase variant.
  if grouping_sets_inner_paren_expects_column(source, offset) {
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
      && let Some(fb) = fallback::scope_from_text(source)
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
      sources::columns_of_table(&cat, schema.as_deref(), &name, &mut out);
    }
    push_aliases(file, scopes, source, offset, &mut out);
    return out;
  }
  // Special context completions (INDEX USING method, TRIGGER EXECUTE
  // FUNCTION, CALL procedure, CREATE POLICY FOR/TO, ALTER COLUMN TYPE,
  // index opclass slot, trigger event slot, trigger ON table). All
  // run *before* the index/table phases because they're more specific
  // than the column dump those phases would emit.
  if let Some(items) = crate::contexts::detect(source, offset, &cat) {
    return items;
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

fn route_phase(
  ph: Phase,
  file: &ParsedFile,
  scopes: &[Scope],
  source: &str,
  cat: &Catalog,
  offset: TextSize,
) -> Vec<Item> {
  let mut out = Vec::new();
  // Slot-keyword shortcuts that beat every Phase variant. UNION/INTERSECT/
  // EXCEPT trailing slot expects ALL/DISTINCT/SELECT/VALUES, never an
  // expression-list dump; WINDOW <name> AS ( ... ROWS BETWEEN <cursor> )
  // wants the frame-bound kw menu, not catalog columns.
  if let Some(kws) = set_op_followup_next_keyword(source, offset) {
    push_keyword_kvs(&mut out, kws);
    return out;
  }
  // GROUP BY GROUPING SETS ((<cursor>...)) -- inner tuple is a column-
  // list slot. Phase machine sees the double paren as expression /
  // function-call context and otherwise dumps the function library.
  // This must beat every phase variant.
  if grouping_sets_inner_paren_expects_column(source, offset) {
    push_scope_columns_or_all(file, scopes, source, cat, offset, &mut out);
    push_aliases(file, scopes, source, offset, &mut out);
    return out;
  }
  // CREATE TRANSFORM ... otherwise gets swallowed by the Phase::Start
  // statement-keyword dump.
  if let Some(kws) = create_transform_next_keyword(source, offset) {
    push_keyword_kvs(&mut out, kws);
    return out;
  }
  // TABLESAMPLE REPEATABLE / SELECT FETCH chain must beat the SELECT
  // trailing-clause menu (which would emit JOIN/WHERE/ORDER BY).
  if let Some(kws) = tablesample_after_paren_next_keyword(source, offset) {
    push_keyword_kvs(&mut out, kws);
    return out;
  }
  if let Some(kws) = select_fetch_offset_next_keyword(source, offset) {
    let (_, upper) = stmt_slice_upper(source, offset);
    let words: Vec<&str> = upper.split_ascii_whitespace().collect();
    let last = words.last().copied();
    if matches!(last, Some("FETCH") | Some("FIRST") | Some("NEXT") | Some("ROW") | Some("ROWS")) {
      push_keyword_kvs(&mut out, kws);
      return out;
    }
  }
  // CREATE VIEW / MATERIALIZED VIEW trailing WITH clauses (CHECK OPTION,
  // WITH DATA) must beat the SELECT-body phase that otherwise emits
  // join/order kws after `WITH `.
  if let Some(kws) = create_view_post_name_next_keyword(source, offset) {
    let (_, upper) = stmt_slice_upper(source, offset);
    let words: Vec<&str> = upper.split_ascii_whitespace().collect();
    let last = words.last().copied();
    if matches!(last, Some("WITH") | Some("NO") | Some("CASCADED") | Some("LOCAL")) {
      push_keyword_kvs(&mut out, kws);
      return out;
    }
  }
  // Only short-circuit on the frame-bound sub-chain. The fresh-slot
  // and PARTITION/ORDER BY column-list paths must keep flowing to the
  // catalog so existing tests stay green.
  if !window_clause_partition_or_order_by_expects_column(source, offset)
    && let Some(kws) = window_clause_as_paren_keyword(source, offset)
  {
    // Use a token sniff: only short-circuit when last word is a frame-
    // family kw (RANGE/ROWS/GROUPS/BETWEEN/AND/PRECEDING/FOLLOWING).
    let pos: usize = (u32::from(offset) as usize).min(source.len());
    let pre = &source[..pos];
    let stmt_start_idx = pre.rfind(';').map(|i| i + 1).unwrap_or(0);
    let words: Vec<&str> = pre[stmt_start_idx..].split_ascii_whitespace().collect();
    let last_up = words.last().map(|s| s.to_ascii_uppercase()).unwrap_or_default();
    if matches!(last_up.as_str(), "RANGE" | "ROWS" | "GROUPS" | "BETWEEN" | "AND" | "PRECEDING" | "FOLLOWING" | "UNBOUNDED" | "CURRENT" | "EXCLUDE") {
      push_keyword_kvs(&mut out, kws);
      return out;
    }
  }
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
      } else if merge_insert_col_list_slot(source, offset) {
        // MERGE ... WHEN NOT MATCHED THEN INSERT (<cursor>) -- column
        // list slot scoped to the MERGE target table. Surface its
        // columns instead of the generic table dump.
        let (tgt, _) = merge_target_and_source(source);
        if let Some(t) = tgt.as_deref() {
          sources::columns_of_table(cat, None, t, &mut out);
        }
      } else if merge_update_set_lhs_slot(source, offset) {
        // MERGE ... WHEN MATCHED THEN UPDATE SET <cursor> / SET c=v,
        // <cursor> -- LHS column slot. Must beat `dml_drop_or_truncate_
        // expects_table` which otherwise matches `MERGE INTO` and
        // dumps every catalog table (e.g. when the slice ends in a
        // trailing comma).
        let (tgt, _) = merge_target_and_source(source);
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
      } else if merge_update_set_lhs_slot(source, offset) {
        // `MERGE ... THEN UPDATE SET <cursor>` / `... SET col=v, <cursor>`
        // -- LHS slot wants target table columns only.
        let (tgt, _) = merge_target_and_source(source);
        if let Some(t) = tgt.as_deref() {
          sources::columns_of_table(cat, None, t, &mut out);
        }
      } else if merge_when_matched_and_predicate_slot(source, offset)
        || merge_update_set_rhs_expr_slot(source, offset)
      {
        // `MERGE ... WHEN [NOT] MATCHED AND <cursor>` -- expression
        // slot. Same shape for `... UPDATE SET <col> = <cursor>` RHS.
        // Surface columns from both the MERGE target and the USING
        // source, plus aliases, functions, expression kws.
        let (tgt, src_tbl) = merge_target_and_source(source);
        if let Some(t) = tgt.as_deref() {
          sources::columns_of_table(cat, None, t, &mut out);
        }
        if let Some(s) = src_tbl.as_deref() {
          sources::columns_of_table(cat, None, s, &mut out);
        }
        for alias in merge_aliases(source) {
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
      } else if let Some(kws) = merge_next_keyword(source, offset) {
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

/// True for completion items that should be filtered when the column
/// is already in the current clause's comma list. Matches both bare
/// columns (`id`) and qualified ones (`u.id`) by checking the tail.
/// True when the cursor sits at a column-LHS slot of an `ON CONFLICT`
/// clause: either the conflict-target list `ON CONFLICT (<cursor>)` or
/// the `DO UPDATE SET <cursor>` assignment target. Both expect a
/// column of the INSERT target table -- not a free expression.
/// True when the cursor in an `UPDATE ... SET ...` clause sits at a
/// column-LHS slot (right after SET or after a top-level comma, with
/// no `=` since). Returns false for value-expression positions so the
/// caller can keep emitting functions/keywords there.
fn update_set_at_column_slot(source: &str, offset: TextSize) -> bool {
  let pos: usize = (u32::from(offset) as usize).min(source.len());
  let bytes = source.as_bytes();
  // Walk back to the SET keyword that anchors this assignment list.
  let mut anchor: Option<usize> = None;
  let mut depth = 0i32;
  let mut i = pos;
  while i > 0 {
    let b = bytes[i - 1];
    if b >= 128 {
      i -= 1;
      continue;
    }
    let c = b as char;
    if c == ')' {
      depth += 1;
      i -= 1;
      continue;
    }
    if c == '(' {
      if depth == 0 {
        return false;
      }
      depth -= 1;
      i -= 1;
      continue;
    }
    if c == ';' {
      return false;
    }
    if match_kw_at(bytes, i, b"SET") {
      anchor = Some(i);
      break;
    }
    i -= 1;
  }
  let Some(anchor) = anchor else {
    return false;
  };
  // Scan forward `[anchor..pos)` and locate the latest top-level
  // `=` and `,`. The column slot rule: no `=` since the most recent
  // `,` (or since SET, when no comma yet).
  if !(source.is_char_boundary(anchor) && source.is_char_boundary(pos)) {
    return false;
  }
  let region = &source[anchor..pos];
  let rbytes = region.as_bytes();
  let n = rbytes.len();
  let mut depth = 0i32;
  let mut last_eq: Option<usize> = None;
  let mut last_comma: Option<usize> = None;
  let mut i = 0usize;
  while i < n {
    let c = rbytes[i] as char;
    if c == '\'' {
      i += 1;
      while i < n && rbytes[i] != b'\'' {
        i += 1;
      }
      i = (i + 1).min(n);
      continue;
    }
    if c == '(' {
      depth += 1;
    } else if c == ')' {
      depth -= 1;
    } else if depth == 0 {
      if c == ',' {
        last_comma = Some(i);
      } else if c == '=' {
        last_eq = Some(i);
      }
    }
    i += 1;
  }
  match (last_eq, last_comma) {
    (None, _) => true,
    (Some(_), None) => false,
    (Some(eq), Some(co)) => co > eq,
  }
}

fn on_conflict_expects_target_column(source: &str, offset: TextSize) -> bool {
  let (_slice, upper) = stmt_slice_upper(source, offset);
  let Some(oc_at) = upper.rfind("ON CONFLICT") else { return false };
  // Everything after ON CONFLICT until the cursor.
  let after = &upper[oc_at + "ON CONFLICT".len()..];
  // Case 1: `ON CONFLICT (<cursor>)` -- inside a paren list with no
  // matching `)`. Track depth from the first `(` after ON CONFLICT.
  let mut depth = 0i32;
  let mut in_target = false;
  for c in after.chars() {
    match c {
      '(' => {
        depth += 1;
        in_target = true;
      },
      ')' => depth -= 1,
      _ => {},
    }
  }
  if in_target && depth > 0 {
    return true;
  }
  // Case 2: `DO UPDATE SET <cursor>` LHS slot -- cursor is right after
  // the SET keyword and not yet inside an expression (no `=` since last
  // top-level comma).
  let trimmed = after.trim_end();
  let after_set = trimmed.rsplit("SET").next().unwrap_or("");
  if trimmed.contains("DO UPDATE SET") {
    // If after_set has no `=` and we're either right after SET or after
    // a comma, we're naming a column LHS.
    let cleaned = after_set.trim_start();
    let no_assignment_yet = !cleaned.contains('=');
    let after_comma_or_start = cleaned.is_empty()
      || cleaned.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == ',' || c.is_whitespace());
    if no_assignment_yet && after_comma_or_start {
      return true;
    }
  }
  false
}

/// True when the cursor sits at `RAISE <cursor>` (PL/pgSQL) -- the
/// next token is a level keyword (DEBUG / LOG / INFO / NOTICE /
/// WARNING / EXCEPTION). Also fires for a partial level word.
fn raise_expects_level_keyword(source: &str, offset: TextSize) -> bool {
  let (slice, upper) = stmt_slice_upper(source, offset);
  let stmt = slice.trim_start();
  let words: Vec<&str> = upper.split_ascii_whitespace().collect();
  if words.len() == 1 && words[0] == "RAISE" && stmt.ends_with(char::is_whitespace) {
    return true;
  }
  false
}

/// True when the cursor sits at `RESET <cursor>` -- the next token is
/// ALL / ROLE / SESSION AUTHORIZATION / a GUC name. We surface only
/// the keyword forms; GUC names are freeform.
fn reset_expects_subkeyword(source: &str, offset: TextSize) -> bool {
  let (slice, upper) = stmt_slice_upper(source, offset);
  let stmt = slice.trim_start();
  let words: Vec<&str> = upper.split_ascii_whitespace().collect();
  if words.len() == 1 && words[0] == "RESET" && stmt.ends_with(char::is_whitespace) {
    return true;
  }
  false
}

/// True when the cursor sits at the start of a `WINDOW <name> AS (...)`
/// body -- i.e. right after the opening `(`. Detects an unclosed
/// window paren and rejects positions past a sub-clause keyword.
fn window_clause_paren_expects_subclause(source: &str, offset: TextSize) -> bool {
  let pos: usize = u32::from(offset) as usize;
  let pos = pos.min(source.len());
  let before = &source[..pos];
  let upper = before.to_ascii_uppercase();
  let Some(win_at) = upper.rfind("WINDOW ") else { return false };
  // `WINDOW <name> AS (...)` is the only valid form.
  let after = &before[win_at + "WINDOW ".len()..];
  if !after.to_ascii_uppercase().contains(" AS ") {
    return false;
  }
  // Find the opening `(` after AS and verify the paren is still open.
  let Some(open) = after.find('(') else { return false };
  let body = &after[open + 1..];
  let mut depth = 1i32;
  for c in body.chars() {
    match c {
      '(' => depth += 1,
      ')' => depth -= 1,
      _ => {},
    }
  }
  if depth <= 0 {
    return false;
  }
  // Cursor is inside the paren. Only fire when no sub-clause keyword
  // has been typed yet -- so the FIRST thing in the body is the slot.
  let body_trim = body.trim();
  body_trim.is_empty()
    || !["PARTITION", "ORDER", "ROWS", "RANGE", "GROUPS"]
      .iter()
      .any(|kw| body_trim.to_ascii_uppercase().starts_with(kw))
}

/// True when the cursor sits at `WINDOW w AS (PARTITION BY <cursor>` or
/// `... (ORDER BY <cursor>` inside the window body -- expects a column
/// from the FROM tables.
fn window_clause_partition_or_order_by_expects_column(source: &str, offset: TextSize) -> bool {
  let pos: usize = u32::from(offset) as usize;
  let pos = pos.min(source.len());
  let before = &source[..pos];
  if !before.ends_with(char::is_whitespace) {
    return false;
  }
  let upper = before.to_ascii_uppercase();
  let Some(win_at) = upper.rfind("WINDOW ") else { return false };
  let after = &upper[win_at + "WINDOW ".len()..];
  if !after.contains(" AS ") {
    return false;
  }
  // Paren depth check: must still be unclosed.
  let Some(open) = after.find('(') else { return false };
  let body = &after[open + 1..];
  let mut depth = 1i32;
  for c in body.chars() {
    match c {
      '(' => depth += 1,
      ')' => depth -= 1,
      _ => {},
    }
  }
  if depth <= 0 {
    return false;
  }
  // Trailing PARTITION BY / ORDER BY / comma after one of them.
  // Use word-bounded match so `(PARTITION BY` (no leading space) also
  // matches alongside ` PARTITION BY`.
  let trimmed = upper.trim_end();
  let ends_partition_by = trimmed.ends_with("PARTITION BY")
    && {
      let n = trimmed.len() - "PARTITION BY".len();
      n == 0 || !trimmed.as_bytes()[n - 1].is_ascii_alphanumeric()
    };
  let ends_order_by = trimmed.ends_with("ORDER BY")
    && {
      let n = trimmed.len() - "ORDER BY".len();
      n == 0 || !trimmed.as_bytes()[n - 1].is_ascii_alphanumeric()
    };
  ends_partition_by || ends_order_by || trimmed.ends_with(',')
}

/// True when the cursor sits at the sampling-method slot after
/// `TABLESAMPLE` in a SELECT FROM clause.
/// Curated set-returning function list used to narrow the slot right
/// after `LATERAL`. Not exhaustive -- covers the SRFs that are
/// genuinely common in LATERAL joins. The catalog's own function list
/// is not used because it would re-introduce non-SRF noise.
const LATERAL_TARGETS: &[(&str, &str)] = &[
  ("generate_series", "generate_series(start, stop[, step]) -> setof <type>"),
  ("unnest", "unnest(<array>) -> setof <element-type>"),
  ("jsonb_array_elements", "jsonb_array_elements(<jsonb>) -> setof jsonb"),
  ("jsonb_array_elements_text", "jsonb_array_elements_text(<jsonb>) -> setof text"),
  ("jsonb_each", "jsonb_each(<jsonb>) -> setof (key text, value jsonb)"),
  ("jsonb_each_text", "jsonb_each_text(<jsonb>) -> setof (key text, value text)"),
  ("jsonb_object_keys", "jsonb_object_keys(<jsonb>) -> setof text"),
  ("jsonb_to_record", "jsonb_to_record(<jsonb>) AS x(...) -- requires column-def alias"),
  ("jsonb_to_recordset", "jsonb_to_recordset(<jsonb>) AS x(...) -- requires column-def alias"),
  ("json_array_elements", "json_array_elements(<json>) -> setof json"),
  ("json_array_elements_text", "json_array_elements_text(<json>) -> setof text"),
  ("json_each", "json_each(<json>) -> setof (key text, value json)"),
  ("json_each_text", "json_each_text(<json>) -> setof (key text, value text)"),
  ("json_object_keys", "json_object_keys(<json>) -> setof text"),
  ("regexp_split_to_table", "regexp_split_to_table(<str>, <pattern>) -> setof text"),
  ("string_to_table", "string_to_table(<str>, <delim>) -> setof text"),
  ("array_to_table", "(SELECT ...) -- a parenthesized subquery is also a valid LATERAL target"),
];

/// True when the cursor is positioned at a LATERAL-target slot:
/// directly after a word-bounded `LATERAL` keyword, optionally with
/// a partial identifier the user is starting to type.
fn lateral_target_expected(source: &str, offset: TextSize) -> bool {
  let pos: usize = u32::from(offset) as usize;
  let pos = pos.min(source.len());
  let before = &source[..pos];
  // Trim a trailing partial word (the identifier under the cursor).
  let trimmed = before.trim_end_matches(|c: char| c.is_alphanumeric() || c == '_');
  if !trimmed.ends_with(char::is_whitespace) {
    return false;
  }
  let upper = trimmed.trim_end().to_ascii_uppercase();
  // The preceding token must be `LATERAL`. Use ends_with-with-boundary
  // check so we don't match identifiers ending in LATERAL.
  if !upper.ends_with("LATERAL") {
    return false;
  }
  let len = upper.len();
  if len > 7 {
    let prev = upper.as_bytes()[len - 8] as char;
    if prev.is_alphanumeric() || prev == '_' {
      return false;
    }
  }
  true
}

fn tablesample_expects_method(source: &str, offset: TextSize) -> bool {
  let pos: usize = u32::from(offset) as usize;
  let pos = pos.min(source.len());
  let before = &source[..pos];
  if !before.ends_with(char::is_whitespace) {
    return false;
  }
  let upper = before.trim_end().to_ascii_uppercase();
  upper.ends_with(" TABLESAMPLE") || upper == "TABLESAMPLE"
}

/// Detect a SELECT locking-clause slot:
///   `... FOR <cursor>`            -> UPDATE / NO KEY UPDATE / SHARE / KEY SHARE
///   `... FOR UPDATE <cursor>`     -> OF / NOWAIT / SKIP LOCKED
///   `... FOR NO KEY UPDATE <cur>` -> OF / NOWAIT / SKIP LOCKED
///   `... FOR SHARE <cursor>`      -> OF / NOWAIT / SKIP LOCKED
///   `... FOR KEY SHARE <cursor>`  -> OF / NOWAIT / SKIP LOCKED
/// Returns None when not in such a slot (so the regular AfterTable
/// handler runs).
fn select_for_locking_keywords(
  source: &str,
  offset: TextSize,
) -> Option<&'static [(&'static str, &'static str)]> {
  let pos: usize = u32::from(offset) as usize;
  let pos = pos.min(source.len());
  let before = &source[..pos];
  if !before.ends_with(char::is_whitespace) {
    return None;
  }
  let trimmed = before.trim_end();
  let upper = trimmed.to_ascii_uppercase();
  const STRENGTH: &[(&str, &str)] = &[
    ("UPDATE", "FOR UPDATE -- exclusive row lock"),
    ("NO KEY UPDATE", "FOR NO KEY UPDATE -- weaker than UPDATE"),
    ("SHARE", "FOR SHARE -- shared row lock"),
    ("KEY SHARE", "FOR KEY SHARE -- weakest row lock"),
  ];
  const MODIFIERS: &[(&str, &str)] = &[
    ("OF", "OF <table>[, ...] -- restrict to specific tables"),
    ("NOWAIT", "NOWAIT -- error instead of waiting for the lock"),
    ("SKIP LOCKED", "SKIP LOCKED -- skip rows already locked"),
  ];
  if upper.ends_with(" FOR") || upper == "FOR" {
    return Some(STRENGTH);
  }
  for tail in &[" FOR UPDATE", " FOR NO KEY UPDATE", " FOR SHARE", " FOR KEY SHARE"] {
    if upper.ends_with(tail) {
      return Some(MODIFIERS);
    }
  }
  None
}

/// Return the IS-continuation keywords when the cursor sits right
/// after `IS` or `IS NOT` in a predicate context. None otherwise.
fn is_predicate_continuation_keywords(
  source: &str,
  offset: TextSize,
) -> Option<&'static [(&'static str, &'static str)]> {
  let pos: usize = u32::from(offset) as usize;
  let pos = pos.min(source.len());
  // Use the raw text up to the cursor; cheap and avoids parser sensitivity.
  let before = &source[..pos];
  let trimmed = before.trim_end();
  if !before.ends_with(char::is_whitespace) {
    return None;
  }
  // Detect a trailing `IS NOT` token (word-bounded: preceded by
  // whitespace or at start of statement).
  let upper_trim = trimmed.to_ascii_uppercase();
  // `IS DISTINCT` / `IS NOT DISTINCT` -- next required token is FROM.
  if upper_trim.ends_with(" IS DISTINCT") || upper_trim.ends_with(" IS NOT DISTINCT") {
    return Some(&[("FROM", "IS [NOT] DISTINCT FROM <expr> -- NULL-aware compare")]);
  }
  // `IS JSON` / `IS NOT JSON` -- SQL/JSON predicate (PG16+); next is the
  // optional shape token (VALUE/SCALAR/ARRAY/OBJECT). End-of-clause is
  // also valid, but surfacing the four options helps discovery.
  if upper_trim.ends_with(" IS JSON") || upper_trim.ends_with(" IS NOT JSON") {
    return Some(&[
      ("VALUE", "IS [NOT] JSON VALUE -- any JSON value (default)"),
      ("SCALAR", "IS [NOT] JSON SCALAR -- number/string/boolean/null"),
      ("ARRAY", "IS [NOT] JSON ARRAY"),
      ("OBJECT", "IS [NOT] JSON OBJECT [WITH|WITHOUT UNIQUE KEYS]"),
    ]);
  }
  // `... IS JSON OBJECT WITH` / `... WITHOUT` -- UNIQUE KEYS tail.
  if upper_trim.ends_with(" IS JSON OBJECT WITH") || upper_trim.ends_with(" IS NOT JSON OBJECT WITH") {
    return Some(&[("UNIQUE KEYS", "WITH UNIQUE KEYS -- forbid duplicate keys at any level")]);
  }
  if upper_trim.ends_with(" IS JSON OBJECT WITHOUT") || upper_trim.ends_with(" IS NOT JSON OBJECT WITHOUT") {
    return Some(&[("UNIQUE KEYS", "WITHOUT UNIQUE KEYS -- explicit default (duplicates allowed)")]);
  }
  let last_is_not = upper_trim.ends_with(" IS NOT") || upper_trim == "IS NOT";
  if last_is_not {
    // After `IS NOT`: NULL / TRUE / FALSE / UNKNOWN / DISTINCT FROM.
    const IS_NOT: &[(&str, &str)] = &[
      ("NULL", "IS NOT NULL"),
      ("TRUE", "IS NOT TRUE"),
      ("FALSE", "IS NOT FALSE"),
      ("UNKNOWN", "IS NOT UNKNOWN"),
      ("JSON", "IS NOT JSON [VALUE|SCALAR|ARRAY|OBJECT] (PG16+)"),
      ("DISTINCT FROM", "IS NOT DISTINCT FROM <expr> -- NULL-aware equality"),
    ];
    return Some(IS_NOT);
  }
  // Check ` IS` at end (word-bounded: preceded by whitespace).
  let ends_with_is = trimmed.len() >= 2
    && trimmed.as_bytes()[trimmed.len() - 2..]
      .eq_ignore_ascii_case(b"IS")
    && (trimmed.len() == 2 || trimmed.as_bytes()[trimmed.len() - 3].is_ascii_whitespace());
  if ends_with_is {
    const IS: &[(&str, &str)] = &[
      ("NULL", "IS NULL"),
      ("NOT NULL", "IS NOT NULL"),
      ("TRUE", "IS TRUE"),
      ("FALSE", "IS FALSE"),
      ("UNKNOWN", "IS UNKNOWN"),
      ("DISTINCT FROM", "IS DISTINCT FROM <expr> -- NULL-aware inequality"),
      ("NOT DISTINCT FROM", "IS NOT DISTINCT FROM <expr> -- NULL-aware equality"),
      ("JSON", "IS JSON [VALUE|SCALAR|ARRAY|OBJECT] (PG16+)"),
    ];
    return Some(IS);
  }
  None
}

/// True when the cursor sits at `DISCARD <cursor>` -- the next token
/// is one of ALL / PLANS / SEQUENCES / TEMP / TEMPORARY.
fn discard_expects_subkeyword(source: &str, offset: TextSize) -> bool {
  let (slice, upper) = stmt_slice_upper(source, offset);
  let stmt = slice.trim_start();
  let words: Vec<&str> = upper.split_ascii_whitespace().collect();
  if words.len() == 1 && words[0] == "DISCARD" && stmt.ends_with(char::is_whitespace) {
    return true;
  }
  // Partial sub-keyword being typed: `DISCARD AL` etc.
  if words.len() == 2 && words[0] == "DISCARD" && !stmt.ends_with(char::is_whitespace) {
    return true;
  }
  false
}

/// True when the cursor sits at the `ON` slot of `CREATE [UNIQUE]
/// INDEX [CONCURRENTLY] [IF NOT EXISTS] <name> <cursor>` -- the next
/// required token is `ON`.
fn create_index_expects_on(source: &str, offset: TextSize) -> bool {
  let (slice, upper) = stmt_slice_upper(source, offset);
  let stmt = slice.trim_start();
  if !upper.starts_with("CREATE INDEX") && !upper.starts_with("CREATE UNIQUE INDEX") {
    return false;
  }
  // Walk past the keyword sequence and optional modifiers, then expect
  // exactly one name token then whitespace at cursor.
  let after = if let Some(s) = upper.strip_prefix("CREATE UNIQUE INDEX") {
    s
  } else if let Some(s) = upper.strip_prefix("CREATE INDEX") {
    s
  } else {
    return false;
  };
  let words: Vec<&str> = after.split_ascii_whitespace().collect();
  // Skip CONCURRENTLY / IF NOT EXISTS.
  let mut i = 0;
  if i < words.len() && words[i] == "CONCURRENTLY" {
    i += 1;
  }
  if i + 3 <= words.len() && words[i] == "IF" && words[i + 1] == "NOT" && words[i + 2] == "EXISTS" {
    i += 3;
  }
  // Expect single <name> then cursor whitespace, no ON yet.
  if i + 1 == words.len() && stmt.ends_with(char::is_whitespace) && !after.contains(" ON ") {
    return true;
  }
  false
}

/// True when the cursor sits at the `ON` slot of `CREATE POLICY <name>
/// <cursor>` -- the next required token is `ON`.
fn create_policy_expects_on(source: &str, offset: TextSize) -> bool {
  let (slice, upper) = stmt_slice_upper(source, offset);
  let stmt = slice.trim_start();
  if !upper.starts_with("CREATE POLICY") {
    return false;
  }
  let after = &upper["CREATE POLICY".len()..];
  let words: Vec<&str> = after.split_ascii_whitespace().collect();
  // Expect <name> (1 word) then cursor at whitespace, no ON yet.
  words.len() == 1 && stmt.ends_with(char::is_whitespace) && !after.contains(" ON ")
}

/// True when the cursor sits at the table slot of `CREATE POLICY <name>
/// ON <cursor>`.
fn create_policy_expects_table(source: &str, offset: TextSize) -> bool {
  let (slice, upper) = stmt_slice_upper(source, offset);
  let stmt = slice.trim_start();
  if !upper.starts_with("CREATE POLICY") {
    return false;
  }
  let after = &upper["CREATE POLICY".len()..];
  let words: Vec<&str> = after.split_ascii_whitespace().collect();
  // `<name> ON <cursor>` -- 2 tokens then whitespace.
  if words.len() == 2 && words[1] == "ON" && stmt.ends_with(char::is_whitespace) {
    return true;
  }
  // Partial table name being typed: `<name> ON tab` -- 3 tokens, last
  // partial.
  if words.len() == 3 && words[1] == "ON" && !stmt.ends_with(char::is_whitespace) {
    return true;
  }
  false
}

/// True when the cursor sits at the timing slot of a
/// `CREATE [OR REPLACE] TRIGGER <name> <cursor>` statement -- the
/// next token should be BEFORE / AFTER / INSTEAD OF.
fn create_trigger_expects_timing(source: &str, offset: TextSize) -> bool {
  let (slice, upper) = stmt_slice_upper(source, offset);
  let stmt = slice.trim_start();
  let starts =
    upper.starts_with("CREATE TRIGGER") || upper.starts_with("CREATE OR REPLACE TRIGGER") || upper.starts_with("CREATE CONSTRAINT TRIGGER");
  if !starts {
    return false;
  }
  // Tokens after the TRIGGER keyword: the next word is the trigger name,
  // and the cursor sits at the slot right after it (whitespace).
  let after = upper.split_once("TRIGGER").map(|x| x.1).unwrap_or("");
  let words: Vec<&str> = after.split_ascii_whitespace().collect();
  // Skip optional IF NOT EXISTS.
  let mut i = 0;
  if i + 3 <= words.len() && words[i] == "IF" && words[i + 1] == "NOT" && words[i + 2] == "EXISTS" {
    i += 3;
  }
  // Expect <name> then cursor.
  if i + 1 == words.len() && stmt.ends_with(char::is_whitespace) {
    return true;
  }
  false
}

/// True when the cursor sits at a role-name slot of a command that
/// takes an existing role from the catalog. Covers ALTER/DROP ROLE,
/// DROP USER/GROUP, REASSIGN OWNED BY, DROP OWNED BY. The caller
/// emits the catalog roles + PUBLIC pseudo-role.
fn command_expects_role_name(source: &str, offset: TextSize) -> bool {
  let (slice, upper) = stmt_slice_upper(source, offset);
  let stmt = slice.trim_start();
  let kinds: &[(&[&str], &[&str])] = &[
    (&["ALTER", "ROLE"], &["IF", "EXISTS"]),
    (&["ALTER", "USER"], &["IF", "EXISTS"]),
    (&["ALTER", "GROUP"], &["IF", "EXISTS"]),
    (&["DROP", "ROLE"], &["IF", "EXISTS"]),
    (&["DROP", "USER"], &["IF", "EXISTS"]),
    (&["DROP", "GROUP"], &["IF", "EXISTS"]),
    (&["REASSIGN", "OWNED", "BY"], &[]),
    (&["DROP", "OWNED", "BY"], &[]),
    (&["CREATE", "SCHEMA", "AUTHORIZATION"], &[]),
    (&["GRANT", "ROLE"], &[]),
  ];
  let words: Vec<&str> = upper.split_ascii_whitespace().collect();
  for (kw_seq, mods) in kinds {
    if words.len() < kw_seq.len() {
      continue;
    }
    if !kw_seq.iter().zip(words.iter()).all(|(k, w)| {
      let cleaned = w.trim_end_matches(|c: char| !c.is_ascii_alphanumeric() && c != '_');
      cleaned.eq_ignore_ascii_case(k)
    }) {
      continue;
    }
    let mut i = kw_seq.len();
    while i < words.len() && mods.iter().any(|m| words[i].eq_ignore_ascii_case(m)) {
      i += 1;
    }
    let ends_with_comma = stmt.trim_end().ends_with(',');
    let last_word_is_partial = !stmt.ends_with(char::is_whitespace);
    if i >= words.len() || ends_with_comma || last_word_is_partial {
      return true;
    }
  }
  false
}

/// Detect a `SET` / `SET LOCAL` / `SET SESSION` GUC-name slot.
/// `SET <cursor>` -> suggest the scope modifiers (LOCAL, SESSION),
/// since the actual GUC name comes next and isn't catalog-resolvable.
/// `SET LOCAL <cursor>` / `SET SESSION <cursor>` -> empty (the GUC
/// name is freeform). Returns None when not in a SET slot.
fn set_statement_completion(source: &str, offset: TextSize) -> Option<&'static [(&'static str, &'static str)]> {
  let (slice, upper) = stmt_slice_upper(source, offset);
  let stmt = slice.trim_start();
  // Skip SET ROLE / SET CONSTRAINTS / SET TRANSACTION (those are
  // their own slots; routing them as GUC would be confusing).
  if upper.starts_with("SET ROLE") || upper.starts_with("SET SESSION AUTHORIZATION")
    || upper.starts_with("SET CONSTRAINTS") || upper.starts_with("SET TRANSACTION")
  {
    return None;
  }
  const SET_MODS: &[(&str, &str)] = &[
    ("LOCAL", "SET LOCAL <var> = <value>  -- transaction-scoped"),
    ("SESSION", "SET SESSION <var> = <value>  -- session-scoped (default)"),
  ];
  let words: Vec<&str> = upper.split_ascii_whitespace().collect();
  // `SET <cursor>` -- emit LOCAL / SESSION modifiers. GUC name slot
  // (`SET LOCAL <cursor>` etc) is filled by `show_or_set_guc_names`.
  if words.len() == 1 && words[0] == "SET" && stmt.ends_with(char::is_whitespace) {
    return Some(SET_MODS);
  }
  None
}

/// Detect a transaction-control statement slot. Returns the keyword
/// list to emit (empty for COMMIT/ROLLBACK/END/ABORT/SAVEPOINT which
/// take no further token or a fresh identifier). None when the cursor
/// isn't in a recognised transaction-control slot.
fn transaction_control_completion(source: &str, offset: TextSize) -> Option<&'static [(&'static str, &'static str)]> {
  let (_slice, upper) = stmt_slice_upper(source, offset);
  // Slots that take transaction modifiers (ISOLATION LEVEL, READ ONLY,
  // READ WRITE, DEFERRABLE, NOT DEFERRABLE). `ATOMIC` is the standard-SQL
  // form for CREATE FUNCTION bodies (`BEGIN ATOMIC ... END;`).
  const TXN_MODIFIERS: &[(&str, &str)] = &[
    ("ISOLATION LEVEL", "ISOLATION LEVEL SERIALIZABLE|REPEATABLE READ|READ COMMITTED|READ UNCOMMITTED"),
    ("READ ONLY", "READ ONLY"),
    ("READ WRITE", "READ WRITE"),
    ("DEFERRABLE", "DEFERRABLE"),
    ("NOT DEFERRABLE", "NOT DEFERRABLE"),
    ("ATOMIC", "ATOMIC -- standard-SQL CREATE FUNCTION body: BEGIN ATOMIC <stmts> END;"),
  ];
  // Slots that take nothing useful (fresh savepoint name / no args).
  const EMPTY: &[(&str, &str)] = &[];
  let starts_with = |kw: &str| {
    upper.starts_with(kw)
      && (upper.len() == kw.len() || upper.as_bytes()[kw.len()].is_ascii_whitespace())
  };
  if starts_with("BEGIN TRANSACTION") || starts_with("START TRANSACTION") || starts_with("BEGIN") {
    return Some(TXN_MODIFIERS);
  }
  if starts_with("COMMIT") || starts_with("ROLLBACK") || starts_with("END") || starts_with("ABORT") || starts_with("SAVEPOINT") || starts_with("RELEASE") {
    return Some(EMPTY);
  }
  None
}

/// Object classes that COMMENT ON accepts. Full PG kind list.
const COMMENT_ON_CLASSES: &[(&str, &str)] = &[
  ("TABLE", "COMMENT ON TABLE <name> IS '...'"),
  ("COLUMN", "COMMENT ON COLUMN <table>.<column> IS '...'"),
  ("SCHEMA", "COMMENT ON SCHEMA <name> IS '...'"),
  ("DATABASE", "COMMENT ON DATABASE <name> IS '...'"),
  ("FUNCTION", "COMMENT ON FUNCTION <name>(...) IS '...'"),
  ("PROCEDURE", "COMMENT ON PROCEDURE <name>(...) IS '...'"),
  ("ROUTINE", "COMMENT ON ROUTINE <name>(...) IS '...'"),
  ("INDEX", "COMMENT ON INDEX <name> IS '...'"),
  ("VIEW", "COMMENT ON VIEW <name> IS '...'"),
  ("MATERIALIZED VIEW", "COMMENT ON MATERIALIZED VIEW <name> IS '...'"),
  ("FOREIGN TABLE", "COMMENT ON FOREIGN TABLE <name> IS '...'"),
  ("FOREIGN DATA WRAPPER", "COMMENT ON FOREIGN DATA WRAPPER <name> IS '...'"),
  ("SERVER", "COMMENT ON SERVER <name> IS '...'"),
  ("SEQUENCE", "COMMENT ON SEQUENCE <name> IS '...'"),
  ("TYPE", "COMMENT ON TYPE <name> IS '...'"),
  ("DOMAIN", "COMMENT ON DOMAIN <name> IS '...'"),
  ("EXTENSION", "COMMENT ON EXTENSION <name> IS '...'"),
  ("ROLE", "COMMENT ON ROLE <name> IS '...'"),
  ("TRIGGER", "COMMENT ON TRIGGER <name> ON <table> IS '...'"),
  ("CONSTRAINT", "COMMENT ON CONSTRAINT <name> ON <table> IS '...'"),
  ("POLICY", "COMMENT ON POLICY <name> ON <table> IS '...'"),
  ("RULE", "COMMENT ON RULE <name> ON <table> IS '...'"),
  ("AGGREGATE", "COMMENT ON AGGREGATE <name>(args) IS '...'"),
  ("CAST", "COMMENT ON CAST (src AS dst) IS '...'"),
  ("COLLATION", "COMMENT ON COLLATION <name> IS '...'"),
  ("CONVERSION", "COMMENT ON CONVERSION <name> IS '...'"),
  ("OPERATOR", "COMMENT ON OPERATOR <op>(args) IS '...'"),
  ("OPERATOR CLASS", "COMMENT ON OPERATOR CLASS <name> USING <am> IS '...'"),
  ("OPERATOR FAMILY", "COMMENT ON OPERATOR FAMILY <name> USING <am> IS '...'"),
  ("STATISTICS", "COMMENT ON STATISTICS <name> IS '...'"),
  ("ACCESS METHOD", "COMMENT ON ACCESS METHOD <name> IS '...'"),
  ("LANGUAGE", "COMMENT ON LANGUAGE <name> IS '...'"),
  ("LARGE OBJECT", "COMMENT ON LARGE OBJECT <oid> IS '...'"),
  ("TABLESPACE", "COMMENT ON TABLESPACE <name> IS '...'"),
  ("PUBLICATION", "COMMENT ON PUBLICATION <name> IS '...'"),
  ("SUBSCRIPTION", "COMMENT ON SUBSCRIPTION <name> IS '...'"),
  ("EVENT TRIGGER", "COMMENT ON EVENT TRIGGER <name> IS '...'"),
  ("TEXT SEARCH CONFIGURATION", "COMMENT ON TEXT SEARCH CONFIGURATION <name> IS '...'"),
  ("TEXT SEARCH DICTIONARY", "COMMENT ON TEXT SEARCH DICTIONARY <name> IS '...'"),
  ("TEXT SEARCH PARSER", "COMMENT ON TEXT SEARCH PARSER <name> IS '...'"),
  ("TEXT SEARCH TEMPLATE", "COMMENT ON TEXT SEARCH TEMPLATE <name> IS '...'"),
  ("TRANSFORM", "COMMENT ON TRANSFORM FOR <type> LANGUAGE <lang> IS '...'"),
];

/// True when the cursor sits right after `COMMENT ON` with no class
/// keyword typed yet -- the user must pick a class before naming the
/// target object.
/// COMMENT ON ... IS <cursor> -> NULL or empty-string hints.
fn comment_on_is_value_next_keyword(
  source: &str,
  offset: TextSize,
) -> Option<&'static [(&'static str, &'static str)]> {
  if cursor_not_at_ws_boundary(source, offset) {
    return None;
  }
  let (_, upper) = stmt_slice_upper(source, offset);
  if !upper.starts_with("COMMENT ON") {
    return None;
  }
  let words: Vec<&str> = upper.split_ascii_whitespace().collect();
  let last = *words.last()?;
  if last == "IS" {
    return Some(&[
      ("NULL", "IS NULL -- drop the comment"),
      ("''", "'' -- empty comment (equivalent to NULL)"),
    ]);
  }
  // After class + name, expect IS.
  if words.contains(&"ON") && !words.contains(&"IS") && words.len() >= 4 {
    return Some(&[("IS", "IS '<comment_text>' | IS NULL -- drop")]);
  }
  None
}

fn comment_on_expects_class_keyword(source: &str, offset: TextSize) -> bool {
  let (slice, upper) = stmt_slice_upper(source, offset);
  let stmt = slice.trim_start();
  if !upper.starts_with("COMMENT ON") {
    return false;
  }
  let after = &upper["COMMENT ON".len()..];
  let after_trim = after.trim_start();
  if after_trim.is_empty() && stmt.ends_with(char::is_whitespace) {
    return true;
  }
  // A single partial class word being typed (no further tokens).
  let words: Vec<&str> = after_trim.split_ascii_whitespace().collect();
  if words.len() == 1 && !stmt.ends_with(char::is_whitespace) {
    // Partial class keyword: still in the class slot.
    return true;
  }
  false
}

/// True when the cursor sits at the body slot of `DECLARE <name>
/// [...] CURSOR FOR <cursor>` -- expects a SELECT statement.
fn declare_cursor_for_expects_statement(source: &str, offset: TextSize) -> bool {
  let (slice, upper) = stmt_slice_upper(source, offset);
  let stmt = slice.trim_start();
  if !upper.starts_with("DECLARE") {
    return false;
  }
  // The text ending in `CURSOR FOR ` (possibly with options between
  // <name> and CURSOR) is the trigger.
  let trimmed = upper.trim_end();
  trimmed.ends_with("CURSOR FOR") && stmt.ends_with(char::is_whitespace)
}

/// True when the cursor sits at `WITH [RECURSIVE] <name> [(cols)] AS
/// <cursor>` -- the next token is either MATERIALIZED, NOT
/// MATERIALIZED, or `(` (the CTE body).
fn with_cte_after_as_expects_materialized(source: &str, offset: TextSize) -> bool {
  let (slice, upper) = stmt_slice_upper(source, offset);
  let stmt = slice.trim_start();
  if !upper.starts_with("WITH ") && !upper.starts_with("WITH\n") && !upper.starts_with("WITH\t") {
    return false;
  }
  // Cursor must follow ` AS` token at top level.
  let trimmed = upper.trim_end();
  trimmed.ends_with(" AS") && stmt.ends_with(char::is_whitespace)
}

/// True when the cursor sits at `DO <cursor>` -- expects LANGUAGE or
/// a dollar-quoted body.
fn do_expects_language_or_body(source: &str, offset: TextSize) -> bool {
  let (slice, upper) = stmt_slice_upper(source, offset);
  let stmt = slice.trim_start();
  let words: Vec<&str> = upper.split_ascii_whitespace().collect();
  words.len() == 1 && words[0] == "DO" && stmt.ends_with(char::is_whitespace)
}

/// True when the cursor sits at the option slot of
/// `CREATE [TEMP|TEMPORARY|UNLOGGED] SEQUENCE [IF NOT EXISTS] <name>
/// <cursor>` -- the next token is one of the sequence-option keywords.
fn create_sequence_expects_option(source: &str, offset: TextSize) -> bool {
  let (slice, upper) = stmt_slice_upper(source, offset);
  let stmt = slice.trim_start();
  if !upper.contains("SEQUENCE") || !upper.starts_with("CREATE") {
    return false;
  }
  let after = if let Some(s) = upper.split_once("SEQUENCE").map(|x| x.1) { s } else { return false };
  let words: Vec<&str> = after.split_ascii_whitespace().collect();
  let mut i = 0;
  // Optional IF NOT EXISTS.
  if i + 3 <= words.len() && words[i] == "IF" && words[i + 1] == "NOT" && words[i + 2] == "EXISTS" {
    i += 3;
  }
  // Expect <name> then cursor whitespace.
  if i + 1 == words.len() && stmt.ends_with(char::is_whitespace) {
    return true;
  }
  false
}

/// True when the cursor sits at `CREATE TYPE <name> AS <cursor>` --
/// expects one of the type-kind keywords (ENUM / RANGE) or `(` for
/// a composite type.
/// True when the cursor sits inside the body of a `CREATE TYPE foo AS
/// ENUM (` or `RANGE (` -- both expect literals or option pairs that
/// have no useful catalog completion. Used to suppress the catch-all
/// keyword dump.
fn create_type_enum_or_range_body(source: &str, offset: TextSize) -> bool {
  // Renamed semantically -- only ENUM body suppresses the menu (label literals).
  // RANGE body wants option-name keywords (SUBTYPE / CANONICAL / ...), which
  // `create_type_next_keyword` emits.
  let (_slice, upper) = stmt_slice_upper(source, offset);
  if !upper.starts_with("CREATE TYPE") {
    return false;
  }
  let Some(open_at) = upper.find(" AS ENUM (").map(|p| p + " AS ENUM (".len()) else {
    return false;
  };
  let bytes = upper.as_bytes();
  let mut depth: i32 = 1;
  let mut i = open_at;
  while i < bytes.len() {
    match bytes[i] {
      b'(' => depth += 1,
      b')' => depth -= 1,
      _ => {},
    }
    if depth == 0 {
      return false;
    }
    i += 1;
  }
  depth > 0
}

fn create_type_as_expects_kind(source: &str, offset: TextSize) -> bool {
  let (slice, upper) = stmt_slice_upper(source, offset);
  let stmt = slice.trim_start();
  if !upper.starts_with("CREATE TYPE") {
    return false;
  }
  let after = &upper["CREATE TYPE".len()..];
  let words: Vec<&str> = after.split_ascii_whitespace().collect();
  // `<name> AS <cursor>` -- 2 tokens with trailing whitespace, slot
  // wants the kind keyword.
  if words.len() == 2 && words[1] == "AS" && stmt.ends_with(char::is_whitespace) {
    return true;
  }
  // `<name> AS <partial>` -- 3 tokens, no trailing whitespace, the
  // user is mid-typing a kind keyword (e.g. `E` for ENUM). The LSP
  // filters the emitted menu by prefix on the client side, so still
  // emit the full kind list rather than fall through to the catch-all.
  if words.len() == 3 && words[1] == "AS" && !stmt.ends_with(char::is_whitespace) {
    return true;
  }
  false
}

/// True when the cursor sits inside the `VACUUM (...)` options paren
/// at an option-name slot (start of paren or after comma).
fn vacuum_paren_expects_option(source: &str, offset: TextSize) -> bool {
  let (slice, upper) = stmt_slice_upper(source, offset);
  let stmt = slice.trim_start();
  if !upper.starts_with("VACUUM") {
    return false;
  }
  let after = &stmt[upper.find("VACUUM").unwrap() + "VACUUM".len()..];
  let mut depth = 0i32;
  let mut saw_open = false;
  for c in after.chars() {
    match c {
      '(' => {
        depth += 1;
        saw_open = true;
      },
      ')' => depth -= 1,
      _ => {},
    }
  }
  if !saw_open || depth == 0 {
    return false;
  }
  let trimmed = stmt.trim_end();
  let last = trimmed.chars().last();
  if matches!(last, Some('(') | Some(',')) {
    return true;
  }
  if !stmt.ends_with(char::is_whitespace) {
    let inner_start = after.rfind('(').unwrap_or(0);
    let last_comma_or_open =
      after[inner_start..].rfind([',', '(']).unwrap_or(0);
    let since = &after[inner_start + last_comma_or_open..];
    if !since.contains('=') {
      return true;
    }
  }
  false
}

/// True when the cursor sits at the FORMAT value slot inside the
/// `EXPLAIN (...)` paren -- i.e. last meaningful token is `FORMAT`.
fn explain_paren_format_value(source: &str, offset: TextSize) -> bool {
  explain_paren_value_after(source, offset, "FORMAT")
}

/// True when the cursor sits at the SERIALIZE value slot inside the
/// `EXPLAIN (...)` paren -- last meaningful token is `SERIALIZE`.
fn explain_paren_serialize_value(source: &str, offset: TextSize) -> bool {
  explain_paren_value_after(source, offset, "SERIALIZE")
}

fn explain_paren_value_after(source: &str, offset: TextSize, kw: &str) -> bool {
  let (slice, upper) = stmt_slice_upper(source, offset);
  let stmt = slice.trim_start();
  if !upper.starts_with("EXPLAIN") {
    return false;
  }
  let after = &stmt[upper.find("EXPLAIN").unwrap() + "EXPLAIN".len()..];
  let opens = after.matches('(').count();
  let closes = after.matches(')').count();
  if opens == 0 || opens <= closes {
    return false;
  }
  let last_tok =
    stmt.split(|c: char| c.is_whitespace() || c == ',' || c == '(').rfind(|s| !s.is_empty()).unwrap_or("");
  last_tok.eq_ignore_ascii_case(kw)
}

/// True when the cursor sits inside the `EXPLAIN (...)` options paren
/// at a fresh option-name slot (start of paren, or right after `,`).
fn explain_paren_expects_option(source: &str, offset: TextSize) -> bool {
  let (slice, upper) = stmt_slice_upper(source, offset);
  let stmt = slice.trim_start();
  if !upper.starts_with("EXPLAIN") {
    return false;
  }
  // Find the `(` opening the options paren after EXPLAIN and verify
  // it isn't closed before the cursor.
  let after = &stmt[upper.find("EXPLAIN").unwrap() + "EXPLAIN".len()..];
  let mut depth = 0i32;
  let mut saw_open = false;
  for c in after.chars() {
    match c {
      '(' => {
        depth += 1;
        saw_open = true;
      },
      ')' => depth -= 1,
      _ => {},
    }
  }
  if !saw_open || depth == 0 {
    return false;
  }
  // Cursor is inside the paren. The slot expects an option name at
  // the start or right after a comma -- detect a freshly-empty option
  // (last non-whitespace in stmt is `(` or `,`), or a partial option
  // word being typed.
  let trimmed = stmt.trim_end();
  let last = trimmed.chars().last();
  if matches!(last, Some('(') | Some(',')) {
    return true;
  }
  // Partial word being typed at end -- only consider it an option
  // slot if no `=` has been typed since the last comma/( inside the
  // paren (otherwise we're in the value slot).
  if !stmt.ends_with(char::is_whitespace) {
    let inner_start = after.rfind('(').unwrap_or(0);
    let last_comma_or_open =
      after[inner_start..].rfind([',', '(']).unwrap_or(0);
    let since = &after[inner_start + last_comma_or_open..];
    if !since.contains('=') {
      return true;
    }
  }
  false
}

/// True when the statement starts with `EXPLAIN` (optionally followed
/// by `(...)` options, `ANALYZE`, `VERBOSE`) and the cursor is at the
/// statement-start slot waiting for the inner SQL command.
fn explain_expects_statement(source: &str, offset: TextSize) -> bool {
  let (slice, upper) = stmt_slice_upper(source, offset);
  let stmt = slice.trim_start();
  if !upper.starts_with("EXPLAIN") {
    return false;
  }
  // Walk past EXPLAIN, the optional paren-options block, and the
  // ANALYZE / VERBOSE modifiers. After those, the cursor must be at
  // whitespace (no inner statement keyword typed yet) and on no
  // recognised statement starter (otherwise the SELECT-walk has
  // already moved us into a real phase).
  let bytes = stmt.as_bytes();
  let mut i = upper.find("EXPLAIN").unwrap_or(0) + "EXPLAIN".len();
  let skip_ws = |b: &[u8], mut p: usize| {
    while p < b.len() && b[p].is_ascii_whitespace() {
      p += 1;
    }
    p
  };
  i = skip_ws(bytes, i);
  // Optional paren-options.
  if i < bytes.len() && bytes[i] == b'(' {
    let mut depth = 1i32;
    i += 1;
    while i < bytes.len() && depth > 0 {
      match bytes[i] {
        b'(' => depth += 1,
        b')' => depth -= 1,
        _ => {},
      }
      if depth == 0 {
        break;
      }
      i += 1;
    }
    if i >= bytes.len() {
      return false; // still inside paren list
    }
    i += 1;
    i = skip_ws(bytes, i);
  }
  // Optional ANALYZE / VERBOSE modifiers (any order).
  loop {
    let rest_upper = stmt[i..].trim_start().to_ascii_uppercase();
    if let Some(stripped) = rest_upper.strip_prefix("ANALYZE") {
      // Make sure ANALYZE is a whole word.
      let consumed = "ANALYZE".len();
      let lead = stmt[i..].len() - stmt[i..].trim_start().len();
      let next = i + lead + consumed;
      if next == bytes.len() || !bytes[next].is_ascii_alphanumeric() && bytes[next] != b'_' {
        i = next;
        i = skip_ws(bytes, i);
        let _ = stripped;
        continue;
      }
    }
    if let Some(stripped) = rest_upper.strip_prefix("VERBOSE") {
      let consumed = "VERBOSE".len();
      let lead = stmt[i..].len() - stmt[i..].trim_start().len();
      let next = i + lead + consumed;
      if next == bytes.len() || !bytes[next].is_ascii_alphanumeric() && bytes[next] != b'_' {
        i = next;
        i = skip_ws(bytes, i);
        let _ = stripped;
        continue;
      }
    }
    break;
  }
  // At this point only the inner statement keyword could remain. If
  // the cursor sits before any keyword (i.e., i == end of stmt), we're
  // in the statement-starter slot.
  i == bytes.len()
}

/// True when the cursor sits at the return-type slot of a
/// `CREATE [OR REPLACE] FUNCTION/PROCEDURE ... RETURNS <cursor>`
/// statement. Used to surface types-only instead of the broad menu.
fn create_function_expects_return_type(source: &str, offset: TextSize) -> bool {
  let (slice, upper) = stmt_slice_upper(source, offset);
  let stmt = slice.as_str();
  // Must be a CREATE [OR REPLACE] FUNCTION/PROCEDURE statement.
  let has_create_fn = upper.contains("CREATE FUNCTION")
    || upper.contains("CREATE OR REPLACE FUNCTION")
    || upper.contains("CREATE PROCEDURE")
    || upper.contains("CREATE OR REPLACE PROCEDURE");
  if !has_create_fn {
    return false;
  }
  let Some(rets_at) = upper.rfind("RETURNS") else {
    return false;
  };
  // Require word boundary before RETURNS.
  let bytes = stmt.as_bytes();
  if rets_at > 0 && (bytes[rets_at - 1].is_ascii_alphanumeric() || bytes[rets_at - 1] == b'_') {
    return false;
  }
  let after = &stmt[rets_at + "RETURNS".len()..];
  // After RETURNS, allow optional SETOF or TABLE keyword.
  let after_trim = after.trim_start();
  if after_trim.is_empty() {
    return true;
  }
  // Strip leading SETOF (single optional modifier).
  let after_setof = after_trim.strip_prefix("SETOF").or_else(|| after_trim.strip_prefix("setof"));
  let rest = after_setof.unwrap_or(after_trim);
  // After modifier whitespace, the only allowed token is the type
  // name -- accept either an empty trailing slot or a single partial
  // identifier being typed.
  let words: Vec<&str> = rest.split_ascii_whitespace().collect();
  match words.len() {
    0 => true,
    1 => !rest.ends_with(char::is_whitespace),
    _ => false,
  }
}

/// True when the cursor sits right after one of the SET sub-keywords
/// inside an ALTER COLUMN clause: `SET DEFAULT|STATISTICS|STORAGE|
/// COMPRESSION <cursor>`. These slots take literals / specific tokens
/// (integers, storage names, compression methods, or freeform DEFAULT
/// expressions) -- the ALTER-table action menu is wrong here.
/// True when the cursor sits right after the `AS` of a CREATE VIEW
/// header (no body content typed yet). Used to narrow the Phase::Start
/// menu to just SELECT / WITH / VALUES / TABLE -- DDL keywords like
/// CREATE TABLE aren't legal as a view body.
fn at_create_view_body_start(source: &str, offset: TextSize) -> bool {
  let (slice, _) = stmt_slice_upper(source, offset);
  let upper = slice.trim().to_ascii_uppercase();
  if !upper.contains("VIEW") || !upper.ends_with("AS") {
    return false;
  }
  upper.as_bytes().starts_with(b"CREATE")
}

/// True when the cursor in a `DROP <class> [IF EXISTS] <name> <cursor>`
/// statement sits right after a finished target name, so the next
/// legal tokens are CASCADE / RESTRICT (or `;`). Returns false when:
/// - the leading keyword isn't DROP
/// - we haven't seen a class keyword yet (TABLE / VIEW / ...)
/// - we haven't seen a name token after the class
/// - the cursor sits inside the name (no trailing whitespace)
/// - the user just typed a comma (continuing the target list)
fn drop_target_trailing_slot(source: &str, offset: TextSize) -> bool {
  let (slice, _) = stmt_slice_upper(source, offset);
  // Must end with whitespace -- otherwise the cursor is mid-name.
  if !slice.ends_with(char::is_whitespace) {
    return false;
  }
  let upper = slice.trim().to_ascii_uppercase();
  if !upper.starts_with("DROP") {
    return false;
  }
  let words: Vec<&str> = upper.split_ascii_whitespace().collect();
  if words.len() < 3 {
    return false; // need at least DROP <CLASS> <NAME>
  }
  if words[0] != "DROP" {
    return false;
  }
  // Class keyword (allow 2-word classes like MATERIALIZED VIEW, FOREIGN TABLE).
  let two_word_classes = [("MATERIALIZED", "VIEW"), ("FOREIGN", "TABLE")];
  let one_word_classes: &[&str] = &[
    "TABLE",
    "VIEW",
    "INDEX",
    "SEQUENCE",
    "SCHEMA",
    "FUNCTION",
    "PROCEDURE",
    "TRIGGER",
    "TYPE",
    "ROLE",
    "USER",
    "DATABASE",
    "EXTENSION",
    "POLICY",
    "DOMAIN",
    "AGGREGATE",
    "CAST",
    "COLLATION",
    "OPERATOR",
    "RULE",
    "TABLESPACE",
    "SUBSCRIPTION",
    "PUBLICATION",
    "SERVER",
  ];
  let mut idx = 1usize;
  if idx + 1 < words.len()
    && two_word_classes.iter().any(|(a, b)| words[idx] == *a && words[idx + 1] == *b)
  {
    idx += 2;
  } else if one_word_classes.contains(&words[idx]) {
    idx += 1;
  } else {
    return false;
  }
  // Optional `IF EXISTS`.
  if idx + 1 < words.len() && words[idx] == "IF" && words[idx + 1] == "EXISTS" {
    idx += 2;
  }
  // Need at least one name word after that.
  if idx >= words.len() {
    return false;
  }
  // Last token must not be a continuation hint (comma) and must look
  // like an identifier (no leading `(`, no CASCADE/RESTRICT already).
  let last = words[words.len() - 1];
  if last == "CASCADE" || last == "RESTRICT" || last.ends_with(',') {
    return false;
  }
  // Check the raw character before trailing whitespace -- if it's a
  // `,`, the user is continuing the list, not done with the target.
  let trim_end = slice.trim_end();
  if trim_end.ends_with(',') {
    return false;
  }
  true
}

/// Common DEFAULT-expression suggestions surfaced anywhere a column
/// default value is expected (CREATE TABLE column entry, ALTER TABLE
/// ADD COLUMN ... DEFAULT, ALTER COLUMN SET DEFAULT). Curated rather
/// than dumping the function set so the menu stays useful.
const DEFAULT_EXPRESSION_SUGGESTIONS: &[(&str, &str)] = &[
  ("NULL", "NULL default (column is nullable)"),
  ("CURRENT_TIMESTAMP", "transaction-start timestamp (timestamptz)"),
  ("CURRENT_DATE", "transaction-start date"),
  ("now()", "transaction-start timestamptz -- same as CURRENT_TIMESTAMP"),
  ("gen_random_uuid()", "random UUID v4 (requires pgcrypto; built-in since PG 13)"),
  ("FALSE", "literal false"),
  ("TRUE", "literal true"),
  ("0", "literal integer zero"),
  ("''", "empty string"),
];

/// True when the cursor in a CREATE TABLE column entry sits in the
/// DEFAULT-expression slot (the most recently committed word in the
/// current column entry, after the column type, is DEFAULT).
fn ctl_column_constraint_after_default(source: &str, offset: TextSize) -> bool {
  let pos: usize = (u32::from(offset) as usize).min(source.len());
  let bytes = source.as_bytes();
  if pos == 0 {
    return false;
  }
  // Last word -- skip whitespace, read identifier letters back.
  let mut end = pos;
  while end > 0 && bytes[end - 1].is_ascii_whitespace() {
    end -= 1;
  }
  // Trailing whitespace required (otherwise user is mid-word).
  if end == pos {
    return false;
  }
  let mut start = end;
  while start > 0 {
    let b = bytes[start - 1];
    if b.is_ascii_alphanumeric() || b == b'_' {
      start -= 1;
    } else {
      break;
    }
  }
  if start == end {
    return false;
  }
  let word = source[start..end].to_ascii_uppercase();
  word == "DEFAULT"
}

/// True when the cursor in an `INSERT INTO ... VALUES (...)` statement
/// sits AFTER a closed top-level tuple (paren depth 0, last
/// meaningful char is `)`, no RETURNING / ON CONFLICT typed yet).
/// The legal continuations here are `,` (another tuple), `RETURNING`,
/// `ON CONFLICT`, or `;` -- not arbitrary expressions, and not DEFAULT.
fn insert_after_values_tuple(source: &str, offset: TextSize) -> bool {
  let (slice_owned, upper) = stmt_slice_upper(source, offset);
  let slice = slice_owned.as_str();
  if !upper.contains("VALUES") || !upper.trim_start().starts_with("INSERT") {
    return false;
  }
  // After RETURNING or ON CONFLICT, the user is in those slots --
  // not the post-tuple slot.
  if upper.contains("RETURNING") || upper.contains("ON CONFLICT") {
    return false;
  }
  // Last non-whitespace char must be `)`.
  let trimmed = slice.trim_end();
  if !trimmed.ends_with(')') {
    return false;
  }
  // Paren-depth at end must be 0 (top-level).
  let bytes = slice.as_bytes();
  let mut depth: i32 = 0;
  let mut i = 0;
  while i < bytes.len() {
    let c = bytes[i];
    if c == b'\'' {
      i += 1;
      while i < bytes.len() && bytes[i] != b'\'' {
        i += 1;
      }
      i = (i + 1).min(bytes.len());
      continue;
    }
    if c == b'(' {
      depth += 1;
    } else if c == b')' {
      depth -= 1;
    }
    i += 1;
  }
  depth == 0
}

/// True when the cursor in `ALTER TABLE <t> ADD COLUMN <name> <type>
/// DEFAULT <cursor>` sits in the DEFAULT-expression slot. Recognises
/// the cursor as right after the `DEFAULT` keyword (whitespace-only
/// between word and cursor).
fn alter_table_add_column_after_default(source: &str, offset: TextSize) -> bool {
  let (slice_owned, upper) = stmt_slice_upper(source, offset);
  let slice = slice_owned.as_str();
  if !upper.contains("ALTER TABLE") || !upper.contains("ADD COLUMN") {
    return false;
  }
  // Last word must be DEFAULT, followed by whitespace at the cursor.
  let trimmed = slice.trim_end();
  if trimmed.len() == slice.len() {
    return false; // no trailing whitespace -> mid-word
  }
  let upper_trim = trimmed.to_ascii_uppercase();
  if !upper_trim.ends_with("DEFAULT") {
    return false;
  }
  // Confirm DEFAULT is a whole word (not the tail of FOO_DEFAULT etc).
  let end = trimmed.len();
  if end == 7 {
    return true;
  }
  let prev = trimmed.as_bytes()[end - 8];
  !(prev.is_ascii_alphanumeric() || prev == b'_')
}

/// True when the cursor sits right after a leading `CREATE` (optionally
/// `CREATE OR REPLACE` / `CREATE [TEMP|TEMPORARY|UNLOGGED|GLOBAL|LOCAL]`)
/// at the start of a statement. Returns the static list of object-type
/// keywords PG accepts after CREATE.
fn after_top_level_create_keyword(source: &str, offset: TextSize) -> Option<&'static [(&'static str, &'static str)]> {
  let pos: usize = (u32::from(offset) as usize).min(source.len());
  let bytes = source.as_bytes();
  let (slice_owned, _) = stmt_slice_upper(source, offset);
  let slice = slice_owned.trim();
  let upper = slice.to_ascii_uppercase();
  // Accept CREATE / CREATE OR REPLACE / CREATE TEMP / etc as the only
  // tokens so far. Reject if a class keyword has already been typed.
  let words: Vec<&str> = upper.split_ascii_whitespace().collect();
  if words.is_empty() || words[0] != "CREATE" {
    return None;
  }
  let modifiers = ["OR", "REPLACE", "TEMP", "TEMPORARY", "UNLOGGED", "GLOBAL", "LOCAL", "UNIQUE", "MATERIALIZED"];
  for w in &words[1..] {
    if !modifiers.contains(w) {
      return None;
    }
  }
  if pos < bytes.len() && !bytes[pos].is_ascii_whitespace() {
    return None;
  }
  // Compute which modifiers have already been typed so we can prune
  // them from the suggestion list. Without this, after `CREATE OR `
  // the LSP would still suggest `OR REPLACE` (duplicating what the
  // user already typed); after `CREATE OR REPLACE ` it would still
  // offer TEMP/UNLOGGED (which can't legally follow OR REPLACE).
  let has_or = words.contains(&"OR");
  let has_replace = words.contains(&"REPLACE");
  let has_or_replace = has_or && has_replace;
  let has_temp = words.contains(&"TEMP") || words.contains(&"TEMPORARY");
  let has_unlogged = words.contains(&"UNLOGGED");
  let has_unique = words.contains(&"UNIQUE");
  let has_materialized = words.contains(&"MATERIALIZED");
  // When ONLY `OR` is typed (no REPLACE yet), the only legal next
  // word is REPLACE. Short-circuit to that single suggestion.
  if has_or && !has_replace {
    return Some(&[("REPLACE", "CREATE OR REPLACE -- replace existing function/view/trigger/etc")]);
  }
  // When `CREATE OR REPLACE` is fully typed, modifiers like TEMP /
  // UNLOGGED / etc can't follow -- only object-kind keywords are
  // legal. Same when TEMP/TEMPORARY/UNLOGGED already typed.
  let suggest_modifiers = !has_or_replace && !has_temp && !has_unlogged && !has_unique && !has_materialized;
  // Static slice + runtime filter doesn't compose; build a dynamic
  // Vec, leak its strings, and return a slice. But we need static
  // lifetime for the existing API. Instead we keep the static menu
  // and filter via a thread-local Vec elsewhere. For now, the easiest
  // path is to return a curated subset via match.
  // Subsets keyed on which modifiers are still legal:
  if !suggest_modifiers {
    // Only object-kind keywords legal after OR REPLACE / TEMP / etc.
    return Some(after_create_object_kinds_only());
  }
  Some(&[
    ("OR REPLACE", "CREATE OR REPLACE -- replace existing function/view/trigger/etc"),
    ("TEMP", "CREATE TEMP TABLE/VIEW/SEQUENCE -- session-scoped"),
    ("TEMPORARY", "CREATE TEMPORARY TABLE/VIEW/SEQUENCE -- session-scoped"),
    ("UNLOGGED", "CREATE UNLOGGED TABLE -- skipped from WAL"),
    ("GLOBAL", "CREATE GLOBAL TEMPORARY -- SQL standard alias"),
    ("LOCAL", "CREATE LOCAL TEMPORARY -- SQL standard alias"),
    ("UNIQUE", "CREATE UNIQUE INDEX -- enforce uniqueness"),
    ("MATERIALIZED", "CREATE MATERIALIZED VIEW <name> AS SELECT ..."),
    ("TABLE", "CREATE TABLE [IF NOT EXISTS] <name> (...)"),
    ("VIEW", "CREATE VIEW <name> AS SELECT ..."),
    ("MATERIALIZED VIEW", "CREATE MATERIALIZED VIEW <name> AS ..."),
    ("INDEX", "CREATE [UNIQUE] INDEX [CONCURRENTLY] [IF NOT EXISTS] <name> ON <table>(<cols>)"),
    ("UNIQUE INDEX", "CREATE UNIQUE INDEX ..."),
    ("SEQUENCE", "CREATE SEQUENCE [IF NOT EXISTS] <name>"),
    ("SCHEMA", "CREATE SCHEMA [IF NOT EXISTS] <name>"),
    ("FUNCTION", "CREATE [OR REPLACE] FUNCTION <name>(args) RETURNS ... AS $$ ... $$ LANGUAGE ..."),
    ("PROCEDURE", "CREATE [OR REPLACE] PROCEDURE <name>(args) ..."),
    ("TRIGGER", "CREATE [OR REPLACE] TRIGGER <name> {BEFORE|AFTER} <event> ON <table>"),
    ("TYPE", "CREATE TYPE <name> AS (...) | AS ENUM (...) | AS RANGE (...)"),
    ("ROLE", "CREATE ROLE <name> [WITH ...]"),
    ("USER", "CREATE USER <name> [WITH PASSWORD '...']"),
    ("DATABASE", "CREATE DATABASE <name> [OWNER <role>]"),
    ("EXTENSION", "CREATE EXTENSION [IF NOT EXISTS] <name>"),
    ("POLICY", "CREATE POLICY <name> ON <table> FOR ... TO ... USING (...)"),
    ("DOMAIN", "CREATE DOMAIN <name> AS <type> [CHECK (...)]"),
    ("AGGREGATE", "CREATE AGGREGATE <name>(arg_types) (SFUNC=..., STYPE=...)"),
    ("CAST", "CREATE CAST (src AS dst) WITH FUNCTION <fn> | WITHOUT FUNCTION"),
    ("COLLATION", "CREATE COLLATION <name> (...)"),
    ("OPERATOR", "CREATE OPERATOR <op> (...)"),
    ("RULE", "CREATE RULE <name> AS ON <event> TO <table> DO ..."),
    ("TABLESPACE", "CREATE TABLESPACE <name> LOCATION '<path>'"),
    ("SUBSCRIPTION", "CREATE SUBSCRIPTION <name> CONNECTION '<conn>' PUBLICATION <pub>"),
    ("PUBLICATION", "CREATE PUBLICATION <name> FOR TABLE ..."),
    ("FOREIGN TABLE", "CREATE FOREIGN TABLE <name> (...) SERVER <srv>"),
    ("SERVER", "CREATE SERVER <name> FOREIGN DATA WRAPPER <fdw>"),
    ("EVENT TRIGGER", "CREATE EVENT TRIGGER <name> ON <event> EXECUTE FUNCTION <fn>()"),
    ("TEXT SEARCH CONFIGURATION", "CREATE TEXT SEARCH CONFIGURATION <name> (PARSER=...)"),
    ("TEXT SEARCH DICTIONARY", "CREATE TEXT SEARCH DICTIONARY <name> (TEMPLATE=...)"),
    ("FOREIGN DATA WRAPPER", "CREATE FOREIGN DATA WRAPPER <name>"),
    ("CONVERSION", "CREATE [DEFAULT] CONVERSION <name> FOR '<src>' TO '<dst>' FROM <fn>"),
    ("TRANSFORM", "CREATE TRANSFORM FOR <type> LANGUAGE <lang> (...)"),
    ("USER MAPPING", "CREATE USER MAPPING FOR <role> SERVER <srv> OPTIONS (...)"),
    ("OPERATOR CLASS", "CREATE OPERATOR CLASS <name> FOR TYPE <type> USING <am> AS ..."),
    ("OPERATOR FAMILY", "CREATE OPERATOR FAMILY <name> USING <am>"),
    ("ACCESS METHOD", "CREATE ACCESS METHOD <name> TYPE {INDEX|TABLE} HANDLER <fn>"),
    ("STATISTICS", "CREATE STATISTICS [IF NOT EXISTS] <name> [(<kinds>)] ON <cols> FROM <tbl>"),
    ("LANGUAGE", "CREATE [OR REPLACE] [TRUSTED] [PROCEDURAL] LANGUAGE <name> HANDLER <fn>"),
    ("GROUP", "CREATE GROUP <name> [WITH <attr>...] -- SQL-standard alias for ROLE"),
    ("ROUTINE", "CREATE [OR REPLACE] ROUTINE <name>(args) ..."),
    ("TEXT SEARCH PARSER", "CREATE TEXT SEARCH PARSER <name> (START=..., GETTOKEN=..., END=..., LEXTYPES=...)"),
    ("TEXT SEARCH TEMPLATE", "CREATE TEXT SEARCH TEMPLATE <name> (INIT=..., LEXIZE=...)"),
  ])
}

/// Object-kind keywords legal after `CREATE OR REPLACE`, `CREATE
/// TEMP`, `CREATE UNLOGGED`, etc. -- i.e. modifiers (OR REPLACE, TEMP,
/// UNIQUE, ...) are already consumed and the only legal next word is
/// the kind of object being created. Same static list as the full
/// menu minus the modifier rows.
fn after_create_object_kinds_only() -> &'static [(&'static str, &'static str)] {
  &[
    ("TABLE", "CREATE TABLE [IF NOT EXISTS] <name> (...)"),
    ("VIEW", "CREATE VIEW <name> AS SELECT ..."),
    ("MATERIALIZED VIEW", "CREATE MATERIALIZED VIEW <name> AS ..."),
    ("INDEX", "CREATE [UNIQUE] INDEX [CONCURRENTLY] [IF NOT EXISTS] <name> ON <table>(<cols>)"),
    ("UNIQUE INDEX", "CREATE UNIQUE INDEX ..."),
    ("SEQUENCE", "CREATE SEQUENCE [IF NOT EXISTS] <name>"),
    ("SCHEMA", "CREATE SCHEMA [IF NOT EXISTS] <name>"),
    ("FUNCTION", "CREATE [OR REPLACE] FUNCTION <name>(args) RETURNS ... AS $$ ... $$ LANGUAGE ..."),
    ("PROCEDURE", "CREATE [OR REPLACE] PROCEDURE <name>(args) ..."),
    ("TRIGGER", "CREATE [OR REPLACE] TRIGGER <name> {BEFORE|AFTER} <event> ON <table>"),
    ("TYPE", "CREATE TYPE <name> AS (...) | AS ENUM (...) | AS RANGE (...)"),
    ("ROLE", "CREATE ROLE <name> [WITH ...]"),
    ("USER", "CREATE USER <name> [WITH PASSWORD '...']"),
    ("DATABASE", "CREATE DATABASE <name> [OWNER <role>]"),
    ("EXTENSION", "CREATE EXTENSION [IF NOT EXISTS] <name>"),
    ("POLICY", "CREATE POLICY <name> ON <table> FOR ... TO ... USING (...)"),
    ("DOMAIN", "CREATE DOMAIN <name> AS <type> [CHECK (...)]"),
    ("AGGREGATE", "CREATE AGGREGATE <name>(arg_types) (SFUNC=..., STYPE=...)"),
    ("CAST", "CREATE CAST (src AS dst) WITH FUNCTION <fn> | WITHOUT FUNCTION"),
    ("COLLATION", "CREATE COLLATION <name> (...)"),
    ("OPERATOR", "CREATE OPERATOR <op> (...)"),
    ("RULE", "CREATE RULE <name> AS ON <event> TO <table> DO ..."),
    ("TABLESPACE", "CREATE TABLESPACE <name> LOCATION '<path>'"),
    ("SUBSCRIPTION", "CREATE SUBSCRIPTION <name> CONNECTION '<conn>' PUBLICATION <pub>"),
    ("PUBLICATION", "CREATE PUBLICATION <name> FOR TABLE ..."),
    ("FOREIGN TABLE", "CREATE FOREIGN TABLE <name> (...) SERVER <srv>"),
    ("SERVER", "CREATE SERVER <name> FOREIGN DATA WRAPPER <fdw>"),
    ("EVENT TRIGGER", "CREATE EVENT TRIGGER <name> ON <event> EXECUTE FUNCTION <fn>()"),
    ("TEXT SEARCH CONFIGURATION", "CREATE TEXT SEARCH CONFIGURATION <name> (PARSER=...)"),
    ("TEXT SEARCH DICTIONARY", "CREATE TEXT SEARCH DICTIONARY <name> (TEMPLATE=...)"),
    ("FOREIGN DATA WRAPPER", "CREATE FOREIGN DATA WRAPPER <name>"),
    ("CONVERSION", "CREATE [DEFAULT] CONVERSION <name> FOR '<src>' TO '<dst>' FROM <fn>"),
    ("TRANSFORM", "CREATE TRANSFORM FOR <type> LANGUAGE <lang> (...)"),
    ("USER MAPPING", "CREATE USER MAPPING FOR <role> SERVER <srv> OPTIONS (...)"),
    ("OPERATOR CLASS", "CREATE OPERATOR CLASS <name> FOR TYPE <type> USING <am> AS ..."),
    ("OPERATOR FAMILY", "CREATE OPERATOR FAMILY <name> USING <am>"),
    ("ACCESS METHOD", "CREATE ACCESS METHOD <name> TYPE {INDEX|TABLE} HANDLER <fn>"),
    ("STATISTICS", "CREATE STATISTICS [IF NOT EXISTS] <name> [(<kinds>)] ON <cols> FROM <tbl>"),
    ("LANGUAGE", "CREATE [OR REPLACE] [TRUSTED] [PROCEDURAL] LANGUAGE <name> HANDLER <fn>"),
    ("GROUP", "CREATE GROUP <name> [WITH <attr>...] -- SQL-standard alias for ROLE"),
    ("ROUTINE", "CREATE [OR REPLACE] ROUTINE <name>(args) ..."),
    ("TEXT SEARCH PARSER", "CREATE TEXT SEARCH PARSER <name> (START=..., GETTOKEN=..., END=..., LEXTYPES=...)"),
    ("TEXT SEARCH TEMPLATE", "CREATE TEXT SEARCH TEMPLATE <name> (INIT=..., LEXIZE=...)"),
  ]
}

/// True when the cursor sits in the name slot right after a leading
/// `CREATE <CLASS>` (so the body's `(` hasn't appeared yet). Returns
/// the optional clarifier keywords PG accepts there -- IF NOT EXISTS,
/// CONCURRENTLY for indexes, ONLY for ALTER, etc.
fn create_class_expects_if_not_exists(
  source: &str,
  offset: TextSize,
) -> Option<&'static [(&'static str, &'static str)]> {
  if cursor_not_at_ws_boundary(source, offset) {
    return None;
  }
  let (slice_owned, _) = stmt_slice_upper(source, offset);
  let slice = slice_owned.trim();
  let upper = slice.to_ascii_uppercase();
  let words: Vec<&str> = upper.split_ascii_whitespace().collect();
  if words.is_empty() || words[0] != "CREATE" {
    return None;
  }
  // Match: CREATE [OR REPLACE | TEMP | UNLOGGED | UNIQUE | MATERIALIZED ...] <CLASS> <cursor>
  // where <CLASS> is one of the recognised class keywords and nothing
  // follows it yet (i.e. the user is about to type the name).
  let classes = [
    "TABLE",
    "VIEW",
    "INDEX",
    "SEQUENCE",
    "SCHEMA",
    "FUNCTION",
    "PROCEDURE",
    "TRIGGER",
    "TYPE",
    "ROLE",
    "USER",
    "DATABASE",
    "EXTENSION",
    "POLICY",
    "DOMAIN",
    "AGGREGATE",
    "COLLATION",
    "RULE",
    "TABLESPACE",
    "SUBSCRIPTION",
    "PUBLICATION",
    "SERVER",
  ];
  if !classes.contains(words.last().unwrap_or(&"")) {
    return None;
  }
  Some(&[
    ("IF NOT EXISTS", "CREATE ... IF NOT EXISTS <name> -- skip silently if it already exists"),
  ])
}

/// True when the cursor sits directly after a leading `ALTER` keyword
/// with no class token typed yet -- emit the class menu PG accepts.
fn after_top_level_alter_keyword(source: &str, offset: TextSize) -> Option<&'static [(&'static str, &'static str)]> {
  let pos: usize = (u32::from(offset) as usize).min(source.len());
  let bytes = source.as_bytes();
  let (slice_owned, _) = stmt_slice_upper(source, offset);
  let slice = slice_owned.trim();
  let upper = slice.to_ascii_uppercase();
  if upper != "ALTER" {
    return None;
  }
  if pos < bytes.len() && !bytes[pos].is_ascii_whitespace() {
    return None;
  }
  Some(&[
    ("TABLE", "ALTER TABLE [IF EXISTS] [ONLY] <name> ..."),
    ("VIEW", "ALTER VIEW [IF EXISTS] <name> ..."),
    ("MATERIALIZED VIEW", "ALTER MATERIALIZED VIEW [IF EXISTS] <name> ..."),
    ("INDEX", "ALTER INDEX [IF EXISTS] <name> ..."),
    ("SEQUENCE", "ALTER SEQUENCE [IF EXISTS] <name> ..."),
    ("SCHEMA", "ALTER SCHEMA <name> RENAME TO ..."),
    ("FUNCTION", "ALTER FUNCTION <name>(args) ..."),
    ("PROCEDURE", "ALTER PROCEDURE <name>(args) ..."),
    ("ROUTINE", "ALTER ROUTINE <name>(args) ..."),
    ("TRIGGER", "ALTER TRIGGER <name> ON <table> ..."),
    ("TYPE", "ALTER TYPE <name> ADD VALUE ... | RENAME ..."),
    ("DOMAIN", "ALTER DOMAIN <name> SET DEFAULT ... | DROP DEFAULT ..."),
    ("ROLE", "ALTER ROLE <name> WITH ..."),
    ("USER", "ALTER USER <name> WITH ..."),
    ("GROUP", "ALTER GROUP <name> ..."),
    ("DATABASE", "ALTER DATABASE <name> ..."),
    ("EXTENSION", "ALTER EXTENSION <name> UPDATE [TO ...]"),
    ("POLICY", "ALTER POLICY <name> ON <table> ..."),
    ("AGGREGATE", "ALTER AGGREGATE <name>(args) ..."),
    ("CAST", "ALTER CAST (src AS dst) ..."),
    ("COLLATION", "ALTER COLLATION <name> ..."),
    ("OPERATOR", "ALTER OPERATOR <op>(args) ..."),
    ("OPERATOR CLASS", "ALTER OPERATOR CLASS <name> USING <am> ..."),
    ("OPERATOR FAMILY", "ALTER OPERATOR FAMILY <name> USING <am> ..."),
    ("RULE", "ALTER RULE <name> ON <table> RENAME TO ..."),
    ("TABLESPACE", "ALTER TABLESPACE <name> ..."),
    ("SUBSCRIPTION", "ALTER SUBSCRIPTION <name> ..."),
    ("PUBLICATION", "ALTER PUBLICATION <name> ..."),
    ("FOREIGN TABLE", "ALTER FOREIGN TABLE <name> ..."),
    ("FOREIGN DATA WRAPPER", "ALTER FOREIGN DATA WRAPPER <name> ..."),
    ("SERVER", "ALTER SERVER <name> ..."),
    ("LANGUAGE", "ALTER LANGUAGE <name> RENAME TO ..."),
    ("CONVERSION", "ALTER CONVERSION <name> RENAME TO ..."),
    ("EVENT TRIGGER", "ALTER EVENT TRIGGER <name> ..."),
    ("STATISTICS", "ALTER STATISTICS <name> ..."),
    ("TEXT SEARCH CONFIGURATION", "ALTER TEXT SEARCH CONFIGURATION <name> ..."),
    ("TEXT SEARCH DICTIONARY", "ALTER TEXT SEARCH DICTIONARY <name> ..."),
    ("TEXT SEARCH PARSER", "ALTER TEXT SEARCH PARSER <name> ..."),
    ("TEXT SEARCH TEMPLATE", "ALTER TEXT SEARCH TEMPLATE <name> ..."),
    ("SYSTEM", "ALTER SYSTEM SET <param> = ..."),
    ("DEFAULT PRIVILEGES", "ALTER DEFAULT PRIVILEGES [FOR ROLE <r>] [IN SCHEMA <s>] {GRANT|REVOKE} ..."),
    ("LARGE OBJECT", "ALTER LARGE OBJECT <oid> OWNER TO <role>"),
    ("USER MAPPING", "ALTER USER MAPPING FOR <role> SERVER <srv> OPTIONS (...)"),
  ])
}

/// True when the cursor sits directly after a leading `DROP` keyword
/// at the start of a statement (no other meaningful tokens between
/// DROP and the cursor). Returns the static (keyword, doc) list of
/// object types the user can drop. None when DROP isn't the leading
/// keyword or another token has already been typed after it.
/// SHOW <guc> / SET <guc> / RESET <guc> -- emit common Postgres GUCs.
fn show_or_set_guc_names(source: &str, offset: TextSize) -> Option<&'static [(&'static str, &'static str)]> {
  if cursor_not_at_ws_boundary(source, offset) {
    return None;
  }
  let (slice_owned, _) = stmt_slice_upper(source, offset);
  let slice = slice_owned.trim();
  let upper = slice.to_ascii_uppercase();
  let words: Vec<&str> = upper.split_ascii_whitespace().collect();
  if words.is_empty() {
    return None;
  }
  let want_guc = (words.len() == 1 && matches!(words[0], "SHOW" | "RESET"))
    || (words.len() == 2 && words[0] == "RESET" && words[1] != "ALL")
    || (words.len() == 2 && words[0] == "SET" && matches!(words[1], "LOCAL" | "SESSION"))
    || (words.len() == 2 && words[0] == "SET" && !matches!(words[1], "LOCAL" | "SESSION" | "TRANSACTION" | "ROLE" | "CONSTRAINTS" | "SESSION_AUTHORIZATION" | "TIME"))
    || (words.len() == 3 && words[0] == "SET" && matches!(words[1], "LOCAL" | "SESSION"))
    // ALTER {ROLE | USER | DATABASE} <name> SET <cursor>
    //   -> GUC name slot. The trailing SET-followup `TABLESPACE` /
    //   `SCHEMA` / etc. are accepted by the parser too, but the common
    //   case is per-role / per-db GUC override, so surface the GUC list.
    || (words.len() >= 4
        && words[0] == "ALTER"
        && matches!(words[1], "ROLE" | "USER" | "DATABASE")
        && words.last() == Some(&"SET"));
  if !want_guc {
    return None;
  }
  Some(&[
    ("ALL", "RESET ALL / SHOW ALL -- every GUC at once"),
    ("search_path", "search_path = 'public, schema2' -- schema resolution order"),
    ("timezone", "timezone = 'UTC'"),
    ("statement_timeout", "statement_timeout = '5s' -- abort long statements"),
    ("lock_timeout", "lock_timeout = '5s'"),
    ("idle_in_transaction_session_timeout", "idle_in_transaction_session_timeout = '60s'"),
    ("work_mem", "work_mem = '64MB' -- per-sort/hash memory"),
    ("maintenance_work_mem", "maintenance_work_mem = '256MB'"),
    ("shared_buffers", "shared_buffers = '2GB' (postgresql.conf only)"),
    ("effective_cache_size", "effective_cache_size = '8GB' -- planner hint"),
    ("default_transaction_isolation", "default_transaction_isolation = 'repeatable read'"),
    ("default_transaction_read_only", "default_transaction_read_only = on"),
    ("default_transaction_deferrable", "default_transaction_deferrable = on"),
    ("application_name", "application_name = 'myapp'"),
    ("log_statement", "log_statement = 'all' | 'mod' | 'ddl' | 'none'"),
    ("log_min_duration_statement", "log_min_duration_statement = '1s'"),
    ("client_encoding", "client_encoding = 'UTF8'"),
    ("DateStyle", "DateStyle = 'ISO, YMD'"),
    ("IntervalStyle", "IntervalStyle = 'iso_8601'"),
    ("synchronous_commit", "synchronous_commit = on | off | local | remote_write | remote_apply"),
    ("enable_seqscan", "enable_seqscan = off"),
    ("enable_hashjoin", "enable_hashjoin = off"),
    ("enable_mergejoin", "enable_mergejoin = off"),
    ("enable_nestloop", "enable_nestloop = off"),
    ("random_page_cost", "random_page_cost = 1.1 -- SSD-friendly"),
    ("seq_page_cost", "seq_page_cost = 1.0"),
    ("cpu_tuple_cost", "cpu_tuple_cost = 0.01"),
    ("jit", "jit = off"),
    ("max_parallel_workers_per_gather", "max_parallel_workers_per_gather = 4"),
    ("row_security", "row_security = on | off"),
    ("transaction_isolation", "transaction_isolation = 'serializable' | 'repeatable read' | 'read committed' (per-tx)"),
    ("transaction_read_only", "transaction_read_only = on | off (per-tx)"),
    ("transaction_deferrable", "transaction_deferrable = on (only with SERIALIZABLE + READ ONLY)"),
    ("vacuum_buffer_usage_limit", "vacuum_buffer_usage_limit = '<size>' (PG16+)"),
    ("temp_buffers", "temp_buffers = '8MB'"),
    ("temp_file_limit", "temp_file_limit = '-1' -- per-session temp disk cap"),
    ("max_parallel_maintenance_workers", "max_parallel_maintenance_workers = <n>"),
    ("max_parallel_workers", "max_parallel_workers = <n>"),
    ("min_parallel_index_scan_size", "min_parallel_index_scan_size = '512kB'"),
    ("min_parallel_table_scan_size", "min_parallel_table_scan_size = '8MB'"),
    ("parallel_setup_cost", "parallel_setup_cost = 1000"),
    ("parallel_tuple_cost", "parallel_tuple_cost = 0.1"),
    ("plan_cache_mode", "plan_cache_mode = 'auto' | 'force_custom_plan' | 'force_generic_plan'"),
    ("commit_delay", "commit_delay = <us>"),
    ("commit_siblings", "commit_siblings = <n>"),
    ("force_parallel_mode", "force_parallel_mode = off | on | regress (debug)"),
    ("session_replication_role", "session_replication_role = 'origin' | 'replica' | 'local'"),
    ("array_nulls", "array_nulls = on | off"),
    ("backslash_quote", "backslash_quote = 'safe_encoding' | 'on' | 'off'"),
    ("escape_string_warning", "escape_string_warning = on | off"),
    ("standard_conforming_strings", "standard_conforming_strings = on"),
    ("log_lock_waits", "log_lock_waits = on -- log waits longer than deadlock_timeout"),
    ("log_temp_files", "log_temp_files = '0' -- log every temp file (any size when 0)"),
    ("log_checkpoints", "log_checkpoints = on"),
    ("log_connections", "log_connections = on"),
    ("log_disconnections", "log_disconnections = on"),
    ("log_autovacuum_min_duration", "log_autovacuum_min_duration = '5s'"),
    ("log_replication_commands", "log_replication_commands = on"),
    ("debug_print_parse", "debug_print_parse = on (debug)"),
    ("debug_print_plan", "debug_print_plan = on (debug)"),
    ("debug_print_rewritten", "debug_print_rewritten = on (debug)"),
    ("tcp_keepalives_idle", "tcp_keepalives_idle = 60"),
    ("tcp_keepalives_interval", "tcp_keepalives_interval = 10"),
    ("tcp_keepalives_count", "tcp_keepalives_count = 3"),
    ("tcp_user_timeout", "tcp_user_timeout = 30000"),
    ("client_min_messages", "client_min_messages = notice | warning | error | log | debug1..5"),
    ("log_min_messages", "log_min_messages = warning | error | log | debug1..5"),
    ("log_min_error_statement", "log_min_error_statement = error"),
    ("log_error_verbosity", "log_error_verbosity = default | terse | verbose"),
    ("log_line_prefix", "log_line_prefix = '%m [%p] %u@%d '"),
    ("scram_iterations", "scram_iterations = 4096 (PG16+)"),
    ("password_encryption", "password_encryption = scram-sha-256"),
    ("ssl", "ssl = on"),
    ("hot_standby_feedback", "hot_standby_feedback = on"),
    ("max_standby_streaming_delay", "max_standby_streaming_delay = '30s'"),
    ("max_locks_per_transaction", "max_locks_per_transaction = 64 (postgresql.conf only)"),
    ("max_pred_locks_per_transaction", "max_pred_locks_per_transaction = 64"),
    ("max_connections", "max_connections = 100 (postgresql.conf only)"),
    ("max_prepared_transactions", "max_prepared_transactions = 0 (2PC; postgresql.conf only)"),
    ("max_replication_slots", "max_replication_slots = 10 (postgresql.conf only)"),
    ("max_wal_senders", "max_wal_senders = 10"),
    ("wal_compression", "wal_compression = on | off | pglz | lz4 | zstd"),
    ("wal_level", "wal_level = replica | logical (postgresql.conf only)"),
    ("archive_mode", "archive_mode = on | off | always"),
    ("checkpoint_timeout", "checkpoint_timeout = '5min'"),
    ("checkpoint_completion_target", "checkpoint_completion_target = 0.9"),
    ("default_toast_compression", "default_toast_compression = pglz | lz4"),
    ("default_table_access_method", "default_table_access_method = heap"),
    ("recovery_init_sync_method", "recovery_init_sync_method = fsync | syncfs (PG14+)"),
    ("restart_after_crash", "restart_after_crash = on"),
    ("xmlbinary", "xmlbinary = base64 | hex"),
    ("xmloption", "xmloption = content | document"),
  ])
}

/// SECURITY LABEL chain.
///   SECURITY LABEL [FOR <provider>] ON <object_kind> <name> IS '<label>'
fn security_label_next_keyword(
  source: &str,
  offset: TextSize,
) -> Option<&'static [(&'static str, &'static str)]> {
  if cursor_not_at_ws_boundary(source, offset) {
    return None;
  }
  let (_, upper) = stmt_slice_upper(source, offset);
  let words: Vec<&str> = upper.split_ascii_whitespace().collect();
  if words.len() < 2 || words[0] != "SECURITY" || words[1] != "LABEL" {
    return None;
  }
  let last = *words.last()?;
  if last == "LABEL" {
    return Some(&[("FOR", "FOR <provider>"), ("ON", "ON <object_kind> <name>")]);
  }
  if last == "FOR" {
    return None; // user types provider name
  }
  if last == "ON" {
    return Some(&[
      ("TABLE", "ON TABLE <name>"),
      ("COLUMN", "ON COLUMN <tbl>.<col>"),
      ("VIEW", "ON VIEW <name>"),
      ("MATERIALIZED VIEW", "ON MATERIALIZED VIEW <name>"),
      ("INDEX", "ON INDEX <name>"),
      ("SEQUENCE", "ON SEQUENCE <name>"),
      ("FUNCTION", "ON FUNCTION <name>(args)"),
      ("PROCEDURE", "ON PROCEDURE <name>(args)"),
      ("ROUTINE", "ON ROUTINE <name>(args)"),
      ("TYPE", "ON TYPE <name>"),
      ("DOMAIN", "ON DOMAIN <name>"),
      ("SCHEMA", "ON SCHEMA <name>"),
      ("ROLE", "ON ROLE <name>"),
      ("DATABASE", "ON DATABASE <name>"),
      ("TABLESPACE", "ON TABLESPACE <name>"),
      ("AGGREGATE", "ON AGGREGATE <name>(args)"),
      ("COLLATION", "ON COLLATION <name>"),
      ("EVENT TRIGGER", "ON EVENT TRIGGER <name>"),
      ("FOREIGN TABLE", "ON FOREIGN TABLE <name>"),
      ("LARGE OBJECT", "ON LARGE OBJECT <oid>"),
      ("LANGUAGE", "ON LANGUAGE <name>"),
      ("PUBLICATION", "ON PUBLICATION <name>"),
      ("SUBSCRIPTION", "ON SUBSCRIPTION <name>"),
    ]);
  }
  // After name -> IS.
  if words.contains(&"ON") && !words.contains(&"IS") && words.len() >= 5 {
    return Some(&[("IS", "IS '<label_text>' | IS NULL -- clear label")]);
  }
  // After IS -> common provider-specific label literal forms.
  if last == "IS" {
    return Some(&[
      ("NULL", "IS NULL -- clear the security label"),
      ("'unclassified'", "'unclassified'"),
      ("'confidential'", "'confidential'"),
      ("'secret'", "'secret'"),
      ("'top_secret'", "'top_secret'"),
    ]);
  }
  None
}

/// ALTER TABLE ATTACH PARTITION / DETACH PARTITION / INHERIT chain.
fn alter_table_attach_detach_next_keyword(
  source: &str,
  offset: TextSize,
) -> Option<&'static [(&'static str, &'static str)]> {
  if cursor_not_at_ws_boundary(source, offset) {
    return None;
  }
  let (_, upper) = stmt_slice_upper(source, offset);
  // Word-boundary check: `ALTER TABLESPACE` must not satisfy this.
  let words: Vec<&str> = upper.split_ascii_whitespace().collect();
  if words.first() != Some(&"ALTER") || words.get(1) != Some(&"TABLE") {
    return None;
  }
  let last = *words.last()?;
  if last == "ATTACH" || last == "DETACH" {
    return Some(&[("PARTITION", "PARTITION <child>")]);
  }
  // After ATTACH PARTITION <child> -> FOR VALUES menu.
  if words.contains(&"ATTACH") && words.contains(&"PARTITION") && words.len() >= 6 && last != "PARTITION" && last != "FOR" && last != "VALUES" && !words.contains(&"VALUES") {
    return Some(&[("FOR", "FOR VALUES { IN (...) | FROM (...) TO (...) | WITH (...) }"), ("DEFAULT", "DEFAULT -- catch-all partition")]);
  }
  // DETACH PARTITION <child> -> CONCURRENTLY / FINALIZE.
  if words.contains(&"DETACH") && words.contains(&"PARTITION") && words.len() >= 6 && !words.contains(&"CONCURRENTLY") && !words.contains(&"FINALIZE") {
    return Some(&[("CONCURRENTLY", "CONCURRENTLY -- detach without long lock"), ("FINALIZE", "FINALIZE -- complete a concurrent detach")]);
  }
  if last == "INHERIT" {
    return None; // user types parent table name
  }
  if last == "NO" && words.contains(&"ALTER") {
    return Some(&[("INHERIT", "NO INHERIT <parent> -- remove inheritance link")]);
  }
  // After ALTER TABLE <name> -> include INHERIT/NO INHERIT/ATTACH/DETACH in the action menu.
  if words.len() == 3 {
    return Some(&[
      ("ATTACH PARTITION", "ATTACH PARTITION <child> FOR VALUES ..."),
      ("DETACH PARTITION", "DETACH PARTITION <child> [CONCURRENTLY|FINALIZE]"),
      ("INHERIT", "INHERIT <parent> -- add inheritance link"),
      ("NO INHERIT", "NO INHERIT <parent> -- remove inheritance link"),
    ]);
  }
  None
}

/// PARTITION chain. Covers `CREATE TABLE <child> PARTITION OF <parent>
/// FOR VALUES {IN | FROM ... TO ... | WITH (...)}` and `PARTITION BY
/// {RANGE | LIST | HASH} (<cols>)`.
fn partition_next_keyword(source: &str, offset: TextSize) -> Option<&'static [(&'static str, &'static str)]> {
  if cursor_not_at_ws_boundary(source, offset) {
    return None;
  }
  let (_, upper) = stmt_slice_upper(source, offset);
  let words: Vec<&str> = upper.split_ascii_whitespace().collect();
  let last = *words.last()?;
  // `PARTITION OF <parent>` slot.
  if last == "PARTITION" {
    return Some(&[("OF", "PARTITION OF <parent> FOR VALUES ..."), ("BY", "PARTITION BY {RANGE|LIST|HASH} (<cols>)")]);
  }
  if last == "OF" && words.contains(&"PARTITION") {
    return None; // parent table name
  }
  // `FOR <cursor>` after PARTITION OF <parent>.
  if last == "FOR" && words.contains(&"PARTITION") && words.contains(&"OF") {
    return Some(&[("VALUES", "FOR VALUES { IN (...) | FROM (...) TO (...) | WITH (MODULUS ..., REMAINDER ...) }"), ("DEFAULT", "FOR VALUES DEFAULT -- catch-all partition")]);
  }
  if last == "VALUES" && words.contains(&"PARTITION") {
    return Some(&[
      ("IN", "IN (<v>[, ...]) -- list partition"),
      ("FROM", "FROM (<lo>) TO (<hi>) -- range partition"),
      ("WITH", "WITH (MODULUS <n>, REMAINDER <r>) -- hash partition"),
      ("DEFAULT", "DEFAULT -- catch-all"),
    ]);
  }
  // `PARTITION BY <cursor>` -> RANGE / LIST / HASH.
  if last == "BY" && words.contains(&"PARTITION") {
    return Some(&[
      ("RANGE", "PARTITION BY RANGE (<cols>)"),
      ("LIST", "PARTITION BY LIST (<col>)"),
      ("HASH", "PARTITION BY HASH (<col>)"),
    ]);
  }
  // `FOR VALUES FROM (<lo>) <cursor>` -> TO
  if last == ")"
    && words.contains(&"FROM")
    && words.contains(&"VALUES")
    && !words.contains(&"TO")
  {
    return Some(&[("TO", "TO (<hi>) -- upper bound of the range")]);
  }
  None
}

/// REFRESH MATERIALIZED VIEW <name> [WITH [NO] DATA] phase.
fn refresh_mv_next_keyword(
  source: &str,
  offset: TextSize,
) -> Option<&'static [(&'static str, &'static str)]> {
  if cursor_not_at_ws_boundary(source, offset) {
    return None;
  }
  let (_, upper) = stmt_slice_upper(source, offset);
  let trimmed = upper.trim_start();
  if !trimmed.starts_with("REFRESH MATERIALIZED VIEW") {
    return None;
  }
  let words: Vec<&str> = upper.split_ascii_whitespace().collect();
  let last = *words.last()?;
  // After the view name, suggest CONCURRENTLY (if not present yet) + WITH ...
  // After `WITH` -> DATA | NO DATA
  if last == "WITH" {
    return Some(&[
      ("DATA", "WITH DATA -- run the refresh query (default)"),
      ("NO DATA", "WITH NO DATA -- mark the MV invalid; useful before bulk reload"),
    ]);
  }
  if last == "NO" && words.contains(&"WITH") {
    return Some(&[("DATA", "NO DATA")]);
  }
  // After the name -- need >= 4 words (REFRESH MATERIALIZED VIEW <name>)
  if words.len() >= 4 && !matches!(last, "REFRESH" | "MATERIALIZED" | "VIEW" | "CONCURRENTLY") {
    let mut menu: Vec<(&str, &str)> = Vec::with_capacity(2);
    if !words.contains(&"CONCURRENTLY") {
      menu.push(("WITH DATA", "WITH DATA -- populate (default)"));
      menu.push(("WITH NO DATA", "WITH NO DATA -- mark invalid"));
    } else {
      menu.push(("WITH DATA", "WITH DATA"));
      menu.push(("WITH NO DATA", "WITH NO DATA"));
    }
    // Static return type required -- pick a hardcoded set.
    return Some(&[
      ("WITH DATA", "WITH DATA -- populate (default)"),
      ("WITH NO DATA", "WITH NO DATA -- mark invalid; refresh later"),
    ]);
  }
  None
}

/// `... AS IDENTITY ( <cursor>` -- sequence-option name slot in column
/// IDENTITY clause. Same option list as CREATE SEQUENCE.
fn identity_paren_next_keyword(
  source: &str,
  offset: TextSize,
) -> Option<&'static [(&'static str, &'static str)]> {
  if cursor_not_at_ws_boundary(source, offset)
    && !source[..offset.into()].ends_with('(')
    && !source[..offset.into()].ends_with(',')
  {
    return None;
  }
  let (slice_owned, upper) = stmt_slice_upper(source, offset);
  if !upper.contains("AS IDENTITY") && !upper.contains("AS  IDENTITY") {
    return None;
  }
  // Must be inside the paren immediately following `AS IDENTITY`.
  let at = upper.rfind("AS IDENTITY")?;
  let after = &slice_owned[at + "AS IDENTITY".len()..];
  let trimmed = after.trim_start();
  if !trimmed.starts_with('(') {
    return None;
  }
  let body = &trimmed[1..];
  let opens = body.matches('(').count();
  let closes = body.matches(')').count();
  if closes > opens {
    return None;
  }
  // Must NOT already be past the closing `)`.
  if body.ends_with(')') && opens == closes {
    return None;
  }
  Some(&[
    ("START WITH", "START [WITH] <n> -- first generated value"),
    ("INCREMENT BY", "INCREMENT [BY] <n> -- step size"),
    ("MINVALUE", "MINVALUE <n> | NO MINVALUE"),
    ("MAXVALUE", "MAXVALUE <n> | NO MAXVALUE"),
    ("CACHE", "CACHE <n> -- preallocate per-backend"),
    ("CYCLE", "CYCLE -- wrap around at MAXVALUE/MINVALUE"),
    ("NO CYCLE", "NO CYCLE -- raise error on exhaust (default)"),
    ("OWNED BY", "OWNED BY <table>.<col> | NONE -- drop sequence when col goes"),
  ])
}

/// CREATE TABLE AS / CREATE MATERIALIZED VIEW chain:
///   `... AS SELECT ... <cursor>` -> WITH DATA | WITH NO DATA
///   `... WITH <cursor>` -> DATA | NO DATA
///   `... WITH NO <cursor>` -> DATA
fn ctas_with_data_next_keyword(
  source: &str,
  offset: TextSize,
) -> Option<&'static [(&'static str, &'static str)]> {
  if cursor_not_at_ws_boundary(source, offset) {
    return None;
  }
  let (_, upper) = stmt_slice_upper(source, offset);
  let is_ctas = upper.starts_with("CREATE TABLE") && upper.contains(" AS ");
  let is_cmv = upper.starts_with("CREATE MATERIALIZED VIEW");
  if !(is_ctas || is_cmv) {
    return None;
  }
  let words: Vec<&str> = upper.split_ascii_whitespace().collect();
  let last = *words.last()?;
  if last == "NO" && words.contains(&"WITH") {
    return Some(&[("DATA", "NO DATA -- create the relation but skip the populate query")]);
  }
  if last == "WITH" {
    return Some(&[
      ("DATA", "WITH DATA -- run the populate query immediately (default)"),
      ("NO DATA", "WITH NO DATA -- create empty; populate later with REFRESH/INSERT"),
    ]);
  }
  // Only emit WITH options after `AS SELECT ...` or after the CMV body.
  if (is_ctas || is_cmv) && !words.contains(&"WITH") {
    // Heuristic: after a SELECT/VALUES body. Last token unlikely to be
    // a keyword we already chained. Skip if too few tokens.
    if words.len() >= 5 {
      return Some(&[("WITH DATA", "WITH DATA -- populate now"), ("WITH NO DATA", "WITH NO DATA -- create empty")]);
    }
  }
  None
}

/// `INSERT INTO <tbl> [(cols)] <cursor>` -> body shape menu.
fn insert_into_next_keyword(
  source: &str,
  offset: TextSize,
) -> Option<&'static [(&'static str, &'static str)]> {
  if cursor_not_at_ws_boundary(source, offset) {
    return None;
  }
  let (slice_owned, upper) = stmt_slice_upper(source, offset);
  let slice = slice_owned.as_str();
  let words: Vec<&str> = upper.split_ascii_whitespace().collect();
  if words.first() != Some(&"INSERT") {
    return None;
  }
  if !words.contains(&"INTO") || words.len() < 3 {
    return None;
  }
  // Only fire when body not yet typed (no VALUES/SELECT/DEFAULT).
  let has_body = words.contains(&"VALUES")
    || words.contains(&"SELECT")
    || words.contains(&"DEFAULT")
    || words.contains(&"CONFLICT");
  if has_body {
    return None;
  }
  // OVERRIDING SYSTEM/USER is a sub-clause that lives between the
  // (col-list) and VALUES; only defer to `insert_overriding_next_keyword`
  // while the sub-clause is mid-typing (last word OVERRIDING/SYSTEM/USER).
  // Once the user reaches VALUE, fall through so the body-shape menu fires.
  if words.contains(&"OVERRIDING")
    && matches!(words.last(), Some(&"OVERRIDING") | Some(&"SYSTEM") | Some(&"USER"))
  {
    return None;
  }
  // After (cols)? require balanced parens.
  let opens = slice.matches('(').count();
  let closes = slice.matches(')').count();
  if opens != closes {
    return None;
  }
  Some(&[
    ("VALUES", "VALUES (<v>[, ...])[, ...]"),
    ("SELECT", "INSERT INTO ... SELECT <cols> FROM <src>"),
    ("DEFAULT VALUES", "DEFAULT VALUES -- all columns take their declared defaults"),
    ("OVERRIDING", "OVERRIDING {SYSTEM|USER} VALUE -- identity-column override"),
    ("ON CONFLICT", "ON CONFLICT [(target)] DO ... -- upsert"),
    ("RETURNING", "RETURNING <cols>"),
  ])
}

/// `UPDATE <tbl> <cursor>` -> SET / FROM / WHERE / RETURNING.
fn update_from_set_next_keyword(
  source: &str,
  offset: TextSize,
) -> Option<&'static [(&'static str, &'static str)]> {
  if cursor_not_at_ws_boundary(source, offset) {
    return None;
  }
  let (_, upper) = stmt_slice_upper(source, offset);
  let words: Vec<&str> = upper.split_ascii_whitespace().collect();
  if words.first() != Some(&"UPDATE") {
    return None;
  }
  // Only fire when no SET typed yet AND at least one ident after UPDATE.
  if words.contains(&"SET") || words.len() < 3 {
    return None;
  }
  // Skip ONLY modifier or alias word.
  Some(&[
    ("SET", "SET <col> = <expr>[, ...]"),
    ("FROM", "FROM <other_tbl> -- multi-table UPDATE"),
    ("WHERE", "WHERE <predicate>"),
    ("RETURNING", "RETURNING <cols>"),
  ])
}

/// `DELETE FROM <tbl> <cursor>` -> USING / WHERE / RETURNING.
fn delete_using_next_keyword(
  source: &str,
  offset: TextSize,
) -> Option<&'static [(&'static str, &'static str)]> {
  if cursor_not_at_ws_boundary(source, offset) {
    return None;
  }
  let (_, upper) = stmt_slice_upper(source, offset);
  let words: Vec<&str> = upper.split_ascii_whitespace().collect();
  if words.first() != Some(&"DELETE") {
    return None;
  }
  if !words.contains(&"FROM") || words.len() < 3 {
    return None;
  }
  if words.contains(&"USING") || words.contains(&"WHERE") || words.contains(&"RETURNING") {
    return None;
  }
  Some(&[
    ("USING", "USING <other_tbl> -- multi-table DELETE"),
    ("WHERE", "WHERE <predicate>"),
    ("RETURNING", "RETURNING <cols>"),
  ])
}

/// TABLESAMPLE <method> (<args>) <cursor> -> REPEATABLE (<seed>).
fn tablesample_after_paren_next_keyword(
  source: &str,
  offset: TextSize,
) -> Option<&'static [(&'static str, &'static str)]> {
  if cursor_not_at_ws_boundary(source, offset) {
    return None;
  }
  let (slice_owned, upper) = stmt_slice_upper(source, offset);
  let slice = slice_owned.as_str();
  if !upper.contains("TABLESAMPLE") {
    return None;
  }
  // Must be past a closing `)` and method was already typed.
  let trimmed = slice.trim_end();
  if !trimmed.ends_with(')') {
    return None;
  }
  let words: Vec<&str> = upper.split_ascii_whitespace().collect();
  if !words.iter().any(|w| *w == "BERNOULLI" || *w == "SYSTEM") {
    return None;
  }
  if words.contains(&"REPEATABLE") {
    return None;
  }
  Some(&[("REPEATABLE", "REPEATABLE (<seed>) -- reproducible sample")])
}

/// WINDOW <name> AS ( <cursor> ) -> emit PARTITION BY / ORDER BY / frame.
fn window_clause_as_paren_keyword(
  source: &str,
  offset: TextSize,
) -> Option<&'static [(&'static str, &'static str)]> {
  let pos: usize = (u32::from(offset) as usize).min(source.len());
  let bytes = source.as_bytes();
  if pos < bytes.len() && !bytes[pos].is_ascii_whitespace() && bytes[pos] != b'(' {
    return None;
  }
  let (slice_owned, upper) = stmt_slice_upper(source, offset);
  let slice = slice_owned.as_str();
  // Accept either a WINDOW clause (`WINDOW name AS (...)`) or an inline
  // OVER (...) window specification. Both share the same body grammar.
  if !upper.contains("WINDOW") && !upper.contains("OVER") {
    return None;
  }
  // Detect a still-open paren -- both forms put the cursor inside the
  // window spec body.
  let opens = slice.matches('(').count();
  let closes = slice.matches(')').count();
  if opens == 0 || opens <= closes {
    return None;
  }
  let words: Vec<&str> = upper.split_ascii_whitespace().collect();
  let last = *words.last()?;
  if last == "PARTITION" {
    return Some(&[("BY", "PARTITION BY <expr>[, ...]")]);
  }
  if last == "ORDER" {
    return Some(&[("BY", "ORDER BY <expr>[, ...]")]);
  }
  // After RANGE/ROWS/GROUPS -> BETWEEN <bound> | <bound>
  if matches!(last, "RANGE" | "ROWS" | "GROUPS") {
    return Some(&[
      ("BETWEEN", "BETWEEN <a> AND <b>"),
      ("UNBOUNDED PRECEDING", "frame starts at partition start"),
      ("CURRENT ROW", "frame starts at the current row"),
    ]);
  }
  // After BETWEEN -> lower bound options
  if last == "BETWEEN" {
    return Some(&[
      ("UNBOUNDED PRECEDING", "frame starts at partition start"),
      ("CURRENT ROW", "current row"),
    ]);
  }
  // After AND inside a BETWEEN -> upper bound options
  if last == "AND" && words.contains(&"BETWEEN") {
    return Some(&[
      ("UNBOUNDED FOLLOWING", "frame ends at partition end"),
      ("CURRENT ROW", "current row"),
    ]);
  }
  // UNBOUNDED followups: PRECEDING (before AND) or FOLLOWING (after AND).
  if last == "UNBOUNDED" {
    let mut after_and = false;
    if let Some(b_idx) = words.iter().rposition(|w| *w == "BETWEEN")
      && words[b_idx + 1..].contains(&"AND")
    {
      after_and = true;
    }
    if after_and {
      return Some(&[("FOLLOWING", "UNBOUNDED FOLLOWING -- frame ends at partition end")]);
    }
    return Some(&[("PRECEDING", "UNBOUNDED PRECEDING -- frame starts at partition start")]);
  }
  // CURRENT followup: ROW.
  if last == "CURRENT" {
    return Some(&[("ROW", "CURRENT ROW -- the current row only")]);
  }
  // EXCLUDE bare: emit the four exclusion modes.
  if last == "EXCLUDE" {
    return Some(&[
      ("CURRENT ROW", "EXCLUDE CURRENT ROW -- skip the current row from the frame"),
      ("GROUP", "EXCLUDE GROUP -- skip the current row and all peers"),
      ("TIES", "EXCLUDE TIES -- skip the peers but keep the current row"),
      ("NO OTHERS", "EXCLUDE NO OTHERS -- default; no exclusion"),
    ]);
  }
  // After PRECEDING / FOLLOWING / CURRENT -- continue with AND or EXCLUDE.
  if matches!(last, "PRECEDING" | "FOLLOWING") {
    let mut needs_and = false;
    if let Some(b_idx) = words.iter().rposition(|w| *w == "BETWEEN")
      && !words[b_idx + 1..].contains(&"AND")
    {
      needs_and = true;
    }
    if needs_and {
      return Some(&[("AND", "BETWEEN <a> AND <b>")]);
    }
    return Some(&[
      ("EXCLUDE CURRENT ROW", "EXCLUDE CURRENT ROW"),
      ("EXCLUDE GROUP", "EXCLUDE GROUP"),
      ("EXCLUDE TIES", "EXCLUDE TIES"),
      ("EXCLUDE NO OTHERS", "EXCLUDE NO OTHERS (default)"),
    ]);
  }
  // Cursor at fresh slot inside the paren.
  let trimmed = slice.trim_end();
  let last_char = trimmed.chars().last();
  if matches!(last_char, Some('(') | Some(',')) || slice.ends_with(char::is_whitespace) {
    return Some(&[
      ("PARTITION BY", "PARTITION BY <expr>[, ...]"),
      ("ORDER BY", "ORDER BY <expr>[, ...]"),
      ("RANGE", "RANGE BETWEEN <a> AND <b>"),
      ("ROWS", "ROWS BETWEEN <a> AND <b>"),
      ("GROUPS", "GROUPS BETWEEN <a> AND <b>"),
    ]);
  }
  None
}

/// `... UNION / INTERSECT / EXCEPT <cursor>` -> ALL | DISTINCT | new
/// SELECT (handled elsewhere).
fn set_op_followup_next_keyword(
  source: &str,
  offset: TextSize,
) -> Option<&'static [(&'static str, &'static str)]> {
  if cursor_not_at_ws_boundary(source, offset) {
    return None;
  }
  let (_, upper) = stmt_slice_upper(source, offset);
  let words: Vec<&str> = upper.split_ascii_whitespace().collect();
  let last = *words.last()?;
  if !matches!(last, "UNION" | "INTERSECT" | "EXCEPT") {
    return None;
  }
  Some(&[
    ("ALL", "ALL -- keep duplicates (no implicit DISTINCT)"),
    ("DISTINCT", "DISTINCT -- explicit form of the default"),
    ("SELECT", "SELECT <next_query>"),
    ("VALUES", "VALUES (<row>) -- inline rows"),
  ])
}

/// GROUP BY GROUPING SETS / CUBE / ROLLUP chain.
/// True when the cursor sits inside `GROUP BY GROUPING SETS ( ( <cursor>
/// ... ) )` -- the inner tuple is a column-list slot, not an expression
/// context. Same shape for nested ROLLUP / CUBE tuples within
/// GROUPING SETS. Phase machine misreads the double paren as a function
/// call argument list, so the completion engine has to detect this
/// explicitly to avoid dumping the full function library.
fn grouping_sets_inner_paren_expects_column(source: &str, offset: TextSize) -> bool {
  let (_, upper) = stmt_slice_upper(source, offset);
  if !upper.contains("GROUPING SETS") {
    return false;
  }
  // Find GROUPING SETS, then count open-parens after it. A 2+ open
  // depth from the cursor's perspective means we're inside the inner
  // tuple ( ( ... | ... ) ).
  let after = match upper.rfind("GROUPING SETS") {
    Some(p) => &upper[p + "GROUPING SETS".len()..],
    None => return false,
  };
  let mut depth = 0i32;
  for b in after.bytes() {
    match b {
      b'(' => depth += 1,
      b')' => depth -= 1,
      _ => {},
    }
  }
  depth >= 2
}

fn group_by_set_op_next_keyword(
  source: &str,
  offset: TextSize,
) -> Option<&'static [(&'static str, &'static str)]> {
  if cursor_not_at_ws_boundary(source, offset) {
    return None;
  }
  let (_, upper) = stmt_slice_upper(source, offset);
  if !upper.contains("GROUP BY") {
    return None;
  }
  let words: Vec<&str> = upper.split_ascii_whitespace().collect();
  let last = *words.last()?;
  if last == "BY" && words.contains(&"GROUP") {
    return Some(&[
      ("GROUPING SETS", "GROUPING SETS ((<a>), (<a>,<b>), ()) -- explicit sets"),
      ("CUBE", "CUBE (<a>, <b>) -- all subsets"),
      ("ROLLUP", "ROLLUP (<a>, <b>) -- nested hierarchy"),
    ]);
  }
  if last == "GROUPING" {
    return Some(&[("SETS", "GROUPING SETS ((<cols>), ...)")]);
  }
  None
}

/// SELECT trailing FETCH/OFFSET chain.
///   ... FETCH {FIRST|NEXT} <n> {ROW|ROWS} {ONLY | WITH TIES}
///   ... OFFSET <n> {ROW|ROWS}
fn select_fetch_offset_next_keyword(
  source: &str,
  offset: TextSize,
) -> Option<&'static [(&'static str, &'static str)]> {
  if cursor_not_at_ws_boundary(source, offset) {
    return None;
  }
  let (_, upper) = stmt_slice_upper(source, offset);
  let words: Vec<&str> = upper.split_ascii_whitespace().collect();
  let last = *words.last()?;
  // Only fire when statement begins with a query verb.
  let starts_query = upper.starts_with("SELECT") || upper.starts_with("WITH") || upper.starts_with("VALUES") || upper.starts_with("TABLE");
  if !starts_query {
    return None;
  }
  // FETCH <cursor> -> FIRST|NEXT
  if last == "FETCH" {
    return Some(&[("FIRST", "FETCH FIRST <n> {ROW|ROWS} {ONLY | WITH TIES}"), ("NEXT", "FETCH NEXT <n> {ROW|ROWS} {ONLY | WITH TIES}")]);
  }
  if matches!(last, "FIRST" | "NEXT") {
    // After a count (we can't easily tell if a count was typed), emit ROW/ROWS as next-best.
    return Some(&[("ROW", "ROW {ONLY | WITH TIES}"), ("ROWS", "ROWS {ONLY | WITH TIES}")]);
  }
  if matches!(last, "ROW" | "ROWS") && words.contains(&"FETCH") {
    return Some(&[("ONLY", "ONLY -- standard"), ("WITH TIES", "WITH TIES -- include rows that tie on ORDER BY (PG 13+)")]);
  }
  if matches!(last, "ROW" | "ROWS") && words.contains(&"OFFSET") {
    return Some(&[("FETCH", "FETCH {FIRST|NEXT} <n> {ROW|ROWS} {ONLY | WITH TIES}")]);
  }
  if last == "OFFSET" {
    return None; // user types the offset count
  }
  if last == "WITH" {
    return Some(&[("TIES", "WITH TIES -- include peers tied on ORDER BY")]);
  }
  None
}

/// INSERT OVERRIDING chain.
///   INSERT INTO ... [OVERRIDING {SYSTEM|USER} VALUE] ...
fn insert_overriding_next_keyword(
  source: &str,
  offset: TextSize,
) -> Option<&'static [(&'static str, &'static str)]> {
  if cursor_not_at_ws_boundary(source, offset) {
    return None;
  }
  let (_, upper) = stmt_slice_upper(source, offset);
  let words: Vec<&str> = upper.split_ascii_whitespace().collect();
  if !upper.starts_with("INSERT") {
    return None;
  }
  let last = *words.last()?;
  if last == "OVERRIDING" {
    return Some(&[("SYSTEM VALUE", "OVERRIDING SYSTEM VALUE -- replace identity"), ("USER VALUE", "OVERRIDING USER VALUE -- accept user's value")]);
  }
  if (last == "SYSTEM" || last == "USER") && words.contains(&"OVERRIDING") {
    return Some(&[("VALUE", "VALUE -- close the OVERRIDING clause")]);
  }
  None
}

/// ON CONFLICT chain.
///   ... ON CONFLICT [ <conflict_target> | ON CONSTRAINT <name> ] DO {NOTHING | UPDATE SET ... [WHERE ...]}
/// `WITH [RECURSIVE] <name> [(<col>, ...)] AS [MATERIALIZED|NOT MATERIALIZED] (<body>) <cursor>`
/// -- next legal token is the main query (SELECT/INSERT/UPDATE/DELETE/MERGE/VALUES)
/// or `,` to add another CTE.
fn with_cte_after_paren_close_next_keyword(
  source: &str,
  offset: TextSize,
) -> Option<&'static [(&'static str, &'static str)]> {
  if cursor_not_at_ws_boundary(source, offset) {
    return None;
  }
  let (slice_owned, upper) = stmt_slice_upper(source, offset);
  let slice = slice_owned.as_str();
  // Must be a WITH statement.
  if !upper.trim_start().starts_with("WITH ")
    && !upper.trim_start().starts_with("WITH\t")
    && !upper.trim_start().starts_with("WITH\n")
  {
    return None;
  }
  // Last non-whitespace char must be `)` and depth must be balanced.
  let trimmed = slice.trim_end();
  if !trimmed.ends_with(')') {
    return None;
  }
  let opens = upper.matches('(').count();
  let closes = upper.matches(')').count();
  if opens == 0 || opens != closes {
    return None;
  }
  // Suppress when a top-level main verb has already been typed.
  let words: Vec<&str> = upper.split_ascii_whitespace().collect();
  if words.iter().any(|w| matches!(*w, "SELECT" | "INSERT" | "UPDATE" | "DELETE" | "MERGE" | "VALUES" | "TABLE")) {
    return None;
  }
  Some(&[
    ("SELECT", "SELECT <cols> FROM ... -- main query"),
    ("INSERT INTO", "INSERT INTO <tbl> ... -- main query"),
    ("UPDATE", "UPDATE <tbl> SET ... -- main query"),
    ("DELETE FROM", "DELETE FROM <tbl> ... -- main query"),
    ("MERGE INTO", "MERGE INTO <tbl> USING ... -- main query"),
    ("VALUES", "VALUES (<row>) -- inline-rows main query"),
    ("TABLE", "TABLE <name> -- shorthand for SELECT * FROM <name>"),
    (",", "<cte_name> AS (<body>) -- continue CTE list"),
  ])
}

fn on_conflict_next_keyword(source: &str, offset: TextSize) -> Option<&'static [(&'static str, &'static str)]> {
  if cursor_not_at_ws_boundary(source, offset) {
    return None;
  }
  let (_, upper) = stmt_slice_upper(source, offset);
  if !upper.contains("ON CONFLICT") {
    return None;
  }
  let words: Vec<&str> = upper.split_ascii_whitespace().collect();
  let last = *words.last()?;
  // `... ON CONFLICT <cursor>` -> target form options.
  if last == "CONFLICT" {
    return Some(&[
      ("ON CONSTRAINT", "ON CONFLICT ON CONSTRAINT <name>"),
      ("(", "ON CONFLICT (<col>[, ...]) -- expression target"),
      ("DO NOTHING", "ON CONFLICT DO NOTHING"),
      ("DO UPDATE", "ON CONFLICT DO UPDATE SET <col>=<val>[, ...]"),
    ]);
  }
  if last == "DO" {
    return Some(&[("NOTHING", "DO NOTHING -- skip the conflicting row"), ("UPDATE", "DO UPDATE SET <col>=<val>[, ...]")]);
  }
  if last == "UPDATE" && words.contains(&"CONFLICT") {
    return Some(&[("SET", "SET <col> = <val>[, ...]")]);
  }
  None
}

/// VACUUM ( <opt> <cursor> ) value slot -- when last word is an option
/// that takes a boolean / value, suggest the legal values.
fn vacuum_paren_value_keyword(
  source: &str,
  offset: TextSize,
) -> Option<&'static [(&'static str, &'static str)]> {
  if cursor_not_at_ws_boundary(source, offset) {
    return None;
  }
  let (slice_owned, upper) = stmt_slice_upper(source, offset);
  let slice = slice_owned.as_str();
  let words: Vec<&str> = upper.split_ascii_whitespace().collect();
  if words.first() != Some(&"VACUUM") && words.first() != Some(&"ANALYZE") {
    return None;
  }
  // Cursor must sit inside the paren options list.
  let opens = slice.matches('(').count();
  let closes = slice.matches(')').count();
  if opens == 0 || opens <= closes {
    return None;
  }
  let last_raw = *words.last()?;
  let last = last_raw.trim_start_matches(['(', ',']);
  let boolean_opts = [
    "FULL", "FREEZE", "VERBOSE", "ANALYZE", "DISABLE_PAGE_SKIPPING", "SKIP_LOCKED",
    "TRUNCATE", "PROCESS_TOAST", "PROCESS_MAIN", "SKIP_DATABASE_STATS", "ONLY_DATABASE_STATS",
  ];
  if boolean_opts.contains(&last) {
    return Some(&[("true", "true -- enable"), ("false", "false -- disable")]);
  }
  if last == "INDEX_CLEANUP" {
    return Some(&[("AUTO", "AUTO -- decide based on table state (default)"), ("ON", "ON -- always clean up indexes"), ("OFF", "OFF -- skip index cleanup")]);
  }
  // BUFFER_USAGE_LIMIT expects a size literal, not a boolean.
  // PARALLEL expects an integer, not a boolean.
  None
}

/// PREPARE chain. `PREPARE <name> [(<arg_types>)] AS <statement>`.
fn prepare_next_keyword(source: &str, offset: TextSize) -> Option<&'static [(&'static str, &'static str)]> {
  if cursor_not_at_ws_boundary(source, offset) {
    return None;
  }
  let (slice_owned, upper) = stmt_slice_upper(source, offset);
  let slice = slice_owned.as_str();
  let words: Vec<&str> = upper.split_ascii_whitespace().collect();
  if words.first() != Some(&"PREPARE") {
    return None;
  }
  let last = *words.last()?;
  if last == "AS" {
    return Some(&[
      ("SELECT", "AS SELECT ..."),
      ("INSERT", "AS INSERT INTO ..."),
      ("UPDATE", "AS UPDATE ..."),
      ("DELETE", "AS DELETE ..."),
      ("VALUES", "AS VALUES (...)"),
    ]);
  }
  // After the (args) paren or right after name -> AS slot.
  let opens = slice.matches('(').count();
  let closes = slice.matches(')').count();
  if words.len() >= 2 && opens == closes && !words.contains(&"AS") {
    return Some(&[("AS", "AS <statement>")]);
  }
  None
}

/// DECLARE cursor phase chain (post-name).
///   DECLARE <name> [BINARY] [INSENSITIVE | ASENSITIVE] [[NO] SCROLL]
///                  CURSOR [{WITH | WITHOUT} HOLD] FOR <query>
fn declare_cursor_next_keyword(
  source: &str,
  offset: TextSize,
) -> Option<&'static [(&'static str, &'static str)]> {
  if cursor_not_at_ws_boundary(source, offset) {
    return None;
  }
  let (_, upper) = stmt_slice_upper(source, offset);
  let words: Vec<&str> = upper.split_ascii_whitespace().collect();
  if words.first() != Some(&"DECLARE") {
    return None;
  }
  let last = *words.last()?;
  if last == "WITH" || last == "WITHOUT" {
    return Some(&[("HOLD", "{WITH|WITHOUT} HOLD -- keep cursor open across commits")]);
  }
  if last == "NO" {
    return Some(&[("SCROLL", "NO SCROLL -- forbid backward fetch")]);
  }
  if last == "CURSOR" {
    return Some(&[
      ("WITH HOLD", "WITH HOLD -- survive COMMIT"),
      ("WITHOUT HOLD", "WITHOUT HOLD -- default"),
      ("FOR", "FOR <query>"),
    ]);
  }
  if last == "DECLARE" {
    return None; // user types cursor name
  }
  // After name token -> modifiers.
  if words.len() == 2 {
    return Some(&[
      ("BINARY", "BINARY -- binary protocol"),
      ("INSENSITIVE", "INSENSITIVE -- materialise the result"),
      ("ASENSITIVE", "ASENSITIVE -- default"),
      ("SCROLL", "SCROLL -- allow PRIOR/BACKWARD fetch"),
      ("NO SCROLL", "NO SCROLL -- forbid backward fetch"),
      ("CURSOR", "CURSOR FOR <query>"),
    ]);
  }
  // After modifiers but before CURSOR -> chain more modifiers / CURSOR.
  if !words.contains(&"CURSOR") && words.len() >= 3 {
    return Some(&[
      ("BINARY", "BINARY"),
      ("INSENSITIVE", "INSENSITIVE"),
      ("ASENSITIVE", "ASENSITIVE"),
      ("SCROLL", "SCROLL"),
      ("NO SCROLL", "NO SCROLL"),
      ("CURSOR", "CURSOR [WITH HOLD] FOR <query>"),
    ]);
  }
  None
}

/// TRUNCATE chain.
///   TRUNCATE [TABLE] [ONLY] <tbl>[, ...] [RESTART IDENTITY | CONTINUE IDENTITY] [CASCADE | RESTRICT]
fn truncate_next_keyword(source: &str, offset: TextSize) -> Option<&'static [(&'static str, &'static str)]> {
  if cursor_not_at_ws_boundary(source, offset) {
    return None;
  }
  let (_, upper) = stmt_slice_upper(source, offset);
  let words: Vec<&str> = upper.split_ascii_whitespace().collect();
  if words.first() != Some(&"TRUNCATE") {
    return None;
  }
  let last = *words.last()?;
  if matches!(last, "RESTART" | "CONTINUE") {
    return Some(&[("IDENTITY", "IDENTITY -- act on sequence-backed columns")]);
  }
  if last == "IDENTITY" {
    return Some(&[("CASCADE", "CASCADE -- also truncate dependent FK tables"), ("RESTRICT", "RESTRICT -- refuse if dependents exist (default)")]);
  }
  // After table list -> trailing-clause menu.
  if words.len() >= 2 && !words.contains(&"CASCADE") && !words.contains(&"RESTRICT") && !words.contains(&"RESTART") && !words.contains(&"CONTINUE") {
    return Some(&[
      ("RESTART IDENTITY", "RESTART IDENTITY -- reset owned sequences"),
      ("CONTINUE IDENTITY", "CONTINUE IDENTITY -- keep current sequence values (default)"),
      ("CASCADE", "CASCADE -- also truncate FK-dependent tables"),
      ("RESTRICT", "RESTRICT -- refuse if dependents exist (default)"),
    ]);
  }
  None
}

/// FETCH / MOVE cursor direction.
/// SET ROLE / SET SESSION AUTHORIZATION chain.
fn set_role_auth_next_keyword(
  source: &str,
  offset: TextSize,
) -> Option<&'static [(&'static str, &'static str)]> {
  if cursor_not_at_ws_boundary(source, offset) {
    return None;
  }
  let (_, upper) = stmt_slice_upper(source, offset);
  let words: Vec<&str> = upper.split_ascii_whitespace().collect();
  if words.first() != Some(&"SET") {
    return None;
  }
  // `SET` alone -> common settable kinds.
  if words.len() == 1 {
    return None; // let other detectors / guc menu fire
  }
  // SET ROLE <cursor>
  if words == ["SET", "ROLE"] {
    return Some(&[("NONE", "SET ROLE NONE -- revert to login role")]);
  }
  // SET SESSION <cursor>
  if words == ["SET", "SESSION"] {
    return Some(&[
      ("AUTHORIZATION", "SET SESSION AUTHORIZATION <role>"),
      ("CHARACTERISTICS", "SET SESSION CHARACTERISTICS AS TRANSACTION ..."),
    ]);
  }
  if words == ["SET", "SESSION", "AUTHORIZATION"] {
    return Some(&[("DEFAULT", "SET SESSION AUTHORIZATION DEFAULT -- restore login role")]);
  }
  if words == ["SET", "SESSION", "CHARACTERISTICS"] {
    return Some(&[("AS TRANSACTION", "AS TRANSACTION ISOLATION LEVEL ...")]);
  }
  // `SET` <cursor> after target name -- after ROLE/AUTHORIZATION name typed.
  None
}

/// GRANT / REVOKE follow-up keyword chain. After the privilege list,
/// suggest ON, then object class, then TO (GRANT) or FROM (REVOKE),
/// then WITH GRANT/ADMIN OPTION or CASCADE/RESTRICT.
fn grant_revoke_followup_next_keyword(
  source: &str,
  offset: TextSize,
) -> Option<&'static [(&'static str, &'static str)]> {
  if cursor_not_at_ws_boundary(source, offset) {
    return None;
  }
  let (_, upper) = stmt_slice_upper(source, offset);
  let words: Vec<&str> = upper.split_ascii_whitespace().collect();
  let first = *words.first()?;
  if !matches!(first, "GRANT" | "REVOKE") {
    return None;
  }
  let last = *words.last()?;
  // ON <cursor> -- object class menu.
  if last == "ON" {
    return Some(&[
      ("TABLE", "ON TABLE <name>[, ...]"),
      ("SEQUENCE", "ON SEQUENCE <name>"),
      ("DATABASE", "ON DATABASE <name>"),
      ("DOMAIN", "ON DOMAIN <name>"),
      ("FOREIGN DATA WRAPPER", "ON FOREIGN DATA WRAPPER <name>"),
      ("FOREIGN SERVER", "ON FOREIGN SERVER <name>"),
      ("FUNCTION", "ON FUNCTION <name>(arg_types)"),
      ("PROCEDURE", "ON PROCEDURE <name>(arg_types)"),
      ("ROUTINE", "ON ROUTINE <name>(arg_types) -- fn or proc"),
      ("LANGUAGE", "ON LANGUAGE <name>"),
      ("LARGE OBJECT", "ON LARGE OBJECT <oid>"),
      ("PARAMETER", "ON PARAMETER <guc_name> -- PG15+"),
      ("SCHEMA", "ON SCHEMA <name>"),
      ("TABLESPACE", "ON TABLESPACE <name>"),
      ("TYPE", "ON TYPE <name>"),
      ("ALL TABLES IN SCHEMA", "ON ALL TABLES IN SCHEMA <s>"),
      ("ALL SEQUENCES IN SCHEMA", "ON ALL SEQUENCES IN SCHEMA <s>"),
      ("ALL FUNCTIONS IN SCHEMA", "ON ALL FUNCTIONS IN SCHEMA <s>"),
      ("ALL ROUTINES IN SCHEMA", "ON ALL ROUTINES IN SCHEMA <s>"),
      ("ALL PROCEDURES IN SCHEMA", "ON ALL PROCEDURES IN SCHEMA <s>"),
    ]);
  }
  // TO <cursor> | FROM <cursor> -- role name slot (no kw).
  if matches!(last, "TO" | "FROM") {
    return None;
  }
  // WITH <cursor> after a TO <role> in GRANT.
  if last == "WITH" {
    if first == "GRANT" {
      return Some(&[
        ("GRANT OPTION", "WITH GRANT OPTION -- recipient may grant onward"),
        ("ADMIN OPTION", "WITH ADMIN OPTION -- recipient may manage role membership"),
      ]);
    }
    if first == "REVOKE" {
      return Some(&[
        ("GRANT OPTION FOR", "REVOKE GRANT OPTION FOR -- drop forwarding right, keep priv"),
        ("ADMIN OPTION FOR", "REVOKE ADMIN OPTION FOR -- drop admin right, keep membership"),
      ]);
    }
  }
  // GRANTED <cursor> -- only on REVOKE.
  if last == "GRANTED" && first == "REVOKE" {
    return Some(&[("BY", "GRANTED BY <role> -- target the grant from <role>")]);
  }
  // After a name (typical heuristic: when TO/FROM not yet seen and ON
  // already), suggest the next link.
  let has_on = words.contains(&"ON");
  let has_to_or_from = words.contains(&"TO") || words.contains(&"FROM");
  if has_on && !has_to_or_from {
    if first == "GRANT" {
      return Some(&[("TO", "TO <role>[, ...]")]);
    }
    return Some(&[("FROM", "FROM <role>[, ...]")]);
  }
  if has_on && has_to_or_from && first == "GRANT" && !words.contains(&"WITH") {
    return Some(&[("WITH GRANT OPTION", "WITH GRANT OPTION -- recipient may grant onward")]);
  }
  if has_on && has_to_or_from && first == "REVOKE" {
    let mut tail: Vec<(&str, &str)> = Vec::new();
    if !words.contains(&"CASCADE") && !words.contains(&"RESTRICT") {
      tail.push(("CASCADE", "CASCADE -- also drop dependent grants"));
      tail.push(("RESTRICT", "RESTRICT -- refuse if dependent grants exist (default)"));
    }
    if !words.contains(&"GRANTED") {
      tail.push(("GRANTED BY", "GRANTED BY <role>"));
    }
    if !tail.is_empty() {
      // Static return; pick the common case.
      return Some(&[
        ("CASCADE", "CASCADE -- also drop dependent grants"),
        ("RESTRICT", "RESTRICT -- refuse if dependent grants exist (default)"),
        ("GRANTED BY", "GRANTED BY <role>"),
      ]);
    }
  }
  None
}

fn fetch_move_direction_keyword(
  source: &str,
  offset: TextSize,
) -> Option<&'static [(&'static str, &'static str)]> {
  if cursor_not_at_ws_boundary(source, offset) {
    return None;
  }
  let (_, upper) = stmt_slice_upper(source, offset);
  let words: Vec<&str> = upper.split_ascii_whitespace().collect();
  if !matches!(words.first(), Some(&"FETCH") | Some(&"MOVE")) {
    return None;
  }
  let last = *words.last()?;
  // First slot: direction keyword.
  if words.len() == 1 {
    return Some(&[
      ("NEXT", "NEXT -- one row forward"),
      ("PRIOR", "PRIOR -- one row backward"),
      ("FIRST", "FIRST -- absolute 1"),
      ("LAST", "LAST -- absolute last"),
      ("ABSOLUTE", "ABSOLUTE <n>"),
      ("RELATIVE", "RELATIVE <n>"),
      ("FORWARD", "FORWARD [<n> | ALL]"),
      ("BACKWARD", "BACKWARD [<n> | ALL]"),
      ("ALL", "ALL -- every remaining row"),
      ("FROM", "FROM <cursor>"),
      ("IN", "IN <cursor>"),
    ]);
  }
  // After FORWARD/BACKWARD/ABSOLUTE/RELATIVE -- optional count or ALL
  // -- then FROM | IN. After a direction kw with no count yet, also
  // suggest FROM/IN/ALL.
  if matches!(last, "FORWARD" | "BACKWARD") {
    return Some(&[("ALL", "ALL -- every remaining row"), ("FROM", "FROM <cursor>"), ("IN", "IN <cursor>")]);
  }
  if matches!(last, "NEXT" | "PRIOR" | "FIRST" | "LAST" | "ALL" | "ABSOLUTE" | "RELATIVE") {
    return Some(&[("FROM", "FROM <cursor>"), ("IN", "IN <cursor>")]);
  }
  // After FROM | IN -- user types cursor name; no kw.
  None
}

/// LOCK TABLE chain.
///   LOCK [TABLE] [ONLY] <tbl>[, ...] [IN <mode> MODE] [NOWAIT]
fn lock_mode_next_keyword(source: &str, offset: TextSize) -> Option<&'static [(&'static str, &'static str)]> {
  if cursor_not_at_ws_boundary(source, offset) {
    return None;
  }
  let (_, upper) = stmt_slice_upper(source, offset);
  let words: Vec<&str> = upper.split_ascii_whitespace().collect();
  if words.first() != Some(&"LOCK") {
    return None;
  }
  let last = *words.last()?;
  if last == "IN" {
    return Some(&[
      ("ACCESS SHARE", "ACCESS SHARE MODE -- weakest; readers"),
      ("ROW SHARE", "ROW SHARE MODE -- SELECT FOR SHARE/UPDATE"),
      ("ROW EXCLUSIVE", "ROW EXCLUSIVE MODE -- INSERT/UPDATE/DELETE"),
      ("SHARE UPDATE EXCLUSIVE", "SHARE UPDATE EXCLUSIVE MODE -- VACUUM/ANALYZE"),
      ("SHARE", "SHARE MODE -- read-only after"),
      ("SHARE ROW EXCLUSIVE", "SHARE ROW EXCLUSIVE MODE"),
      ("EXCLUSIVE", "EXCLUSIVE MODE -- only ACCESS SHARE coexists"),
      ("ACCESS EXCLUSIVE", "ACCESS EXCLUSIVE MODE -- strongest; sole holder"),
    ]);
  }
  // After a mode word (ACCESS / ROW / SHARE / EXCLUSIVE family) -> MODE.
  if matches!(last, "ACCESS" | "ROW" | "SHARE" | "EXCLUSIVE" | "UPDATE") {
    return Some(&[
      ("MODE", "MODE -- close the IN <mode> MODE clause"),
      ("SHARE", "SHARE -- ACCESS SHARE / ROW SHARE / SHARE / SHARE ROW EXCLUSIVE / SHARE UPDATE EXCLUSIVE"),
      ("EXCLUSIVE", "EXCLUSIVE -- ACCESS EXCLUSIVE / ROW EXCLUSIVE / SHARE ROW EXCLUSIVE / SHARE UPDATE EXCLUSIVE"),
    ]);
  }
  if last == "MODE" {
    return Some(&[("NOWAIT", "NOWAIT -- fail instead of waiting on contention")]);
  }
  // After LOCK <tbl>[, ...] -> IN/NOWAIT clarifier.
  if words.len() >= 2 && !words.contains(&"IN") && !words.contains(&"NOWAIT") {
    return Some(&[("IN", "IN <mode> MODE [NOWAIT]"), ("NOWAIT", "NOWAIT -- fail rather than wait")]);
  }
  None
}

/// DEALLOCATE chain.
fn deallocate_next_keyword(source: &str, offset: TextSize) -> Option<&'static [(&'static str, &'static str)]> {
  if cursor_not_at_ws_boundary(source, offset) {
    return None;
  }
  let (_, upper) = stmt_slice_upper(source, offset);
  let words: Vec<&str> = upper.split_ascii_whitespace().collect();
  if words.first() != Some(&"DEALLOCATE") {
    return None;
  }
  if words.len() == 1 {
    return Some(&[("ALL", "DEALLOCATE ALL -- forget every prepared statement"), ("PREPARE", "DEALLOCATE PREPARE <name>")]);
  }
  None
}

/// ALTER DEFAULT PRIVILEGES chain.
///   ALTER DEFAULT PRIVILEGES [FOR {ROLE|USER} <role>[, ...]]
///       [IN SCHEMA <schema>[, ...]]
///       {GRANT | REVOKE} ... ON {TABLES|SEQUENCES|FUNCTIONS|TYPES|SCHEMAS}
///       {TO | FROM} {<role>|PUBLIC} [WITH GRANT OPTION | CASCADE | RESTRICT]
/// CREATE TRANSFORM chain.
///   CREATE [OR REPLACE] TRANSFORM FOR <type> LANGUAGE <lang> (
///       FROM SQL WITH FUNCTION <fn>(...), TO SQL WITH FUNCTION <fn>(...) )
fn create_transform_next_keyword(
  source: &str,
  offset: TextSize,
) -> Option<&'static [(&'static str, &'static str)]> {
  if cursor_not_at_ws_boundary(source, offset) {
    return None;
  }
  let (slice_owned, upper) = stmt_slice_upper(source, offset);
  let slice = slice_owned.as_str();
  let words: Vec<&str> = upper.split_ascii_whitespace().collect();
  let t_idx = words.iter().position(|w| *w == "TRANSFORM")?;
  if !words[..t_idx].iter().all(|w| matches!(*w, "CREATE" | "OR" | "REPLACE")) || words[0] != "CREATE" {
    return None;
  }
  let tail = &words[t_idx + 1..];
  let last = tail.last().copied();
  if last == Some("TYPE") || last == Some("LANGUAGE") {
    return None; // user types name
  }
  if last == Some("FOR") {
    return Some(&[("TYPE", "FOR TYPE <type>")]);
  }
  if last == Some("FROM") {
    return Some(&[("SQL", "FROM SQL WITH FUNCTION <fn>(<types>)")]);
  }
  if last == Some("TO") {
    return Some(&[("SQL", "TO SQL WITH FUNCTION <fn>(<types>)")]);
  }
  if last == Some("WITH") {
    return Some(&[("FUNCTION", "WITH FUNCTION <fn>(<types>)")]);
  }
  if tail.is_empty() {
    return Some(&[("FOR TYPE", "FOR TYPE <type> LANGUAGE <lang> (...)")]);
  }
  if tail.len() == 1 {
    return Some(&[("FOR TYPE", "FOR TYPE <type> LANGUAGE <lang> (...)"), ("LANGUAGE", "LANGUAGE <plpgsql|plperl|...>")]);
  }
  if tail.contains(&"TYPE") && !tail.contains(&"LANGUAGE") {
    return Some(&[("LANGUAGE", "LANGUAGE <plpgsql|plperl|...>")]);
  }
  // Inside ( ... ) -- expect FROM SQL / TO SQL items.
  let opens = slice.matches('(').count();
  let closes = slice.matches(')').count();
  if opens > closes {
    let trimmed = slice.trim_end();
    let last_char = trimmed.chars().last();
    if matches!(last_char, Some('(') | Some(',')) || slice.ends_with(char::is_whitespace) {
      return Some(&[
        ("FROM SQL WITH FUNCTION", "FROM SQL WITH FUNCTION <fn>(<types>)"),
        ("TO SQL WITH FUNCTION", "TO SQL WITH FUNCTION <fn>(<types>)"),
      ]);
    }
  }
  // After LANGUAGE <name> -> need (.
  if tail.contains(&"LANGUAGE") && !tail.contains(&"(") && opens == closes {
    return Some(&[("(", "( FROM SQL WITH FUNCTION ..., TO SQL WITH FUNCTION ... )")]);
  }
  None
}

/// CREATE OPERATOR FAMILY chain.
///   CREATE OPERATOR FAMILY <name> USING <index_method>
fn create_operator_family_next_keyword(
  source: &str,
  offset: TextSize,
) -> Option<&'static [(&'static str, &'static str)]> {
  if cursor_not_at_ws_boundary(source, offset) {
    return None;
  }
  let (_, upper) = stmt_slice_upper(source, offset);
  let words: Vec<&str> = upper.split_ascii_whitespace().collect();
  if words.len() < 3 || words[0] != "CREATE" || words[1] != "OPERATOR" || words[2] != "FAMILY" {
    return None;
  }
  let last = *words.last()?;
  if last == "FAMILY" {
    return None; // user types name
  }
  if last == "USING" {
    return Some(&[
      ("btree", "btree"),
      ("hash", "hash"),
      ("gist", "gist"),
      ("spgist", "spgist"),
      ("gin", "gin"),
      ("brin", "brin"),
    ]);
  }
  if words.len() >= 4 && !words.contains(&"USING") {
    return Some(&[("USING", "USING <index_method>")]);
  }
  None
}

/// CREATE OPERATOR CLASS chain.
///   CREATE OPERATOR CLASS <name> [DEFAULT] FOR TYPE <type>
///     USING <index_method> [FAMILY <family>] AS
///       { OPERATOR <strategy> <op> [(<types>)] [FOR ORDER BY <opfamily>]
///       | FUNCTION <support> [(<types>)] <fn>(<types>)
///       | STORAGE <storage_type> } [, ...]
fn create_operator_class_next_keyword(
  source: &str,
  offset: TextSize,
) -> Option<&'static [(&'static str, &'static str)]> {
  if cursor_not_at_ws_boundary(source, offset) {
    return None;
  }
  let (_, upper) = stmt_slice_upper(source, offset);
  let words: Vec<&str> = upper.split_ascii_whitespace().collect();
  if words.len() < 3 || words[0] != "CREATE" || words[1] != "OPERATOR" || words[2] != "CLASS" {
    return None;
  }
  let last = *words.last()?;
  if last == "CLASS" {
    return None; // user types name
  }
  if last == "DEFAULT" {
    return Some(&[("FOR TYPE", "FOR TYPE <type>")]);
  }
  if last == "FOR" {
    return Some(&[("TYPE", "FOR TYPE <type>")]);
  }
  if last == "TYPE" {
    return None; // user types type name
  }
  if last == "USING" {
    return Some(&[
      ("btree", "btree"),
      ("hash", "hash"),
      ("gist", "gist"),
      ("spgist", "spgist"),
      ("gin", "gin"),
      ("brin", "brin"),
    ]);
  }
  if last == "FAMILY" {
    return None; // user types existing family name
  }
  if last == "AS" {
    return Some(&[
      ("OPERATOR", "OPERATOR <strategy_number> <op> [(<types>)] [FOR ORDER BY <opfamily>]"),
      ("FUNCTION", "FUNCTION <support_number> [(<types>)] <fn>(<types>)"),
      ("STORAGE", "STORAGE <storage_type>"),
    ]);
  }
  // After USING <am> -- next is FAMILY (optional) or AS.
  if words.contains(&"USING") && !words.contains(&"AS") {
    let am_idx = words.iter().rposition(|w| *w == "USING")?;
    if words.len() == am_idx + 2 && !matches!(last, "USING") {
      return Some(&[
        ("FAMILY", "FAMILY <existing_family>"),
        ("AS", "AS OPERATOR ... | FUNCTION ... | STORAGE ..."),
      ]);
    }
    if words.contains(&"FAMILY") && words.len() >= am_idx + 4 {
      return Some(&[("AS", "AS OPERATOR ... | FUNCTION ... | STORAGE ...")]);
    }
  }
  // Inside AS body -- after a `,` -> repeat item kw.
  if words.contains(&"AS") && last == "," {
    return Some(&[
      ("OPERATOR", "OPERATOR <strategy_number> <op>(<types>)"),
      ("FUNCTION", "FUNCTION <support_number> <fn>(<types>)"),
      ("STORAGE", "STORAGE <storage_type>"),
    ]);
  }
  // Top-level after `<name>` -- need FOR TYPE / DEFAULT.
  if words.len() >= 4 && !words.contains(&"FOR") {
    return Some(&[("DEFAULT", "DEFAULT -- mark as default opclass"), ("FOR TYPE", "FOR TYPE <type>")]);
  }
  None
}

fn alter_default_privileges_next_keyword(
  source: &str,
  offset: TextSize,
) -> Option<&'static [(&'static str, &'static str)]> {
  if cursor_not_at_ws_boundary(source, offset) {
    return None;
  }
  let (_, upper) = stmt_slice_upper(source, offset);
  let words: Vec<&str> = upper.split_ascii_whitespace().collect();
  if words.len() < 3 || words[0] != "ALTER" || words[1] != "DEFAULT" || words[2] != "PRIVILEGES" {
    return None;
  }
  let last = *words.last()?;
  if last == "PRIVILEGES" {
    return Some(&[
      ("FOR ROLE", "FOR ROLE <r>[, ...]"),
      ("FOR USER", "FOR USER <r>[, ...]"),
      ("IN SCHEMA", "IN SCHEMA <s>[, ...]"),
      ("GRANT", "GRANT <privs> ON {TABLES|SEQUENCES|FUNCTIONS|TYPES|SCHEMAS} TO ..."),
      ("REVOKE", "REVOKE <privs> ON ... FROM ..."),
    ]);
  }
  if last == "FOR" {
    return Some(&[("ROLE", "FOR ROLE <r>[, ...]"), ("USER", "FOR USER <r>[, ...]")]);
  }
  if last == "IN" {
    return Some(&[("SCHEMA", "IN SCHEMA <s>[, ...]")]);
  }
  if last == "ON" {
    return Some(&[
      ("TABLES", "ON TABLES"),
      ("SEQUENCES", "ON SEQUENCES"),
      ("FUNCTIONS", "ON FUNCTIONS"),
      ("ROUTINES", "ON ROUTINES"),
      ("TYPES", "ON TYPES"),
      ("SCHEMAS", "ON SCHEMAS"),
    ]);
  }
  if matches!(last, "TABLES" | "SEQUENCES" | "FUNCTIONS" | "ROUTINES" | "TYPES" | "SCHEMAS") {
    if words.contains(&"GRANT") {
      return Some(&[("TO", "TO <role>[, ...] | PUBLIC")]);
    }
    if words.contains(&"REVOKE") {
      return Some(&[("FROM", "FROM <role>[, ...] | PUBLIC")]);
    }
  }
  None
}

/// ALTER EXTENSION chain.
///   ALTER EXTENSION <name> { UPDATE [TO <version>] |
///                            SET SCHEMA <schema> |
///                            ADD <member_kind> <name> |
///                            DROP <member_kind> <name> }
fn alter_extension_next_keyword(
  source: &str,
  offset: TextSize,
) -> Option<&'static [(&'static str, &'static str)]> {
  if cursor_not_at_ws_boundary(source, offset) {
    return None;
  }
  let (_, upper) = stmt_slice_upper(source, offset);
  let words: Vec<&str> = upper.split_ascii_whitespace().collect();
  if words.len() < 2 || words[0] != "ALTER" || words[1] != "EXTENSION" {
    return None;
  }
  let last = *words.last()?;
  if last == "UPDATE" {
    return Some(&[("TO", "UPDATE TO '<version>'")]);
  }
  if last == "SET" {
    return Some(&[("SCHEMA", "SET SCHEMA <schema>")]);
  }
  if last == "ADD" || last == "DROP" {
    return Some(&[
      ("TABLE", "ADD/DROP TABLE <name>"),
      ("VIEW", "ADD/DROP VIEW <name>"),
      ("SEQUENCE", "ADD/DROP SEQUENCE <name>"),
      ("FUNCTION", "ADD/DROP FUNCTION <name>(args)"),
      ("PROCEDURE", "ADD/DROP PROCEDURE <name>(args)"),
      ("TYPE", "ADD/DROP TYPE <name>"),
      ("AGGREGATE", "ADD/DROP AGGREGATE <name>(args)"),
      ("OPERATOR", "ADD/DROP OPERATOR <op>(args)"),
      ("CAST", "ADD/DROP CAST (src AS dst)"),
      ("SCHEMA", "ADD/DROP SCHEMA <name>"),
      ("MATERIALIZED VIEW", "ADD/DROP MATERIALIZED VIEW <name>"),
      ("FOREIGN TABLE", "ADD/DROP FOREIGN TABLE <name>"),
      ("FOREIGN DATA WRAPPER", "ADD/DROP FOREIGN DATA WRAPPER <name>"),
      ("SERVER", "ADD/DROP SERVER <name>"),
      ("TEXT SEARCH CONFIGURATION", "ADD/DROP TEXT SEARCH CONFIGURATION <name>"),
      ("TEXT SEARCH DICTIONARY", "ADD/DROP TEXT SEARCH DICTIONARY <name>"),
      ("TEXT SEARCH PARSER", "ADD/DROP TEXT SEARCH PARSER <name>"),
      ("TEXT SEARCH TEMPLATE", "ADD/DROP TEXT SEARCH TEMPLATE <name>"),
    ]);
  }
  if words.len() >= 3 {
    return Some(&[
      ("UPDATE", "UPDATE [TO '<version>']"),
      ("UPDATE TO", "UPDATE TO '<version>'"),
      ("SET SCHEMA", "SET SCHEMA <schema>"),
      ("ADD", "ADD <kind> <name>"),
      ("DROP", "DROP <kind> <name>"),
    ]);
  }
  None
}

/// ALTER SEQUENCE chain.
/// ALTER INDEX chain.
///   ALTER INDEX [IF EXISTS] <name>
///       RENAME TO <new>
///     | SET TABLESPACE <ts>
///     | SET ( <param> = <val>[, ...] )
///     | RESET ( <param>[, ...] )
///     | ATTACH PARTITION <child_idx>
///     | DEPENDS ON EXTENSION <ext>
///     | NO DEPENDS ON EXTENSION <ext>
///     | ALTER COLUMN <col_num> SET STATISTICS <n>
///   ALTER INDEX ALL IN TABLESPACE <ts> [OWNED BY <role>[, ...]] SET TABLESPACE <new_ts> [NOWAIT]
fn alter_index_next_keyword(
  source: &str,
  offset: TextSize,
) -> Option<&'static [(&'static str, &'static str)]> {
  let pos: usize = (u32::from(offset) as usize).min(source.len());
  let bytes = source.as_bytes();
  // Allow cursor right after `(` for the storage-param menu below.
  let at_paren = pos > 0 && bytes[pos - 1] == b'(';
  if !at_paren && cursor_not_at_ws_boundary(source, offset) {
    return None;
  }
  let (slice_owned, upper) = stmt_slice_upper(source, offset);
  let slice = slice_owned.as_str();
  let words: Vec<&str> = upper.split_ascii_whitespace().collect();
  if words.len() < 2 || words[0] != "ALTER" || words[1] != "INDEX" {
    return None;
  }
  // SET ( <param>[, ...] ) -- storage param name slot inside paren.
  let opens = slice.matches('(').count();
  let closes = slice.matches(')').count();
  if opens > closes && upper.contains(" SET (") {
    let trimmed = slice.trim_end();
    let last_char = trimmed.chars().last();
    if matches!(last_char, Some('(') | Some(',')) || slice.ends_with(char::is_whitespace) {
      return Some(&[
        ("fillfactor", "fillfactor = <int> -- index page fillfactor (10..100)"),
        ("deduplicate_items", "deduplicate_items = on|off -- btree dedup (PG13+)"),
        ("buffering", "buffering = on|off|auto -- gist build buffering"),
        ("fastupdate", "fastupdate = on|off -- gin pending-list updates"),
        ("gin_pending_list_limit", "gin_pending_list_limit = '<size>' -- pending-list size"),
        ("pages_per_range", "pages_per_range = <int> -- brin granularity"),
        ("autosummarize", "autosummarize = on|off -- brin auto summarize"),
      ]);
    }
  }
  let last = *words.last()?;
  if matches!(last, "RENAME" | "SET" | "RESET" | "ATTACH" | "DEPENDS" | "ALTER" | "NO" | "ALL" | "IN" | "OWNED") {
    let menu: &[(&str, &str)] = match last {
      "RENAME" => &[("TO", "RENAME TO <new>")],
      "SET" => &[
        ("TABLESPACE", "SET TABLESPACE <ts>"),
        ("(", "SET ( <param> = <val>[, ...] )"),
      ],
      "RESET" => &[("(", "RESET ( <param>[, ...] )")],
      "ATTACH" => &[("PARTITION", "ATTACH PARTITION <child_idx>")],
      "DEPENDS" => &[("ON EXTENSION", "DEPENDS ON EXTENSION <ext>")],
      "NO" => &[("DEPENDS ON EXTENSION", "NO DEPENDS ON EXTENSION <ext>")],
      "ALTER" => &[("COLUMN", "ALTER COLUMN <col_num> SET STATISTICS <n>")],
      "ALL" => &[("IN TABLESPACE", "ALL IN TABLESPACE <ts>")],
      "IN" => &[("TABLESPACE", "IN TABLESPACE <ts>")],
      "OWNED" => &[("BY", "OWNED BY <role>[, ...]")],
      _ => &[],
    };
    if !menu.is_empty() {
      return Some(menu);
    }
  }
  if words.len() >= 3 {
    return Some(&[
      ("RENAME TO", "RENAME TO <new>"),
      ("SET TABLESPACE", "SET TABLESPACE <ts>"),
      ("SET", "SET ( <param> = <val>[, ...] )"),
      ("RESET", "RESET ( <param>[, ...] )"),
      ("ATTACH PARTITION", "ATTACH PARTITION <child_idx>"),
      ("DEPENDS ON EXTENSION", "DEPENDS ON EXTENSION <ext>"),
      ("NO DEPENDS ON EXTENSION", "NO DEPENDS ON EXTENSION <ext>"),
      ("ALTER COLUMN", "ALTER COLUMN <col_num> SET STATISTICS <n>"),
    ]);
  }
  None
}

fn alter_sequence_next_keyword(
  source: &str,
  offset: TextSize,
) -> Option<&'static [(&'static str, &'static str)]> {
  if cursor_not_at_ws_boundary(source, offset) {
    return None;
  }
  let (_, upper) = stmt_slice_upper(source, offset);
  let words: Vec<&str> = upper.split_ascii_whitespace().collect();
  if words.len() < 2 || words[0] != "ALTER" || words[1] != "SEQUENCE" {
    return None;
  }
  let last = *words.last()?;
  if matches!(last, "RESTART" | "INCREMENT" | "START" | "MINVALUE" | "MAXVALUE" | "CACHE") {
    let menu: &[(&str, &str)] = match last {
      "RESTART" => &[("WITH", "RESTART WITH <n>")],
      "INCREMENT" => &[("BY", "INCREMENT BY <n>")],
      "START" => &[("WITH", "START WITH <n>")],
      _ => &[],
    };
    if !menu.is_empty() {
      return Some(menu);
    }
    return None; // user types value
  }
  if last == "NO" {
    return Some(&[("MINVALUE", "NO MINVALUE"), ("MAXVALUE", "NO MAXVALUE"), ("CYCLE", "NO CYCLE")]);
  }
  if last == "OWNED" {
    return Some(&[("BY", "OWNED BY <table>.<col> | NONE")]);
  }
  if last == "OWNER" {
    return Some(&[("TO", "OWNER TO <role>")]);
  }
  if last == "SET" {
    return Some(&[("SCHEMA", "SET SCHEMA <schema>"), ("LOGGED", "SET LOGGED"), ("UNLOGGED", "SET UNLOGGED")]);
  }
  if last == "AS" {
    return Some(&[("smallint", "AS smallint"), ("integer", "AS integer"), ("bigint", "AS bigint")]);
  }
  if words.len() >= 3 {
    return Some(&[
      ("AS", "AS smallint | integer | bigint"),
      ("INCREMENT BY", "INCREMENT BY <n>"),
      ("MINVALUE", "MINVALUE <n>"),
      ("NO MINVALUE", "NO MINVALUE"),
      ("MAXVALUE", "MAXVALUE <n>"),
      ("NO MAXVALUE", "NO MAXVALUE"),
      ("START WITH", "START WITH <n>"),
      ("RESTART", "RESTART [WITH <n>]"),
      ("CACHE", "CACHE <n>"),
      ("CYCLE", "CYCLE"),
      ("NO CYCLE", "NO CYCLE"),
      ("OWNED BY", "OWNED BY <tbl>.<col> | NONE"),
      ("OWNER TO", "OWNER TO <role>"),
      ("RENAME TO", "RENAME TO <new_name>"),
      ("SET SCHEMA", "SET SCHEMA <schema>"),
    ]);
  }
  None
}

/// ALTER POLICY chain.
///   ALTER POLICY <name> ON <table> [RENAME TO <new>]
///       [TO {role|PUBLIC|CURRENT_USER}[, ...]]
///       [USING (<expr>)] [WITH CHECK (<expr>)]
/// ALTER TEXT SEARCH {CONFIGURATION|DICTIONARY|PARSER|TEMPLATE} chain.
fn alter_text_search_next_keyword(
  source: &str,
  offset: TextSize,
) -> Option<&'static [(&'static str, &'static str)]> {
  if cursor_not_at_ws_boundary(source, offset) {
    return None;
  }
  let (_, upper) = stmt_slice_upper(source, offset);
  let words: Vec<&str> = upper.split_ascii_whitespace().collect();
  if words.len() < 3 || words[0] != "ALTER" || words[1] != "TEXT" || words[2] != "SEARCH" {
    return None;
  }
  if words.len() < 4 {
    return Some(&[
      ("CONFIGURATION", "CONFIGURATION <name> ..."),
      ("DICTIONARY", "DICTIONARY <name> ..."),
      ("PARSER", "PARSER <name> ..."),
      ("TEMPLATE", "TEMPLATE <name> ..."),
    ]);
  }
  let kind = words[3];
  let last = *words.last()?;
  // After kind keyword -> name slot.
  if words.len() == 4 {
    return None; // user types name
  }
  if matches!(last, "RENAME" | "OWNER" | "SET" | "ADD" | "ALTER" | "DROP" | "MAPPING") {
    let menu: &[(&str, &str)] = match (kind, last) {
      (_, "RENAME") => &[("TO", "RENAME TO <new_name>")],
      (_, "OWNER") => &[("TO", "OWNER TO <role>")],
      (_, "SET") => &[("SCHEMA", "SET SCHEMA <schema>")],
      ("CONFIGURATION", "ADD") => &[("MAPPING FOR", "ADD MAPPING FOR <token_type>[, ...] WITH <dictionary>[, ...]")],
      ("CONFIGURATION", "ALTER") => &[("MAPPING FOR", "ALTER MAPPING FOR <token_type> WITH <dictionary>"), ("MAPPING REPLACE", "ALTER MAPPING REPLACE <old_dict> WITH <new_dict>")],
      ("CONFIGURATION", "DROP") => &[("MAPPING FOR", "DROP MAPPING [IF EXISTS] FOR <token_type>[, ...]")],
      ("CONFIGURATION", "MAPPING") => &[("FOR", "MAPPING FOR <token_type>[, ...]"), ("REPLACE", "MAPPING REPLACE <old_dict> WITH <new_dict>")],
      _ => &[],
    };
    if !menu.is_empty() {
      return Some(menu);
    }
  }
  // After ALTER TEXT SEARCH <kind> <name> -> action menu.
  if words.len() >= 5 && !matches!(last, "TO" | "FOR" | "WITH" | "MAPPING" | "REPLACE") {
    let menu: &[(&str, &str)] = match kind {
      "CONFIGURATION" => &[
        ("ADD MAPPING FOR", "ADD MAPPING FOR <token_type> WITH <dict>"),
        ("ALTER MAPPING FOR", "ALTER MAPPING FOR <token_type> WITH <dict>"),
        ("ALTER MAPPING REPLACE", "ALTER MAPPING REPLACE <old_dict> WITH <new_dict>"),
        ("DROP MAPPING FOR", "DROP MAPPING [IF EXISTS] FOR <token_type>"),
        ("RENAME TO", "RENAME TO <new>"),
        ("OWNER TO", "OWNER TO <role>"),
        ("SET SCHEMA", "SET SCHEMA <schema>"),
      ],
      _ => &[
        ("RENAME TO", "RENAME TO <new>"),
        ("OWNER TO", "OWNER TO <role>"),
        ("SET SCHEMA", "SET SCHEMA <schema>"),
      ],
    };
    return Some(menu);
  }
  None
}

fn alter_policy_next_keyword(source: &str, offset: TextSize) -> Option<&'static [(&'static str, &'static str)]> {
  if cursor_not_at_ws_boundary(source, offset) {
    return None;
  }
  let (_, upper) = stmt_slice_upper(source, offset);
  let words: Vec<&str> = upper.split_ascii_whitespace().collect();
  if words.len() < 2 || words[0] != "ALTER" || words[1] != "POLICY" {
    return None;
  }
  let last = *words.last()?;
  if last == "RENAME" {
    return Some(&[("TO", "RENAME TO <new_name>")]);
  }
  if last == "WITH" {
    return Some(&[("CHECK", "WITH CHECK (<predicate>) -- INSERT/UPDATE check")]);
  }
  if last == "TO" && !words.contains(&"RENAME") {
    return Some(&[
      ("PUBLIC", "TO PUBLIC -- everyone"),
      ("CURRENT_USER", "TO CURRENT_USER"),
      ("SESSION_USER", "TO SESSION_USER"),
    ]);
  }
  // After ON <tbl> -> action menu.
  if words.contains(&"ON") && words.len() >= 5 {
    return Some(&[
      ("RENAME TO", "RENAME TO <new>"),
      ("TO", "TO <role>[, ...] | PUBLIC"),
      ("USING", "USING (<predicate>) -- visibility filter"),
      ("WITH CHECK", "WITH CHECK (<predicate>) -- write check"),
    ]);
  }
  if words.len() == 3 {
    return Some(&[("ON", "ON <table>")]);
  }
  None
}

/// ALTER DOMAIN chain.
///   ALTER DOMAIN <name> { SET DEFAULT <expr> | DROP DEFAULT |
///                         {SET|DROP} NOT NULL |
///                         ADD <constraint> | DROP CONSTRAINT <name> |
///                         RENAME [CONSTRAINT <old> TO <new> | TO <new_name>] |
///                         OWNER TO <role> | SET SCHEMA <schema> |
///                         VALIDATE CONSTRAINT <name> }
fn alter_domain_next_keyword(source: &str, offset: TextSize) -> Option<&'static [(&'static str, &'static str)]> {
  if cursor_not_at_ws_boundary(source, offset) {
    return None;
  }
  let (_, upper) = stmt_slice_upper(source, offset);
  let words: Vec<&str> = upper.split_ascii_whitespace().collect();
  if words.len() < 2 || words[0] != "ALTER" || words[1] != "DOMAIN" {
    return None;
  }
  let last = *words.last()?;
  if last == "SET" {
    return Some(&[
      ("DEFAULT", "SET DEFAULT <expr>"),
      ("NOT NULL", "SET NOT NULL"),
      ("SCHEMA", "SET SCHEMA <schema>"),
    ]);
  }
  if last == "DROP" {
    return Some(&[
      ("DEFAULT", "DROP DEFAULT"),
      ("NOT NULL", "DROP NOT NULL"),
      ("CONSTRAINT", "DROP CONSTRAINT [IF EXISTS] <name>"),
    ]);
  }
  if last == "RENAME" {
    return Some(&[("CONSTRAINT", "RENAME CONSTRAINT <old> TO <new>"), ("TO", "RENAME TO <new_name>")]);
  }
  if last == "OWNER" {
    return Some(&[("TO", "OWNER TO <role>")]);
  }
  if last == "VALIDATE" {
    return Some(&[("CONSTRAINT", "VALIDATE CONSTRAINT <name>")]);
  }
  if last == "ADD" {
    return Some(&[("CONSTRAINT", "ADD CONSTRAINT <name> CHECK (...)"), ("CHECK", "ADD CHECK (VALUE > 0)")]);
  }
  // After ALTER DOMAIN <name> -> action menu.
  if words.len() >= 3 {
    return Some(&[
      ("SET DEFAULT", "SET DEFAULT <expr>"),
      ("DROP DEFAULT", "DROP DEFAULT"),
      ("SET NOT NULL", "SET NOT NULL"),
      ("DROP NOT NULL", "DROP NOT NULL"),
      ("ADD CONSTRAINT", "ADD CONSTRAINT <name> CHECK (...)"),
      ("DROP CONSTRAINT", "DROP CONSTRAINT [IF EXISTS] <name>"),
      ("RENAME CONSTRAINT", "RENAME CONSTRAINT <old> TO <new>"),
      ("RENAME TO", "RENAME TO <new_name>"),
      ("OWNER TO", "OWNER TO <role>"),
      ("SET SCHEMA", "SET SCHEMA <schema>"),
      ("VALIDATE CONSTRAINT", "VALIDATE CONSTRAINT <name>"),
    ]);
  }
  None
}

/// ALTER COLLATION chain.
/// ALTER MATERIALIZED VIEW chain.
///   ALTER MATERIALIZED VIEW [IF EXISTS] <name>
///       RENAME TO <new>
///     | RENAME COLUMN <old> TO <new>
///     | OWNER TO <role>
///     | SET SCHEMA <schema>
///     | SET TABLESPACE <ts> [NOWAIT]
///     | SET ACCESS METHOD <am>
///     | SET ( <param> = <val>[, ...] )
///     | RESET ( <param>[, ...] )
///     | ALTER COLUMN <col> SET STATISTICS <n>
///     | ALTER COLUMN <col> SET STORAGE { PLAIN | EXTERNAL | EXTENDED | MAIN }
///     | DEPENDS ON EXTENSION <ext>
///     | CLUSTER ON <index> | SET WITHOUT CLUSTER
fn alter_materialized_view_next_keyword(
  source: &str,
  offset: TextSize,
) -> Option<&'static [(&'static str, &'static str)]> {
  if cursor_not_at_ws_boundary(source, offset) {
    return None;
  }
  let (_, upper) = stmt_slice_upper(source, offset);
  let words: Vec<&str> = upper.split_ascii_whitespace().collect();
  if words.len() < 3
    || words[0] != "ALTER"
    || words[1] != "MATERIALIZED"
    || words[2] != "VIEW"
  {
    return None;
  }
  let last = *words.last()?;
  if matches!(last, "RENAME" | "OWNER" | "SET" | "RESET" | "CLUSTER" | "DEPENDS" | "ALTER") {
    let menu: &[(&str, &str)] = match last {
      "RENAME" => &[("TO", "RENAME TO <new>"), ("COLUMN", "RENAME COLUMN <old> TO <new>")],
      "OWNER" => &[("TO", "OWNER TO <role>")],
      "SET" => &[
        ("SCHEMA", "SET SCHEMA <schema>"),
        ("TABLESPACE", "SET TABLESPACE <ts> [NOWAIT]"),
        ("ACCESS METHOD", "SET ACCESS METHOD <am>"),
        ("(", "SET ( <param> = <value>[, ...] )"),
        ("WITHOUT CLUSTER", "SET WITHOUT CLUSTER"),
      ],
      "RESET" => &[("(", "RESET ( <param>[, ...] )")],
      "CLUSTER" => &[("ON", "CLUSTER ON <index_name>")],
      "DEPENDS" => &[("ON EXTENSION", "DEPENDS ON EXTENSION <ext>")],
      "ALTER" => &[("COLUMN", "ALTER COLUMN <col> SET ..."), ("MATERIALIZED VIEW", "ALTER MATERIALIZED VIEW <name> ALL IN TABLESPACE ...")],
      _ => &[],
    };
    if !menu.is_empty() {
      return Some(menu);
    }
  }
  if words.len() >= 4 {
    return Some(&[
      ("RENAME TO", "RENAME TO <new>"),
      ("RENAME COLUMN", "RENAME COLUMN <old> TO <new>"),
      ("OWNER TO", "OWNER TO <role>"),
      ("SET SCHEMA", "SET SCHEMA <schema>"),
      ("SET TABLESPACE", "SET TABLESPACE <ts> [NOWAIT]"),
      ("SET ACCESS METHOD", "SET ACCESS METHOD <am>"),
      ("SET", "SET ( <param> = <val>[, ...] ) | SET WITHOUT CLUSTER"),
      ("RESET", "RESET ( <param>[, ...] )"),
      ("ALTER COLUMN", "ALTER COLUMN <col> SET STATISTICS|STORAGE|..."),
      ("CLUSTER ON", "CLUSTER ON <index_name>"),
      ("DEPENDS ON EXTENSION", "DEPENDS ON EXTENSION <ext>"),
    ]);
  }
  None
}

/// ALTER EVENT TRIGGER chain.
///   ALTER EVENT TRIGGER <name>
///       { ENABLE [REPLICA | ALWAYS] | DISABLE
///       | RENAME TO <new>
///       | OWNER TO <role>
///       | DEPENDS ON EXTENSION <ext>
///       | NO DEPENDS ON EXTENSION <ext> }
fn alter_event_trigger_next_keyword(
  source: &str,
  offset: TextSize,
) -> Option<&'static [(&'static str, &'static str)]> {
  if cursor_not_at_ws_boundary(source, offset) {
    return None;
  }
  let (_, upper) = stmt_slice_upper(source, offset);
  let words: Vec<&str> = upper.split_ascii_whitespace().collect();
  if words.len() < 3 || words[0] != "ALTER" || words[1] != "EVENT" || words[2] != "TRIGGER" {
    return None;
  }
  let last = *words.last()?;
  if matches!(last, "RENAME" | "OWNER" | "ENABLE" | "DEPENDS" | "NO") {
    let menu: &[(&str, &str)] = match last {
      "RENAME" => &[("TO", "RENAME TO <new>")],
      "OWNER" => &[("TO", "OWNER TO <role>")],
      "ENABLE" => &[
        ("REPLICA", "ENABLE REPLICA -- fire on the replica role only"),
        ("ALWAYS", "ENABLE ALWAYS -- fire regardless of session_replication_role"),
      ],
      "DEPENDS" => &[("ON EXTENSION", "DEPENDS ON EXTENSION <ext>")],
      "NO" => &[("DEPENDS ON EXTENSION", "NO DEPENDS ON EXTENSION <ext>")],
      _ => &[],
    };
    if !menu.is_empty() {
      return Some(menu);
    }
  }
  if words.len() >= 4 {
    return Some(&[
      ("ENABLE", "ENABLE [REPLICA | ALWAYS]"),
      ("DISABLE", "DISABLE"),
      ("RENAME TO", "RENAME TO <new>"),
      ("OWNER TO", "OWNER TO <role>"),
      ("DEPENDS ON EXTENSION", "DEPENDS ON EXTENSION <ext>"),
      ("NO DEPENDS ON EXTENSION", "NO DEPENDS ON EXTENSION <ext>"),
    ]);
  }
  None
}

/// ALTER OPERATOR CLASS / ALTER OPERATOR FAMILY chain.
fn alter_operator_class_family_next_keyword(
  source: &str,
  offset: TextSize,
) -> Option<&'static [(&'static str, &'static str)]> {
  if cursor_not_at_ws_boundary(source, offset) {
    return None;
  }
  let (_, upper) = stmt_slice_upper(source, offset);
  let words: Vec<&str> = upper.split_ascii_whitespace().collect();
  if words.len() < 3 || words[0] != "ALTER" || words[1] != "OPERATOR" {
    return None;
  }
  let kind = words[2];
  if !matches!(kind, "CLASS" | "FAMILY") {
    return None;
  }
  let last = *words.last()?;
  if matches!(last, "RENAME" | "OWNER" | "SET" | "USING" | "ADD" | "DROP") {
    let menu: &[(&str, &str)] = match last {
      "RENAME" => &[("TO", "RENAME TO <new>")],
      "OWNER" => &[("TO", "OWNER TO <role>")],
      "SET" => &[("SCHEMA", "SET SCHEMA <schema>")],
      "USING" => &[
        ("btree", "btree"),
        ("hash", "hash"),
        ("gist", "gist"),
        ("spgist", "spgist"),
        ("gin", "gin"),
        ("brin", "brin"),
      ],
      _ => &[],
    };
    if !menu.is_empty() {
      return Some(menu);
    }
  }
  // After name USING <am> <cursor> -- ADD/DROP/etc for OPERATOR FAMILY.
  if kind == "FAMILY" && words.contains(&"USING") && words.len() >= 6 {
    let last = *words.last()?;
    if last == "ADD" {
      return Some(&[
        ("OPERATOR", "ADD OPERATOR <strategy_number> <op>(<types>) FOR ..."),
        ("FUNCTION", "ADD FUNCTION <support_number> [(<types>)] <fn>(<types>)"),
      ]);
    }
    if last == "DROP" {
      return Some(&[
        ("OPERATOR", "DROP OPERATOR <strategy_number> (<types>)"),
        ("FUNCTION", "DROP FUNCTION <support_number> (<types>)"),
      ]);
    }
    return Some(&[
      ("ADD", "ADD OPERATOR | FUNCTION ..."),
      ("DROP", "DROP OPERATOR | FUNCTION ..."),
      ("RENAME TO", "RENAME TO <new>"),
      ("OWNER TO", "OWNER TO <role>"),
      ("SET SCHEMA", "SET SCHEMA <schema>"),
    ]);
  }
  if words.len() >= 4 {
    return Some(&[
      ("USING", "USING <index_method>"),
      ("RENAME TO", "RENAME TO <new>"),
      ("OWNER TO", "OWNER TO <role>"),
      ("SET SCHEMA", "SET SCHEMA <schema>"),
    ]);
  }
  None
}

fn alter_collation_next_keyword(
  source: &str,
  offset: TextSize,
) -> Option<&'static [(&'static str, &'static str)]> {
  if cursor_not_at_ws_boundary(source, offset) {
    return None;
  }
  let (_, upper) = stmt_slice_upper(source, offset);
  let words: Vec<&str> = upper.split_ascii_whitespace().collect();
  if words.len() < 2 || words[0] != "ALTER" || words[1] != "COLLATION" {
    return None;
  }
  let last = *words.last()?;
  if last == "REFRESH" {
    return Some(&[("VERSION", "REFRESH VERSION -- bump after libc/icu upgrade")]);
  }
  if words.len() >= 3 {
    return Some(&[
      ("RENAME TO", "RENAME TO <new_name>"),
      ("OWNER TO", "OWNER TO <role>"),
      ("SET SCHEMA", "SET SCHEMA <schema>"),
      ("REFRESH VERSION", "REFRESH VERSION -- bump after libc/icu upgrade"),
    ]);
  }
  None
}

/// CREATE ACCESS METHOD chain.
/// `CREATE ACCESS METHOD <name> TYPE {INDEX|TABLE} HANDLER <fn>`
/// ALTER RULE chain.
///   ALTER RULE <name> ON <table> RENAME TO <new_name>
fn alter_rule_next_keyword(
  source: &str,
  offset: TextSize,
) -> Option<&'static [(&'static str, &'static str)]> {
  if cursor_not_at_ws_boundary(source, offset) {
    return None;
  }
  let (_, upper) = stmt_slice_upper(source, offset);
  let words: Vec<&str> = upper.split_ascii_whitespace().collect();
  if words.len() < 2 || words[0] != "ALTER" || words[1] != "RULE" {
    return None;
  }
  let last = *words.last()?;
  if last == "RULE" {
    return None; // user types rule name
  }
  if last == "ON" {
    return None; // user types table
  }
  if last == "RENAME" {
    return Some(&[("TO", "RENAME TO <new>")]);
  }
  if words.contains(&"ON") && words.len() >= 5 && !words.contains(&"RENAME") {
    return Some(&[("RENAME TO", "RENAME TO <new>")]);
  }
  if words.len() == 3 && !words.contains(&"ON") {
    return Some(&[("ON", "ON <table>")]);
  }
  None
}

/// ALTER TRIGGER chain.
///   ALTER TRIGGER <name> ON <table>
///       RENAME TO <new>
///     | DEPENDS ON EXTENSION <ext>
///     | NO DEPENDS ON EXTENSION <ext>
fn alter_trigger_next_keyword(
  source: &str,
  offset: TextSize,
) -> Option<&'static [(&'static str, &'static str)]> {
  if cursor_not_at_ws_boundary(source, offset) {
    return None;
  }
  let (_, upper) = stmt_slice_upper(source, offset);
  let words: Vec<&str> = upper.split_ascii_whitespace().collect();
  if words.len() < 2 || words[0] != "ALTER" || words[1] != "TRIGGER" {
    return None;
  }
  let last = *words.last()?;
  if last == "TRIGGER" {
    return None; // user types name
  }
  if last == "ON" {
    return None; // user types table
  }
  if last == "RENAME" {
    return Some(&[("TO", "RENAME TO <new>")]);
  }
  if last == "DEPENDS" {
    return Some(&[("ON EXTENSION", "DEPENDS ON EXTENSION <ext>")]);
  }
  if last == "NO" {
    return Some(&[("DEPENDS ON EXTENSION", "NO DEPENDS ON EXTENSION <ext>")]);
  }
  if words.contains(&"ON") && words.len() >= 5 {
    return Some(&[
      ("RENAME TO", "RENAME TO <new>"),
      ("DEPENDS ON EXTENSION", "DEPENDS ON EXTENSION <ext>"),
      ("NO DEPENDS ON EXTENSION", "NO DEPENDS ON EXTENSION <ext>"),
    ]);
  }
  if words.len() == 3 && !words.contains(&"ON") {
    return Some(&[("ON", "ON <table>")]);
  }
  None
}

/// ALTER ACCESS METHOD chain. PG only supports rename + owner changes.
///   ALTER ACCESS METHOD <name> { RENAME TO <new> | OWNER TO <role> }
fn alter_access_method_next_keyword(
  source: &str,
  offset: TextSize,
) -> Option<&'static [(&'static str, &'static str)]> {
  if cursor_not_at_ws_boundary(source, offset) {
    return None;
  }
  let (_, upper) = stmt_slice_upper(source, offset);
  let words: Vec<&str> = upper.split_ascii_whitespace().collect();
  if words.len() < 3 || words[0] != "ALTER" || words[1] != "ACCESS" || words[2] != "METHOD" {
    return None;
  }
  let last = *words.last()?;
  if matches!(last, "RENAME" | "OWNER") {
    return Some(match last {
      "RENAME" => &[("TO", "RENAME TO <new>")],
      "OWNER" => &[("TO", "OWNER TO <role>")],
      _ => &[],
    });
  }
  if words.len() >= 4 {
    return Some(&[("RENAME TO", "RENAME TO <new>"), ("OWNER TO", "OWNER TO <role>")]);
  }
  None
}

/// CREATE CONVERSION chain.
///   CREATE [DEFAULT] CONVERSION <name> FOR '<src_enc>' TO '<dst_enc>' FROM <fn>
fn create_conversion_next_keyword(
  source: &str,
  offset: TextSize,
) -> Option<&'static [(&'static str, &'static str)]> {
  if cursor_not_at_ws_boundary(source, offset) {
    return None;
  }
  let (_, upper) = stmt_slice_upper(source, offset);
  let words: Vec<&str> = upper.split_ascii_whitespace().collect();
  let conv_idx = words.iter().position(|w| *w == "CONVERSION")?;
  if !words[..conv_idx].iter().all(|w| matches!(*w, "CREATE" | "DEFAULT")) || words[0] != "CREATE" {
    return None;
  }
  let tail = &words[conv_idx + 1..];
  let last = tail.last().copied();
  if tail.is_empty() {
    return None; // user types name
  }
  if last == Some("FOR") {
    return None; // user types '<src_enc>'
  }
  if last == Some("TO") && tail.contains(&"FOR") {
    return None; // user types '<dst_enc>'
  }
  if last == Some("FROM") && tail.contains(&"FOR") {
    return None; // user types function name
  }
  if tail.len() == 1 {
    return Some(&[("FOR", "FOR '<src_encoding>' TO '<dst_encoding>' FROM <function>")]);
  }
  if tail.contains(&"FOR") && !tail.contains(&"TO") {
    return Some(&[("TO", "TO '<dst_encoding>'")]);
  }
  if tail.contains(&"TO") && !tail.contains(&"FROM") {
    return Some(&[("FROM", "FROM <conversion_function>")]);
  }
  None
}

/// ALTER CONVERSION chain.
fn alter_conversion_next_keyword(
  source: &str,
  offset: TextSize,
) -> Option<&'static [(&'static str, &'static str)]> {
  if cursor_not_at_ws_boundary(source, offset) {
    return None;
  }
  let (_, upper) = stmt_slice_upper(source, offset);
  let words: Vec<&str> = upper.split_ascii_whitespace().collect();
  if words.len() < 2 || words[0] != "ALTER" || words[1] != "CONVERSION" {
    return None;
  }
  let last = *words.last()?;
  if matches!(last, "RENAME" | "OWNER" | "SET") {
    return Some(match last {
      "RENAME" => &[("TO", "RENAME TO <new>")],
      "OWNER" => &[("TO", "OWNER TO <role>")],
      "SET" => &[("SCHEMA", "SET SCHEMA <schema>")],
      _ => &[],
    });
  }
  if words.len() >= 3 {
    return Some(&[
      ("RENAME TO", "RENAME TO <new>"),
      ("OWNER TO", "OWNER TO <role>"),
      ("SET SCHEMA", "SET SCHEMA <schema>"),
    ]);
  }
  None
}

fn create_access_method_next_keyword(
  source: &str,
  offset: TextSize,
) -> Option<&'static [(&'static str, &'static str)]> {
  if cursor_not_at_ws_boundary(source, offset) {
    return None;
  }
  let (_, upper) = stmt_slice_upper(source, offset);
  let words: Vec<&str> = upper.split_ascii_whitespace().collect();
  if words.len() < 3 || words[0] != "CREATE" || words[1] != "ACCESS" || words[2] != "METHOD" {
    return None;
  }
  let last = *words.last()?;
  if last == "TYPE" {
    return Some(&[("INDEX", "TYPE INDEX -- index access method"), ("TABLE", "TYPE TABLE -- table access method (PG12+)")]);
  }
  if last == "HANDLER" {
    return None; // user types function name
  }
  if words.len() >= 4 && !words.contains(&"TYPE") {
    return Some(&[("TYPE", "TYPE {INDEX | TABLE}")]);
  }
  if words.contains(&"TYPE") && !words.contains(&"HANDLER") {
    return Some(&[("HANDLER", "HANDLER <function> -- AM entrypoint")]);
  }
  None
}

/// ALTER OPERATOR chain.
/// CREATE [OR REPLACE] VIEW / CREATE MATERIALIZED VIEW chain.
///   CREATE VIEW <name> [(cols)] [WITH (storage_params)] AS <select>
///                                [WITH [CASCADED|LOCAL] CHECK OPTION]
///   CREATE MATERIALIZED VIEW [IF NOT EXISTS] <name> [(cols)]
///                                [USING <am>] [WITH (storage_params)]
///                                [TABLESPACE <ts>] AS <select> [WITH [NO] DATA]
fn create_view_post_name_next_keyword(
  source: &str,
  offset: TextSize,
) -> Option<&'static [(&'static str, &'static str)]> {
  if cursor_not_at_ws_boundary(source, offset) {
    return None;
  }
  let (slice_owned, upper) = stmt_slice_upper(source, offset);
  let slice = slice_owned.as_str();
  let trimmed = upper.trim_start();
  let is_view = trimmed.starts_with("CREATE VIEW")
    || trimmed.starts_with("CREATE OR REPLACE VIEW")
    || trimmed.starts_with("CREATE TEMP VIEW")
    || trimmed.starts_with("CREATE TEMPORARY VIEW");
  let is_mv = trimmed.starts_with("CREATE MATERIALIZED VIEW");
  if !is_view && !is_mv {
    return None;
  }
  // Suppress when AS is already typed -- Phase machine handles the SELECT body.
  let words: Vec<&str> = upper.split_ascii_whitespace().collect();
  if words.contains(&"AS") {
    // After AS handled by SELECT phase; but trailing WITH after the body
    // (WITH CHECK OPTION for views, WITH DATA for MVs) is still our slot.
    let last = *words.last()?;
    if last == "WITH" {
      if is_view {
        return Some(&[
          ("CHECK OPTION", "WITH CHECK OPTION -- enforce predicate on writes"),
          ("CASCADED CHECK OPTION", "WITH CASCADED CHECK OPTION -- + parent views"),
          ("LOCAL CHECK OPTION", "WITH LOCAL CHECK OPTION -- this view only"),
        ]);
      }
      if is_mv {
        return Some(&[
          ("DATA", "WITH DATA -- populate now (default)"),
          ("NO DATA", "WITH NO DATA -- create empty; populate later with REFRESH"),
        ]);
      }
    }
    if last == "NO" && words.contains(&"WITH") && is_mv {
      return Some(&[("DATA", "NO DATA -- mark MV invalid")]);
    }
    if matches!(last, "CASCADED" | "LOCAL") && is_view {
      return Some(&[("CHECK OPTION", "CHECK OPTION -- enforce predicate on writes")]);
    }
    return None;
  }
  // Body paren count.
  let opens = slice.matches('(').count();
  let closes = slice.matches(')').count();
  let paren_balanced = opens == 0 || opens == closes;
  let last = *words.last()?;
  if last == "USING" && is_mv {
    return Some(&[
      ("heap", "USING heap -- default access method"),
    ]);
  }
  if last == "WITH" {
    if is_mv {
      return Some(&[("(", "WITH ( fillfactor = <n>, autovacuum_<param> = <v>, ... )")]);
    }
    return Some(&[("(", "WITH ( security_barrier = true|false, security_invoker = true|false, check_option = local|cascaded )")]);
  }
  if last == "TABLESPACE" && is_mv {
    return None; // user types tablespace name
  }
  if paren_balanced && words.len() >= 3 {
    // Fresh post-name slot.
    let mut menu: Vec<(&str, &str)> = Vec::with_capacity(6);
    if is_mv {
      menu.push(("USING", "USING <access_method> -- PG12+"));
      menu.push(("WITH", "WITH ( fillfactor = ..., autovacuum_<param> = ..., ... )"));
      menu.push(("TABLESPACE", "TABLESPACE <name>"));
      menu.push(("AS", "AS <select_statement>"));
    } else {
      menu.push(("WITH", "WITH ( security_barrier|security_invoker|check_option )"));
      menu.push(("AS", "AS <select_statement>"));
    }
    // Return the deterministic static menu.
    if is_mv {
      return Some(&[
        ("USING", "USING <access_method> -- PG12+"),
        ("WITH", "WITH ( fillfactor = ..., autovacuum_<param> = ..., ... )"),
        ("TABLESPACE", "TABLESPACE <name>"),
        ("AS", "AS <select_statement>"),
      ]);
    }
    return Some(&[
      ("WITH", "WITH ( security_barrier | security_invoker | check_option )"),
      ("AS", "AS <select_statement>"),
    ]);
  }
  None
}

fn alter_operator_next_keyword(
  source: &str,
  offset: TextSize,
) -> Option<&'static [(&'static str, &'static str)]> {
  if cursor_not_at_ws_boundary(source, offset) {
    return None;
  }
  let (slice_owned, upper) = stmt_slice_upper(source, offset);
  let slice = slice_owned.as_str();
  let words: Vec<&str> = upper.split_ascii_whitespace().collect();
  if words.len() < 2 || words[0] != "ALTER" || words[1] != "OPERATOR" {
    return None;
  }
  let last = *words.last()?;
  // Sub-keyword followups must beat the post-signature action menu so
  // `ALTER OPERATOR + (int,int) SET <cursor>` surfaces SCHEMA, not the
  // whole top-level menu again.
  if last == "OWNER" {
    return Some(&[("TO", "OWNER TO <role>")]);
  }
  if last == "SET" {
    return Some(&[("SCHEMA", "SET SCHEMA <schema>"), ("(", "SET ( RESTRICT = <fn>, JOIN = <fn> )")]);
  }
  // After the (left_type, right_type) signature -> action menu.
  let opens = slice.matches('(').count();
  let closes = slice.matches(')').count();
  if opens > 0 && opens == closes {
    return Some(&[
      ("OWNER TO", "OWNER TO <role>"),
      ("SET SCHEMA", "SET SCHEMA <schema>"),
      ("SET", "SET ( RESTRICT = <fn>, JOIN = <fn> )"),
    ]);
  }
  None
}

/// ALTER AGGREGATE chain.
fn alter_aggregate_next_keyword(
  source: &str,
  offset: TextSize,
) -> Option<&'static [(&'static str, &'static str)]> {
  if cursor_not_at_ws_boundary(source, offset) {
    return None;
  }
  let (slice_owned, upper) = stmt_slice_upper(source, offset);
  let slice = slice_owned.as_str();
  let words: Vec<&str> = upper.split_ascii_whitespace().collect();
  if words.len() < 2 || words[0] != "ALTER" || words[1] != "AGGREGATE" {
    return None;
  }
  let opens = slice.matches('(').count();
  let closes = slice.matches(')').count();
  if opens > 0 && opens == closes {
    return Some(&[
      ("RENAME TO", "RENAME TO <new_name>"),
      ("OWNER TO", "OWNER TO <role>"),
      ("SET SCHEMA", "SET SCHEMA <schema>"),
    ]);
  }
  let last = *words.last()?;
  if last == "RENAME" {
    return Some(&[("TO", "RENAME TO <new_name>")]);
  }
  if last == "OWNER" {
    return Some(&[("TO", "OWNER TO <role>")]);
  }
  if last == "SET" {
    return Some(&[("SCHEMA", "SET SCHEMA <schema>")]);
  }
  None
}

/// ALTER PUBLICATION chain.
///   ALTER PUBLICATION <name> {ADD|SET|DROP} {TABLE|TABLES IN SCHEMA} ...
///   ALTER PUBLICATION <name> SET (publish = '...')
///   ALTER PUBLICATION <name> {RENAME TO | OWNER TO} ...
fn alter_publication_next_keyword(
  source: &str,
  offset: TextSize,
) -> Option<&'static [(&'static str, &'static str)]> {
  if cursor_not_at_ws_boundary(source, offset) {
    return None;
  }
  let (_, upper) = stmt_slice_upper(source, offset);
  let words: Vec<&str> = upper.split_ascii_whitespace().collect();
  if words.len() < 2 || words[0] != "ALTER" || words[1] != "PUBLICATION" {
    return None;
  }
  let last = *words.last()?;
  if matches!(last, "ADD" | "SET" | "DROP") {
    let menu: &[(&str, &str)] = match last {
      "ADD" => &[("TABLE", "ADD TABLE <tbl>[, ...]"), ("TABLES IN SCHEMA", "ADD TABLES IN SCHEMA <schema>[, ...]")],
      "SET" => &[
        ("TABLE", "SET TABLE <tbl>[, ...] -- replace the table list"),
        ("TABLES IN SCHEMA", "SET TABLES IN SCHEMA <schema>[, ...]"),
        ("(", "SET (publish = 'insert, ...')"),
      ],
      "DROP" => &[("TABLE", "DROP TABLE <tbl>[, ...]"), ("TABLES IN SCHEMA", "DROP TABLES IN SCHEMA <schema>[, ...]")],
      _ => &[],
    };
    if !menu.is_empty() {
      return Some(menu);
    }
  }
  if last == "RENAME" {
    return Some(&[("TO", "RENAME TO <new_name>")]);
  }
  if last == "OWNER" {
    return Some(&[("TO", "OWNER TO <role>")]);
  }
  // `... ADD|SET|DROP TABLES IN <cursor>` -> SCHEMA.
  if last == "IN" && words.contains(&"TABLES") {
    return Some(&[("SCHEMA", "TABLES IN SCHEMA <schema>[, ...]")]);
  }
  // `... ADD|SET|DROP TABLES <cursor>` -> IN SCHEMA.
  if last == "TABLES" && (words.contains(&"ADD") || words.contains(&"SET") || words.contains(&"DROP")) {
    return Some(&[("IN SCHEMA", "TABLES IN SCHEMA <schema>[, ...]")]);
  }
  if words.len() >= 3 {
    return Some(&[
      ("ADD", "ADD TABLE / TABLES IN SCHEMA ..."),
      ("SET", "SET TABLE / TABLES IN SCHEMA / (...) ..."),
      ("DROP", "DROP TABLE / TABLES IN SCHEMA ..."),
      ("RENAME TO", "RENAME TO <new_name>"),
      ("OWNER TO", "OWNER TO <role>"),
    ]);
  }
  None
}

/// ALTER SUBSCRIPTION chain.
fn alter_subscription_next_keyword(
  source: &str,
  offset: TextSize,
) -> Option<&'static [(&'static str, &'static str)]> {
  if cursor_not_at_ws_boundary(source, offset) {
    return None;
  }
  let (_, upper) = stmt_slice_upper(source, offset);
  let words: Vec<&str> = upper.split_ascii_whitespace().collect();
  if words.len() < 2 || words[0] != "ALTER" || words[1] != "SUBSCRIPTION" {
    return None;
  }
  let last = *words.last()?;
  if matches!(last, "RENAME" | "OWNER") {
    let menu: &[(&str, &str)] = match last {
      "RENAME" => &[("TO", "RENAME TO <new_name>")],
      "OWNER" => &[("TO", "OWNER TO <role>")],
      _ => &[],
    };
    if !menu.is_empty() {
      return Some(menu);
    }
  }
  if last == "SET" {
    return Some(&[
      ("PUBLICATION", "SET PUBLICATION <pub>[, ...]"),
      ("(", "( slot_name=..., synchronous_commit=..., binary=..., disable_on_error=... )"),
    ]);
  }
  if last == "ADD" {
    return Some(&[("PUBLICATION", "ADD PUBLICATION <pub>[, ...]")]);
  }
  if last == "DROP" {
    return Some(&[("PUBLICATION", "DROP PUBLICATION <pub>[, ...]")]);
  }
  if last == "REFRESH" {
    return Some(&[("PUBLICATION", "REFRESH PUBLICATION [WITH (copy_data = true|false)]")]);
  }
  if last == "SKIP" {
    return Some(&[("(", "SKIP ( lsn = '0/0' )")]);
  }
  if words.len() >= 3 {
    return Some(&[
      ("CONNECTION", "CONNECTION '<conn_string>'"),
      ("SET PUBLICATION", "SET PUBLICATION <pub>[, ...]"),
      ("ADD PUBLICATION", "ADD PUBLICATION <pub>[, ...]"),
      ("DROP PUBLICATION", "DROP PUBLICATION <pub>[, ...]"),
      ("REFRESH PUBLICATION", "REFRESH PUBLICATION [WITH (copy_data = true|false)]"),
      ("ENABLE", "ENABLE"),
      ("DISABLE", "DISABLE"),
      ("SET", "SET (slot_name = ..., ...)"),
      ("RENAME TO", "RENAME TO <new_name>"),
      ("OWNER TO", "OWNER TO <role>"),
      ("SKIP", "SKIP ( lsn = '0/0' )"),
    ]);
  }
  None
}

/// ALTER SCHEMA chain.
/// ALTER DATABASE chain. After name, emit the action menu:
///   RENAME TO / OWNER TO / SET TABLESPACE / REFRESH COLLATION VERSION /
///   WITH ALLOW_CONNECTIONS / CONNECTION LIMIT / IS_TEMPLATE / RESET ALL
///   / SET <param> = <val> / RESET <param>
fn alter_database_next_keyword(
  source: &str,
  offset: TextSize,
) -> Option<&'static [(&'static str, &'static str)]> {
  if cursor_not_at_ws_boundary(source, offset) {
    return None;
  }
  let (_, upper) = stmt_slice_upper(source, offset);
  let words: Vec<&str> = upper.split_ascii_whitespace().collect();
  if words.len() < 2 || words[0] != "ALTER" || words[1] != "DATABASE" {
    return None;
  }
  let last = *words.last()?;
  if matches!(last, "RENAME" | "OWNER" | "SET" | "RESET") {
    let menu: &[(&str, &str)] = match last {
      "RENAME" => &[("TO", "RENAME TO <new_name>")],
      "OWNER" => &[("TO", "OWNER TO <role>")],
      "SET" => &[
        ("TABLESPACE", "SET TABLESPACE <tablespace>"),
        ("ALL", "RESET ALL -- restore every database GUC default"),
      ],
      "RESET" => &[
        ("ALL", "RESET ALL"),
      ],
      _ => &[],
    };
    if !menu.is_empty() {
      return Some(menu);
    }
  }
  if last == "REFRESH" {
    return Some(&[("COLLATION VERSION", "REFRESH COLLATION VERSION -- record current collversion (PG15+)")]);
  }
  if last == "WITH" {
    return Some(&[
      ("ALLOW_CONNECTIONS", "ALLOW_CONNECTIONS true|false"),
      ("CONNECTION LIMIT", "CONNECTION LIMIT <n>"),
      ("IS_TEMPLATE", "IS_TEMPLATE true|false"),
    ]);
  }
  if last == "CONNECTION" {
    return Some(&[("LIMIT", "CONNECTION LIMIT <n>")]);
  }
  if matches!(last, "ALLOW_CONNECTIONS" | "IS_TEMPLATE") {
    return Some(&[("true", "true"), ("false", "false")]);
  }
  // Top-level action menu (after `ALTER DATABASE <name>`).
  if words.len() >= 3 {
    return Some(&[
      ("RENAME TO", "RENAME TO <new_name>"),
      ("OWNER TO", "OWNER TO <role>"),
      ("SET TABLESPACE", "SET TABLESPACE <tablespace>"),
      ("SET", "SET <param> = <val>"),
      ("RESET", "RESET <param> | RESET ALL"),
      ("WITH", "WITH ALLOW_CONNECTIONS|CONNECTION LIMIT|IS_TEMPLATE ..."),
      ("REFRESH COLLATION VERSION", "REFRESH COLLATION VERSION -- PG15+"),
      ("ALLOW_CONNECTIONS", "ALLOW_CONNECTIONS true|false (shorthand without WITH)"),
      ("CONNECTION LIMIT", "CONNECTION LIMIT <n>"),
      ("IS_TEMPLATE", "IS_TEMPLATE true|false"),
    ]);
  }
  None
}

/// ALTER TABLESPACE chain. After name:
///   RENAME TO / OWNER TO / SET ( <param> = <val> ) / RESET ( <param> )
fn alter_tablespace_next_keyword(
  source: &str,
  offset: TextSize,
) -> Option<&'static [(&'static str, &'static str)]> {
  if cursor_not_at_ws_boundary(source, offset) {
    return None;
  }
  let (_, upper) = stmt_slice_upper(source, offset);
  let words: Vec<&str> = upper.split_ascii_whitespace().collect();
  if words.len() < 2 || words[0] != "ALTER" || words[1] != "TABLESPACE" {
    return None;
  }
  let last = *words.last()?;
  if matches!(last, "RENAME" | "OWNER" | "SET" | "RESET") {
    let menu: &[(&str, &str)] = match last {
      "RENAME" => &[("TO", "RENAME TO <new_name>")],
      "OWNER" => &[("TO", "OWNER TO <role>")],
      "SET" => &[("(", "SET ( seq_page_cost = <n>, random_page_cost = <n>, effective_io_concurrency = <n> )")],
      "RESET" => &[("(", "RESET ( <param>[, ...] )")],
      _ => &[],
    };
    if !menu.is_empty() {
      return Some(menu);
    }
  }
  if words.len() >= 3 {
    return Some(&[
      ("RENAME TO", "RENAME TO <new_name>"),
      ("OWNER TO", "OWNER TO <role>"),
      ("SET", "SET ( seq_page_cost = <n>, random_page_cost = <n>, effective_io_concurrency = <n> )"),
      ("RESET", "RESET ( <param>[, ...] )"),
    ]);
  }
  None
}

/// CREATE USER MAPPING chain.
///   CREATE USER MAPPING [IF NOT EXISTS] FOR <role> SERVER <srv> OPTIONS (<k> '<v>', ...)
fn create_user_mapping_next_keyword(
  source: &str,
  offset: TextSize,
) -> Option<&'static [(&'static str, &'static str)]> {
  if cursor_not_at_ws_boundary(source, offset) {
    return None;
  }
  let (_, upper) = stmt_slice_upper(source, offset);
  let words: Vec<&str> = upper.split_ascii_whitespace().collect();
  if words.len() < 3 || words[0] != "CREATE" || words[1] != "USER" || words[2] != "MAPPING" {
    return None;
  }
  let last = *words.last()?;
  if last == "MAPPING" {
    return Some(&[("FOR", "FOR <role> SERVER <name> [OPTIONS (...)]"), ("IF NOT EXISTS", "IF NOT EXISTS -- skip silently when present")]);
  }
  if last == "FOR" {
    return Some(&[
      ("CURRENT_USER", "FOR CURRENT_USER -- current session role"),
      ("CURRENT_ROLE", "FOR CURRENT_ROLE -- synonym"),
      ("PUBLIC", "FOR PUBLIC -- everyone (default fallback)"),
      ("USER", "FOR USER -- alias of CURRENT_USER"),
    ]);
  }
  if last == "SERVER" {
    return None; // user types server name
  }
  if last == "OPTIONS" {
    return Some(&[("(", "( <option_name> '<value>'[, ...] )")]);
  }
  if !words.contains(&"SERVER") && (words.contains(&"CURRENT_USER") || words.contains(&"CURRENT_ROLE") || words.contains(&"PUBLIC") || words.contains(&"USER") || words.len() >= 5) {
    return Some(&[("SERVER", "SERVER <name>")]);
  }
  if words.contains(&"SERVER") && !words.contains(&"OPTIONS") {
    return Some(&[("OPTIONS", "OPTIONS (<key> '<val>'[, ...])")]);
  }
  None
}

/// ALTER USER MAPPING chain.
///   ALTER USER MAPPING FOR <role> SERVER <srv> OPTIONS ( ADD|SET|DROP <k> ['<v>'] )
fn alter_user_mapping_next_keyword(
  source: &str,
  offset: TextSize,
) -> Option<&'static [(&'static str, &'static str)]> {
  if cursor_not_at_ws_boundary(source, offset) {
    return None;
  }
  let (slice_owned, upper) = stmt_slice_upper(source, offset);
  let slice = slice_owned.as_str();
  let words: Vec<&str> = upper.split_ascii_whitespace().collect();
  if words.len() < 3 || words[0] != "ALTER" || words[1] != "USER" || words[2] != "MAPPING" {
    return None;
  }
  let last = *words.last()?;
  if last == "MAPPING" {
    return Some(&[("FOR", "FOR <role> SERVER <name> OPTIONS (...)")]);
  }
  if last == "FOR" {
    return Some(&[
      ("CURRENT_USER", "FOR CURRENT_USER"),
      ("CURRENT_ROLE", "FOR CURRENT_ROLE"),
      ("PUBLIC", "FOR PUBLIC"),
      ("USER", "FOR USER"),
    ]);
  }
  if last == "SERVER" {
    return None;
  }
  if last == "OPTIONS" {
    return Some(&[("(", "( ADD|SET|DROP <k> '<v>'[, ...] )")]);
  }
  // Inside OPTIONS paren -> ADD / SET / DROP at fresh slot.
  let opens = slice.matches('(').count();
  let closes = slice.matches(')').count();
  if opens > closes {
    let trimmed = slice.trim_end();
    let last_char = trimmed.chars().last();
    if matches!(last_char, Some('(') | Some(',')) || slice.ends_with(char::is_whitespace) {
      return Some(&[
        ("ADD", "ADD <key> '<value>'"),
        ("SET", "SET <key> '<value>'"),
        ("DROP", "DROP <key>"),
      ]);
    }
  }
  if words.contains(&"SERVER") && !words.contains(&"OPTIONS") {
    return Some(&[("OPTIONS", "OPTIONS (<key> '<val>'[, ...])")]);
  }
  if !words.contains(&"SERVER") && (words.contains(&"CURRENT_USER") || words.contains(&"CURRENT_ROLE") || words.contains(&"PUBLIC") || words.contains(&"USER") || words.len() >= 5) {
    return Some(&[("SERVER", "SERVER <name>")]);
  }
  None
}

fn alter_schema_next_keyword(source: &str, offset: TextSize) -> Option<&'static [(&'static str, &'static str)]> {
  if cursor_not_at_ws_boundary(source, offset) {
    return None;
  }
  let (_, upper) = stmt_slice_upper(source, offset);
  let words: Vec<&str> = upper.split_ascii_whitespace().collect();
  if words.len() < 2 || words[0] != "ALTER" || words[1] != "SCHEMA" {
    return None;
  }
  if words.len() >= 3 {
    return Some(&[("RENAME TO", "RENAME TO <new_name>"), ("OWNER TO", "OWNER TO <role>")]);
  }
  None
}

/// CREATE TEXT SEARCH {CONFIGURATION|DICTIONARY|PARSER|TEMPLATE} phase chain.
fn create_text_search_next_keyword(
  source: &str,
  offset: TextSize,
) -> Option<&'static [(&'static str, &'static str)]> {
  let pos: usize = (u32::from(offset) as usize).min(source.len());
  let bytes = source.as_bytes();
  if pos < bytes.len() && !bytes[pos].is_ascii_whitespace() && bytes[pos] != b'(' && bytes[pos] != b',' {
    return None;
  }
  let (slice_owned, upper) = stmt_slice_upper(source, offset);
  let slice = slice_owned.as_str();
  let words: Vec<&str> = upper.split_ascii_whitespace().collect();
  if words.len() < 3 || words[0] != "CREATE" || words[1] != "TEXT" || words[2] != "SEARCH" {
    return None;
  }
  if words.len() < 4 {
    return Some(&[
      ("CONFIGURATION", "CREATE TEXT SEARCH CONFIGURATION <name> ( PARSER = ... )"),
      ("DICTIONARY", "CREATE TEXT SEARCH DICTIONARY <name> ( TEMPLATE = ... )"),
      ("PARSER", "CREATE TEXT SEARCH PARSER <name> ( START = ..., GETTOKEN = ..., END = ..., LEXTYPES = ... )"),
      ("TEMPLATE", "CREATE TEXT SEARCH TEMPLATE <name> ( INIT = ..., LEXIZE = ... )"),
    ]);
  }
  let kind = words[3];
  let opens = slice.matches('(').count();
  let closes = slice.matches(')').count();
  if opens > closes {
    let trimmed = slice.trim_end();
    let last_char = trimmed.chars().last();
    if !matches!(last_char, Some('(') | Some(',')) && !slice.ends_with(char::is_whitespace) {
      return None;
    }
    return Some(match kind {
      "CONFIGURATION" => &[
        ("PARSER", "PARSER = <parser>"),
        ("COPY", "COPY = <existing_config>"),
      ],
      "DICTIONARY" => &[("TEMPLATE", "TEMPLATE = <template>")],
      "PARSER" => &[
        ("START", "START = <function>"),
        ("GETTOKEN", "GETTOKEN = <function>"),
        ("END", "END = <function>"),
        ("LEXTYPES", "LEXTYPES = <function>"),
        ("HEADLINE", "HEADLINE = <function>"),
      ],
      "TEMPLATE" => &[("INIT", "INIT = <function>"), ("LEXIZE", "LEXIZE = <function>")],
      _ => &[],
    });
  }
  None
}

/// CREATE EXTENSION chain.
///   CREATE EXTENSION [IF NOT EXISTS] <name> [WITH] [SCHEMA <schema>]
///     [VERSION <ver>] [CASCADE]
fn create_extension_next_keyword(
  source: &str,
  offset: TextSize,
) -> Option<&'static [(&'static str, &'static str)]> {
  if cursor_not_at_ws_boundary(source, offset) {
    return None;
  }
  let (_, upper) = stmt_slice_upper(source, offset);
  let words: Vec<&str> = upper.split_ascii_whitespace().collect();
  if words.len() < 2 || words[0] != "CREATE" || words[1] != "EXTENSION" {
    return None;
  }
  let last = *words.last()?;
  if matches!(last, "SCHEMA" | "VERSION") {
    return None; // user types the value
  }
  if last == "WITH" {
    return Some(&[("SCHEMA", "WITH SCHEMA <schema>"), ("VERSION", "WITH VERSION '<ver>'"), ("CASCADE", "WITH CASCADE -- auto-install required extensions")]);
  }
  if words.len() >= 3 {
    return Some(&[
      ("WITH", "WITH SCHEMA / VERSION / CASCADE clarifier"),
      ("SCHEMA", "SCHEMA <schema>"),
      ("VERSION", "VERSION '<ver>'"),
      ("CASCADE", "CASCADE -- auto-install required extensions"),
    ]);
  }
  None
}

/// ALTER FUNCTION / ALTER PROCEDURE chain after the function signature.
/// CREATE [OR REPLACE] FUNCTION/PROCEDURE attribute chain. Fires after
/// the return type (or after the `(args)` paren for PROCEDURE) and
/// before `AS $$ ... $$` / `RETURN <expr>`. Emits the attribute menu.
fn create_function_attribute_next_keyword(
  source: &str,
  offset: TextSize,
) -> Option<&'static [(&'static str, &'static str)]> {
  if cursor_not_at_ws_boundary(source, offset) {
    return None;
  }
  let (_, upper) = stmt_slice_upper(source, offset);
  let trimmed = upper.trim_start();
  let is_fn = trimmed.starts_with("CREATE FUNCTION")
    || trimmed.starts_with("CREATE OR REPLACE FUNCTION");
  let is_proc = trimmed.starts_with("CREATE PROCEDURE")
    || trimmed.starts_with("CREATE OR REPLACE PROCEDURE");
  if !is_fn && !is_proc {
    return None;
  }
  let words: Vec<&str> = upper.split_ascii_whitespace().collect();
  let last = *words.last()?;
  // Inside or right after dollar-quoted body -- bail out (PL/pgSQL body
  // owns those slots). Heuristic: count `$$` markers.
  let dq = upper.matches("$$").count();
  if dq > 0 && dq % 2 == 1 {
    return None;
  }
  // After LANGUAGE -> language name slot (no kw).
  if last == "LANGUAGE" {
    return Some(&[
      ("plpgsql", "plpgsql -- procedural Postgres"),
      ("sql", "sql -- pure-SQL function body"),
      ("c", "c -- C extension"),
      ("plpython3u", "plpython3u -- Python 3 (untrusted)"),
      ("plperl", "plperl -- Perl"),
      ("pltcl", "pltcl -- Tcl"),
    ]);
  }
  // After SECURITY -> DEFINER / INVOKER.
  if last == "SECURITY" {
    return Some(&[
      ("DEFINER", "SECURITY DEFINER -- run with definer's privileges"),
      ("INVOKER", "SECURITY INVOKER -- run with caller's privileges (default)"),
    ]);
  }
  // After PARALLEL -> SAFE / RESTRICTED / UNSAFE.
  if last == "PARALLEL" {
    return Some(&[
      ("SAFE", "PARALLEL SAFE -- safe for parallel workers"),
      ("RESTRICTED", "PARALLEL RESTRICTED -- leader only"),
      ("UNSAFE", "PARALLEL UNSAFE -- forbid parallel (default)"),
    ]);
  }
  // After RETURNS -- delegate to existing detector (returns type slot
  // suppresses the keyword menu). Don't fire here.
  if upper.rfind("RETURNS")
    .map(|p| upper[p + "RETURNS".len()..].trim().is_empty())
    .unwrap_or(false)
  {
    return None;
  }
  // Procedure form: after the closing `)` of the args paren -- need the
  // paren counts balanced AND no LANGUAGE yet.
  // Function form: after RETURNS <type>.
  let opens = upper.matches('(').count();
  let closes = upper.matches(')').count();
  let paren_balanced = opens > 0 && opens == closes;
  let has_returns = upper.contains("RETURNS");
  if is_proc && !paren_balanced {
    return None;
  }
  if is_fn && !has_returns {
    return None;
  }
  // After RETURNS, ensure the return type token is present.
  if is_fn
    && let Some(p) = upper.rfind("RETURNS")
  {
    let after = upper[p + "RETURNS".len()..].trim();
    if after.is_empty() {
      return None;
    }
    // First token after RETURNS = type or SETOF/TABLE. Be liberal --
    // if at least one identifier-like token sits there, treat the type
    // as committed.
    if after.split_ascii_whitespace().next().is_none_or(|t| t.is_empty()) {
      return None;
    }
  }
  // After AS -- body literal slot. The PL/pgSQL body handler owns it.
  if last == "AS" {
    return None;
  }
  Some(&[
    ("LANGUAGE", "LANGUAGE plpgsql|sql|c|plpythonu|... -- pick body language"),
    ("AS", "AS $$ <body> $$ -- function body"),
    ("COST", "COST <n> -- per-row execution cost hint"),
    ("ROWS", "ROWS <n> -- estimated set-returning row count"),
    ("VOLATILE", "VOLATILE (default) -- result may change across calls"),
    ("STABLE", "STABLE -- same result within a single statement"),
    ("IMMUTABLE", "IMMUTABLE -- same args always yield the same result"),
    ("STRICT", "STRICT -- skip body when any arg is NULL (synonym of RETURNS NULL ON NULL INPUT)"),
    ("CALLED ON NULL INPUT", "CALLED ON NULL INPUT (default)"),
    ("RETURNS NULL ON NULL INPUT", "RETURNS NULL ON NULL INPUT (synonym of STRICT)"),
    ("LEAKPROOF", "LEAKPROOF -- doesn't leak side-info; required for some RLS views"),
    ("SECURITY DEFINER", "SECURITY DEFINER -- run as the role that defined the fn"),
    ("SECURITY INVOKER", "SECURITY INVOKER (default)"),
    ("PARALLEL", "PARALLEL SAFE | RESTRICTED | UNSAFE"),
    ("WINDOW", "WINDOW -- mark this fn as a window fn (C only)"),
    ("SET", "SET <param> [TO|=] <value> -- per-call GUC override"),
    ("SUPPORT", "SUPPORT <fn> -- planner-support function (C only)"),
    ("TRANSFORM", "TRANSFORM FOR TYPE <type> -- attach a TRANSFORM"),
    ("DEPENDS ON EXTENSION", "DEPENDS ON EXTENSION <ext>"),
  ])
}

fn alter_function_next_keyword(
  source: &str,
  offset: TextSize,
) -> Option<&'static [(&'static str, &'static str)]> {
  if cursor_not_at_ws_boundary(source, offset) {
    return None;
  }
  let (_, upper) = stmt_slice_upper(source, offset);
  let words: Vec<&str> = upper.split_ascii_whitespace().collect();
  if words.len() < 2 || words[0] != "ALTER" || !matches!(words[1], "FUNCTION" | "PROCEDURE" | "ROUTINE") {
    return None;
  }
  let last = *words.last()?;
  if matches!(last, "RENAME" | "OWNER" | "SET" | "RESET" | "DEPENDS" | "PARALLEL" | "SECURITY" | "LANGUAGE" | "SUPPORT") {
    let menu: &[(&str, &str)] = match last {
      "RENAME" => &[("TO", "RENAME TO <new_name>")],
      "OWNER" => &[("TO", "OWNER TO <role>")],
      "SET" => &[("SCHEMA", "SET SCHEMA <schema>"), ("ROLE", "SET ROLE <role>"), ("SEARCH_PATH", "SET search_path = '...'")],
      "RESET" => &[("ALL", "RESET ALL -- restore all session GUCs")],
      "DEPENDS" => &[("ON EXTENSION", "DEPENDS ON EXTENSION <ext>")],
      "PARALLEL" => &[
        ("SAFE", "PARALLEL SAFE -- usable inside parallel workers"),
        ("RESTRICTED", "PARALLEL RESTRICTED -- only the leader runs it"),
        ("UNSAFE", "PARALLEL UNSAFE -- forces a non-parallel plan"),
      ],
      "SECURITY" => &[
        ("DEFINER", "SECURITY DEFINER -- runs with the function owner's rights"),
        ("INVOKER", "SECURITY INVOKER -- runs with the caller's rights (default)"),
      ],
      "LANGUAGE" => &[
        ("sql", "LANGUAGE sql -- pure SQL function body"),
        ("plpgsql", "LANGUAGE plpgsql -- PL/pgSQL function body"),
        ("c", "LANGUAGE c -- C-language function (requires AS '<obj>')"),
        ("plpython3u", "LANGUAGE plpython3u -- PL/Python (untrusted)"),
        ("plperl", "LANGUAGE plperl -- PL/Perl (trusted)"),
        ("plperlu", "LANGUAGE plperlu -- PL/Perl (untrusted)"),
        ("pltcl", "LANGUAGE pltcl -- PL/Tcl"),
        ("internal", "LANGUAGE internal -- bound to a builtin symbol"),
      ],
      "SUPPORT" => &[("support_fn", "SUPPORT <support_fn_name> -- planner support function (advanced)")],
      _ => &[],
    };
    if !menu.is_empty() {
      return Some(menu);
    }
  }
  // After the (args) signature -> action menu.
  let opens = upper.matches('(').count();
  let closes = upper.matches(')').count();
  if opens > 0 && opens == closes {
    return Some(&[
      ("RENAME TO", "RENAME TO <new_name>"),
      ("OWNER TO", "OWNER TO <role>"),
      ("SET SCHEMA", "SET SCHEMA <schema>"),
      ("COST", "COST <n>"),
      ("ROWS", "ROWS <n>"),
      ("VOLATILE", "VOLATILE / STABLE / IMMUTABLE"),
      ("STABLE", "STABLE"),
      ("IMMUTABLE", "IMMUTABLE"),
      ("STRICT", "STRICT / RETURNS NULL ON NULL INPUT"),
      ("LEAKPROOF", "LEAKPROOF"),
      ("SECURITY DEFINER", "SECURITY DEFINER"),
      ("SECURITY INVOKER", "SECURITY INVOKER"),
      ("PARALLEL", "PARALLEL SAFE | RESTRICTED | UNSAFE"),
      ("DEPENDS ON EXTENSION", "DEPENDS ON EXTENSION <ext>"),
    ]);
  }
  None
}

/// ALTER VIEW chain.
/// ALTER STATISTICS chain.
///   ALTER STATISTICS <name> { RENAME TO <new>
///                            | SET SCHEMA <schema>
///                            | SET STATISTICS <n>
///                            | OWNER TO <role> }
fn alter_statistics_next_keyword(
  source: &str,
  offset: TextSize,
) -> Option<&'static [(&'static str, &'static str)]> {
  if cursor_not_at_ws_boundary(source, offset) {
    return None;
  }
  let (_, upper) = stmt_slice_upper(source, offset);
  let words: Vec<&str> = upper.split_ascii_whitespace().collect();
  if words.len() < 2 || words[0] != "ALTER" || words[1] != "STATISTICS" {
    return None;
  }
  let last = *words.last()?;
  if matches!(last, "RENAME" | "OWNER" | "SET") {
    let menu: &[(&str, &str)] = match last {
      "RENAME" => &[("TO", "RENAME TO <new>")],
      "OWNER" => &[("TO", "OWNER TO <role>")],
      "SET" => &[
        ("SCHEMA", "SET SCHEMA <schema>"),
        ("STATISTICS", "SET STATISTICS <n> -- per-extended-stats target (PG13+)"),
      ],
      _ => &[],
    };
    if !menu.is_empty() {
      return Some(menu);
    }
  }
  if words.len() >= 3 {
    return Some(&[
      ("RENAME TO", "RENAME TO <new>"),
      ("OWNER TO", "OWNER TO <role>"),
      ("SET SCHEMA", "SET SCHEMA <schema>"),
      ("SET STATISTICS", "SET STATISTICS <n> -- PG13+"),
    ]);
  }
  None
}

fn alter_view_next_keyword(source: &str, offset: TextSize) -> Option<&'static [(&'static str, &'static str)]> {
  if cursor_not_at_ws_boundary(source, offset) {
    return None;
  }
  let (_, upper) = stmt_slice_upper(source, offset);
  let words: Vec<&str> = upper.split_ascii_whitespace().collect();
  if words.len() < 2 || words[0] != "ALTER" || words[1] != "VIEW" {
    return None;
  }
  let last = *words.last()?;
  if matches!(last, "RENAME" | "OWNER" | "SET" | "RESET" | "ALTER") {
    let menu: &[(&str, &str)] = match last {
      "RENAME" => &[("TO", "RENAME TO <new_name>"), ("COLUMN", "RENAME COLUMN <old> TO <new>")],
      "OWNER" => &[("TO", "OWNER TO <role>")],
      "SET" => &[("SCHEMA", "SET SCHEMA <schema>"), ("(", "SET ( <storage_param> = <value> )")],
      "RESET" => &[("(", "RESET ( <storage_param> )")],
      "ALTER" => &[("COLUMN", "ALTER COLUMN <name> SET DEFAULT <expr>")],
      _ => &[],
    };
    if !menu.is_empty() {
      return Some(menu);
    }
  }
  if words.len() >= 3 && !matches!(last, "RENAME" | "OWNER" | "SET" | "RESET" | "ALTER") {
    return Some(&[
      ("RENAME TO", "RENAME TO <new_name>"),
      ("RENAME COLUMN", "RENAME COLUMN <old> TO <new>"),
      ("OWNER TO", "OWNER TO <role>"),
      ("SET SCHEMA", "SET SCHEMA <schema>"),
      ("SET (", "SET ( security_barrier = true, ... )"),
      ("RESET (", "RESET ( <param> )"),
      ("ALTER COLUMN", "ALTER COLUMN <name> SET DEFAULT <expr>"),
    ]);
  }
  None
}

/// CREATE TABLESPACE chain.
///   CREATE TABLESPACE <name> [OWNER <role>] LOCATION '<dir>' [WITH (...)]
/// CREATE DATABASE chain. After name, emit the option keywords:
///   [WITH] OWNER = <role>
///         TEMPLATE = <db>
///         ENCODING = '<charset>'
///         LOCALE = '<locale>' / LC_COLLATE = ... / LC_CTYPE = ...
///         LOCALE_PROVIDER = libc | icu | builtin
///         ICU_LOCALE = '<icu>' / ICU_RULES = ...
///         COLLATION_VERSION = '<v>'
///         BUILTIN_LOCALE = '<l>'
///         TABLESPACE = <name>
///         ALLOW_CONNECTIONS = true|false
///         CONNECTION LIMIT = <n>
///         IS_TEMPLATE = true|false
///         OID = <n>
///         STRATEGY = wal_log | file_copy
fn create_database_next_keyword(
  source: &str,
  offset: TextSize,
) -> Option<&'static [(&'static str, &'static str)]> {
  if cursor_not_at_ws_boundary(source, offset) {
    return None;
  }
  let (_, upper) = stmt_slice_upper(source, offset);
  let words: Vec<&str> = upper.split_ascii_whitespace().collect();
  if words.len() < 2 || words[0] != "CREATE" || words[1] != "DATABASE" {
    return None;
  }
  let last = *words.last()?;
  // Value slots -- silence.
  if matches!(
    last,
    "OWNER" | "TEMPLATE" | "ENCODING" | "LOCALE" | "LC_COLLATE" | "LC_CTYPE"
      | "ICU_LOCALE" | "ICU_RULES" | "COLLATION_VERSION" | "BUILTIN_LOCALE"
      | "TABLESPACE" | "OID"
  ) {
    return None;
  }
  if last == "LIMIT" && words.contains(&"CONNECTION") {
    return None; // user types <n>
  }
  if last == "WITH" {
    // After WITH, same option menu (the WITH keyword is optional in PG).
    // Fall through to the regular menu below.
  }
  if last == "LOCALE_PROVIDER" {
    return Some(&[
      ("libc", "LOCALE_PROVIDER libc -- default; OS-supplied collation"),
      ("icu", "LOCALE_PROVIDER icu -- ICU library collation"),
      ("builtin", "LOCALE_PROVIDER builtin -- PG16+ built-in basic locales"),
    ]);
  }
  if last == "ALLOW_CONNECTIONS" || last == "IS_TEMPLATE" {
    return Some(&[("true", "true"), ("false", "false")]);
  }
  if last == "STRATEGY" {
    return Some(&[
      ("wal_log", "wal_log -- default; copy via WAL records (safe for replicas)"),
      ("file_copy", "file_copy -- copy template files directly (faster, but bypasses WAL)"),
    ]);
  }
  if last == "CONNECTION" {
    return Some(&[("LIMIT", "CONNECTION LIMIT <n> -- per-database parallel-session cap")]);
  }
  // Fresh option slot.
  if words.len() >= 3 {
    return Some(&[
      ("WITH", "WITH <options> -- noise word (optional)"),
      ("OWNER", "OWNER = <role>"),
      ("TEMPLATE", "TEMPLATE = <existing_db>"),
      ("ENCODING", "ENCODING = '<charset>'"),
      ("LOCALE", "LOCALE = '<locale>' -- shorthand for LC_COLLATE + LC_CTYPE"),
      ("LC_COLLATE", "LC_COLLATE = '<locale>'"),
      ("LC_CTYPE", "LC_CTYPE = '<locale>'"),
      ("LOCALE_PROVIDER", "LOCALE_PROVIDER libc | icu | builtin"),
      ("ICU_LOCALE", "ICU_LOCALE = '<icu_locale>'"),
      ("ICU_RULES", "ICU_RULES = '<icu_rule_string>' -- PG16+"),
      ("BUILTIN_LOCALE", "BUILTIN_LOCALE = 'C' | 'C.UTF-8' -- PG17+"),
      ("COLLATION_VERSION", "COLLATION_VERSION = '<v>' -- set the collation-version sentinel"),
      ("TABLESPACE", "TABLESPACE = <tablespace>"),
      ("ALLOW_CONNECTIONS", "ALLOW_CONNECTIONS true|false"),
      ("CONNECTION LIMIT", "CONNECTION LIMIT <n>"),
      ("IS_TEMPLATE", "IS_TEMPLATE true|false"),
      ("OID", "OID = <n> -- mostly internal; pg_upgrade uses it"),
      ("STRATEGY", "STRATEGY wal_log | file_copy"),
    ]);
  }
  None
}

fn create_tablespace_next_keyword(
  source: &str,
  offset: TextSize,
) -> Option<&'static [(&'static str, &'static str)]> {
  if cursor_not_at_ws_boundary(source, offset) {
    return None;
  }
  let (_, upper) = stmt_slice_upper(source, offset);
  let words: Vec<&str> = upper.split_ascii_whitespace().collect();
  if words.len() < 2 || words[0] != "CREATE" || words[1] != "TABLESPACE" {
    return None;
  }
  let last = *words.last()?;
  if last == "OWNER" {
    return None; // user types role
  }
  if last == "LOCATION" {
    return None; // user types '<dir>'
  }
  if words.len() >= 3 && !words.contains(&"LOCATION") {
    return Some(&[("OWNER", "OWNER <role>"), ("LOCATION", "LOCATION '<directory_path>' -- required")]);
  }
  if words.contains(&"LOCATION") && !words.contains(&"WITH") {
    return Some(&[("WITH", "WITH ( <param> = <value>, ... )")]);
  }
  None
}

/// CREATE AGGREGATE paren options.
fn create_aggregate_next_keyword(
  source: &str,
  offset: TextSize,
) -> Option<&'static [(&'static str, &'static str)]> {
  let pos: usize = (u32::from(offset) as usize).min(source.len());
  let bytes = source.as_bytes();
  if pos < bytes.len() && !bytes[pos].is_ascii_whitespace() && bytes[pos] != b'(' && bytes[pos] != b',' {
    return None;
  }
  let (slice_owned, upper) = stmt_slice_upper(source, offset);
  let slice = slice_owned.as_str();
  let words: Vec<&str> = upper.split_ascii_whitespace().collect();
  if words.len() < 2 || words[0] != "CREATE" || words[1] != "AGGREGATE" {
    return None;
  }
  // Must be at fresh-option slot inside the SECOND paren (after arg-types paren).
  let opens = slice.matches('(').count();
  let closes = slice.matches(')').count();
  if opens < 2 || opens - closes != 1 {
    return None;
  }
  let trimmed = slice.trim_end();
  let last_char = trimmed.chars().last();
  if !matches!(last_char, Some('(') | Some(',')) && !slice.ends_with(char::is_whitespace) {
    return None;
  }
  Some(&[
    ("SFUNC", "SFUNC = <transition_fn> -- required"),
    ("STYPE", "STYPE = <state_type> -- required"),
    ("SSPACE", "SSPACE = <avg_state_bytes>"),
    ("FINALFUNC", "FINALFUNC = <final_fn>"),
    ("FINALFUNC_EXTRA", "FINALFUNC_EXTRA -- pass extra args to FINALFUNC"),
    ("FINALFUNC_MODIFY", "FINALFUNC_MODIFY = READ_ONLY | SHAREABLE | READ_WRITE"),
    ("COMBINEFUNC", "COMBINEFUNC = <merge_fn> -- enables parallel aggregation"),
    ("SERIALFUNC", "SERIALFUNC = <serialize_fn>"),
    ("DESERIALFUNC", "DESERIALFUNC = <deserialize_fn>"),
    ("INITCOND", "INITCOND = '<state_initial>'"),
    ("MSFUNC", "MSFUNC = <moving_state_fn>"),
    ("MINVFUNC", "MINVFUNC = <inverse_state_fn>"),
    ("MSTYPE", "MSTYPE = <moving_state_type>"),
    ("MSSPACE", "MSSPACE = <avg_bytes>"),
    ("MFINALFUNC", "MFINALFUNC = <final_fn>"),
    ("MFINALFUNC_EXTRA", "MFINALFUNC_EXTRA"),
    ("MFINALFUNC_MODIFY", "MFINALFUNC_MODIFY = ..."),
    ("MINITCOND", "MINITCOND = '<state_initial>'"),
    ("SORTOP", "SORTOP = <operator>"),
    ("PARALLEL", "PARALLEL = SAFE | RESTRICTED | UNSAFE"),
    ("HYPOTHETICAL", "HYPOTHETICAL -- ordered-set hypothetical aggregate"),
  ])
}

/// CREATE CAST phase chain.
///   CREATE CAST (src AS dst) {WITH FUNCTION <fn>(args) | WITHOUT FUNCTION | WITH INOUT}
///     [AS ASSIGNMENT | AS IMPLICIT]
fn create_cast_next_keyword(source: &str, offset: TextSize) -> Option<&'static [(&'static str, &'static str)]> {
  if cursor_not_at_ws_boundary(source, offset) {
    return None;
  }
  let (_, upper) = stmt_slice_upper(source, offset);
  let words: Vec<&str> = upper.split_ascii_whitespace().collect();
  if words.len() < 2 || words[0] != "CREATE" || words[1] != "CAST" {
    return None;
  }
  let last = *words.last()?;
  if last == "WITH" {
    return Some(&[
      ("FUNCTION", "WITH FUNCTION <fn>(args) -- use this function"),
      ("INOUT", "WITH INOUT -- use IO conversion"),
    ]);
  }
  if last == "WITHOUT" {
    return Some(&[("FUNCTION", "WITHOUT FUNCTION -- binary-compatible cast")]);
  }
  if last == "AS" {
    return Some(&[("ASSIGNMENT", "AS ASSIGNMENT -- automatic in assignments"), ("IMPLICIT", "AS IMPLICIT -- automatic everywhere")]);
  }
  // After the (src AS dst) header -> WITH | WITHOUT.
  let closes = upper.matches(')').count();
  let opens = upper.matches('(').count();
  if opens > 0 && closes == opens && !words.contains(&"WITH") && !words.contains(&"WITHOUT") {
    return Some(&[
      ("WITH FUNCTION", "WITH FUNCTION <fn>(args)"),
      ("WITHOUT FUNCTION", "WITHOUT FUNCTION -- binary-compatible"),
      ("WITH INOUT", "WITH INOUT -- IO conversion"),
    ]);
  }
  // Trailing AS slot once WITH/WITHOUT chosen.
  if (words.contains(&"WITH") || words.contains(&"WITHOUT")) && !words.contains(&"AS") {
    return Some(&[("AS ASSIGNMENT", "AS ASSIGNMENT -- automatic in assignments"), ("AS IMPLICIT", "AS IMPLICIT -- automatic everywhere")]);
  }
  None
}

/// CREATE RULE phase chain.
///   CREATE [OR REPLACE] RULE <name> AS ON {SELECT|INSERT|UPDATE|DELETE}
///       TO <table> [WHERE <expr>] DO [ALSO|INSTEAD] {NOTHING | <command>}
fn create_rule_next_keyword(source: &str, offset: TextSize) -> Option<&'static [(&'static str, &'static str)]> {
  if cursor_not_at_ws_boundary(source, offset) {
    return None;
  }
  let (_, upper) = stmt_slice_upper(source, offset);
  let words: Vec<&str> = upper.split_ascii_whitespace().collect();
  let rule_idx = words.iter().position(|w| *w == "RULE")?;
  if !words[..rule_idx].iter().all(|w| matches!(*w, "CREATE" | "OR" | "REPLACE")) || words[0] != "CREATE" {
    return None;
  }
  let tail = &words[rule_idx + 1..];
  if tail.is_empty() {
    return None;
  }
  let last = *tail.last()?;
  if last == "AS" {
    return Some(&[("ON", "AS ON {SELECT | INSERT | UPDATE | DELETE}")]);
  }
  if last == "ON" {
    return Some(&[("SELECT", "ON SELECT"), ("INSERT", "ON INSERT"), ("UPDATE", "ON UPDATE"), ("DELETE", "ON DELETE")]);
  }
  if matches!(last, "SELECT" | "INSERT" | "UPDATE" | "DELETE") {
    return Some(&[("TO", "TO <table>")]);
  }
  if last == "DO" {
    return Some(&[("ALSO", "DO ALSO <command>"), ("INSTEAD", "DO INSTEAD {NOTHING | <command>}"), ("NOTHING", "DO NOTHING")]);
  }
  if last == "INSTEAD" {
    return Some(&[("NOTHING", "INSTEAD NOTHING"), ("SELECT", "INSTEAD SELECT ..."), ("INSERT", "INSTEAD INSERT ..."), ("UPDATE", "INSTEAD UPDATE ..."), ("DELETE", "INSTEAD DELETE ...")]);
  }
  if tail.contains(&"TO") && !tail.contains(&"DO") {
    return Some(&[("WHERE", "WHERE <predicate>"), ("DO", "DO {ALSO|INSTEAD} {NOTHING | <command>}")]);
  }
  None
}

/// CREATE STATISTICS phase chain.
///   CREATE STATISTICS [IF NOT EXISTS] <name> [(<kind>[, ...])] ON <col_list> FROM <table>
/// CREATE TABLE post-body trailing clauses:
///   CREATE TABLE t (...) <cursor>
///     -> INHERITS / PARTITION BY / USING / WITH / WITHOUT OIDS /
///        ON COMMIT / TABLESPACE
fn create_table_post_body_next_keyword(
  source: &str,
  offset: TextSize,
) -> Option<&'static [(&'static str, &'static str)]> {
  if cursor_not_at_ws_boundary(source, offset) {
    return None;
  }
  let (slice_owned, upper) = stmt_slice_upper(source, offset);
  let slice = slice_owned.as_str();
  let trimmed = upper.trim_start();
  if !trimmed.starts_with("CREATE TABLE")
    && !trimmed.starts_with("CREATE TEMP TABLE")
    && !trimmed.starts_with("CREATE TEMPORARY TABLE")
    && !trimmed.starts_with("CREATE UNLOGGED TABLE")
    && !trimmed.starts_with("CREATE GLOBAL TABLE")
    && !trimmed.starts_with("CREATE LOCAL TABLE")
  {
    return None;
  }
  // Body paren must have closed.
  let opens = slice.matches('(').count();
  let closes = slice.matches(')').count();
  if opens == 0 || opens != closes {
    return None;
  }
  let words: Vec<&str> = upper.split_ascii_whitespace().collect();
  let last = *words.last()?;
  // After INHERITS / PARTITION / WITH / WITHOUT / USING / ON / TABLESPACE,
  // continue the sub-chain.
  if last == "INHERITS" {
    return Some(&[("(", "( <parent_table>[, ...] )")]);
  }
  if last == "PARTITION" {
    return Some(&[("BY", "PARTITION BY { RANGE | LIST | HASH } (<cols>)")]);
  }
  if last == "WITHOUT" {
    return Some(&[("OIDS", "WITHOUT OIDS -- legacy; no effect in PG12+")]);
  }
  if last == "WITH" {
    return Some(&[
      ("OIDS", "WITH OIDS -- legacy; rejected in PG12+"),
      ("(", "( fillfactor=<n>, autovacuum_<param>=<v>, ... )"),
    ]);
  }
  if last == "ON" {
    return Some(&[("COMMIT", "ON COMMIT { PRESERVE ROWS | DELETE ROWS | DROP }")]);
  }
  if last == "COMMIT" && words.contains(&"ON") {
    return Some(&[
      ("PRESERVE ROWS", "ON COMMIT PRESERVE ROWS (default for TEMP)"),
      ("DELETE ROWS", "ON COMMIT DELETE ROWS -- empty rows at COMMIT"),
      ("DROP", "ON COMMIT DROP -- drop the temp table at COMMIT"),
    ]);
  }
  // Fresh trailing slot menu.
  Some(&[
    ("INHERITS", "INHERITS ( <parent>[, ...] )"),
    ("PARTITION BY", "PARTITION BY { RANGE | LIST | HASH } (<cols>)"),
    ("USING", "USING <access_method>"),
    ("WITH", "WITH ( fillfactor=<n>, autovacuum_<param>=<v>, ... )"),
    ("WITHOUT OIDS", "WITHOUT OIDS -- legacy"),
    ("ON COMMIT", "ON COMMIT { PRESERVE ROWS | DELETE ROWS | DROP }"),
    ("TABLESPACE", "TABLESPACE <name>"),
  ])
}

/// EXCLUDE constraint chain inside CREATE TABLE body:
///   EXCLUDE <cursor>                  -> USING / (
///   EXCLUDE USING <cursor>            -> btree / gist / spgist / brin (index methods)
///   EXCLUDE USING gist (<cursor>      -> column or expression (catalog)
///   EXCLUDE ... WITH <cursor>         -> operator placeholder (no kw)
fn exclude_constraint_next_keyword(
  source: &str,
  offset: TextSize,
) -> Option<&'static [(&'static str, &'static str)]> {
  if cursor_not_at_ws_boundary(source, offset) {
    return None;
  }
  let (_, upper) = stmt_slice_upper(source, offset);
  if !upper.contains("EXCLUDE") {
    return None;
  }
  if !upper.contains("CREATE TABLE")
    && !upper.contains("ALTER TABLE")
  {
    return None;
  }
  let words: Vec<&str> = upper.split_ascii_whitespace().collect();
  let last = *words.last()?;
  if last == "EXCLUDE" {
    return Some(&[
      ("USING", "USING <index_method>"),
      ("(", "( <col> WITH <op>, ... ) -- exclusion list"),
    ]);
  }
  if last == "USING" && words.contains(&"EXCLUDE") {
    return Some(&[
      ("gist", "gist -- generalized search tree (recommended for ranges)"),
      ("spgist", "spgist -- space-partitioned GiST"),
      ("btree", "btree -- only for equality EXCLUDE"),
      ("brin", "brin -- block range index"),
    ]);
  }
  None
}

fn create_statistics_next_keyword(
  source: &str,
  offset: TextSize,
) -> Option<&'static [(&'static str, &'static str)]> {
  let pos: usize = (u32::from(offset) as usize).min(source.len());
  let bytes = source.as_bytes();
  if pos < bytes.len() && !bytes[pos].is_ascii_whitespace() && bytes[pos] != b'(' && bytes[pos] != b',' {
    return None;
  }
  let (slice_owned, upper) = stmt_slice_upper(source, offset);
  let slice = slice_owned.as_str();
  let words: Vec<&str> = upper.split_ascii_whitespace().collect();
  if words.len() < 2 || words[0] != "CREATE" || words[1] != "STATISTICS" {
    return None;
  }
  let last = *words.last()?;
  let opens = slice.matches('(').count();
  let closes = slice.matches(')').count();
  if opens > closes {
    // Inside the kinds paren `(<kind>[, ...])`.
    let trimmed = slice.trim_end();
    let last_char = trimmed.chars().last();
    if matches!(last_char, Some('(') | Some(',')) || slice.ends_with(char::is_whitespace) {
      return Some(&[
        ("ndistinct", "ndistinct -- multi-column n-distinct estimate"),
        ("dependencies", "dependencies -- functional-dependency stats"),
        ("mcv", "mcv -- multi-column most-common-values list"),
      ]);
    }
  }
  if last == "ON" {
    return None; // column list slot
  }
  if last == "FROM" {
    return None; // table slot
  }
  if words.len() >= 3 && !words.contains(&"ON") && !words.contains(&"FROM") {
    return Some(&[("(", "( ndistinct, dependencies, mcv )"), ("ON", "ON <col_or_expr>[, ...]")]);
  }
  if words.contains(&"ON") && !words.contains(&"FROM") {
    return Some(&[("FROM", "FROM <table>")]);
  }
  None
}

/// CREATE TYPE phase chain.
///   CREATE TYPE <name> AS (col type, ...)
///                     | AS ENUM ('a', 'b', ...)
///                     | AS RANGE (SUBTYPE = ..., ...)
///                     | (INPUT = ..., OUTPUT = ..., LIKE = ..., ...)
fn create_type_next_keyword(source: &str, offset: TextSize) -> Option<&'static [(&'static str, &'static str)]> {
  if cursor_not_at_ws_boundary(source, offset) {
    return None;
  }
  let (slice_owned, upper) = stmt_slice_upper(source, offset);
  let slice = slice_owned.as_str();
  let words: Vec<&str> = upper.split_ascii_whitespace().collect();
  if words.len() < 2 || words[0] != "CREATE" || words[1] != "TYPE" {
    return None;
  }
  let n = words.len();
  let last = words[n - 1];
  // CREATE TYPE name <cursor> -> AS or `(` for base types.
  if n == 3 {
    return Some(&[
      ("AS", "AS ( <col> <type>, ... ) -- composite type"),
      ("AS ENUM", "AS ENUM ('a', 'b', ...) -- discrete labels"),
      ("AS RANGE", "AS RANGE ( SUBTYPE = <type>, ... )"),
      ("(", "( INPUT = ..., OUTPUT = ..., LIKE = ..., ... ) -- base type"),
    ]);
  }
  if last == "AS" {
    return Some(&[
      ("ENUM", "AS ENUM ('a', 'b', ...)"),
      ("RANGE", "AS RANGE ( SUBTYPE = ... )"),
      ("(", "AS ( <col> <type>, ... ) -- composite"),
    ]);
  }
  // Inside `AS RANGE ( <cursor>` -> option-name keywords.
  if upper.contains("AS RANGE") && slice.matches('(').count() > slice.matches(')').count() {
    let trimmed = slice.trim_end();
    let last_char = trimmed.chars().last();
    if matches!(last_char, Some('(') | Some(',')) || slice.ends_with(char::is_whitespace) {
      return Some(&[
        ("SUBTYPE", "SUBTYPE = <type>"),
        ("SUBTYPE_OPCLASS", "SUBTYPE_OPCLASS = <opclass>"),
        ("COLLATION", "COLLATION = <collation>"),
        ("CANONICAL", "CANONICAL = <function>"),
        ("SUBTYPE_DIFF", "SUBTYPE_DIFF = <function>"),
        ("MULTIRANGE_TYPE_NAME", "MULTIRANGE_TYPE_NAME = <name>"),
      ]);
    }
  }
  // Inside the base-type paren `CREATE TYPE name ( <cursor>`.
  if !upper.contains(" AS ") && slice.matches('(').count() > slice.matches(')').count() {
    let trimmed = slice.trim_end();
    let last_char = trimmed.chars().last();
    if matches!(last_char, Some('(') | Some(',')) || slice.ends_with(char::is_whitespace) {
      return Some(&[
        ("INPUT", "INPUT = <function>"),
        ("OUTPUT", "OUTPUT = <function>"),
        ("RECEIVE", "RECEIVE = <function>"),
        ("SEND", "SEND = <function>"),
        ("TYPMOD_IN", "TYPMOD_IN = <function>"),
        ("TYPMOD_OUT", "TYPMOD_OUT = <function>"),
        ("INTERNALLENGTH", "INTERNALLENGTH = <n>"),
        ("PASSEDBYVALUE", "PASSEDBYVALUE"),
        ("ALIGNMENT", "ALIGNMENT = <type>"),
        ("STORAGE", "STORAGE = plain | external | extended | main"),
        ("LIKE", "LIKE = <existing_type>"),
        ("CATEGORY", "CATEGORY = '<char>'"),
        ("PREFERRED", "PREFERRED = true | false"),
        ("DEFAULT", "DEFAULT = <default_value>"),
        ("ELEMENT", "ELEMENT = <type> -- array element"),
        ("DELIMITER", "DELIMITER = '<char>'"),
        ("COLLATABLE", "COLLATABLE = true | false"),
      ]);
    }
  }
  None
}

/// CREATE EVENT TRIGGER phase chain.
///   CREATE EVENT TRIGGER <name> ON <event>
///     [WHEN <filter_var> IN ('tag', ...) AND ...]
///     EXECUTE FUNCTION <fn>()
fn create_event_trigger_next_keyword(
  source: &str,
  offset: TextSize,
) -> Option<&'static [(&'static str, &'static str)]> {
  if cursor_not_at_ws_boundary(source, offset) {
    return None;
  }
  let (_, upper) = stmt_slice_upper(source, offset);
  let words: Vec<&str> = upper.split_ascii_whitespace().collect();
  if words.len() < 3 || words[0] != "CREATE" || words[1] != "EVENT" || words[2] != "TRIGGER" {
    return None;
  }
  let last = *words.last()?;
  if last == "ON" {
    return Some(&[
      ("ddl_command_start", "ddl_command_start -- fires before a DDL"),
      ("ddl_command_end", "ddl_command_end -- fires after a DDL"),
      ("table_rewrite", "table_rewrite -- fires when a DDL rewrites a table"),
      ("sql_drop", "sql_drop -- fires after a DROP"),
      ("login", "login -- fires on session start (PG17+)"),
    ]);
  }
  if last == "EXECUTE" {
    return Some(&[("FUNCTION", "EXECUTE FUNCTION <fn>()"), ("PROCEDURE", "EXECUTE PROCEDURE <fn>()")]);
  }
  if last == "WHEN" {
    return Some(&[("tag", "tag IN ('CREATE TABLE', ...) -- filter by command tag")]);
  }
  // WHEN tag <cursor> -> IN
  if last == "TAG" && words.contains(&"WHEN") {
    return Some(&[("IN", "IN ('<cmd_tag1>', '<cmd_tag2>', ...)")]);
  }
  // WHEN tag IN <cursor> -> (
  if last == "IN" && words.contains(&"WHEN") {
    return Some(&[("(", "( '<cmd_tag1>', '<cmd_tag2>', ... )")]);
  }
  // WHEN tag IN ( <cursor> ) -> common command-tag suggestions
  if (last == "(" || last.starts_with("('"))
    && words.contains(&"IN")
    && words.contains(&"WHEN")
  {
    return Some(&[
      ("'CREATE TABLE'", "'CREATE TABLE'"),
      ("'DROP TABLE'", "'DROP TABLE'"),
      ("'ALTER TABLE'", "'ALTER TABLE'"),
      ("'CREATE INDEX'", "'CREATE INDEX'"),
      ("'DROP INDEX'", "'DROP INDEX'"),
      ("'CREATE FUNCTION'", "'CREATE FUNCTION'"),
      ("'DROP FUNCTION'", "'DROP FUNCTION'"),
      ("'CREATE TRIGGER'", "'CREATE TRIGGER'"),
      ("'CREATE VIEW'", "'CREATE VIEW'"),
      ("'CREATE SCHEMA'", "'CREATE SCHEMA'"),
      ("'CREATE EXTENSION'", "'CREATE EXTENSION'"),
    ]);
  }
  if words.len() >= 5 && words.contains(&"ON") && !words.contains(&"EXECUTE") {
    return Some(&[("WHEN", "WHEN tag IN ('CREATE TABLE', ...) -- filter"), ("EXECUTE", "EXECUTE FUNCTION <fn>()")]);
  }
  if words.len() == 4 {
    return Some(&[("ON", "ON <event>")]);
  }
  None
}

/// CREATE SERVER / CREATE FOREIGN DATA WRAPPER phase chain.
/// CREATE LANGUAGE chain.
///   CREATE [OR REPLACE] [TRUSTED] [PROCEDURAL] LANGUAGE <name>
///     [ HANDLER <call_handler> [ INLINE <inline_handler> ] [ VALIDATOR <fn> ] ]
fn create_language_next_keyword(
  source: &str,
  offset: TextSize,
) -> Option<&'static [(&'static str, &'static str)]> {
  if cursor_not_at_ws_boundary(source, offset) {
    return None;
  }
  let (_, upper) = stmt_slice_upper(source, offset);
  let words: Vec<&str> = upper.split_ascii_whitespace().collect();
  let lang_idx = words.iter().position(|w| *w == "LANGUAGE")?;
  if !words[..lang_idx].iter().all(|w| matches!(*w, "CREATE" | "OR" | "REPLACE" | "TRUSTED" | "PROCEDURAL"))
    || words[0] != "CREATE"
  {
    return None;
  }
  let tail = &words[lang_idx + 1..];
  let last = tail.last().copied();
  // Right after LANGUAGE -> user types name; suggest TRUSTED/PROCEDURAL as
  // forward-looking modifiers won't help. Skip until name committed.
  if tail.is_empty() {
    return None;
  }
  if tail.len() == 1 {
    // After language name -> HANDLER / nothing.
    return Some(&[
      ("HANDLER", "HANDLER <call_handler_fn>"),
      ("INLINE", "INLINE <inline_handler_fn>"),
      ("VALIDATOR", "VALIDATOR <validator_fn>"),
    ]);
  }
  if last == Some("HANDLER") || last == Some("INLINE") || last == Some("VALIDATOR") {
    return None; // user types fn name
  }
  // After a fn name in the chain -> next optional clause.
  let mut menu: Vec<(&str, &str)> = Vec::new();
  if !tail.contains(&"INLINE") {
    menu.push(("INLINE", "INLINE <inline_handler_fn>"));
  }
  if !tail.contains(&"VALIDATOR") {
    menu.push(("VALIDATOR", "VALIDATOR <validator_fn>"));
  }
  if menu.is_empty() {
    return None;
  }
  // Return a static slice -- emit both (validator second) regardless.
  Some(&[
    ("INLINE", "INLINE <inline_handler_fn>"),
    ("VALIDATOR", "VALIDATOR <validator_fn>"),
  ])
}

/// ALTER LANGUAGE chain.
///   ALTER [PROCEDURAL] LANGUAGE <name> RENAME TO <new_name>
///   ALTER [PROCEDURAL] LANGUAGE <name> OWNER TO <role>
fn alter_language_next_keyword(
  source: &str,
  offset: TextSize,
) -> Option<&'static [(&'static str, &'static str)]> {
  if cursor_not_at_ws_boundary(source, offset) {
    return None;
  }
  let (_, upper) = stmt_slice_upper(source, offset);
  let words: Vec<&str> = upper.split_ascii_whitespace().collect();
  let lang_idx = words.iter().position(|w| *w == "LANGUAGE")?;
  if !words[..lang_idx].iter().all(|w| matches!(*w, "ALTER" | "PROCEDURAL")) || words[0] != "ALTER" {
    return None;
  }
  let tail = &words[lang_idx + 1..];
  if tail.is_empty() {
    return None;
  }
  let last = *tail.last()?;
  if matches!(last, "RENAME" | "OWNER") {
    let menu: &[(&str, &str)] = match last {
      "RENAME" => &[("TO", "RENAME TO <new_name>")],
      "OWNER" => &[("TO", "OWNER TO <role>")],
      _ => &[],
    };
    if !menu.is_empty() {
      return Some(menu);
    }
  }
  if !tail.is_empty() && last != "TO" {
    return Some(&[("RENAME TO", "RENAME TO <new_name>"), ("OWNER TO", "OWNER TO <role>")]);
  }
  None
}

/// ALTER SERVER chain.
///   ALTER SERVER <name> [VERSION '<v>'] [OPTIONS (ADD|SET|DROP <k> ['<v>'])]
///                       | RENAME TO <new_name>
///                       | OWNER TO <role>
fn alter_server_next_keyword(
  source: &str,
  offset: TextSize,
) -> Option<&'static [(&'static str, &'static str)]> {
  if cursor_not_at_ws_boundary(source, offset) {
    return None;
  }
  let (slice_owned, upper) = stmt_slice_upper(source, offset);
  let slice = slice_owned.as_str();
  let words: Vec<&str> = upper.split_ascii_whitespace().collect();
  if words.len() < 2 || words[0] != "ALTER" || words[1] != "SERVER" {
    return None;
  }
  let last = *words.last()?;
  if matches!(last, "RENAME" | "OWNER") {
    return Some(match last {
      "RENAME" => &[("TO", "RENAME TO <new_name>")],
      "OWNER" => &[("TO", "OWNER TO <role>")],
      _ => &[],
    });
  }
  if last == "VERSION" {
    return None; // user types '<version>'
  }
  if last == "OPTIONS" {
    return Some(&[("(", "( ADD|SET|DROP <key> '<val>'[, ...] )")]);
  }
  let opens = slice.matches('(').count();
  let closes = slice.matches(')').count();
  if opens > closes {
    let trimmed = slice.trim_end();
    let last_char = trimmed.chars().last();
    if matches!(last_char, Some('(') | Some(',')) || slice.ends_with(char::is_whitespace) {
      return Some(&[
        ("ADD", "ADD <key> '<value>'"),
        ("SET", "SET <key> '<value>'"),
        ("DROP", "DROP <key>"),
      ]);
    }
  }
  if words.len() >= 3 {
    return Some(&[
      ("VERSION", "VERSION '<version>'"),
      ("OPTIONS", "OPTIONS ( ADD|SET|DROP <key> '<val>'[, ...] )"),
      ("RENAME TO", "RENAME TO <new_name>"),
      ("OWNER TO", "OWNER TO <role>"),
    ]);
  }
  None
}

/// ALTER FOREIGN DATA WRAPPER chain.
///   ALTER FOREIGN DATA WRAPPER <name>
///       [HANDLER <fn> | NO HANDLER]
///       [VALIDATOR <fn> | NO VALIDATOR]
///       [OPTIONS (ADD|SET|DROP <k> ['<v>']...)]
///       | RENAME TO <new_name>
///       | OWNER TO <role>
fn alter_fdw_next_keyword(
  source: &str,
  offset: TextSize,
) -> Option<&'static [(&'static str, &'static str)]> {
  if cursor_not_at_ws_boundary(source, offset) {
    return None;
  }
  let (slice_owned, upper) = stmt_slice_upper(source, offset);
  let slice = slice_owned.as_str();
  let words: Vec<&str> = upper.split_ascii_whitespace().collect();
  if words.len() < 5
    || words[0] != "ALTER"
    || words[1] != "FOREIGN"
    || words[2] != "DATA"
    || words[3] != "WRAPPER"
  {
    return None;
  }
  let last = *words.last()?;
  if matches!(last, "RENAME" | "OWNER") {
    return Some(match last {
      "RENAME" => &[("TO", "RENAME TO <new_name>")],
      "OWNER" => &[("TO", "OWNER TO <role>")],
      _ => &[],
    });
  }
  if matches!(last, "HANDLER" | "VALIDATOR") {
    return None; // user types fn name
  }
  if last == "NO" && (words.contains(&"WRAPPER") || words.contains(&"DATA")) {
    return Some(&[("HANDLER", "NO HANDLER"), ("VALIDATOR", "NO VALIDATOR")]);
  }
  if last == "OPTIONS" {
    return Some(&[("(", "( ADD|SET|DROP <key> '<val>'[, ...] )")]);
  }
  let opens = slice.matches('(').count();
  let closes = slice.matches(')').count();
  if opens > closes {
    let trimmed = slice.trim_end();
    let last_char = trimmed.chars().last();
    if matches!(last_char, Some('(') | Some(',')) || slice.ends_with(char::is_whitespace) {
      return Some(&[
        ("ADD", "ADD <key> '<value>'"),
        ("SET", "SET <key> '<value>'"),
        ("DROP", "DROP <key>"),
      ]);
    }
  }
  if words.len() >= 5 {
    return Some(&[
      ("HANDLER", "HANDLER <function>"),
      ("NO HANDLER", "NO HANDLER"),
      ("VALIDATOR", "VALIDATOR <function>"),
      ("NO VALIDATOR", "NO VALIDATOR"),
      ("OPTIONS", "OPTIONS ( ADD|SET|DROP <key> '<val>'[, ...] )"),
      ("RENAME TO", "RENAME TO <new_name>"),
      ("OWNER TO", "OWNER TO <role>"),
    ]);
  }
  None
}

fn create_server_next_keyword(source: &str, offset: TextSize) -> Option<&'static [(&'static str, &'static str)]> {
  if cursor_not_at_ws_boundary(source, offset) {
    return None;
  }
  let (_, upper) = stmt_slice_upper(source, offset);
  let words: Vec<&str> = upper.split_ascii_whitespace().collect();
  let is_server = words.len() >= 2 && words[0] == "CREATE" && words[1] == "SERVER";
  let is_fdw = words.len() >= 5
    && words[0] == "CREATE"
    && words[1] == "FOREIGN"
    && words[2] == "DATA"
    && words[3] == "WRAPPER";
  if !is_server && !is_fdw {
    return None;
  }
  let last = *words.last()?;
  if is_server {
    if last == "TYPE" {
      return None; // user types a type literal
    }
    if last == "VERSION" {
      return None;
    }
    if last == "WRAPPER" && words.contains(&"FOREIGN") {
      return None; // user types the FDW name
    }
    return Some(&[
      ("TYPE", "TYPE '<type>'"),
      ("VERSION", "VERSION '<version>'"),
      ("FOREIGN DATA WRAPPER", "FOREIGN DATA WRAPPER <fdw_name>"),
      ("OPTIONS", "OPTIONS (key 'value', ...)"),
    ]);
  }
  if is_fdw {
    return Some(&[
      ("HANDLER", "HANDLER <function>"),
      ("NO HANDLER", "NO HANDLER -- placeholder, can't be used"),
      ("VALIDATOR", "VALIDATOR <function>"),
      ("NO VALIDATOR", "NO VALIDATOR"),
      ("OPTIONS", "OPTIONS (key 'value', ...)"),
    ]);
  }
  None
}

/// CREATE OPERATOR phase chain. PG syntax:
///   CREATE OPERATOR <op> ( FUNCTION = <fn>, LEFTARG = <type>,
///       RIGHTARG = <type>, [COMMUTATOR = OPERATOR(<schema>.<op>)],
///       [NEGATOR = ...], [RESTRICT = <selfn>], [JOIN = <joinfn>],
///       [HASHES], [MERGES] )
fn create_operator_next_keyword(source: &str, offset: TextSize) -> Option<&'static [(&'static str, &'static str)]> {
  let pos: usize = (u32::from(offset) as usize).min(source.len());
  let bytes = source.as_bytes();
  if pos < bytes.len() && !bytes[pos].is_ascii_whitespace() && bytes[pos] != b',' && bytes[pos] != b'(' {
    return None;
  }
  let (slice_owned, upper) = stmt_slice_upper(source, offset);
  let slice = slice_owned.as_str();
  let words: Vec<&str> = upper.split_ascii_whitespace().collect();
  if words.len() < 2 || words[0] != "CREATE" || words[1] != "OPERATOR" {
    return None;
  }
  // Cursor must sit inside the paren options list.
  let opens = slice.matches('(').count();
  let closes = slice.matches(')').count();
  if opens == 0 || opens <= closes {
    return None;
  }
  // Either at a fresh option-name slot (right after `(` or `,`).
  let trimmed = slice.trim_end();
  let last_char = trimmed.chars().last();
  if matches!(last_char, Some('(') | Some(',')) || slice.ends_with(char::is_whitespace) {
    return Some(&[
      ("FUNCTION", "FUNCTION = <fn_name>"),
      ("LEFTARG", "LEFTARG = <type>"),
      ("RIGHTARG", "RIGHTARG = <type>"),
      ("COMMUTATOR", "COMMUTATOR = OPERATOR(<schema>.<op>)"),
      ("NEGATOR", "NEGATOR = OPERATOR(<schema>.<op>)"),
      ("RESTRICT", "RESTRICT = <selectivity_fn>"),
      ("JOIN", "JOIN = <join_selectivity_fn>"),
      ("HASHES", "HASHES -- supports hash join"),
      ("MERGES", "MERGES -- supports merge join"),
    ]);
  }
  None
}

/// CREATE DOMAIN phase chain.
///   CREATE DOMAIN <name> [AS] <basetype> [DEFAULT <expr>] [<constraint>...]
///   where <constraint> = [CONSTRAINT <name>] {NOT NULL | NULL | CHECK (<expr>)}
fn create_domain_next_keyword(source: &str, offset: TextSize) -> Option<&'static [(&'static str, &'static str)]> {
  if cursor_not_at_ws_boundary(source, offset) {
    return None;
  }
  let (_, upper) = stmt_slice_upper(source, offset);
  let words: Vec<&str> = upper.split_ascii_whitespace().collect();
  if words.len() < 2 || words[0] != "CREATE" || words[1] != "DOMAIN" {
    return None;
  }
  let last = *words.last()?;
  // `CREATE DOMAIN name <cursor>` -> AS or constraints.
  if words.len() == 3 {
    return Some(&[("AS", "AS <base_type>"), ("CHECK", "CHECK ( VALUE > 0 )"), ("DEFAULT", "DEFAULT <expr>"), ("NOT NULL", "NOT NULL")]);
  }
  if last == "AS" {
    return None; // user types the base type
  }
  // After the base type or after a constraint, expect more constraints.
  if matches!(last, "NULL" | "DEFAULT" | "CHECK") {
    return None; // expression / value slot
  }
  Some(&[
    ("CONSTRAINT", "CONSTRAINT <name> CHECK (...)"),
    ("CHECK", "CHECK ( VALUE > 0 )"),
    ("NOT NULL", "NOT NULL"),
    ("NULL", "NULL -- explicit nullable (default)"),
    ("DEFAULT", "DEFAULT <expr>"),
  ])
}

/// CREATE COLLATION phase chain.
///   CREATE COLLATION [IF NOT EXISTS] <name> ( LOCALE = '...' | LC_COLLATE = '...', ... )
///   CREATE COLLATION <name> FROM <existing>
fn create_collation_next_keyword(
  source: &str,
  offset: TextSize,
) -> Option<&'static [(&'static str, &'static str)]> {
  let pos: usize = (u32::from(offset) as usize).min(source.len());
  let bytes = source.as_bytes();
  if pos < bytes.len() && !bytes[pos].is_ascii_whitespace() && bytes[pos] != b'(' && bytes[pos] != b',' {
    return None;
  }
  let (slice_owned, upper) = stmt_slice_upper(source, offset);
  let slice = slice_owned.as_str();
  let words: Vec<&str> = upper.split_ascii_whitespace().collect();
  if words.len() < 2 || words[0] != "CREATE" || words[1] != "COLLATION" {
    return None;
  }
  // Cursor inside the paren options list -> emit option-name keywords.
  let opens = slice.matches('(').count();
  let closes = slice.matches(')').count();
  if opens > 0 && opens > closes {
    let trimmed = slice.trim_end();
    let last_char = trimmed.chars().last();
    if matches!(last_char, Some('(') | Some(',')) || slice.ends_with(char::is_whitespace) {
      return Some(&[
        ("LOCALE", "LOCALE = '<locale_name>' -- e.g. 'en_US.UTF-8'"),
        ("LC_COLLATE", "LC_COLLATE = '<lc_collate>'"),
        ("LC_CTYPE", "LC_CTYPE = '<lc_ctype>'"),
        ("PROVIDER", "PROVIDER = libc | icu | builtin"),
        ("DETERMINISTIC", "DETERMINISTIC = true | false"),
        ("RULES", "RULES = '<ICU_rules>'"),
        ("VERSION", "VERSION = '<version_string>'"),
      ]);
    }
  }
  // After name -> FROM or `(`.
  let last = *words.last()?;
  if last == "FROM" {
    return None; // user types an existing collation
  }
  if words.len() >= 3 && !words.contains(&"FROM") && opens == 0 {
    return Some(&[("FROM", "FROM <existing_collation> -- clone an existing collation"), ("(", "( LOCALE = '...', LC_COLLATE = '...' )")]);
  }
  None
}

/// ALTER SYSTEM phase chain.
/// GENERATED column chain inside CREATE TABLE / ALTER TABLE / CREATE FOREIGN TABLE:
///   <col> <type> GENERATED <cursor>                 -> ALWAYS / BY DEFAULT
///   <col> <type> GENERATED ALWAYS <cursor>          -> AS IDENTITY / AS (...)
///   <col> <type> GENERATED BY DEFAULT <cursor>      -> AS IDENTITY
///   <col> <type> GENERATED ... AS <cursor>          -> IDENTITY / (
///   <col> ... AS IDENTITY <cursor>                  -> ( <seq_opts>
///   <col> ... AS (<expr>) <cursor>                  -> STORED / VIRTUAL
fn column_generated_next_keyword(
  source: &str,
  offset: TextSize,
) -> Option<&'static [(&'static str, &'static str)]> {
  if cursor_not_at_ws_boundary(source, offset) {
    return None;
  }
  let (slice_owned, upper) = stmt_slice_upper(source, offset);
  if !upper.contains("CREATE TABLE")
    && !upper.contains("ALTER TABLE")
    && !upper.contains("CREATE FOREIGN TABLE")
  {
    return None;
  }
  let words: Vec<&str> = upper.split_ascii_whitespace().collect();
  let n = words.len();
  let last = *words.last()?;
  let second_last = if n >= 2 { Some(words[n - 2]) } else { None };
  let third_last = if n >= 3 { Some(words[n - 3]) } else { None };

  // GENERATED <cursor> -> ALWAYS | BY DEFAULT
  if last == "GENERATED" {
    return Some(&[
      ("ALWAYS", "GENERATED ALWAYS AS IDENTITY | AS (<expr>) STORED"),
      ("BY DEFAULT", "GENERATED BY DEFAULT AS IDENTITY"),
    ]);
  }
  // GENERATED ALWAYS <cursor> -> AS
  if last == "ALWAYS" && second_last == Some("GENERATED") {
    return Some(&[("AS", "AS IDENTITY | AS (<expression>) STORED")]);
  }
  // GENERATED BY DEFAULT <cursor> -> AS IDENTITY
  if last == "DEFAULT" && second_last == Some("BY") && third_last == Some("GENERATED") {
    return Some(&[("AS IDENTITY", "AS IDENTITY -- sequence-backed column")]);
  }
  // GENERATED ALWAYS AS <cursor> -> IDENTITY | (
  if last == "AS"
    && (second_last == Some("ALWAYS") || (second_last == Some("DEFAULT") && third_last == Some("BY")))
  {
    return Some(&[
      ("IDENTITY", "AS IDENTITY [(<seq_options>)]"),
      ("(", "AS ( <expression> ) STORED -- computed column"),
    ]);
  }
  // ... AS (<expr>) <cursor> -> STORED
  // Detect: last char of slice is `)`, and a `GENERATED` precedes it.
  let trimmed = slice_owned.trim_end();
  if trimmed.ends_with(')') && upper.contains("GENERATED") && !upper.contains("STORED") && !upper.contains("VIRTUAL") {
    // make sure last keyword before `(` was AS
    return Some(&[
      ("STORED", "STORED -- materialised generated column"),
      ("VIRTUAL", "VIRTUAL -- on-the-fly generated column (PG18+ / reserved)"),
    ]);
  }
  None
}

/// CREATE INDEX trailing-clause slot: cursor sits past the closing
/// `)` of the expression list. Emit the trailing clauses PG allows.
fn create_index_trailing_next_keyword(
  source: &str,
  offset: TextSize,
) -> Option<&'static [(&'static str, &'static str)]> {
  if cursor_not_at_ws_boundary(source, offset) {
    return None;
  }
  let (_, upper) = stmt_slice_upper(source, offset);
  let trimmed = upper.trim_start();
  if !trimmed.starts_with("CREATE INDEX")
    && !trimmed.starts_with("CREATE UNIQUE INDEX")
  {
    return None;
  }
  // Must be past a `(` body that closed.
  let opens = upper.matches('(').count();
  let closes = upper.matches(')').count();
  if opens == 0 || opens > closes {
    return None;
  }
  let words: Vec<&str> = upper.split_ascii_whitespace().collect();
  let last = *words.last()?;
  // Avoid re-emitting if the user has already started a trailing clause.
  if matches!(last, "INCLUDE" | "WHERE" | "WITH" | "TABLESPACE" | "USING") {
    return None;
  }
  Some(&[
    ("INCLUDE", "INCLUDE (<col>[, ...]) -- non-key payload columns"),
    ("WHERE", "WHERE <predicate> -- partial index"),
    ("WITH", "WITH (<option>=<value>, ...) -- storage params"),
    ("TABLESPACE", "TABLESPACE <name> -- place index in a specific tablespace"),
  ])
}

/// CHECK constraint trailing slot:
///   ... CHECK (<expr>) <cursor>  -> NO INHERIT
fn check_constraint_no_inherit_next_keyword(
  source: &str,
  offset: TextSize,
) -> Option<&'static [(&'static str, &'static str)]> {
  if cursor_not_at_ws_boundary(source, offset) {
    return None;
  }
  let (slice_owned, upper) = stmt_slice_upper(source, offset);
  if !upper.contains("CHECK") {
    return None;
  }
  if !upper.contains("CREATE TABLE")
    && !upper.contains("ALTER TABLE")
    && !upper.contains("CREATE DOMAIN")
  {
    return None;
  }
  // The latest CHECK must have a matching closing `)`.
  let after_check = match upper.rfind("CHECK") {
    Some(p) => &upper[p + "CHECK".len()..],
    None => return None,
  };
  let opens = after_check.matches('(').count();
  let closes = after_check.matches(')').count();
  if opens == 0 || opens > closes {
    return None;
  }
  // Must end with whitespace boundary.
  if !slice_owned.ends_with(char::is_whitespace) {
    return None;
  }
  // Don't re-emit when NO is already typed.
  let words: Vec<&str> = upper.split_ascii_whitespace().collect();
  if matches!(words.last(), Some(&"NO") | Some(&"INHERIT")) {
    return None;
  }
  Some(&[("NO INHERIT", "NO INHERIT -- child tables don't enforce this CHECK")])
}

/// Column / table constraint sub-keywords inside CREATE TABLE / ALTER TABLE:
///   REFERENCES <t>(<c>) ON DELETE <cursor>   -> CASCADE / RESTRICT / NO ACTION / SET NULL / SET DEFAULT
///   REFERENCES <t>(<c>) ON UPDATE <cursor>   -> same
///   ... ON DELETE SET <cursor>               -> NULL / DEFAULT
///   ... DEFERRABLE <cursor>                  -> INITIALLY
///   ... DEFERRABLE INITIALLY <cursor>        -> DEFERRED / IMMEDIATE
///   ... INITIALLY <cursor>                   -> DEFERRED / IMMEDIATE
fn column_constraint_next_keyword(
  source: &str,
  offset: TextSize,
) -> Option<&'static [(&'static str, &'static str)]> {
  if cursor_not_at_ws_boundary(source, offset) {
    return None;
  }
  let (_, upper) = stmt_slice_upper(source, offset);
  // Must be inside a TABLE/COLUMN DDL context.
  if !upper.contains("CREATE TABLE")
    && !upper.contains("ALTER TABLE")
    && !upper.contains("CREATE FOREIGN TABLE")
  {
    return None;
  }
  let words: Vec<&str> = upper.split_ascii_whitespace().collect();
  let n = words.len();
  let last = *words.last()?;
  let second_last = if n >= 2 { Some(words[n - 2]) } else { None };
  let third_last = if n >= 3 { Some(words[n - 3]) } else { None };

  // ON DELETE <cursor> | ON UPDATE <cursor>
  if matches!(last, "DELETE" | "UPDATE") && second_last == Some("ON") {
    return Some(&[
      ("CASCADE", "CASCADE -- propagate deletes/updates"),
      ("RESTRICT", "RESTRICT -- refuse if dependent rows exist (check immediate)"),
      ("NO ACTION", "NO ACTION -- default; check deferred if constraint is DEFERRABLE"),
      ("SET NULL", "SET NULL -- NULL out the FK columns"),
      ("SET DEFAULT", "SET DEFAULT -- reset FK columns to their DEFAULT"),
    ]);
  }
  // ON {DELETE|UPDATE} SET <cursor> -> NULL | DEFAULT
  if last == "SET"
    && matches!(second_last, Some("DELETE") | Some("UPDATE"))
    && third_last == Some("ON")
  {
    return Some(&[("NULL", "SET NULL"), ("DEFAULT", "SET DEFAULT")]);
  }
  // DEFERRABLE <cursor> | NOT DEFERRABLE <cursor>
  if last == "DEFERRABLE" {
    return Some(&[("INITIALLY", "INITIALLY DEFERRED | IMMEDIATE")]);
  }
  // INITIALLY <cursor>
  if last == "INITIALLY" {
    return Some(&[
      ("DEFERRED", "INITIALLY DEFERRED -- check at COMMIT"),
      ("IMMEDIATE", "INITIALLY IMMEDIATE -- check at statement end (default)"),
    ]);
  }
  None
}

/// WITH RECURSIVE ... SEARCH / CYCLE clause chain (PG14+).
///   ... SEARCH { DEPTH | BREADTH } FIRST BY <cols> SET <out_col>
///   ... CYCLE <cols> SET <flag_col> [TO 't' DEFAULT 'f'] USING <path_col>
fn cte_search_cycle_next_keyword(
  source: &str,
  offset: TextSize,
) -> Option<&'static [(&'static str, &'static str)]> {
  if cursor_not_at_ws_boundary(source, offset) {
    return None;
  }
  let (_, upper) = stmt_slice_upper(source, offset);
  // Must follow a WITH RECURSIVE somewhere in the statement.
  if !upper.contains("WITH RECURSIVE") {
    return None;
  }
  let words: Vec<&str> = upper.split_ascii_whitespace().collect();
  let last = *words.last()?;
  // Bare `... SEARCH <cursor>` -> DEPTH | BREADTH
  if last == "SEARCH" {
    return Some(&[
      ("DEPTH FIRST BY", "DEPTH FIRST BY <cols> SET <out_col>"),
      ("BREADTH FIRST BY", "BREADTH FIRST BY <cols> SET <out_col>"),
    ]);
  }
  // `... SEARCH DEPTH | BREADTH <cursor>` -> FIRST
  if matches!(last, "DEPTH" | "BREADTH") && words.contains(&"SEARCH") {
    return Some(&[("FIRST BY", "FIRST BY <cols> SET <out_col>")]);
  }
  // `... DEPTH FIRST <cursor>` -> BY
  if last == "FIRST" && (words.contains(&"DEPTH") || words.contains(&"BREADTH")) {
    return Some(&[("BY", "BY <cols> SET <out_col>")]);
  }
  // `... CYCLE <cursor>` -> after the cycle-column list typed by user;
  // common next slot is SET.
  if last == "CYCLE" {
    return Some(&[("<cols>", "CYCLE <col1>, <col2>, ... SET <flag_col>")]);
  }
  // Inside the CYCLE clause -- after SET <flag>, optional TO / DEFAULT / USING
  if last == "SET" && words.contains(&"CYCLE") {
    return Some(&[("<flag_col>", "SET <flag_col> [TO 't' DEFAULT 'f'] USING <path_col>")]);
  }
  if last == "USING" && words.contains(&"CYCLE") {
    return Some(&[("<path_col>", "USING <path_col> -- array tracking visited rows")]);
  }
  None
}

/// `ALTER SYSTEM {SET|RESET} <param> [= value | TO value]`
fn alter_system_next_keyword(source: &str, offset: TextSize) -> Option<&'static [(&'static str, &'static str)]> {
  if cursor_not_at_ws_boundary(source, offset) {
    return None;
  }
  let (_, upper) = stmt_slice_upper(source, offset);
  let words: Vec<&str> = upper.split_ascii_whitespace().collect();
  if words.len() < 2 || words[0] != "ALTER" || words[1] != "SYSTEM" {
    return None;
  }
  let n = words.len();
  if n == 2 {
    return Some(&[("SET", "ALTER SYSTEM SET <param> = value -- write to postgresql.auto.conf"), ("RESET", "ALTER SYSTEM RESET <param> | ALL")]);
  }
  // After SET -> common GUC names that ALTER SYSTEM is typically used for.
  if n == 3 && words[2] == "SET" {
    return Some(&[
      ("shared_buffers", "shared_buffers = '<size>'"),
      ("work_mem", "work_mem = '<size>'"),
      ("maintenance_work_mem", "maintenance_work_mem = '<size>'"),
      ("effective_cache_size", "effective_cache_size = '<size>'"),
      ("max_connections", "max_connections = <n> -- restart required"),
      ("max_wal_size", "max_wal_size = '<size>'"),
      ("checkpoint_timeout", "checkpoint_timeout = '<duration>'"),
      ("random_page_cost", "random_page_cost = <float>"),
      ("seq_page_cost", "seq_page_cost = <float>"),
      ("default_statistics_target", "default_statistics_target = <int>"),
      ("synchronous_commit", "synchronous_commit = 'on'|'off'|'remote_apply'|..."),
      ("log_statement", "log_statement = 'none'|'ddl'|'mod'|'all'"),
      ("log_min_duration_statement", "log_min_duration_statement = <ms>"),
      ("autovacuum_naptime", "autovacuum_naptime = '<duration>'"),
      ("autovacuum_vacuum_scale_factor", "autovacuum_vacuum_scale_factor = <float>"),
      ("wal_level", "wal_level = 'replica'|'logical' -- restart required"),
      ("max_replication_slots", "max_replication_slots = <n>"),
      ("max_wal_senders", "max_wal_senders = <n>"),
    ]);
  }
  // After SET <param> -> TO | =
  if n == 4 && matches!(words[2], "SET") {
    return Some(&[("TO", "TO <value>"), ("=", "= <value>")]);
  }
  // After RESET -> ALL or stay silent (user types GUC name).
  if n == 3 && words[2] == "RESET" {
    return Some(&[("ALL", "RESET ALL -- restore all GUCs to default")]);
  }
  None
}

/// ALTER LARGE OBJECT chain.
///   ALTER LARGE OBJECT <oid> OWNER TO <role>
fn alter_large_object_next_keyword(
  source: &str,
  offset: TextSize,
) -> Option<&'static [(&'static str, &'static str)]> {
  if cursor_not_at_ws_boundary(source, offset) {
    return None;
  }
  let (_, upper) = stmt_slice_upper(source, offset);
  let words: Vec<&str> = upper.split_ascii_whitespace().collect();
  if words.len() < 3 || words[0] != "ALTER" || words[1] != "LARGE" || words[2] != "OBJECT" {
    return None;
  }
  let last = *words.last()?;
  if last == "OBJECT" {
    return None; // user types loid
  }
  if last == "OWNER" {
    return Some(&[("TO", "OWNER TO <role>")]);
  }
  if words.len() >= 4 {
    return Some(&[("OWNER TO", "OWNER TO <role>")]);
  }
  None
}

/// CREATE PUBLICATION phase chain.
/// `CREATE PUBLICATION <name> [FOR ALL TABLES | FOR TABLE <tbl>[, ...] | FOR TABLES IN SCHEMA <s>] [WITH (...)]`
/// CREATE SUBSCRIPTION chain.
///   CREATE SUBSCRIPTION <name> CONNECTION '<conninfo>' PUBLICATION <pub>[, ...]
///     [WITH (<param> = <value>[, ...])]
fn create_subscription_next_keyword(
  source: &str,
  offset: TextSize,
) -> Option<&'static [(&'static str, &'static str)]> {
  if cursor_not_at_ws_boundary(source, offset) {
    return None;
  }
  let (slice_owned, upper) = stmt_slice_upper(source, offset);
  let slice = slice_owned.as_str();
  let words: Vec<&str> = upper.split_ascii_whitespace().collect();
  if words.len() < 2 || words[0] != "CREATE" || words[1] != "SUBSCRIPTION" {
    return None;
  }
  let last = *words.last()?;
  if last == "SUBSCRIPTION" {
    return None; // user types the subscription name
  }
  if last == "CONNECTION" {
    return None; // user types '<conninfo>' literal
  }
  if last == "PUBLICATION" {
    return None; // user types publication name
  }
  if last == "WITH" {
    return Some(&[("(", "WITH ( <param> = <value>[, ...] )")]);
  }
  // Inside WITH ( ... ) paren -> option name slot.
  let opens = slice.matches('(').count();
  let closes = slice.matches(')').count();
  if opens > closes {
    let trimmed = slice.trim_end();
    let last_char = trimmed.chars().last();
    if matches!(last_char, Some('(') | Some(',')) || slice.ends_with(char::is_whitespace) {
      return Some(&[
        ("connect", "connect = true|false -- default true; false = create slot but don't subscribe yet"),
        ("create_slot", "create_slot = true|false -- create the replication slot on the publisher"),
        ("enabled", "enabled = true|false -- whether the subscription starts active"),
        ("slot_name", "slot_name = '<name>' | NONE -- replication slot to attach to"),
        ("synchronous_commit", "synchronous_commit = 'on'|'off'|'remote_apply'|... -- apply-worker setting"),
        ("binary", "binary = true|false -- use binary protocol (PG14+)"),
        ("streaming", "streaming = 'off'|'on'|'parallel' -- streaming-in-progress xacts"),
        ("two_phase", "two_phase = true|false -- enable 2PC streaming (PG15+)"),
        ("disable_on_error", "disable_on_error = true|false -- auto-disable on apply error (PG15+)"),
        ("run_as_owner", "run_as_owner = true|false -- run the apply worker as the subscription owner (PG16+)"),
        ("password_required", "password_required = true|false -- require password in CONNECTION (PG16+)"),
        ("origin", "origin = 'none'|'any' -- which origin's data to apply (PG16+)"),
        ("failover", "failover = true|false -- allow synchronous failover (PG17+)"),
        ("copy_data", "copy_data = true|false -- initial table sync"),
      ]);
    }
  }
  // After the name + extra tokens, suggest CONNECTION.
  if !words.contains(&"CONNECTION") && words.len() >= 3 {
    return Some(&[("CONNECTION", "CONNECTION '<conn_string>'")]);
  }
  if words.contains(&"CONNECTION") && !words.contains(&"PUBLICATION") {
    return Some(&[("PUBLICATION", "PUBLICATION <pub>[, ...]")]);
  }
  if words.contains(&"PUBLICATION") && !words.contains(&"WITH") {
    return Some(&[("WITH", "WITH ( <options> )")]);
  }
  None
}

fn create_publication_next_keyword(
  source: &str,
  offset: TextSize,
) -> Option<&'static [(&'static str, &'static str)]> {
  if cursor_not_at_ws_boundary(source, offset) {
    return None;
  }
  let (_, upper) = stmt_slice_upper(source, offset);
  let words: Vec<&str> = upper.split_ascii_whitespace().collect();
  if words.len() < 2 || words[0] != "CREATE" || words[1] != "PUBLICATION" {
    return None;
  }
  let last = *words.last()?;
  if last == "PUBLICATION" {
    return None; // user types the publication name
  }
  if last == "FOR" {
    return Some(&[
      ("ALL TABLES", "FOR ALL TABLES -- every existing + future table"),
      ("TABLE", "FOR TABLE <tbl>[, ...]"),
      ("TABLES IN SCHEMA", "FOR TABLES IN SCHEMA <schema>[, ...] (PG15+)"),
    ]);
  }
  // `... FOR ALL <cursor>` -> TABLES.
  if last == "ALL" && words.contains(&"FOR") {
    return Some(&[("TABLES", "FOR ALL TABLES -- every existing + future table")]);
  }
  // `... FOR ALL TABLES IN <cursor>` / `... FOR TABLES IN <cursor>` -> SCHEMA.
  if last == "IN" && words.contains(&"TABLES") {
    return Some(&[("SCHEMA", "TABLES IN SCHEMA <schema>[, ...] (PG15+)")]);
  }
  // `... FOR TABLES <cursor>` -> IN SCHEMA.
  if last == "TABLES" && words.contains(&"FOR") && !words.contains(&"ALL") {
    return Some(&[("IN SCHEMA", "TABLES IN SCHEMA <schema>[, ...] (PG15+)")]);
  }
  if last == "WITH" {
    return Some(&[
      ("(publish", "WITH (publish = 'insert, update, delete, truncate')"),
      ("(publish_via_partition_root", "WITH (publish_via_partition_root = true)"),
    ]);
  }
  // After name + extra tokens, suggest FOR | WITH.
  if !words.contains(&"FOR") && !words.contains(&"WITH") && words.len() >= 3 {
    return Some(&[("FOR", "FOR { ALL TABLES | TABLE ... | TABLES IN SCHEMA ... }"), ("WITH", "WITH (publish = '...')")]);
  }
  if words.contains(&"FOR") && !words.contains(&"WITH") {
    return Some(&[("WITH", "WITH (publish = 'insert, update, delete, truncate')")]);
  }
  None
}

/// CLUSTER phase chain.
/// `CLUSTER [VERBOSE] [<table_name> [USING <index_name>]]`
/// ALTER TABLE ... REPLICA IDENTITY <cursor> chain.
fn replica_identity_next_keyword(
  source: &str,
  offset: TextSize,
) -> Option<&'static [(&'static str, &'static str)]> {
  if cursor_not_at_ws_boundary(source, offset) {
    return None;
  }
  let (_, upper) = stmt_slice_upper(source, offset);
  if !upper.contains("REPLICA IDENTITY") {
    return None;
  }
  let words: Vec<&str> = upper.split_ascii_whitespace().collect();
  let last = *words.last()?;
  // `... REPLICA IDENTITY <cursor>` -> DEFAULT / FULL / NOTHING / USING INDEX
  if last == "IDENTITY" && words.contains(&"REPLICA") {
    return Some(&[
      ("DEFAULT", "DEFAULT -- record PK columns (standard)"),
      ("FULL", "FULL -- record old image of every column (heavy WAL)"),
      ("NOTHING", "NOTHING -- record only TOAST columns"),
      ("USING INDEX", "USING INDEX <unique_index> -- use a specific UNIQUE index"),
    ]);
  }
  // `... REPLICA IDENTITY USING <cursor>` -> INDEX
  if last == "USING" && words.contains(&"REPLICA") {
    return Some(&[("INDEX", "INDEX <unique_index_name>")]);
  }
  None
}

fn cluster_next_keyword(source: &str, offset: TextSize) -> Option<&'static [(&'static str, &'static str)]> {
  if cursor_not_at_ws_boundary(source, offset) {
    return None;
  }
  let (_, upper) = stmt_slice_upper(source, offset);
  let words: Vec<&str> = upper.split_ascii_whitespace().collect();
  if words.first() != Some(&"CLUSTER") {
    return None;
  }
  let n = words.len();
  if n == 1 {
    return Some(&[("VERBOSE", "CLUSTER VERBOSE -- per-table progress")]);
  }
  // After a table name -> USING.
  if n >= 3 && !words.contains(&"USING") {
    return Some(&[("USING", "USING <index_name> -- pick a non-default cluster index")]);
  }
  None
}

/// IMPORT FOREIGN SCHEMA phase chain.
/// `IMPORT FOREIGN SCHEMA <name> [LIMIT TO|EXCEPT (...)] FROM SERVER <srv> INTO <schema> [OPTIONS (...)]`
fn import_foreign_schema_next_keyword(
  source: &str,
  offset: TextSize,
) -> Option<&'static [(&'static str, &'static str)]> {
  if cursor_not_at_ws_boundary(source, offset) {
    return None;
  }
  let (_, upper) = stmt_slice_upper(source, offset);
  let words: Vec<&str> = upper.split_ascii_whitespace().collect();
  if words.len() < 3 || words[0] != "IMPORT" || words[1] != "FOREIGN" || words[2] != "SCHEMA" {
    return None;
  }
  let last = *words.last()?;
  if last == "SCHEMA" && words.len() == 3 {
    return None; // user about to type the schema name
  }
  if matches!(last, "LIMIT" | "TO" | "EXCEPT" | "FROM" | "SERVER" | "INTO") {
    let menu: &[(&str, &str)] = match last {
      "LIMIT" => &[("TO", "LIMIT TO (<rel>[, ...])")],
      "TO" => &[("(", "( <rel>[, ...] )")],
      "EXCEPT" => &[("(", "EXCEPT ( <rel>[, ...] )")],
      "FROM" => &[("SERVER", "FROM SERVER <name>")],
      "SERVER" => &[],
      "INTO" => &[],
      _ => &[],
    };
    if !menu.is_empty() {
      return Some(menu);
    }
    return None;
  }
  // After the source-schema name -> LIMIT TO / EXCEPT / FROM SERVER.
  Some(&[
    ("LIMIT TO", "LIMIT TO (<rel>[, ...]) -- import only these"),
    ("EXCEPT", "EXCEPT (<rel>[, ...]) -- import everything except"),
    ("FROM SERVER", "FROM SERVER <fdw_server> INTO <local_schema>"),
    ("INTO", "INTO <local_schema>"),
    ("OPTIONS", "OPTIONS (key 'value', ...)"),
  ])
}

/// CREATE SCHEMA phase chain.
/// `CREATE SCHEMA [IF NOT EXISTS] [<name>] [AUTHORIZATION <role>] [<schema_element>]`
fn create_schema_next_keyword(source: &str, offset: TextSize) -> Option<&'static [(&'static str, &'static str)]> {
  if cursor_not_at_ws_boundary(source, offset) {
    return None;
  }
  let (_, upper) = stmt_slice_upper(source, offset);
  let words: Vec<&str> = upper.split_ascii_whitespace().collect();
  if words.len() < 2 || words[0] != "CREATE" || words[1] != "SCHEMA" {
    return None;
  }
  let last = *words.last()?;
  if last == "AUTHORIZATION" {
    return None; // user types the role name
  }
  if words.len() == 2 {
    return Some(&[("IF NOT EXISTS", "IF NOT EXISTS <name> -- silently skip if present"), ("AUTHORIZATION", "AUTHORIZATION <role> -- omits <name>, uses role name")]);
  }
  if words.len() >= 3 && !words.contains(&"AUTHORIZATION") {
    return Some(&[("AUTHORIZATION", "AUTHORIZATION <role> -- assign owner")]);
  }
  None
}

/// REINDEX phase chain.
/// `REINDEX [CONCURRENTLY] {INDEX|TABLE|SCHEMA|DATABASE|SYSTEM} <name>`
/// CREATE FOREIGN TABLE phase chain.
///   CREATE FOREIGN TABLE [IF NOT EXISTS] <name> (...) [INHERITS (...)]
///     SERVER <srv> [OPTIONS (<key> '<val>', ...)]
///   CREATE FOREIGN TABLE <name> PARTITION OF <parent>
///     FOR VALUES ... SERVER <srv> [OPTIONS ...]
fn create_foreign_table_next_keyword(
  source: &str,
  offset: TextSize,
) -> Option<&'static [(&'static str, &'static str)]> {
  if cursor_not_at_ws_boundary(source, offset) {
    return None;
  }
  let (slice_owned, upper) = stmt_slice_upper(source, offset);
  let slice = slice_owned.as_str();
  let trimmed = upper.trim_start();
  if !trimmed.starts_with("CREATE FOREIGN TABLE") {
    return None;
  }
  let words: Vec<&str> = upper.split_ascii_whitespace().collect();
  let last = *words.last()?;
  // OPTIONS ( <cursor> -- inside paren -> no kw; user types option name
  // (free-form).
  let opens = slice.matches('(').count();
  let closes = slice.matches(')').count();
  if last == "OPTIONS" {
    return Some(&[("(", "( <option_name> '<value>', ... )")]);
  }
  if last == "SERVER" {
    return None; // server name slot
  }
  if last == "INHERITS" {
    return None; // parent table list
  }
  // After SERVER <name> -> OPTIONS or PARTITION-of trailing.
  if let Some(srv_idx) = words.iter().rposition(|w| *w == "SERVER")
    && srv_idx == words.len() - 2
  {
    return Some(&[
      ("OPTIONS", "OPTIONS (<key> '<value>', ...)"),
    ]);
  }
  // After top-level closing `)` of the column body -> INHERITS | SERVER.
  // Heuristic: opens == closes and opens >= 1 means body paren closed.
  if opens >= 1 && opens == closes && !words.contains(&"SERVER") {
    return Some(&[
      ("INHERITS", "INHERITS (<parent>[, ...])"),
      ("SERVER", "SERVER <fdw_server_name>"),
    ]);
  }
  None
}

fn reindex_next_keyword(source: &str, offset: TextSize) -> Option<&'static [(&'static str, &'static str)]> {
  if cursor_not_at_ws_boundary(source, offset) {
    return None;
  }
  let (slice_owned, upper) = stmt_slice_upper(source, offset);
  let slice = slice_owned.as_str();
  let words: Vec<&str> = upper.split_ascii_whitespace().collect();
  if words.first() != Some(&"REINDEX") {
    return None;
  }
  // REINDEX ( <cursor> ) -- paren options menu (PG14+).
  let opens = slice.matches('(').count();
  let closes = slice.matches(')').count();
  if opens > closes {
    let last_tok = slice.trim_end().chars().last();
    if matches!(last_tok, Some('(') | Some(',')) || slice.ends_with(char::is_whitespace) {
      // After CONCURRENTLY -> bool. After TABLESPACE -> name (no kw).
      let last_raw = *words.last()?;
      let last = last_raw.trim_start_matches(['(', ',']);
      if last == "CONCURRENTLY" || last == "VERBOSE" {
        return Some(&[("true", "true -- enable"), ("false", "false -- disable")]);
      }
      if last == "TABLESPACE" {
        return None; // tablespace name slot
      }
      return Some(&[
        ("CONCURRENTLY", "CONCURRENTLY [true|false] -- avoid heavy locks"),
        ("TABLESPACE", "TABLESPACE <name> -- move rebuilt indexes into <name>"),
        ("VERBOSE", "VERBOSE [true|false] -- per-index progress"),
      ]);
    }
  }
  // REINDEX <cursor>  or  REINDEX CONCURRENTLY <cursor>.
  if words.len() == 1 || (words.len() == 2 && words[1] == "CONCURRENTLY") {
    return Some(&[
      ("INDEX", "REINDEX INDEX <name> -- single index"),
      ("TABLE", "REINDEX TABLE <name> -- all indexes on a table"),
      ("SCHEMA", "REINDEX SCHEMA <name> -- every index in the schema"),
      ("DATABASE", "REINDEX DATABASE <name> -- every index in the database"),
      ("SYSTEM", "REINDEX SYSTEM <name> -- only system catalog indexes"),
      ("CONCURRENTLY", "REINDEX CONCURRENTLY <kind> <name> (PG 12+)"),
      ("(", "( CONCURRENTLY|TABLESPACE|VERBOSE ... ) -- paren options (PG14+)"),
    ]);
  }
  None
}

/// True when the cursor sits right after `CALL` (the procedure-name slot).
fn call_expects_procedure(source: &str, offset: TextSize) -> bool {
  if cursor_not_at_ws_boundary(source, offset) {
    return false;
  }
  let (slice_owned, _) = stmt_slice_upper(source, offset);
  slice_owned.trim().eq_ignore_ascii_case("CALL")
}

/// SET TRANSACTION / BEGIN ISOLATION ... phase chain.
fn set_transaction_next_keyword(source: &str, offset: TextSize) -> Option<&'static [(&'static str, &'static str)]> {
  if cursor_not_at_ws_boundary(source, offset) {
    return None;
  }
  let (slice_owned, upper) = stmt_slice_upper(source, offset);
  let _ = slice_owned;
  let words: Vec<&str> = upper.split_ascii_whitespace().collect();
  if words.is_empty() {
    return None;
  }
  let is_set_tx = (words[0] == "SET" && words.contains(&"TRANSACTION"))
    || words[0] == "BEGIN"
    || (words[0] == "START" && words.contains(&"TRANSACTION"));
  if !is_set_tx {
    return None;
  }
  let last = *words.last()?;
  if last == "ISOLATION" {
    return Some(&[("LEVEL", "ISOLATION LEVEL <name>")]);
  }
  if last == "LEVEL" {
    return Some(&[
      ("READ UNCOMMITTED", "READ UNCOMMITTED (treated as READ COMMITTED in PG)"),
      ("READ COMMITTED", "READ COMMITTED -- default"),
      ("REPEATABLE READ", "REPEATABLE READ -- snapshot isolation"),
      ("SERIALIZABLE", "SERIALIZABLE -- strictest"),
    ]);
  }
  if last == "READ" {
    return Some(&[("ONLY", "READ ONLY -- block writes"), ("WRITE", "READ WRITE -- default")]);
  }
  if last == "NOT" {
    return Some(&[("DEFERRABLE", "NOT DEFERRABLE -- default")]);
  }
  Some(&[
    ("ISOLATION LEVEL", "ISOLATION LEVEL READ COMMITTED | REPEATABLE READ | SERIALIZABLE"),
    ("READ ONLY", "READ ONLY"),
    ("READ WRITE", "READ WRITE (default)"),
    ("DEFERRABLE", "DEFERRABLE (REPEATABLE READ + READ ONLY only)"),
    ("NOT DEFERRABLE", "NOT DEFERRABLE (default)"),
    ("SNAPSHOT", "SNAPSHOT '<snapshot_id>' -- import a snapshot exported via pg_export_snapshot()"),
  ])
}

/// REASSIGN OWNED chain: `REASSIGN OWNED BY <old>[, ...] TO <new>`.
fn reassign_owned_next_keyword(
  source: &str,
  offset: TextSize,
) -> Option<&'static [(&'static str, &'static str)]> {
  if cursor_not_at_ws_boundary(source, offset) {
    return None;
  }
  let (_, upper) = stmt_slice_upper(source, offset);
  let words: Vec<&str> = upper.split_ascii_whitespace().collect();
  if words.first() != Some(&"REASSIGN") {
    return None;
  }
  let last = *words.last()?;
  if last == "REASSIGN" {
    return Some(&[("OWNED", "OWNED BY <old_role>[, ...] TO <new_role>")]);
  }
  if last == "OWNED" {
    return Some(&[("BY", "BY <old_role>[, ...]")]);
  }
  if last == "BY" {
    return None; // role name
  }
  // After role list -> TO.
  if !words.contains(&"TO") && words.len() >= 4 {
    return Some(&[("TO", "TO <new_role>")]);
  }
  None
}

/// Followup keywords for COMMIT / ROLLBACK / PREPARE TRANSACTION shapes.
///   COMMIT <cursor>                  -> AND / TRANSACTION / WORK / PREPARED
///   COMMIT AND <cursor>              -> CHAIN / NO CHAIN
///   COMMIT AND NO <cursor>           -> CHAIN
///   ROLLBACK <cursor>                -> AND / TO / TRANSACTION / WORK / PREPARED
///   ROLLBACK TO <cursor>             -> SAVEPOINT
///   ROLLBACK AND <cursor>            -> CHAIN / NO CHAIN
///   PREPARE TRANSACTION <cursor>     -> '<gxid_literal>' (no kw)
fn txn_followup_next_keyword(
  source: &str,
  offset: TextSize,
) -> Option<&'static [(&'static str, &'static str)]> {
  if cursor_not_at_ws_boundary(source, offset) {
    return None;
  }
  let (_, upper) = stmt_slice_upper(source, offset);
  let words: Vec<&str> = upper.split_ascii_whitespace().collect();
  let last = *words.last()?;
  let first = *words.first()?;
  if !matches!(first, "COMMIT" | "ROLLBACK" | "ABORT" | "END" | "PREPARE") {
    return None;
  }
  // PREPARE TRANSACTION <cursor> -- user types '<gxid_literal>'; no kw.
  if first == "PREPARE" {
    if words.len() == 1 {
      return Some(&[("TRANSACTION", "PREPARE TRANSACTION '<gxid>'")]);
    }
    return None;
  }
  // COMMIT / ROLLBACK / ABORT / END alone.
  if words.len() == 1 {
    let mut menu: &[(&str, &str)] = &[
      ("AND", "AND { CHAIN | NO CHAIN }"),
      ("TRANSACTION", "TRANSACTION -- SQL-standard noise"),
      ("WORK", "WORK -- SQL-standard noise"),
      ("PREPARED", "PREPARED '<gxid>' -- second phase of 2PC"),
    ];
    if first == "ROLLBACK" {
      menu = &[
        ("AND", "AND { CHAIN | NO CHAIN }"),
        ("TO", "TO [SAVEPOINT] <name>"),
        ("TRANSACTION", "TRANSACTION -- SQL-standard noise"),
        ("WORK", "WORK -- SQL-standard noise"),
        ("PREPARED", "PREPARED '<gxid>' -- second phase of 2PC"),
      ];
    }
    return Some(menu);
  }
  // <verb> AND <cursor>
  if last == "AND" {
    return Some(&[("CHAIN", "CHAIN -- start a new xact with same options"), ("NO CHAIN", "NO CHAIN (default)")]);
  }
  if last == "NO" && words.contains(&"AND") {
    return Some(&[("CHAIN", "NO CHAIN")]);
  }
  // ROLLBACK TO <cursor>
  if last == "TO" && first == "ROLLBACK" {
    return Some(&[("SAVEPOINT", "TO SAVEPOINT <name> -- optional kw")]);
  }
  // COMMIT/ROLLBACK PREPARED <cursor> -- user types '<gxid>'; no kw.
  if last == "PREPARED" {
    return None;
  }
  None
}

/// RELEASE SAVEPOINT chain. `RELEASE [SAVEPOINT] <name>`.
fn release_savepoint_next_keyword(
  source: &str,
  offset: TextSize,
) -> Option<&'static [(&'static str, &'static str)]> {
  if cursor_not_at_ws_boundary(source, offset) {
    return None;
  }
  let (_, upper) = stmt_slice_upper(source, offset);
  let words: Vec<&str> = upper.split_ascii_whitespace().collect();
  if words.first() != Some(&"RELEASE") {
    return None;
  }
  if words.len() == 1 {
    return Some(&[("SAVEPOINT", "SAVEPOINT <name> -- optional keyword")]);
  }
  None
}

/// COPY (...) paren options menu. Fires inside `COPY ... WITH ( <cursor> )`
/// or directly inside `COPY ... ( <cursor> )` (PG9.6+ accepts both).
/// Also handles the value slot for FORMAT.
fn copy_paren_options_keyword(
  source: &str,
  offset: TextSize,
) -> Option<&'static [(&'static str, &'static str)]> {
  let pos: usize = (u32::from(offset) as usize).min(source.len());
  let bytes = source.as_bytes();
  if pos < bytes.len() && !bytes[pos].is_ascii_whitespace() && bytes[pos] != b'(' && bytes[pos] != b',' {
    return None;
  }
  let (slice_owned, upper) = stmt_slice_upper(source, offset);
  let slice = slice_owned.as_str();
  if !upper.starts_with("COPY") {
    return None;
  }
  // Cursor must sit inside an open paren past `WITH` or directly after
  // the table list (the modern form).
  let opens = slice.matches('(').count();
  let closes = slice.matches(')').count();
  if opens <= closes {
    return None;
  }
  let words: Vec<&str> = upper.split_ascii_whitespace().collect();
  let last_tok =
    upper.split(|c: char| c.is_whitespace() || c == ',' || c == '(').rfind(|s| !s.is_empty()).unwrap_or("");
  if last_tok == "FORMAT" {
    return Some(&[
      ("TEXT", "FORMAT TEXT -- default tab-separated"),
      ("CSV", "FORMAT CSV"),
      ("BINARY", "FORMAT BINARY"),
    ]);
  }
  if last_tok == "HEADER" {
    return Some(&[
      ("TRUE", "HEADER TRUE -- emit/skip a header row"),
      ("FALSE", "HEADER FALSE -- default"),
      ("MATCH", "HEADER MATCH -- verify header columns match (PG16+)"),
    ]);
  }
  if matches!(last_tok, "FREEZE" | "OIDS" | "DEFAULT") {
    return Some(&[("TRUE", "TRUE"), ("FALSE", "FALSE")]);
  }
  if !words.contains(&"WITH") && opens == 1 {
    // We're inside COPY <tbl> (cols) -- emit nothing; user is typing columns.
    return None;
  }
  // Fresh option slot inside WITH paren.
  Some(&[
    ("FORMAT", "FORMAT { TEXT | CSV | BINARY }"),
    ("FREEZE", "FREEZE -- mark imported tuples as already-committed (load only)"),
    ("DELIMITER", "DELIMITER '<char>'"),
    ("NULL", "NULL '<text>' -- literal that means NULL"),
    ("HEADER", "HEADER { TRUE | FALSE | MATCH }"),
    ("QUOTE", "QUOTE '<char>'"),
    ("ESCAPE", "ESCAPE '<char>'"),
    ("FORCE_QUOTE", "FORCE_QUOTE { * | (col[, ...]) }"),
    ("FORCE_NOT_NULL", "FORCE_NOT_NULL (col[, ...])"),
    ("FORCE_NULL", "FORCE_NULL (col[, ...])"),
    ("ENCODING", "ENCODING '<name>'"),
    ("DEFAULT", "DEFAULT '<text>' -- string treated as column DEFAULT (PG16+)"),
    ("ON_ERROR", "ON_ERROR { STOP | IGNORE } -- per-row error handling (PG17+)"),
    ("LOG_VERBOSITY", "LOG_VERBOSITY { DEFAULT | VERBOSE } (PG17+)"),
  ])
}

/// COPY statement phase detector. Slot chain:
///   COPY <tbl> <cursor> -> FROM | TO | (<cols>)
///   COPY <tbl> FROM <cursor> -> STDIN | PROGRAM | '<file>'
///   COPY <tbl> TO <cursor>   -> STDOUT | PROGRAM | '<file>'
///   COPY ... WITH <cursor>   -> ( FORMAT / DELIMITER / HEADER / ... )
///   COPY ... <after target> <cursor> -> WITH | (options)
fn copy_next_keyword(source: &str, offset: TextSize) -> Option<&'static [(&'static str, &'static str)]> {
  if cursor_not_at_ws_boundary(source, offset) {
    return None;
  }
  let (slice_owned, upper) = stmt_slice_upper(source, offset);
  let _ = slice_owned;
  let words: Vec<&str> = upper.split_ascii_whitespace().collect();
  if words.first() != Some(&"COPY") {
    return None;
  }
  let n = words.len();
  // COPY alone -> nothing useful; let catalog table list flow through.
  if n < 2 {
    return None;
  }
  let last = words[n - 1];
  if last == "FROM" {
    return Some(&[
      ("STDIN", "FROM STDIN -- pipe data on the wire"),
      ("PROGRAM", "FROM PROGRAM '<cmd>' -- spawn cmd, read its stdout"),
    ]);
  }
  if last == "TO" {
    return Some(&[
      ("STDOUT", "TO STDOUT -- stream data on the wire"),
      ("PROGRAM", "TO PROGRAM '<cmd>' -- spawn cmd, write to its stdin"),
    ]);
  }
  if last == "WITH" {
    return Some(&[
      ("(FORMAT CSV", "WITH (FORMAT CSV, ...)"),
      ("(FORMAT TEXT", "WITH (FORMAT TEXT, ...) -- default"),
      ("(FORMAT BINARY", "WITH (FORMAT BINARY, ...)"),
    ]);
  }
  // After COPY <tbl> (n == 2) or COPY <tbl> (<col_list>) -> FROM | TO
  let has_from_or_to = words.contains(&"FROM") || words.contains(&"TO");
  if !has_from_or_to {
    return Some(&[("FROM", "FROM <source>"), ("TO", "TO <destination>")]);
  }
  // After STDIN/STDOUT/<file>/PROGRAM body -> WITH | DELIMITER | etc.
  if matches!(last, "STDIN" | "STDOUT") || last.ends_with('\'') {
    return Some(&[
      ("WITH", "WITH (<options>) -- group all options in one paren list"),
      ("DELIMITER", "DELIMITER '<char>'"),
      ("HEADER", "HEADER -- skip first row on COPY FROM, write on COPY TO"),
      ("CSV", "CSV -- shorthand for FORMAT CSV"),
      ("BINARY", "BINARY -- shorthand for FORMAT BINARY"),
      ("FREEZE", "FREEZE -- mark imported tuples as already committed"),
      ("NULL", "NULL '<text>' -- literal that means NULL"),
      ("QUOTE", "QUOTE '<char>'"),
      ("ESCAPE", "ESCAPE '<char>'"),
      ("ENCODING", "ENCODING '<name>'"),
    ]);
  }
  None
}

/// DO $$ ... $$ phase detector. Triggered when the cursor sits right
/// after `DO ` (the dollar-quoted code body is handled by the PL/pgSQL
/// in-body detector; this is the LANGUAGE / dollar-tag slot before the
/// body opens).
fn do_block_next_keyword(source: &str, offset: TextSize) -> Option<&'static [(&'static str, &'static str)]> {
  if cursor_not_at_ws_boundary(source, offset) {
    return None;
  }
  let (slice_owned, upper) = stmt_slice_upper(source, offset);
  let _ = slice_owned;
  let words: Vec<&str> = upper.split_ascii_whitespace().collect();
  if words.first() != Some(&"DO") {
    return None;
  }
  let n = words.len();
  // `DO <cursor>` -> LANGUAGE / dollar-quoted code start hint.
  if n == 1 {
    return Some(&[("LANGUAGE", "LANGUAGE plpgsql/sql -- explicit (default is plpgsql)"), ("$$", "$$ BEGIN ... END; $$ -- anonymous code block")]);
  }
  if matches!(words.last(), Some(&"LANGUAGE")) {
    return Some(&[
      ("plpgsql", "plpgsql -- procedural Postgres"),
      ("sql", "sql -- pure SQL function body"),
      ("plpython3u", "plpython3u -- Python 3 (untrusted)"),
      ("plperl", "plperl -- Perl"),
      ("pltcl", "pltcl -- Tcl"),
    ]);
  }
  None
}

/// CREATE / ALTER ROLE | USER phase detector. After `<name>` or `WITH`,
/// emits role attribute keywords (LOGIN, NOLOGIN, SUPERUSER, PASSWORD,
/// VALID UNTIL, IN ROLE, IN GROUP, ADMIN, INHERIT, BYPASSRLS, REPLICATION,
/// CREATEDB, CREATEROLE, CONNECTION LIMIT, ENCRYPTED PASSWORD).
fn role_next_keyword(source: &str, offset: TextSize) -> Option<&'static [(&'static str, &'static str)]> {
  if cursor_not_at_ws_boundary(source, offset) {
    return None;
  }
  let (slice_owned, upper) = stmt_slice_upper(source, offset);
  let _ = slice_owned;
  let words: Vec<&str> = upper.split_ascii_whitespace().collect();
  if words.len() < 2 || !matches!(words[0], "CREATE" | "ALTER") {
    return None;
  }
  if !matches!(words[1], "ROLE" | "USER" | "GROUP") {
    return None;
  }
  // CREATE ROLE alice <cursor>  or  ALTER ROLE alice WITH <cursor>  or
  // CREATE ROLE alice WITH PASSWORD <cursor> -> need value (skip emit).
  let last = *words.last()?;
  // After PASSWORD / ENCRYPTED PASSWORD / VALID UNTIL we expect a literal
  // value typed by the user. Stay silent there.
  if matches!(last, "PASSWORD" | "UNTIL" | "LIMIT" | "ROLE" | "GROUP" | "ADMIN") && words.len() >= 3 {
    if last == "ROLE" && words[words.len() - 2] == "IN" {
      return None;
    }
    if last == "GROUP" && words[words.len() - 2] == "IN" {
      return None;
    }
  }
  // Need at least 3 words: CREATE/ALTER + ROLE/USER + <name>
  if words.len() < 3 {
    return None;
  }
  // Allowed slots: directly after name (n==3) or after WITH (last=="WITH")
  // or after any previously-emitted attribute keyword. Always emit the
  // full attribute menu -- client filters by prefix.
  Some(&[
    ("WITH", "WITH <attr_list> -- introduces role attributes"),
    ("LOGIN", "LOGIN -- can authenticate"),
    ("NOLOGIN", "NOLOGIN -- group-only role"),
    ("SUPERUSER", "SUPERUSER -- bypass every permission check"),
    ("NOSUPERUSER", "NOSUPERUSER -- not a superuser (default)"),
    ("CREATEDB", "CREATEDB -- can create databases"),
    ("NOCREATEDB", "NOCREATEDB -- cannot create databases (default)"),
    ("CREATEROLE", "CREATEROLE -- can create/manage other roles"),
    ("NOCREATEROLE", "NOCREATEROLE -- default"),
    ("INHERIT", "INHERIT -- auto-inherit privs from member-of roles (default)"),
    ("NOINHERIT", "NOINHERIT -- require explicit SET ROLE"),
    ("REPLICATION", "REPLICATION -- can initiate streaming replication"),
    ("NOREPLICATION", "NOREPLICATION -- default"),
    ("BYPASSRLS", "BYPASSRLS -- bypass row-level security"),
    ("NOBYPASSRLS", "NOBYPASSRLS -- default"),
    ("PASSWORD", "PASSWORD '<text>' -- role password"),
    ("ENCRYPTED PASSWORD", "ENCRYPTED PASSWORD '<text>' -- pre-hashed password"),
    ("VALID UNTIL", "VALID UNTIL '<timestamptz>' -- expiry"),
    ("CONNECTION LIMIT", "CONNECTION LIMIT <n> -- per-role parallel session cap"),
    ("IN ROLE", "IN ROLE <r>[, ...] -- inherit from these roles"),
    ("IN GROUP", "IN GROUP <r>[, ...] -- SQL-standard alias for IN ROLE"),
    ("ROLE", "ROLE <r>[, ...] -- members of THIS role"),
    ("ADMIN", "ADMIN <r>[, ...] -- members with WITH ADMIN OPTION"),
    ("USER", "USER <r>[, ...] -- alias for ROLE"),
    ("SYSID", "SYSID <oid> -- legacy; ignored"),
  ])
}

/// True when the cursor sits inside the `WHEN (...)` paren of a
/// CREATE TRIGGER statement. Returns the trigger's target table name
/// so the engine can pull column entries (and `OLD.<col>`/`NEW.<col>`
/// row-alias suggestions) for that table.
fn create_trigger_when_table(source: &str, offset: TextSize) -> Option<String> {
  let (slice_owned, upper) = stmt_slice_upper(source, offset);
  let slice = slice_owned.as_str();
  if !upper.contains("CREATE") || !upper.contains("TRIGGER") {
    return None;
  }
  // Find the WHEN keyword (whole-word, case-insensitive) before the cursor.
  let bytes = upper.as_bytes();
  let mut from = 0usize;
  let mut when_at: Option<usize> = None;
  while let Some(rel) = upper[from..].find("WHEN") {
    let at = from + rel;
    let prev_ok = at == 0 || !(bytes[at - 1].is_ascii_alphanumeric() || bytes[at - 1] == b'_');
    let next_ok =
      at + 4 >= bytes.len() || !(bytes[at + 4].is_ascii_alphanumeric() || bytes[at + 4] == b'_');
    if prev_ok && next_ok {
      when_at = Some(at);
    }
    from = at + 4;
  }
  let when_at = when_at?;
  // Cursor must sit inside the WHEN paren -- look for `(` after WHEN
  // and verify paren depth > 0 at the cursor.
  let after_when = &slice[when_at + 4..];
  let paren_open = after_when.find('(')?;
  let mut depth = 0i32;
  for c in after_when[paren_open..].chars() {
    match c {
      '(' => depth += 1,
      ')' => depth -= 1,
      _ => {},
    }
  }
  if depth <= 0 {
    return None;
  }
  // Pull the target table from `ON <tbl>` between TRIGGER and WHEN.
  let on_rel = upper[..when_at].rfind(" ON ")?;
  let tbl_start = on_rel + 4;
  let tbl_str: String = upper[tbl_start..]
    .chars()
    .take_while(|c| c.is_alphanumeric() || *c == '_' || *c == '.')
    .collect();
  let tbl = tbl_str.rsplit('.').next().unwrap_or(&tbl_str).to_ascii_lowercase();
  if tbl.is_empty() {
    return None;
  }
  Some(tbl)
}

/// CREATE [OR REPLACE] TRIGGER phase detector. Slot chain after the
/// trigger name + ON table is reached:
///   CREATE TRIGGER <name> <cursor> -> BEFORE / AFTER / INSTEAD OF
///   ... BEFORE <cursor> -> INSERT / UPDATE / DELETE / TRUNCATE
///   ... <event> <cursor> -> OR (chain another event) | ON
///   ... ON <tbl> <cursor> -> FROM | DEFERRABLE | NOT DEFERRABLE | FOR | WHEN | EXECUTE
///   ... FOR <cursor> -> EACH
///   ... FOR EACH <cursor> -> ROW | STATEMENT
///   ... FOR EACH ROW <cursor> -> WHEN | EXECUTE
///   ... WHEN <cursor>  -- expression context; stay silent
///   ... EXECUTE <cursor> -> FUNCTION | PROCEDURE
fn create_trigger_next_keyword(source: &str, offset: TextSize) -> Option<&'static [(&'static str, &'static str)]> {
  if cursor_not_at_ws_boundary(source, offset) {
    return None;
  }
  let (slice_owned, upper) = stmt_slice_upper(source, offset);
  let slice = slice_owned.as_str();
  let words: Vec<&str> = upper.split_ascii_whitespace().collect();
  // CREATE [OR REPLACE] [CONSTRAINT] TRIGGER <name> ...
  let trigger_idx = words.iter().position(|w| *w == "TRIGGER")?;
  if !words[..trigger_idx].iter().all(|w| matches!(*w, "CREATE" | "OR" | "REPLACE" | "CONSTRAINT")) || words[0] != "CREATE" {
    return None;
  }
  let after_kw = &words[trigger_idx + 1..];
  if after_kw.is_empty() {
    return None;
  }
  // First word after TRIGGER is the trigger name. Slot chain starts at
  // position 1 of `after_kw`.
  if after_kw.len() == 1 {
    return None;
  }
  let tail = &after_kw[1..]; // drop the trigger name
  // tail empty here means cursor is right after <name>.
  if tail.is_empty() || !slice.ends_with(char::is_whitespace) {
    return Some(&[
      ("BEFORE", "BEFORE <event> ON <table>"),
      ("AFTER", "AFTER <event> ON <table>"),
      ("INSTEAD OF", "INSTEAD OF <event> ON <view>"),
    ]);
  }
  let last = *tail.last()?;
  let upper_tail: Vec<&str> = tail.to_vec();
  // EXECUTE <cursor>
  if last == "EXECUTE" {
    return Some(&[("FUNCTION", "EXECUTE FUNCTION <fn>(<args>)"), ("PROCEDURE", "EXECUTE PROCEDURE <fn>(<args>) -- pre-PG11")]);
  }
  // FOR / FOR EACH
  if last == "FOR" {
    return Some(&[("EACH", "FOR EACH ROW | STATEMENT")]);
  }
  if last == "EACH" {
    return Some(&[("ROW", "FOR EACH ROW -- per-row trigger"), ("STATEMENT", "FOR EACH STATEMENT -- one fire per statement")]);
  }
  if matches!(last, "ROW" | "STATEMENT") && upper_tail.contains(&"EACH") {
    return Some(&[("WHEN", "WHEN (<predicate>) -- conditional"), ("EXECUTE", "EXECUTE {FUNCTION|PROCEDURE} <fn>(<args>)")]);
  }
  // After BEFORE/AFTER/INSTEAD OF, expect an event keyword.
  if matches!(last, "BEFORE" | "AFTER") {
    return Some(&[
      ("INSERT", "INSERT -- fire on INSERT"),
      ("UPDATE", "UPDATE -- fire on UPDATE"),
      ("DELETE", "DELETE -- fire on DELETE"),
      ("TRUNCATE", "TRUNCATE -- fire on TRUNCATE (statement-level only)"),
    ]);
  }
  if matches!(last, "OF") && upper_tail.contains(&"INSTEAD") {
    return Some(&[("INSERT", "INSERT"), ("UPDATE", "UPDATE"), ("DELETE", "DELETE")]);
  }
  // After an event word, chain via OR <event> or ON <table>.
  if matches!(last, "INSERT" | "UPDATE" | "DELETE" | "TRUNCATE") {
    return Some(&[("ON", "ON <table>"), ("OR", "OR <event> -- chain another event"), ("OF", "UPDATE OF <col>[, ...] -- column-list-scoped UPDATE event")]);
  }
  // REFERENCING chain: REFERENCING { OLD | NEW } TABLE [AS] <alias> [...]
  if last == "REFERENCING" {
    return Some(&[
      ("OLD TABLE AS", "OLD TABLE AS <old_rows_alias>"),
      ("NEW TABLE AS", "NEW TABLE AS <new_rows_alias>"),
    ]);
  }
  if matches!(last, "OLD" | "NEW") && upper_tail.contains(&"REFERENCING") {
    return Some(&[("TABLE AS", "TABLE AS <alias_name>")]);
  }
  if last == "TABLE" && upper_tail.contains(&"REFERENCING") {
    return Some(&[("AS", "AS <alias_name>")]);
  }
  // After ON <tbl>, expect FROM / DEFERRABLE / FOR / WHEN / EXECUTE / REFERENCING.
  if upper_tail.contains(&"ON") && !upper_tail.contains(&"FOR") && !upper_tail.contains(&"EXECUTE") {
    return Some(&[
      ("FROM", "FROM <referenced_table> -- only on CONSTRAINT triggers"),
      ("DEFERRABLE", "DEFERRABLE -- CONSTRAINT trigger only"),
      ("NOT DEFERRABLE", "NOT DEFERRABLE (default)"),
      ("INITIALLY DEFERRED", "INITIALLY DEFERRED"),
      ("INITIALLY IMMEDIATE", "INITIALLY IMMEDIATE (default)"),
      ("REFERENCING", "REFERENCING OLD/NEW TABLE AS <name>"),
      ("FOR", "FOR EACH {ROW|STATEMENT}"),
      ("WHEN", "WHEN (<predicate>) -- conditional"),
      ("EXECUTE", "EXECUTE {FUNCTION|PROCEDURE} <fn>()"),
    ]);
  }
  None
}

/// CREATE INDEX phase detector. Handles slots NOT covered by the
/// existing `create_index::detect` (which routes inside-paren / ON
/// slots). Adds: after-table-no-paren -> USING|(; after-USING -> method;
/// after-closing-paren -> INCLUDE | NULLS [NOT] DISTINCT | WITH |
/// TABLESPACE | WHERE.
fn create_index_next_keyword(source: &str, offset: TextSize) -> Option<&'static [(&'static str, &'static str)]> {
  if cursor_not_at_ws_boundary(source, offset) {
    return None;
  }
  let (slice_owned, upper) = stmt_slice_upper(source, offset);
  let slice = slice_owned.as_str();
  let words: Vec<&str> = upper.split_ascii_whitespace().collect();
  let starts_index = words.starts_with(&["CREATE", "INDEX"])
    || words.starts_with(&["CREATE", "UNIQUE", "INDEX"])
    || (words.len() >= 3 && words[0] == "CREATE" && words[1] == "INDEX" && words[2] == "CONCURRENTLY")
    || (words.len() >= 4 && words[0] == "CREATE" && words[1] == "UNIQUE" && words[2] == "INDEX" && words[3] == "CONCURRENTLY");
  if !starts_index {
    return None;
  }
  // After USING <cursor> -> emit method names.
  if matches!(words.last(), Some(&"USING")) {
    return Some(&[
      ("btree", "B-tree (default) -- equality + range"),
      ("hash", "Hash -- equality only"),
      ("gist", "GiST -- geometric / range / fts / array"),
      ("gin", "GIN -- inverted index for arrays / jsonb / fts"),
      ("brin", "BRIN -- block-range, small index for sorted big tables"),
      ("spgist", "SP-GiST -- space-partitioned GiST"),
    ]);
  }
  // After closing `)` of the column list -> trailing-clause keywords.
  // Heuristic: paren depth at cursor == 0 AND a USING-or-paren run has
  // appeared before the cursor.
  let has_on = words.contains(&"ON");
  let opens = slice.matches('(').count();
  let closes = slice.matches(')').count();
  if has_on && opens > 0 && closes >= opens && slice.trim_end().ends_with(')') {
    return Some(&[
      ("INCLUDE", "INCLUDE (<cols>) -- non-key columns stored on leaf (PG11+)"),
      ("NULLS DISTINCT", "NULLS DISTINCT -- treat NULLs as different (default for non-unique)"),
      ("NULLS NOT DISTINCT", "NULLS NOT DISTINCT -- treat all NULLs as equal in UNIQUE (PG15+)"),
      ("WITH", "WITH (<storage_params>) -- e.g. fillfactor = 80"),
      ("TABLESPACE", "TABLESPACE <name> -- pin index storage"),
      ("WHERE", "WHERE <predicate> -- partial index"),
    ]);
  }
  None
}

/// ALTER TYPE phase detector. PG syntax:
///   ALTER TYPE <name> ADD VALUE [IF NOT EXISTS] 'newval' [BEFORE|AFTER 'existing']
///   ALTER TYPE <name> RENAME VALUE 'old' TO 'new'
///   ALTER TYPE <name> RENAME TO <new>
///   ALTER TYPE <name> OWNER TO <role>
///   ALTER TYPE <name> SET SCHEMA <schema>
///   ALTER TYPE <name> ADD ATTRIBUTE <name> <type> [CASCADE|RESTRICT]
fn alter_type_next_keyword(source: &str, offset: TextSize) -> Option<&'static [(&'static str, &'static str)]> {
  if cursor_not_at_ws_boundary(source, offset) {
    return None;
  }
  let (slice_owned, _) = stmt_slice_upper(source, offset);
  let slice = slice_owned.trim();
  let upper = slice.to_ascii_uppercase();
  let words: Vec<&str> = upper.split_ascii_whitespace().collect();
  if words.len() < 2 || words[0] != "ALTER" || words[1] != "TYPE" {
    return None;
  }
  let n = words.len();
  // ALTER TYPE <name> <cursor>
  if n == 3 {
    return Some(&[
      ("ADD VALUE", "ADD VALUE [IF NOT EXISTS] '<lit>' [BEFORE|AFTER '<existing>']"),
      ("RENAME VALUE", "RENAME VALUE '<old>' TO '<new>'"),
      ("RENAME TO", "RENAME TO <new_type_name>"),
      ("OWNER TO", "OWNER TO <role>"),
      ("SET SCHEMA", "SET SCHEMA <schema>"),
      ("ADD ATTRIBUTE", "ADD ATTRIBUTE <name> <type> [CASCADE|RESTRICT]"),
      ("DROP ATTRIBUTE", "DROP ATTRIBUTE [IF EXISTS] <name> [CASCADE|RESTRICT]"),
      ("ALTER ATTRIBUTE", "ALTER ATTRIBUTE <name> [SET DATA] TYPE <type>"),
    ]);
  }
  // ALTER TYPE <name> ADD <cursor>
  if n == 4 && words[3] == "ADD" {
    return Some(&[("VALUE", "VALUE [IF NOT EXISTS] '<lit>'"), ("ATTRIBUTE", "ATTRIBUTE <name> <type>")]);
  }
  // ALTER TYPE <name> ADD VALUE <cursor> -> IF NOT EXISTS
  if n == 5 && words[3] == "ADD" && words[4] == "VALUE" {
    return Some(&[("IF NOT EXISTS", "IF NOT EXISTS '<lit>' -- silently skip if value already present")]);
  }
  // ALTER TYPE <name> ADD VALUE 'lit' <cursor> -> BEFORE|AFTER
  if n >= 6 && words[3] == "ADD" && words[4] == "VALUE" && !words.contains(&"BEFORE") && !words.contains(&"AFTER") {
    let last = words.last()?;
    if last.starts_with('\'') && last.ends_with('\'') {
      return Some(&[("BEFORE", "BEFORE '<existing>' -- insert before this value"), ("AFTER", "AFTER '<existing>' -- insert after this value")]);
    }
  }
  // ALTER TYPE <name> RENAME <cursor>
  if n == 4 && words[3] == "RENAME" {
    return Some(&[("VALUE", "VALUE '<old>' TO '<new>'"), ("TO", "TO <new_type_name>"), ("ATTRIBUTE", "ATTRIBUTE <old> TO <new>")]);
  }
  // ALTER TYPE <name> RENAME VALUE 'lit' <cursor>
  if n == 6 && words[3] == "RENAME" && words[4] == "VALUE" {
    return Some(&[("TO", "TO '<new>' -- new value literal")]);
  }
  // ALTER TYPE <name> SET <cursor>
  if n == 4 && words[3] == "SET" {
    return Some(&[("SCHEMA", "SCHEMA <schema_name>")]);
  }
  // ALTER TYPE <name> OWNER <cursor>
  if n == 4 && words[3] == "OWNER" {
    return Some(&[("TO", "TO <role>")]);
  }
  None
}

/// MERGE statement phase detector. Walks the trailing tokens of the
/// statement at the cursor and returns the next legal keyword slot, if
/// the statement is a MERGE. Returns None on non-MERGE or when the
/// cursor sits in an expression / identifier slot the generic engine
/// handles already.
/// True when the cursor sits at `MERGE ... THEN UPDATE SET <col> =
/// <cursor>` -- the RHS expression slot. Detect by walking back: the
/// last `=` token (word-bounded) sits between SET and the cursor, and
/// the statement starts with MERGE.
/// True when the cursor sits at `MERGE ... THEN UPDATE SET <cursor>`
/// or `... SET col=v, <cursor>` — the LHS column slot. Distinguished
/// from RHS by: SET present, and either no `=` after SET yet, OR the
/// last word is a comma-trailing comma terminator (so a new assignment
/// is starting).
fn merge_update_set_lhs_slot(source: &str, offset: TextSize) -> bool {
  if cursor_not_at_ws_boundary(source, offset) {
    return false;
  }
  let (slice, upper) = stmt_slice_upper(source, offset);
  let words: Vec<&str> = upper.split_ascii_whitespace().collect();
  if words.first() != Some(&"MERGE") {
    return false;
  }
  let set_idx = match words.iter().rposition(|w| *w == "SET") {
    Some(i) => i,
    None => return false,
  };
  let after_set = &words[set_idx + 1..];
  // Bare SET → first LHS slot.
  if after_set.is_empty() {
    return true;
  }
  // Trailing comma → next LHS slot.
  let trimmed = slice.trim_end();
  trimmed.ends_with(',')
}

fn merge_update_set_rhs_expr_slot(source: &str, offset: TextSize) -> bool {
  if cursor_not_at_ws_boundary(source, offset) {
    return false;
  }
  let (_, upper) = stmt_slice_upper(source, offset);
  let words: Vec<&str> = upper.split_ascii_whitespace().collect();
  if words.first() != Some(&"MERGE") {
    return false;
  }
  // Need SET earlier and `=` between SET and cursor.
  let set_idx = match words.iter().rposition(|w| *w == "SET") {
    Some(i) => i,
    None => return false,
  };
  let after_set = &words[set_idx + 1..];
  // Allow trailing comma-separated assignments too: just verify any
  // `=` is present in the tail.
  after_set.contains(&"=")
}

/// True when the cursor sits at `MERGE ... WHEN NOT MATCHED THEN
/// INSERT (<cursor>` -- the column-list slot. Detect by walking back
/// to an unmatched `(` whose preceding word is INSERT, with MERGE as
/// the statement-leading keyword.
fn merge_insert_col_list_slot(source: &str, offset: TextSize) -> bool {
  let pos: usize = (u32::from(offset) as usize).min(source.len());
  let bytes = source.as_bytes();
  let (_, upper) = stmt_slice_upper(source, offset);
  let words: Vec<&str> = upper.split_ascii_whitespace().collect();
  if words.first() != Some(&"MERGE") {
    return false;
  }
  // Walk back from cursor to the nearest unmatched `(`.
  let mut depth = 0i32;
  let mut i = pos;
  while i > 0 {
    i -= 1;
    match bytes[i] {
      b')' => depth += 1,
      b'(' => {
        if depth == 0 {
          break;
        }
        depth -= 1;
      },
      _ => {},
    }
  }
  if bytes.get(i) != Some(&b'(') {
    return false;
  }
  // Preceding word must be INSERT.
  let mut e = i;
  while e > 0 && bytes[e - 1].is_ascii_whitespace() {
    e -= 1;
  }
  let mut s = e;
  while s > 0 && (bytes[s - 1].is_ascii_alphanumeric() || bytes[s - 1] == b'_') {
    s -= 1;
  }
  s != e && source[s..e].eq_ignore_ascii_case("INSERT")
}

/// True when the cursor sits at `MERGE ... WHEN [NOT] MATCHED AND
/// <cursor>` -- the predicate slot. Returns false when AND is absent
/// or the statement isn't a MERGE.
fn merge_when_matched_and_predicate_slot(source: &str, offset: TextSize) -> bool {
  if cursor_not_at_ws_boundary(source, offset) {
    return false;
  }
  let (_, upper) = stmt_slice_upper(source, offset);
  let words: Vec<&str> = upper.split_ascii_whitespace().collect();
  if words.first() != Some(&"MERGE") {
    return false;
  }
  let n = words.len();
  if n < 2 || words[n - 1] != "AND" {
    return false;
  }
  // Verify the AND is the WHEN [NOT] MATCHED AND form, not some inner AND.
  // Walk backwards: skip arbitrary tokens between MATCHED and AND -- there
  // must be no THEN between them.
  let mut i = n - 1;
  while i > 0 {
    i -= 1;
    if words[i] == "THEN" {
      return false;
    }
    if words[i] == "MATCHED" {
      return true;
    }
  }
  false
}

/// Parse `MERGE INTO <target> [AS alias_t] USING <source> [AS alias_s]`
/// from the buffer and return (target, source) table names. Crude
/// whitespace tokenization; preserves original case.
fn merge_target_and_source(source: &str) -> (Option<String>, Option<String>) {
  let upper = source.to_ascii_uppercase();
  let into = upper.find("MERGE INTO ");
  let using = upper.find(" USING ");
  let into_pos = match into { Some(p) => p + "MERGE INTO ".len(), None => return (None, None) };
  let using_pos = match using { Some(p) => p, None => return (None, None) };
  if using_pos <= into_pos {
    return (None, None);
  }
  let target = extract_first_ident(&source[into_pos..using_pos]);
  let after_using = &source[using_pos + " USING ".len()..];
  let on_at = after_using.to_ascii_uppercase().find(" ON ").unwrap_or(after_using.len());
  let src = extract_first_ident(&after_using[..on_at]);
  (target, src)
}

/// Extract `MERGE INTO <t> <alias_t>, USING <s> <alias_s>` aliases
/// (the bare identifiers right after the table names).
fn merge_aliases(source: &str) -> Vec<String> {
  let mut out = Vec::new();
  let upper = source.to_ascii_uppercase();
  if let Some(p) = upper.find("MERGE INTO ") {
    let rest = &source[p + "MERGE INTO ".len()..];
    if let Some(alias) = nth_ident(rest, 1) {
      out.push(alias);
    }
  }
  if let Some(p) = upper.find(" USING ") {
    let rest = &source[p + " USING ".len()..];
    if let Some(alias) = nth_ident(rest, 1) {
      out.push(alias);
    }
  }
  out
}

fn extract_first_ident(s: &str) -> Option<String> {
  nth_ident(s, 0)
}

/// Pick the n-th whitespace-separated bare identifier (alphanumeric +
/// underscore + dot). Skips the optional `AS` keyword.
fn nth_ident(s: &str, n: usize) -> Option<String> {
  let mut idx = 0;
  for tok in s.split_ascii_whitespace() {
    let t = tok.trim_end_matches([',', ';']);
    if t.eq_ignore_ascii_case("AS") {
      continue;
    }
    if !t.chars().next().map(|c| c.is_ascii_alphabetic() || c == '_').unwrap_or(false) {
      return None;
    }
    let ident: String = t.chars().take_while(|c| c.is_ascii_alphanumeric() || *c == '_' || *c == '.').collect();
    if ident.is_empty() {
      return None;
    }
    if idx == n {
      // Drop schema-qualifier prefix; keep just the last segment.
      return Some(ident.rsplit('.').next().unwrap_or(&ident).to_string());
    }
    idx += 1;
  }
  None
}

fn merge_next_keyword(source: &str, offset: TextSize) -> Option<&'static [(&'static str, &'static str)]> {
  if cursor_not_at_ws_boundary(source, offset) {
    return None;
  }
  let (slice_owned, _) = stmt_slice_upper(source, offset);
  let slice = slice_owned.trim();
  let upper = slice.to_ascii_uppercase();
  let words: Vec<&str> = upper.split_ascii_whitespace().collect();
  if words.first() != Some(&"MERGE") {
    return None;
  }
  // Map trailing-token shape to the next keyword set.
  let n = words.len();
  // `MERGE` alone -> INTO
  if n == 1 {
    return Some(&[("INTO", "MERGE INTO <target_table>")]);
  }
  // `MERGE INTO <target>` -> USING
  if n >= 3 && words[1] == "INTO" && !words[n - 1].chars().all(|c| c.is_ascii_uppercase()) {
    // crude: last word looks like an ident -> we have the target.
    if !["USING", "AS", "ON", "WHEN"].contains(&words[n - 1]) {
      return Some(&[("USING", "USING <source_table_or_subquery>"), ("AS", "AS <alias>")]);
    }
  }
  // `... USING <source>` -> ON
  if words.contains(&"USING") && !words.contains(&"ON") && !words.contains(&"WHEN") {
    let after_using_idx = words.iter().position(|w| *w == "USING").unwrap();
    if n > after_using_idx + 1 {
      return Some(&[("ON", "ON <join_condition>")]);
    }
  }
  // After ON ... -> WHEN
  if words.contains(&"ON") && !words.contains(&"WHEN") {
    return Some(&[("WHEN", "WHEN [NOT] MATCHED [AND ...] THEN ...")]);
  }
  // `WHEN <cursor>` -> MATCHED / NOT MATCHED
  if matches!(words.last(), Some(&"WHEN")) {
    return Some(&[("MATCHED", "WHEN MATCHED [AND ...] THEN ..."), ("NOT MATCHED", "WHEN NOT MATCHED [AND ...] THEN ...")]);
  }
  // `... MATCHED` (no THEN yet) -> THEN | AND
  if matches!(words.last(), Some(&"MATCHED")) && !words.contains(&"THEN") {
    return Some(&[("THEN", "THEN <action>"), ("AND", "AND <extra_condition>")]);
  }
  // `WHEN MATCHED THEN <cursor>` -> UPDATE / DELETE / DO NOTHING
  if let Some(then_idx) = words.iter().rposition(|w| *w == "THEN")
    && then_idx == n - 1
  {
    let prior = &words[..then_idx];
    let is_not_matched = prior.windows(2).any(|w| w[0] == "NOT" && w[1] == "MATCHED");
    if is_not_matched {
      return Some(&[("INSERT", "INSERT (<cols>) VALUES (<vals>)"), ("DO NOTHING", "DO NOTHING")]);
    }
    return Some(&[("UPDATE", "UPDATE SET <col> = <val> [, ...]"), ("DELETE", "DELETE"), ("DO NOTHING", "DO NOTHING")]);
  }
  // `WHEN [NOT] MATCHED THEN UPDATE <cursor>` -> SET
  if matches!(words.last(), Some(&"UPDATE"))
    && words.iter().rposition(|w| *w == "THEN").map(|t| t == n - 2).unwrap_or(false)
  {
    return Some(&[("SET", "SET <col> = <val> [, <col> = <val> ...]")]);
  }
  // `WHEN NOT MATCHED THEN INSERT <cursor>` -> VALUES / OVERRIDING / `(`
  if matches!(words.last(), Some(&"INSERT"))
    && words.iter().rposition(|w| *w == "THEN").map(|t| t == n - 2).unwrap_or(false)
  {
    return Some(&[
      ("VALUES", "VALUES (<v1>, <v2>, ...)"),
      ("OVERRIDING", "OVERRIDING { SYSTEM | USER } VALUE"),
      ("DEFAULT VALUES", "DEFAULT VALUES"),
      ("(", "( <col1>, <col2>, ... ) VALUES (...)"),
    ]);
  }
  None
}

/// DROP USER MAPPING follow-up:
///   DROP USER MAPPING <cursor>             -> FOR / IF EXISTS
///   DROP USER MAPPING IF EXISTS <cursor>   -> FOR
///   DROP USER MAPPING FOR <cursor>         -> CURRENT_USER/CURRENT_ROLE/PUBLIC/USER
///   DROP USER MAPPING FOR <role> <cursor>  -> SERVER
fn drop_user_mapping_next_keyword(
  source: &str,
  offset: TextSize,
) -> Option<&'static [(&'static str, &'static str)]> {
  if cursor_not_at_ws_boundary(source, offset) {
    return None;
  }
  let (_, upper) = stmt_slice_upper(source, offset);
  let words: Vec<&str> = upper.split_ascii_whitespace().collect();
  if words.len() < 3 || words[0] != "DROP" || words[1] != "USER" || words[2] != "MAPPING" {
    return None;
  }
  let last = *words.last()?;
  if last == "MAPPING" {
    return Some(&[
      ("FOR", "FOR <role> SERVER <srv>"),
      ("IF EXISTS", "IF EXISTS FOR <role> SERVER <srv>"),
    ]);
  }
  if last == "EXISTS" && words.contains(&"IF") {
    return Some(&[("FOR", "FOR <role> SERVER <srv>")]);
  }
  if last == "FOR" {
    return Some(&[
      ("CURRENT_USER", "FOR CURRENT_USER"),
      ("CURRENT_ROLE", "FOR CURRENT_ROLE"),
      ("PUBLIC", "FOR PUBLIC"),
      ("USER", "FOR USER -- alias of CURRENT_USER"),
    ]);
  }
  if last == "SERVER" {
    return None; // user types server name
  }
  if words.contains(&"FOR") && !words.contains(&"SERVER") {
    return Some(&[("SERVER", "SERVER <name>")]);
  }
  None
}

fn after_top_level_drop_keyword(source: &str, offset: TextSize) -> Option<&'static [(&'static str, &'static str)]> {
  if cursor_not_at_ws_boundary(source, offset) {
    return None;
  }
  let (slice_owned, _) = stmt_slice_upper(source, offset);
  if !slice_owned.trim().eq_ignore_ascii_case("DROP") {
    return None;
  }
  Some(&[
    ("TABLE", "DROP TABLE [IF EXISTS] <name> [CASCADE|RESTRICT]"),
    ("VIEW", "DROP VIEW [IF EXISTS] <name> [CASCADE|RESTRICT]"),
    ("MATERIALIZED VIEW", "DROP MATERIALIZED VIEW [IF EXISTS] <name>"),
    ("INDEX", "DROP INDEX [CONCURRENTLY] [IF EXISTS] <name>"),
    ("SEQUENCE", "DROP SEQUENCE [IF EXISTS] <name>"),
    ("SCHEMA", "DROP SCHEMA [IF EXISTS] <name> [CASCADE|RESTRICT]"),
    ("FUNCTION", "DROP FUNCTION [IF EXISTS] <name>(args)"),
    ("PROCEDURE", "DROP PROCEDURE [IF EXISTS] <name>(args)"),
    ("TRIGGER", "DROP TRIGGER [IF EXISTS] <name> ON <table>"),
    ("TYPE", "DROP TYPE [IF EXISTS] <name> [CASCADE|RESTRICT]"),
    ("ROLE", "DROP ROLE [IF EXISTS] <name>"),
    ("USER", "DROP USER [IF EXISTS] <name>"),
    ("DATABASE", "DROP DATABASE [IF EXISTS] <name>"),
    ("EXTENSION", "DROP EXTENSION [IF EXISTS] <name> [CASCADE]"),
    ("POLICY", "DROP POLICY [IF EXISTS] <name> ON <table>"),
    ("DOMAIN", "DROP DOMAIN [IF EXISTS] <name>"),
    ("AGGREGATE", "DROP AGGREGATE [IF EXISTS] <name>(args)"),
    ("CAST", "DROP CAST [IF EXISTS] (src AS dst)"),
    ("COLLATION", "DROP COLLATION [IF EXISTS] <name>"),
    ("OPERATOR", "DROP OPERATOR [IF EXISTS] <op>(args)"),
    ("RULE", "DROP RULE [IF EXISTS] <name> ON <table>"),
    ("TABLESPACE", "DROP TABLESPACE [IF EXISTS] <name>"),
    ("SUBSCRIPTION", "DROP SUBSCRIPTION [IF EXISTS] <name>"),
    ("PUBLICATION", "DROP PUBLICATION [IF EXISTS] <name>"),
    ("FOREIGN TABLE", "DROP FOREIGN TABLE [IF EXISTS] <name>"),
    ("FOREIGN DATA WRAPPER", "DROP FOREIGN DATA WRAPPER [IF EXISTS] <name>"),
    ("SERVER", "DROP SERVER [IF EXISTS] <name>"),
    ("ROUTINE", "DROP ROUTINE [IF EXISTS] <name>(args)"),
    ("OPERATOR CLASS", "DROP OPERATOR CLASS [IF EXISTS] <name> USING <am>"),
    ("OPERATOR FAMILY", "DROP OPERATOR FAMILY [IF EXISTS] <name> USING <am>"),
    ("STATISTICS", "DROP STATISTICS [IF EXISTS] <name>"),
    ("ACCESS METHOD", "DROP ACCESS METHOD [IF EXISTS] <name>"),
    ("LANGUAGE", "DROP LANGUAGE [IF EXISTS] <name>"),
    ("CONVERSION", "DROP CONVERSION [IF EXISTS] <name>"),
    ("EVENT TRIGGER", "DROP EVENT TRIGGER [IF EXISTS] <name>"),
    ("TRANSFORM", "DROP TRANSFORM FOR <type> LANGUAGE <lang>"),
    ("TEXT SEARCH CONFIGURATION", "DROP TEXT SEARCH CONFIGURATION [IF EXISTS] <name>"),
    ("TEXT SEARCH DICTIONARY", "DROP TEXT SEARCH DICTIONARY [IF EXISTS] <name>"),
    ("TEXT SEARCH PARSER", "DROP TEXT SEARCH PARSER [IF EXISTS] <name>"),
    ("TEXT SEARCH TEMPLATE", "DROP TEXT SEARCH TEMPLATE [IF EXISTS] <name>"),
    ("OWNED", "DROP OWNED BY <role>[, ...] [CASCADE|RESTRICT]"),
    ("MAPPING", "DROP USER MAPPING [IF EXISTS] FOR <role> SERVER <srv>"),
  ])
}

/// True when the cursor in an `ALTER TABLE <t> <ACTION> <cursor>`
/// statement is right after a top-level action keyword (ADD / DROP /
/// RENAME / nested ALTER) and should narrow to that action's
/// sub-keywords (COLUMN / CONSTRAINT / TO / ...). Returns None when
/// the cursor is elsewhere (e.g. mid-name, after the sub-keyword,
/// inside a column declaration), letting other branches handle it.
fn alter_table_subaction_at(source: &str, offset: TextSize) -> Option<&'static [(&'static str, &'static str)]> {
  let pos: usize = (u32::from(offset) as usize).min(source.len());
  let bytes = source.as_bytes();
  // Walk back through trailing whitespace.
  let mut end = pos;
  while end > 0 && bytes[end - 1].is_ascii_whitespace() {
    end -= 1;
  }
  // Read the previous word.
  let mut start = end;
  while start > 0 {
    let b = bytes[start - 1];
    if !(b.is_ascii_alphanumeric() || b == b'_') {
      break;
    }
    start -= 1;
  }
  if start == end {
    return None;
  }
  let word = source[start..end].to_ascii_uppercase();
  // The cursor must sit after that word with only whitespace between
  // (no partial third token).
  if end > pos || pos != end + (pos - end) {
    // (sanity) end <= pos by construction; the cursor at `end` (zero ws)
    // also counts as "right after the word".
  }
  let between = &source[end..pos];
  if !between.chars().all(|c| c.is_ascii_whitespace()) {
    return None;
  }
  // Also: the word before should NOT itself be ADD/DROP/RENAME/ALTER
  // (which would mean we're past the sub-keyword slot already, e.g.
  // `ALTER TABLE users ADD COLUMN` -- cursor here is column-name slot,
  // not sub-keyword slot).
  let mut prev_end = start;
  while prev_end > 0 && bytes[prev_end - 1].is_ascii_whitespace() {
    prev_end -= 1;
  }
  let mut prev_start = prev_end;
  while prev_start > 0 {
    let b = bytes[prev_start - 1];
    if !(b.is_ascii_alphanumeric() || b == b'_') {
      break;
    }
    prev_start -= 1;
  }
  let prev_word = if prev_start < prev_end {
    source[prev_start..prev_end].to_ascii_uppercase()
  } else {
    String::new()
  };
  if matches!(prev_word.as_str(), "ADD" | "DROP" | "RENAME" | "ALTER") {
    return None;
  }
  match word.as_str() {
    "ADD" => Some(&[
      ("COLUMN", "ADD COLUMN <name> <type>"),
      ("CONSTRAINT", "ADD CONSTRAINT <name> <kind>"),
      ("PRIMARY KEY", "ADD PRIMARY KEY (cols)"),
      ("UNIQUE", "ADD UNIQUE (cols)"),
      ("FOREIGN KEY", "ADD FOREIGN KEY (col) REFERENCES other(col)"),
      ("CHECK", "ADD CHECK (predicate)"),
      ("EXCLUDE", "ADD EXCLUDE USING gist (col WITH op)"),
    ]),
    "DROP" => Some(&[
      ("COLUMN", "DROP COLUMN <name> [CASCADE|RESTRICT]"),
      ("CONSTRAINT", "DROP CONSTRAINT <name> [CASCADE|RESTRICT]"),
    ]),
    "RENAME" => Some(&[
      ("COLUMN", "RENAME COLUMN <old> TO <new>"),
      ("CONSTRAINT", "RENAME CONSTRAINT <old> TO <new>"),
      ("TO", "RENAME TO <new_table>"),
    ]),
    "ALTER" => Some(&[
      ("COLUMN", "ALTER COLUMN <name> <action>"),
      ("CONSTRAINT", "ALTER CONSTRAINT <name> DEFERRABLE|...]"),
    ]),
    // OWNER TO <role> -- top-level ALTER TABLE action.
    "OWNER" => Some(&[("TO", "OWNER TO <role>")]),
    // NULLS NOT DISTINCT inside a UNIQUE/PRIMARY KEY constraint.
    "NULLS" => Some(&[("NOT DISTINCT", "NULLS NOT DISTINCT -- treat NULLs as equal for uniqueness")]),
    _ => None,
  }
}

/// True when the cursor sits at `CAST(<expr> AS <cursor>` -- the next
/// legal token is a type identifier. Walks back to the nearest
/// unmatched `(` and checks the preceding word.
fn cast_as_expects_type(source: &str, offset: TextSize) -> bool {
  let pos: usize = (u32::from(offset) as usize).min(source.len());
  let bytes = source.as_bytes();
  // Walk back to the unmatched open paren.
  let mut depth = 0i32;
  let mut i = pos;
  while i > 0 {
    i -= 1;
    match bytes[i] {
      b')' => depth += 1,
      b'(' => {
        if depth == 0 {
          break;
        }
        depth -= 1;
      },
      _ => {},
    }
  }
  if i == 0 && bytes.first() != Some(&b'(') {
    return false;
  }
  // i is the open paren. The preceding word should be CAST.
  let mut e = i;
  while e > 0 && bytes[e - 1].is_ascii_whitespace() {
    e -= 1;
  }
  let mut s = e;
  while s > 0 && (bytes[s - 1].is_ascii_alphanumeric() || bytes[s - 1] == b'_') {
    s -= 1;
  }
  if s == e {
    return false;
  }
  let word = &source[s..e];
  if !word.eq_ignore_ascii_case("CAST") {
    return false;
  }
  // Inside the paren, the substring between `(` and cursor must contain
  // a word-bounded ` AS ` (case-insensitive).
  let inside = &source[i + 1..pos];
  let inside_upper = inside.to_ascii_uppercase();
  inside_upper.contains(" AS ")
}

/// True when the cursor sits at `ALTER TABLE <t> [NO] INHERIT
/// <cursor>` -- the parent-table slot. Without this branch the
/// generic action menu fires and re-offers `INHERIT` / `NO INHERIT`,
/// which makes no sense after the user has already picked one.
/// True when the cursor sits at `ORDER BY <col> [ASC|DESC] NULLS
/// <cursor>` -- the trailing keyword is `NULLS` so the next legal
/// token is FIRST or LAST.
fn order_by_nulls_expects_first_last(source: &str, offset: TextSize) -> bool {
  let pos: usize = (u32::from(offset) as usize).min(source.len());
  let before = &source[..pos];
  if !before.ends_with(char::is_whitespace) {
    return false;
  }
  let upper = before.to_ascii_uppercase();
  let trimmed = upper.trim_end();
  if !trimmed.ends_with(" NULLS") && trimmed != "NULLS" {
    return false;
  }
  upper.contains("ORDER BY")
}

fn alter_table_inherit_expects_parent(source: &str, offset: TextSize) -> bool {
  let (slice, upper) = stmt_slice_upper(source, offset);
  if !slice.ends_with(char::is_whitespace) {
    return false;
  }
  let words: Vec<&str> = upper.split_ascii_whitespace().collect();
  if words.first() != Some(&"ALTER") || words.get(1) != Some(&"TABLE") {
    return false;
  }
  let last = words.last().copied();
  let second = if words.len() >= 2 { Some(words[words.len() - 2]) } else { None };
  matches!(last, Some("INHERIT")) && (second != Some("NO") && second != Some("CHECK") || second == Some("NO"))
}

fn alter_column_set_value_keywords(
  source: &str,
  offset: TextSize,
) -> Option<&'static [(&'static str, &'static str)]> {
  let (slice, upper_full) = stmt_slice_upper(source, offset);
  let upper = upper_full.trim_end();
  if !upper.contains("ALTER COLUMN") || !slice.ends_with(char::is_whitespace) {
    return None;
  }
  if upper.ends_with(" SET STORAGE") {
    return Some(&[
      ("PLAIN", "PLAIN -- inline, never TOASTed"),
      ("EXTERNAL", "EXTERNAL -- TOAST, no compression"),
      ("EXTENDED", "EXTENDED -- TOAST + compression (default for varlena)"),
      ("MAIN", "MAIN -- prefer inline, compress before TOAST"),
      ("DEFAULT", "DEFAULT -- per-type default"),
    ]);
  }
  if upper.ends_with(" SET COMPRESSION") {
    return Some(&[
      ("pglz", "pglz -- the original PG compressor (always available)"),
      ("lz4", "lz4 -- faster, needs build-time --with-lz4"),
      ("default", "default -- inherit per-column / cluster default"),
    ]);
  }
  None
}

fn alter_column_after_set_subkeyword_expects_silence(source: &str, offset: TextSize) -> bool {
  let (slice, upper_full) = stmt_slice_upper(source, offset);
  let upper = upper_full.trim_end().to_string();
  if !upper.contains("ALTER COLUMN") {
    return false;
  }
  for kw in &[" SET DEFAULT", " SET STATISTICS", " SET STORAGE", " SET COMPRESSION"] {
    if upper.ends_with(kw) && slice.ends_with(char::is_whitespace) {
      return true;
    }
  }
  false
}

/// Discriminates SET vs DROP sub-keyword slots within an ALTER COLUMN
/// clause.
#[derive(Copy, Clone)]
enum AlterColumnAction {
  Set,
  Drop,
}

/// True when the cursor sits at the `ALTER COLUMN <name> SET <cursor>`
/// or `... DROP <cursor>` sub-keyword slot. Returns the discriminating
/// kind so the caller can emit the appropriate keyword menu.
fn alter_column_action_kind(source: &str, offset: TextSize) -> Option<AlterColumnAction> {
  let (slice, upper) = stmt_slice_upper(source, offset);
  let stmt = slice.as_str();
  if !upper.contains("ALTER COLUMN") {
    return None;
  }
  // Trim trailing whitespace so we can match the final keyword exactly.
  let trimmed = upper.trim_end();
  // The cursor's most recent keyword must be SET or DROP, and that
  // keyword must follow an ALTER COLUMN <name> sequence (not just any
  // SET / DROP in the statement).
  let after_alter_col = upper.rsplit("ALTER COLUMN").next()?;
  let after_trim = after_alter_col.trim_end();
  // Has a partial type word already been typed after the kind? If so
  // we're past the kind slot and should fall through.
  if after_trim.ends_with(" SET") && stmt.ends_with(char::is_whitespace) {
    return Some(AlterColumnAction::Set);
  }
  if after_trim.ends_with(" DROP") && stmt.ends_with(char::is_whitespace) {
    return Some(AlterColumnAction::Drop);
  }
  let _ = trimmed;
  None
}

/// True when the cursor sits at the type-expected slot right after a
/// freshly-typed column name in `ALTER TABLE <t> ADD COLUMN <name>`.
/// The user has finished naming the new column and is now picking its
/// type, so the menu should be types-only, not the ADD/DROP action set.
fn alter_table_expects_type(source: &str, offset: TextSize) -> bool {
  let (slice, upper) = stmt_slice_upper(source, offset);
  let stmt = slice.as_str();
  // Must be an ALTER TABLE statement with an ADD COLUMN clause.
  if !upper.contains("ALTER TABLE") {
    return false;
  }
  let Some(add_at) = upper.rfind("ADD COLUMN") else { return false };
  // Tokens following ADD COLUMN. Skip the IF NOT EXISTS modifier.
  let after = &stmt[add_at + "ADD COLUMN".len()..];
  let mut tokens = after.split_ascii_whitespace().peekable();
  // Optional IF NOT EXISTS.
  if tokens.peek().is_some_and(|w| w.eq_ignore_ascii_case("IF")) {
    tokens.next();
    if tokens.peek().is_some_and(|w| w.eq_ignore_ascii_case("NOT")) {
      tokens.next();
    }
    if tokens.peek().is_some_and(|w| w.eq_ignore_ascii_case("EXISTS")) {
      tokens.next();
    }
  }
  // The freshly-typed column name. Required.
  if tokens.next().is_none() {
    return false;
  }
  // After the name, the slot expects a type. The cursor is in the type
  // slot when:
  //   - There are no more whitespace-separated tokens (just trailing
  //     whitespace after the name), or
  //   - The last token is a single partial identifier (no further tokens).
  let rest_after_name = stmt[add_at + "ADD COLUMN".len()..]
    .trim_start()
    .split_ascii_whitespace()
    .collect::<Vec<_>>();
  // rest_after_name[0] is the name (possibly with IF NOT EXISTS prefix
  // tokens already consumed conceptually). We accept the type-slot when
  // there's exactly the name and an optional partial type word, with
  // the cursor not having moved on to constraint keywords.
  // The slot is type-expected if total tokens after ADD COLUMN (minus
  // IF NOT EXISTS) is 1 (name) or 2 (name + partial type word).
  let mut effective = rest_after_name.iter().peekable();
  if effective.peek().is_some_and(|w| w.eq_ignore_ascii_case("IF")) {
    effective.next();
    if effective.peek().is_some_and(|w| w.eq_ignore_ascii_case("NOT")) {
      effective.next();
    }
    if effective.peek().is_some_and(|w| w.eq_ignore_ascii_case("EXISTS")) {
      effective.next();
    }
  }
  let rest: Vec<&&str> = effective.collect();
  match rest.len() {
    1 => stmt.ends_with(char::is_whitespace), // just the name typed, cursor in fresh type slot
    2 => !stmt.ends_with(char::is_whitespace), // name + partial type word being typed
    _ => false,
  }
}

/// True when the cursor sits at a target-name slot of a `DROP TABLE`
/// / `DROP TABLE IF EXISTS` / `DROP VIEW` / `DROP MATERIALIZED VIEW` /
/// `TRUNCATE [TABLE] [ONLY]` statement. Used to bypass the generic
/// Unknown catch-all menu that otherwise dumps 600+ items.
fn dml_drop_or_truncate_expects_table(source: &str, offset: TextSize) -> bool {
  let (slice, upper) = stmt_slice_upper(source, offset);
  let stmt = slice.as_str();
  let trimmed_upper = upper.trim_start();
  // Walk through tokens up to the cursor, skip optional modifiers, and
  // confirm we're still in the target-name slot of a recognised kind.
  let kinds: &[(&[&str], &[&str])] = &[
    (&["DROP", "TABLE"], &["IF", "EXISTS", "ONLY"]),
    (&["DROP", "VIEW"], &["IF", "EXISTS"]),
    (&["DROP", "MATERIALIZED", "VIEW"], &["IF", "EXISTS"]),
    (&["DROP", "SEQUENCE"], &["IF", "EXISTS"]),
    (&["DROP", "INDEX"], &["IF", "EXISTS", "CONCURRENTLY"]),
    (&["TRUNCATE", "TABLE"], &["ONLY"]),
    (&["TRUNCATE"], &["ONLY", "TABLE"]),
    // Admin commands whose target is a table-class name.
    (&["VACUUM"], &["FULL", "FREEZE", "VERBOSE", "ANALYZE"]),
    (&["VACUUM", "ANALYZE"], &["VERBOSE"]),
    (&["VACUUM", "FULL"], &["VERBOSE", "ANALYZE"]),
    (&["ANALYZE"], &["VERBOSE", "SKIP_LOCKED"]),
    (&["COPY"], &[]),
    (&["COMMENT", "ON", "TABLE"], &[]),
    (&["COMMENT", "ON", "VIEW"], &[]),
    (&["COMMENT", "ON", "MATERIALIZED", "VIEW"], &[]),
    (&["COMMENT", "ON", "INDEX"], &[]),
    (&["COMMENT", "ON", "SEQUENCE"], &[]),
    (&["REFRESH", "MATERIALIZED", "VIEW"], &["CONCURRENTLY"]),
    (&["LOCK"], &["TABLE"]),
    (&["LOCK", "TABLE"], &["ONLY"]),
    // (CLUSTER handled by `cluster_next_keyword` -- emits VERBOSE first.)
    (&["REINDEX", "TABLE"], &["CONCURRENTLY"]),
    (&["REINDEX", "INDEX"], &["CONCURRENTLY"]),
    // MERGE INTO <table> -- target table.
    (&["MERGE", "INTO"], &["ONLY"]),
  ];
  let words: Vec<&str> = trimmed_upper.split_ascii_whitespace().collect();
  for (kw_seq, mods) in kinds {
    if words.len() < kw_seq.len() {
      continue;
    }
    if kw_seq.iter().zip(words.iter()).all(|(k, w)| {
      // Strip trailing punctuation/commas so e.g. `users,` still matches.
      let cleaned = w.trim_end_matches(|c: char| !c.is_ascii_alphanumeric() && c != '_');
      cleaned.eq_ignore_ascii_case(k)
    }) {
      // Skip optional modifier words; trailing position must still be
      // expecting a name (we don't try to be precise about how many
      // names have been typed -- the comma case below covers it).
      let mut i = kw_seq.len();
      while i < words.len() && mods.iter().any(|m| words[i].eq_ignore_ascii_case(m)) {
        i += 1;
      }
      // Cursor at the end of the slice means we're typing a name.
      // Allow a trailing partial identifier or a trailing `,` (next
      // name in a list).
      let ends_with_comma = stmt.trim_end().ends_with(',');
      let last_word_is_partial = !stmt.ends_with(char::is_whitespace);
      if i >= words.len() || ends_with_comma || last_word_is_partial {
        return true;
      }
    }
  }
  false
}

/// Return the ALTER TABLE target table when the cursor sits right
/// after a `DROP COLUMN [IF EXISTS]` / `RENAME COLUMN` / `ALTER COLUMN`
/// keyword phrase -- the slot that expects an EXISTING column name.
/// None otherwise (so the action-keyword menu still fires).
fn alter_table_existing_column_target(source: &str, offset: TextSize) -> Option<String> {
  let (slice, upper) = stmt_slice_upper(source, offset);
  let stmt = slice.as_str();
  // Must be an ALTER TABLE statement.
  let alter_at = upper.find("ALTER TABLE")?;
  // The cursor's nearest preceding column-introducing phrase.
  let trimmed = upper.trim_end();
  let preceded = |kw: &str| trimmed.ends_with(kw);
  let in_column_slot = preceded(" DROP COLUMN")
    || preceded(" DROP COLUMN IF EXISTS")
    || preceded(" RENAME COLUMN")
    || preceded(" ALTER COLUMN");
  if !in_column_slot {
    return None;
  }
  // Pull the table name right after `ALTER TABLE [IF EXISTS] [ONLY]`.
  let after = alter_at + "ALTER TABLE".len();
  let rest = &stmt[after..];
  let mut cursor = 0usize;
  let mut tokens: Vec<&str> = Vec::new();
  for tok in rest.split_ascii_whitespace() {
    let tok_pos = rest[cursor..].find(tok).map(|p| cursor + p).unwrap_or(cursor);
    cursor = tok_pos + tok.len();
    tokens.push(tok);
    if tokens.len() >= 4 {
      break;
    }
  }
  // Skip optional modifiers, take the first non-modifier token.
  let mods: &[&str] = &["IF", "EXISTS", "ONLY"];
  let table_tok =
    tokens.into_iter().find(|t| !mods.iter().any(|m| t.eq_ignore_ascii_case(m)))?;
  let table = table_tok.rsplit('.').next().unwrap_or(table_tok).trim_matches('"').trim_end_matches(';').to_string();
  if table.is_empty() {
    return None;
  }
  Some(table)
}

/// Return the DML target table of the statement enclosing `offset`:
///   - `INSERT INTO <table> ...` -> `<table>`
///   - `UPDATE <table> ...`      -> `<table>`
///   - `DELETE FROM <table> ...` -> `<table>`
///
/// `<table>` may be schema-qualified; the bare name is returned. None
/// when no DML target keyword is found in the current statement.
fn dml_target_table(source: &str, offset: TextSize) -> Option<String> {
  let (slice, upper) = stmt_slice_upper(source, offset);
  let stmt = slice.as_str();
  let (anchor, kw_len) = if let Some(a) = upper.rfind("INSERT INTO") {
    (a, "INSERT INTO".len())
  } else if let Some(a) = upper.rfind("DELETE FROM") {
    (a, "DELETE FROM".len())
  } else if let Some(a) = upper.rfind("UPDATE ") {
    (a, "UPDATE".len())
  } else {
    return None;
  };
  let after = anchor + kw_len;
  let rest = &stmt[after..];
  let lead = rest.len() - rest.trim_start().len();
  let raw = &rest[lead..];
  let id_end = raw.find(|c: char| !c.is_ascii_alphanumeric() && c != '_' && c != '.' && c != '"').unwrap_or(raw.len());
  let table = raw[..id_end].rsplit('.').next().unwrap_or(&raw[..id_end]).trim_matches('"').to_string();
  if table.is_empty() {
    return None;
  }
  Some(table)
}

/// Walk back from `offset` to find `INSERT INTO <table> (` and return
/// the bare table name. None when the cursor isn't inside an INSERT
/// column list.
/// Just the target table identifier of the buffer's `INSERT INTO`,
/// without the cursor-inside-paren-list constraint. Used by EXCLUDED
/// virtual-alias resolution (cursor sits past VALUES (...) close paren).
fn insert_target_table_name_only(source: &str) -> Option<String> {
  let upper = source.to_ascii_uppercase();
  let at = upper.rfind("INSERT INTO")?;
  let after = at + "INSERT INTO".len();
  let rest = &source[after..];
  let lead = rest.len() - rest.trim_start().len();
  let raw = &rest[lead..];
  let id_end = raw.find(|c: char| !c.is_ascii_alphanumeric() && c != '_' && c != '.' && c != '"').unwrap_or(raw.len());
  let table = raw[..id_end].rsplit('.').next().unwrap_or(&raw[..id_end]).trim_matches('"').to_string();
  if table.is_empty() {
    return None;
  }
  Some(table)
}

fn insert_target_table(source: &str, offset: TextSize) -> Option<String> {
  let (slice, upper) = stmt_slice_upper(source, offset);
  let stmt = slice.as_str();
  let at = upper.rfind("INSERT INTO")?;
  let after = at + "INSERT INTO".len();
  let rest = &stmt[after..];
  let lead = rest.len() - rest.trim_start().len();
  let raw = &rest[lead..];
  let id_end = raw.find(|c: char| !c.is_ascii_alphanumeric() && c != '_' && c != '.' && c != '"').unwrap_or(raw.len());
  let table = raw[..id_end].rsplit('.').next().unwrap_or(&raw[..id_end]).trim_matches('"').to_string();
  if table.is_empty() {
    return None;
  }
  // Must be inside the paren list: the chars between `<table>` and
  // cursor must contain `(` and not a closing `)`.
  let after_table = after + lead + id_end;
  let trail = &stmt[after_table..];
  let paren_at = trail.find('(')?;
  let paren_body = &trail[paren_at + 1..];
  if paren_body.contains(')') {
    return None;
  }
  Some(table)
}

fn is_column_listed(it: &Item, used: &std::collections::HashSet<String>) -> bool {
  if !matches!(it.kind, crate::item::ItemKind::Column) {
    return false;
  }
  let tail = it.label.rsplit('.').next().unwrap_or(&it.label);
  used.contains(&tail.to_ascii_lowercase())
}

/// Walk back from `offset` to the nearest clause-start anchor
/// (`SELECT`, `SET`, `GROUP BY`, `ORDER BY`, `RETURNING`, `(`) and
/// collect every bare identifier (split by top-level `,`) seen
/// between the anchor and the cursor. Lowercased for case-insensitive
/// matching. Multi-byte safe: only treats bytes < 128 as ASCII.
fn used_columns_in_clause(source: &str, offset: TextSize) -> std::collections::HashSet<String> {
  let pos: usize = u32::from(offset) as usize;
  let mut pos = pos.min(source.len());
  while pos > 0 && !source.is_char_boundary(pos) {
    pos -= 1;
  }
  let bytes = source.as_bytes();
  let mut anchor = 0usize;
  let mut depth = 0i32;
  let mut i = pos;
  while i > 0 {
    let b = bytes[i - 1];
    // Non-ASCII: skip without inspecting (continuation byte / multi-
    // byte char). Word-boundary logic only cares about ASCII keywords.
    if b >= 128 {
      i -= 1;
      continue;
    }
    let c = b as char;
    if c == ')' {
      depth += 1;
      i -= 1;
      continue;
    }
    if c == '(' {
      if depth == 0 {
        anchor = i;
        break;
      }
      depth -= 1;
      i -= 1;
      continue;
    }
    if c == ';' {
      anchor = i;
      break;
    }
    let _ = c;
    if match_kw_at(bytes, i, b"SELECT")
      || match_kw_at(bytes, i, b"RETURNING")
      || match_kw_at(bytes, i, b"VALUES")
      || match_kw_at(bytes, i, b"SET")
      || match_kw_at(bytes, i, b"BY")
    {
      anchor = i;
      break;
    }
    i -= 1;
  }
  let region = if source.is_char_boundary(anchor) && source.is_char_boundary(pos) { &source[anchor..pos] } else { "" };
  collect_idents_csv(region)
}

/// True when `bytes[end - kw.len()..end]` (case-insensitive) equals
/// `kw` AND the byte before AND after are non-word chars (word-
/// boundary on both sides). Skips bounds checks cleanly when
/// end < kw.len().
fn match_kw_at(bytes: &[u8], end: usize, kw: &[u8]) -> bool {
  let len = kw.len();
  if end < len {
    return false;
  }
  let start = end - len;
  for k in 0..len {
    let a = bytes[start + k];
    let b = kw[k];
    if !a.is_ascii() {
      return false;
    }
    if a.to_ascii_uppercase() != b {
      return false;
    }
  }
  let prev_ok = if start == 0 {
    true
  } else {
    let prev = bytes[start - 1];
    prev >= 128 || !is_word_char(prev as char)
  };
  let next_ok = if end >= bytes.len() {
    true
  } else {
    let next = bytes[end];
    next >= 128 || !is_word_char(next as char)
  };
  prev_ok && next_ok
}

fn is_word_char(c: char) -> bool {
  c.is_ascii_alphanumeric() || c == '_'
}

/// Split `region` on top-level `,` (paren-depth aware), pull the
/// first identifier off each item, lowercase + collect.
fn collect_idents_csv(region: &str) -> std::collections::HashSet<String> {
  let mut out = std::collections::HashSet::new();
  let bytes = region.as_bytes();
  let n = bytes.len();
  let mut depth = 0i32;
  let mut item_start = 0usize;
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
    if c == '(' {
      depth += 1;
      i += 1;
      continue;
    }
    if c == ')' {
      depth -= 1;
      i += 1;
      continue;
    }
    if c == ',' && depth == 0 {
      push_first_ident(&region[item_start..i], &mut out);
      item_start = i + 1;
    }
    i += 1;
  }
  push_first_ident(&region[item_start..], &mut out);
  out
}

fn push_first_ident(item: &str, out: &mut std::collections::HashSet<String>) {
  let item = item.trim();
  if item.is_empty() {
    return;
  }
  // Take the trailing dotted segment (`u.email` -> `email`) so we
  // don't match aliases. Strip everything after the first
  // non-word/dot char (`u.email = ...` -> `u.email`).
  let head_end =
    item.find(|c: char| !c.is_ascii_alphanumeric() && c != '_' && c != '.' && c != '"').unwrap_or(item.len());
  let head = item[..head_end].trim_matches('"');
  if head.is_empty() {
    return;
  }
  let tail = head.rsplit('.').next().unwrap_or(head).trim_matches('"');
  if !tail.is_empty() {
    out.insert(tail.to_ascii_lowercase());
  }
}

/// Walk back from `pos` to find the enclosing `CREATE [OR REPLACE]
/// FUNCTION <name>(...)` header. Then search the buffer + the live
/// catalog's triggers for a CREATE TRIGGER bound to `<name>` and
/// return its target table. None when the function is never used by a
/// trigger we can see -- the caller should suppress completion in
/// that case so the user doesn't get bogus suggestions.
fn enclosing_fn_trigger_table(source: &str, pos: usize, cat: &Catalog) -> Option<String> {
  let upper = source.to_ascii_uppercase();
  let mut latest: Option<usize> = None;
  for needle in ["CREATE OR REPLACE FUNCTION ", "CREATE FUNCTION ", "CREATE OR REPLACE PROCEDURE ", "CREATE PROCEDURE "]
  {
    let mut from = 0usize;
    while let Some(rel) = upper[from..].find(needle) {
      let p = from + rel;
      if p > pos {
        break;
      }
      latest = Some(p + needle.len());
      from = p + needle.len();
    }
  }
  let after = source[latest?..].trim_start();
  let fn_name: String = after.chars().take_while(|c| c.is_alphanumeric() || *c == '_' || *c == '.').collect();
  let fn_short = fn_name.rsplit('.').next().unwrap_or(&fn_name);
  if fn_short.is_empty() {
    return None;
  }

  // (a) Search buffer for `CREATE TRIGGER ... ON <table> ... EXECUTE
  // [FUNCTION|PROCEDURE] <fn_short>`. Multiple triggers can target
  // the same function but typically all bind to the same table.
  if let Some(t) = scan_buffer_for_trigger_fn(source, fn_short) {
    return Some(t);
  }
  // (b) Live catalog. dsl-catalog tracks triggers per table; locate
  // the table whose triggers reference this function.
  for t in cat.tables() {
    for tg in &t.triggers {
      // `function` field stores the raw action_statement, which
      // usually reads "EXECUTE FUNCTION schema.fn_name()". We
      // tolerate either form.
      let fn_upper = tg.function.to_ascii_uppercase();
      let needle = fn_short.to_ascii_uppercase();
      if fn_upper.contains(&needle) {
        return Some(t.name.clone());
      }
    }
  }
  None
}

/// Find the first `CREATE TRIGGER ... ON <table> ... EXECUTE [FUNCTION|
/// PROCEDURE] <fn>` referencing `fn_name` in the source. Returns the
/// table name (sans schema prefix).
fn scan_buffer_for_trigger_fn(source: &str, fn_name: &str) -> Option<String> {
  let upper = source.to_ascii_uppercase();
  let mut from = 0usize;
  while let Some(rel) = upper[from..].find("CREATE TRIGGER") {
    let p = from + rel;
    // Find the end of this statement (next `;` or buffer end).
    let stmt_end = upper[p..].find(';').map(|e| p + e).unwrap_or(upper.len());
    let stmt_upper = &upper[p..stmt_end];
    // Must reference our function.
    let fn_upper = fn_name.to_ascii_uppercase();
    let mentions = stmt_upper.contains(&format!("EXECUTE FUNCTION {}", fn_upper))
      || stmt_upper.contains(&format!("EXECUTE PROCEDURE {}", fn_upper))
      || stmt_upper.contains(&format!("EXECUTE FUNCTION PUBLIC.{}", fn_upper))
      || stmt_upper.contains(&format!("EXECUTE PROCEDURE PUBLIC.{}", fn_upper));
    if mentions && let Some(on_pos) = stmt_upper.find(" ON ") {
      let after = &source[p + on_pos + 4..stmt_end];
      let tok: String =
        after.trim_start().chars().take_while(|c| c.is_alphanumeric() || *c == '_' || *c == '.').collect();
      if !tok.is_empty() {
        return Some(tok.rsplit('.').next().unwrap_or(&tok).to_string());
      }
    }
    from = stmt_end.max(p + 1);
  }
  None
}

/// Find the table name in a `CREATE TRIGGER ... ON <table>` clause in
/// the buffer. Used for NEW / OLD resolution.
fn trigger_target_table(source: &str) -> Option<String> {
  let upper = source.to_uppercase();
  // Accept every CREATE TRIGGER variant: bare, `CREATE OR REPLACE
  // TRIGGER`, and `CREATE CONSTRAINT TRIGGER`. We anchor on the
  // earliest match so a script with multiple triggers still picks the
  // one we're cursoring in (caller passes the slice up to cursor).
  let idx = ["CREATE OR REPLACE TRIGGER", "CREATE CONSTRAINT TRIGGER", "CREATE TRIGGER"]
    .iter()
    .filter_map(|kw| upper.rfind(kw))
    .max()?;
  let rest_upper = &upper[idx..];
  let on_idx = rest_upper.find(" ON ")?;
  let after = &source[idx + on_idx + 4..];
  let tok = after
    .trim_start()
    .split(|c: char| c.is_whitespace() || c == '(' || c == ';' || c == ',')
    .find(|s| !s.is_empty())?;
  // Strip optional `ONLY` keyword introducing the table name.
  let tok = if tok.eq_ignore_ascii_case("ONLY") {
    after
      .trim_start()
      .split_ascii_whitespace()
      .nth(1)
      .unwrap_or(tok)
  } else {
    tok
  };
  // Strip schema prefix + surrounding quotes.
  Some(tok.split('.').next_back().unwrap_or(tok).trim_matches('"').to_string())
}

/// Quick dot detection: returns the alias before the cursor's `.`.
fn dot_alias(source: &str, offset: TextSize) -> Option<String> {
  let pos: usize = offset.into();
  let pos = floor_char_boundary(source, pos.min(source.len()));
  let before = &source[..pos];
  let dot_idx = before.rfind('.')?;
  let after_dot = &before[dot_idx + 1..];
  if !after_dot.chars().all(|c| c.is_alphanumeric() || c == '_') {
    return None;
  }
  let pre_dot = &before[..dot_idx];
  // Double-quoted identifier alias: `"Foo Bar".col` -- walk back to the
  // matching opening `"` and return the content (without quotes).
  // Bindings are stored unquoted by the resolver so this matches.
  if pre_dot.ends_with('"') {
    let body_end = pre_dot.len() - 1;
    if let Some(open) = pre_dot[..body_end].rfind('"') {
      let inner = &pre_dot[open + 1..body_end];
      if !inner.is_empty() {
        return Some(inner.to_string());
      }
    }
  }
  let alias: String =
    pre_dot.chars().rev().take_while(|c| c.is_alphanumeric() || *c == '_').collect::<String>().chars().rev().collect();
  if alias.is_empty() { None } else { Some(alias) }
}

/// Return the largest valid UTF-8 char boundary at or before `byte`.
/// Mirrors the (unstable) `str::floor_char_boundary` API.
fn floor_char_boundary(s: &str, byte: usize) -> usize {
  let mut b = byte.min(s.len());
  while b > 0 && !s.is_char_boundary(b) {
    b -= 1;
  }
  b
}

/// True when the cursor at `offset` sits inside a SQL string literal,
/// line comment (`-- ...`), or block comment (`/* ... */`). Dollar-quoted
/// bodies (`$tag$...$tag$`) are NOT treated as inert -- they hold
/// PL/pgSQL code and completion remains useful there. When the cursor
/// lands inside a dollar-quote body the scan recurses with the body as a
/// fresh source so an inner `'literal'` still suppresses completion.
fn cursor_in_inert_span(source: &str, offset: usize) -> bool {
  let bytes = source.as_bytes();
  let n = bytes.len();
  let limit = offset.min(n);
  let mut i = 0usize;
  // 0 = code, 1 = single-quoted string, 2 = line comment, 3 = block comment.
  let mut state: u8 = 0;
  while i < limit {
    match state {
      0 => {
        // Dollar-quoted string `$tag$ ... $tag$` (tag may be empty).
        if bytes[i] == b'$' {
          let mut t = i + 1;
          while t < n && (bytes[t].is_ascii_alphanumeric() || bytes[t] == b'_') {
            t += 1;
          }
          if t < n && bytes[t] == b'$' {
            let tag_end = t + 1;
            let tag = &bytes[i..tag_end];
            let mut k = tag_end;
            let mut close = None;
            while k + tag.len() <= n {
              if &bytes[k..k + tag.len()] == tag {
                close = Some(k);
                break;
              }
              k += 1;
            }
            let body_close_end = close.map(|p| p + tag.len()).unwrap_or(n);
            // Cursor inside the body? Treat body as fresh source.
            if offset > tag_end && offset <= close.unwrap_or(n) {
              let body_off = offset - tag_end;
              let body_src = &source[tag_end..close.unwrap_or(n)];
              return cursor_in_inert_span(body_src, body_off);
            }
            i = body_close_end;
            continue;
          }
        }
        match bytes[i] {
          b'\'' => {
            state = 1;
            i += 1;
            continue;
          },
          b'-' if i + 1 < n && bytes[i + 1] == b'-' => {
            state = 2;
            i += 2;
            continue;
          },
          b'/' if i + 1 < n && bytes[i + 1] == b'*' => {
            state = 3;
            i += 2;
            continue;
          },
          _ => {
            i += 1;
          },
        }
      },
      1 => {
        if bytes[i] == b'\'' {
          // Doubled '' is an escape.
          if i + 1 < n && bytes[i + 1] == b'\'' {
            i += 2;
            continue;
          }
          state = 0;
          i += 1;
          continue;
        }
        i += 1;
      },
      2 => {
        if bytes[i] == b'\n' {
          state = 0;
        }
        i += 1;
      },
      3 => {
        if bytes[i] == b'*' && i + 1 < n && bytes[i + 1] == b'/' {
          state = 0;
          i += 2;
          continue;
        }
        i += 1;
      },
      _ => break,
    }
  }
  state != 0
}

/// True when the cursor is at a "fresh name" slot -- i.e. immediately
/// after a `CREATE [OR REPLACE] <KIND> [IF NOT EXISTS]` keyword (or
/// while typing the name itself). SQL DDL invents these names, so the
/// LSP should offer NO completion candidates -- existing catalog
/// names would be wrong (collision) and keyword suggestions would
/// also be wrong (the user is mid-identifier).
/// Return the single optional clarifier keyword that legally fits in
/// a fresh-name slot, when it does. Used by the early-return guard so
/// `CREATE TABLE <cursor>` still emits `IF NOT EXISTS` instead of an
/// empty menu. None for slots where no optional keyword applies.
fn fresh_name_slot_optional_keyword(source: &str, offset: TextSize) -> Option<&'static str> {
  let (slice_owned, _) = stmt_slice_upper(source, offset);
  let before = slice_owned.as_str();
  let trimmed = before.trim_end_matches(|c: char| c.is_whitespace());
  // Only fire when no name partial has been typed yet -- the trimmed
  // text ends right after the class keyword.
  if before.len() == trimmed.len() {
    return None; // no trailing whitespace -> user mid-typing the name
  }
  let upper = trimmed.to_ascii_uppercase();
  let supports_inex: &[&str] = &[
    "CREATE TABLE",
    "CREATE TEMP TABLE",
    "CREATE TEMPORARY TABLE",
    "CREATE UNLOGGED TABLE",
    "CREATE GLOBAL TABLE",
    "CREATE LOCAL TABLE",
    "CREATE VIEW",
    "CREATE MATERIALIZED VIEW",
    "CREATE INDEX",
    "CREATE UNIQUE INDEX",
    "CREATE SCHEMA",
    "CREATE SEQUENCE",
    "CREATE TYPE",
    "CREATE DOMAIN",
    "CREATE TRIGGER",
    "CREATE POLICY",
    "CREATE EXTENSION",
    "CREATE FOREIGN TABLE",
    "CREATE PROCEDURE",
    "CREATE FUNCTION",
    "CREATE OR REPLACE TRIGGER",
    "CREATE OR REPLACE FUNCTION",
    "CREATE OR REPLACE PROCEDURE",
    "CREATE OR REPLACE VIEW",
  ];
  if supports_inex.iter().any(|p| upper.ends_with(p)) {
    return Some("IF NOT EXISTS");
  }
  None
}

fn at_fresh_name_slot(source: &str, offset: TextSize) -> bool {
  let (slice_owned, upper) = stmt_slice_upper(source, offset);
  let _before = slice_owned.as_str();
  // Trim leading whitespace + skip the partial identifier the user
  // is currently typing (the "fresh name" itself).
  let bytes = upper.as_bytes();
  let n = bytes.len();
  let mut end = n;
  // Skip the identifier being typed (alphanumeric + underscore).
  while end > 0 && (bytes[end - 1].is_ascii_alphanumeric() || bytes[end - 1] == b'_') {
    end -= 1;
  }
  let lead = &upper[..end].trim_end();
  // Allow optional `IF NOT EXISTS` before the name.
  let lead = lead.strip_suffix("IF NOT EXISTS").unwrap_or(lead).trim_end();
  // Patterns that announce a fresh DDL object name:
  const PATTERNS: &[&str] = &[
    "CREATE TABLE",
    "CREATE TEMP TABLE",
    "CREATE TEMPORARY TABLE",
    "CREATE UNLOGGED TABLE",
    "CREATE GLOBAL TABLE",
    "CREATE LOCAL TABLE",
    "CREATE VIEW",
    "CREATE OR REPLACE VIEW",
    "CREATE MATERIALIZED VIEW",
    "CREATE FUNCTION",
    "CREATE OR REPLACE FUNCTION",
    "CREATE PROCEDURE",
    "CREATE OR REPLACE PROCEDURE",
    "CREATE INDEX",
    "CREATE UNIQUE INDEX",
    "CREATE SCHEMA",
    "CREATE SEQUENCE",
    "CREATE TYPE",
    "CREATE DOMAIN",
    "CREATE TRIGGER",
    "CREATE OR REPLACE TRIGGER",
    "CREATE POLICY",
    "CREATE EXTENSION",
    "CREATE ROLE",
    "CREATE USER",
    "CREATE GROUP",
    "CREATE DATABASE",
    "CREATE TABLESPACE",
    "CREATE LANGUAGE",
    "CREATE AGGREGATE",
    "CREATE CAST",
    "CREATE COLLATION",
    "CREATE CONVERSION",
    "CREATE OPERATOR",
    "CREATE RULE",
    "CREATE PUBLICATION",
    "CREATE SUBSCRIPTION",
    "CREATE FOREIGN TABLE",
    "CREATE FOREIGN DATA WRAPPER",
    "CREATE SERVER",
    "CREATE USER MAPPING",
    "CREATE TEXT SEARCH CONFIGURATION",
    "CREATE TEXT SEARCH DICTIONARY",
    "CREATE TEXT SEARCH PARSER",
    "CREATE TEXT SEARCH TEMPLATE",
    "CONSTRAINT",
    // ALTER TABLE sub-actions that take a fresh name. The user is
    // inventing a brand-new identifier; no catalog name and no SQL
    // keyword is a useful suggestion in this slot.
    "ADD COLUMN",
    "ADD CONSTRAINT",
    "RENAME TO",
    "RENAME CONSTRAINT",
    // Single-token-followup commands that take a fresh identifier
    // (LISTEN/NOTIFY channel name, PREPARE/DEALLOCATE statement name,
    // DECLARE cursor name). No catalog completion makes sense.
    "LISTEN",
    "UNLISTEN",
    "NOTIFY",
    "PREPARE",
    "DEALLOCATE",
    "DECLARE",
    "IMPORT FOREIGN SCHEMA",
    "CHECKPOINT",
    // Cursor-management commands (PL/pgSQL OPEN/FETCH/MOVE/CLOSE):
    // cursor names are user-defined; no catalog target to suggest.
    "FETCH",
    "MOVE",
    "CLOSE",
    "OPEN",
    // CREATE EVENT TRIGGER <name> -- fresh trigger name.
    "CREATE EVENT TRIGGER",
    // CREATE INDEX [UNIQUE] [CONCURRENTLY] [IF NOT EXISTS] <name>
    "CREATE INDEX CONCURRENTLY",
    "CREATE UNIQUE INDEX CONCURRENTLY",
    // `WITH RECURSIVE <name>` -- fresh CTE name.
    "WITH RECURSIVE",
    // (SHOW <guc> is handled by `show_or_set_guc_names` which emits the
    // common GUC name set; no entry here so that path can fire.)
    // `USE <name>` -- not PG syntax but users type it; stay quiet.
    "USE",
    // Top-level `VALUES (...)` -- the next required char is `(`.
    // Stay quiet rather than dumping the 641-item catch-all.
    "VALUES",
    // RENAME COLUMN's *destination* name (`RENAME COLUMN old TO new`)
    // -- the existing-column slot is handled separately by
    // alter_table_existing_column_target().
  ];
  if PATTERNS.iter().any(|p| lead.ends_with(p)) {
    return true;
  }
  // ALTER TABLE ... RENAME COLUMN <name> TO <cursor> -- fresh name
  // slot. The column name is variable so we can't match a fixed
  // suffix; instead check that the last 4 word-tokens are
  // `RENAME COLUMN <name> TO`.
  let tokens: Vec<&str> = lead.split_ascii_whitespace().collect();
  let n = tokens.len();
  if n >= 4
    && tokens[n - 4] == "RENAME"
    && tokens[n - 3] == "COLUMN"
    && tokens[n - 1] == "TO"
  {
    return true;
  }
  false
}

/// Drop later items whose (label, kind) collide with an earlier item.
/// Keeps the first occurrence (which is also the highest-priority emit
/// per call ordering). Matched case-insensitively because PG keyword
/// completion may be cased differently across sources.
/// Merge entries with the same `(label, kind)`. The first occurrence
/// wins for `insert_text` / `documentation_md`, but later occurrences
/// can contribute origin info into the `detail` field so a column that
/// lives in multiple tables surfaces as `id (users, orders, ...)`
/// instead of silently hiding the second binding.
fn dedup_items(items: Vec<Item>) -> Vec<Item> {
  use crate::item::ItemKind;
  use std::collections::HashMap;
  let mut idx: HashMap<(String, ItemKind), usize> = HashMap::new();
  let mut out: Vec<Item> = Vec::with_capacity(items.len());
  for it in items {
    let key = (it.label.to_ascii_lowercase(), it.kind);
    if let Some(&existing) = idx.get(&key) {
      // Merge origin info into detail.
      if let (Some(existing_detail), Some(new_detail)) = (out[existing].detail.as_ref(), it.detail.as_ref()) {
        if existing_detail != new_detail && !existing_detail.contains(new_detail.as_str()) {
          let merged = format!("{existing_detail}, {new_detail}");
          out[existing].detail = Some(merged);
        }
      } else if out[existing].detail.is_none() && it.detail.is_some() {
        out[existing].detail = it.detail;
      }
      // Pick the lower (= higher-priority) sort, so a column that
      // is in-scope (priority 0) wins over a catalog-wide bare
      // version (priority 3) regardless of insertion order.
      if it.sort_priority < out[existing].sort_priority {
        out[existing].sort_priority = it.sort_priority;
      }
      continue;
    }
    idx.insert(key, out.len());
    out.push(it);
  }
  out
}

/// Push every function the LSP knows about -- built-in PG functions
/// from the knowledge base plus user-defined functions from the live
/// catalog. Both stamped with `sort_priority = 3` so they sit below
/// in-scope columns (0) but above generic keywords.
fn push_all_functions(cat: &Catalog, out: &mut Vec<Item>) {
  let start = out.len();
  sources::functions(out);
  sources::db_functions(cat, out);
  for it in &mut out[start..] {
    it.sort_priority = 3;
  }
}

/// Emit the FROM/JOIN aliases declared in the statement enclosing
/// `offset`. Priority 1 so they appear right under in-scope columns.
/// Falls back to a raw-text scan when the parser produced no useful
/// scope (common while the user is still typing the SELECT projection).
/// Emit the CTE names declared by the statement enclosing `offset` as
/// Table-kind candidates. Used in FROM / JOIN slots so a CTE name is
/// a first-class completion target alongside catalog tables. Falls
/// back to a text-level scan when the parser failed (the common case
/// while the user is still typing the outer FROM).
fn push_cte_names(file: &ParsedFile, scopes: &[Scope], source: &str, offset: TextSize, out: &mut Vec<Item>) {
  let mut names: Vec<String> = Vec::new();
  if let Some(scope) = scope_for_offset(file, scopes, offset) {
    for name in scope.cte_columns.keys() {
      if !name.is_empty() {
        names.push(name.clone());
      }
    }
  }
  if names.is_empty() {
    names = fallback::cte_names_from_text(source);
  }
  for name in names {
    if name.is_empty() {
      continue;
    }
    out.push(crate::item::Item {
      label: name.clone(),
      kind: crate::item::ItemKind::Table,
      detail: Some("CTE".into()),
      description: None,
      documentation_md: Some(format!("**CTE** `{name}` (defined by WITH in this statement)\n")),
      insert_text: name.clone(),
      is_snippet: false,
      sort_priority: 0,
    });
  }
}

fn push_aliases(file: &ParsedFile, scopes: &[Scope], source: &str, offset: TextSize, out: &mut Vec<Item>) {
  let start = out.len();
  if let Some(scope) = scope_for_offset(file, scopes, offset) {
    sources::aliases_in_scope(scope, out);
  }
  if out.len() == start
    && let Some(scope) = fallback::scope_from_text(source)
  {
    sources::aliases_in_scope(&scope, out);
  }
}

fn push_scope_columns_or_all(
  file: &ParsedFile,
  scopes: &[Scope],
  source: &str,
  cat: &Catalog,
  offset: TextSize,
  out: &mut Vec<Item>,
) {
  let start = out.len();
  // Whether ANY in-scope binding contributed real catalog columns.
  // Synthetic bindings (CTEs, subquery aliases, function-call FROM)
  // don't count -- when the only scope is `t AS (SELECT ...)`, the
  // resolver maps `t` to a name that's not in the catalog, so
  // push_scope_columns emits nothing and we still need the text
  // fallback to surface any real `FROM users` columns in the body.
  let mut had_scope = false;
  if let Some(scope) = scope_for_offset(file, scopes, offset) {
    if scope_has_catalog_binding(scope, cat) {
      had_scope = true;
    }
    push_scope_columns(scope, cat, out);
  }
  if !had_scope && let Some(fb) = fallback::scope_from_text(source) {
    had_scope = scope_has_catalog_binding(&fb, cat);
    push_scope_columns(&fb, cat, out);
  }
  // Promote in-scope column items to the top of the menu. They beat
  // catalog-wide columns + functions + keywords on sort.
  for it in &mut out[start..] {
    it.sort_priority = 0;
  }
  if !had_scope {
    // No FROM resolved yet -- emit every (table, column) pair plus
    // the table names themselves so the user can browse the catalog
    // without first picking a table.
    let fb_start = out.len();
    sources::columns_all(cat, out);
    sources::tables(cat, out);
    for it in &mut out[fb_start..] {
      it.sort_priority = 4;
    }
  }
}

/// True when at least one in-scope binding maps to a real catalog
/// table. CTE / subquery / function-call FROM bindings don't count;
/// they're useful for qualified column lookups but contribute zero
/// bare columns to the projection menu.
fn scope_has_catalog_binding(scope: &Scope, cat: &Catalog) -> bool {
  scope.tables().any(|b| cat.find_table(b.table.schema.as_deref(), &b.table.name).is_some())
}

fn push_scope_columns(scope: &Scope, cat: &Catalog, out: &mut Vec<Item>) {
  // Dedup by table name + skip aliased tables: when the user wrote
  // `FROM users AS u`, hide the bare column completion so they're
  // forced to type `u.id` (which the dot-context resolver then
  // expands). This matches the DX users expect from JetBrains /
  // VSCode SQL tools and keeps the menu honest about which columns
  // are actually reachable without qualification.
  use std::collections::HashSet;
  let mut seen: HashSet<String> = HashSet::new();
  for b in scope.tables() {
    if b.alias != b.table.name {
      continue;
    }
    if !seen.insert(b.table.name.to_ascii_lowercase()) {
      continue;
    }
    let Some(t) = cat.find_table(b.table.schema.as_deref(), &b.table.name) else {
      continue;
    };
    for c in &t.columns {
      out.push(sources::column_item(t, c));
    }
  }
}

fn scope_for_offset<'a>(file: &ParsedFile, scopes: &'a [Scope], offset: TextSize) -> Option<&'a Scope> {
  let idx = file.statements.iter().position(|s| s.range.contains_inclusive(offset))?;
  scopes.get(idx)
}

/// When the cursor sits inside the literal of `<expr>->'<cursor>` or
/// `<expr>->>'<cursor>`, return the JSON keys observed in same-buffer
/// jsonb literals (`'{"key":...}'`) so the user can autocomplete
/// instead of guessing. Handles chained paths: `col->'a'->'b'->'<cursor>'`
/// walks into nested objects of the harvested literals and surfaces
/// only the keys present at depth a.b. Returns None outside this
/// context.
fn json_path_keys_at(source: &str, offset: TextSize) -> Option<Vec<String>> {
  let pos: usize = u32::from(offset) as usize;
  let bytes = source.as_bytes();
  let n = bytes.len().min(source.len());
  if pos > n {
    return None;
  }
  // Walk back to the opening `'` of the string the cursor is in.
  let mut s = pos;
  while s > 0 && bytes[s - 1] != b'\'' {
    s -= 1;
  }
  if s == 0 || bytes[s - 1] != b'\'' {
    return None;
  }
  // The string must be preceded by `->` or `->>`. Skip whitespace.
  let mut k = s - 1; // points at the `'`
  while k > 0 && bytes[k - 1].is_ascii_whitespace() {
    k -= 1;
  }
  if k < 2 {
    return None;
  }
  // `->>` form: bytes[k-3]='-', bytes[k-2]='>', bytes[k-1]='>'.
  // `->`  form: bytes[k-2]='-', bytes[k-1]='>'.
  let has_double = k >= 3 && bytes[k - 1] == b'>' && bytes[k - 2] == b'>' && bytes[k - 3] == b'-';
  let has_single = bytes[k - 1] == b'>' && bytes[k - 2] == b'-';
  if !has_double && !has_single {
    return None;
  }
  // Walk further back to harvest the chain of preceding `->'KEY'`
  // segments so we know what depth to look up in the JSON blobs.
  let chain_end = if has_double { k - 3 } else { k - 2 };
  let chain = collect_json_path_chain(source, chain_end);

  // Harvest jsonb keys from same-buffer literals -- at the requested depth.
  let mut keys: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
  let mut i = 0;
  while i < n {
    if bytes[i] == b'\'' && i + 1 < n && (bytes[i + 1] == b'{' || bytes[i + 1] == b'[') {
      let lit_start = i + 1;
      let mut j = lit_start;
      while j < n && bytes[j] != b'\'' {
        j += 1;
      }
      if j < n {
        let blob = &source[lit_start..j];
        let nested = navigate_json(blob, &chain).unwrap_or(blob);
        harvest_json_keys(nested, &mut keys);
        i = j + 1;
        continue;
      }
    }
    i += 1;
  }
  if keys.is_empty() {
    return None;
  }
  Some(keys.into_iter().collect())
}

/// Like [`json_path_keys_at`] but also consults `Column.json_keys` from
/// the catalog. When the cursor sits on a `col->'...'` chain whose head
/// resolves to a known jsonb column with stored top-level keys, surface
/// those even when the buffer has no example literal to harvest.
pub fn json_path_keys_at_with_catalog(
  source: &str,
  offset: TextSize,
  catalog: &dsl_catalog::Catalog,
) -> Option<Vec<String>> {
  if let Some(keys) = json_path_keys_at(source, offset) {
    return Some(keys);
  }
  // Walk back to the column name at the head of the `->'k'` chain.
  let pos: usize = u32::from(offset) as usize;
  let bytes = source.as_bytes();
  if pos > bytes.len() {
    return None;
  }
  let mut s = pos;
  while s > 0 && bytes[s - 1] != b'\'' {
    s -= 1
  }
  if s == 0 {
    return None;
  }
  let mut k = s - 1;
  while k > 0 && bytes[k - 1].is_ascii_whitespace() {
    k -= 1
  }
  let has_double = k >= 3 && bytes[k - 1] == b'>' && bytes[k - 2] == b'>' && bytes[k - 3] == b'-';
  let has_single = bytes[k - 1] == b'>' && bytes[k - 2] == b'-';
  if !has_double && !has_single {
    return None;
  }
  let arrow_at = if has_double { k - 3 } else { k - 2 };
  // Identifier just before the first arrow.
  let mut j = arrow_at;
  while j > 0 && bytes[j - 1].is_ascii_whitespace() {
    j -= 1
  }
  let id_end = j;
  while j > 0
    && (bytes[j - 1].is_ascii_alphanumeric() || bytes[j - 1] == b'_' || bytes[j - 1] == b'.' || bytes[j - 1] == b'"')
  {
    j -= 1;
  }
  if j == id_end {
    return None;
  }
  let col_full = &source[j..id_end];
  let col_bare = col_full.rsplit('.').next().unwrap_or(col_full).trim_matches('"');
  for t in catalog.tables() {
    if let Some(c) = t.columns.iter().find(|c| c.name.eq_ignore_ascii_case(col_bare))
      && let Some(keys) = &c.json_keys
      && !keys.is_empty()
    {
      return Some(keys.clone());
    }
  }
  None
}

/// Walk backwards from `end` collecting `->'KEY'` (or `->>'KEY'`)
/// segments in left-to-right order. Stops at the first non-segment
/// token. Whitespace between segments is tolerated.
fn collect_json_path_chain(source: &str, end: usize) -> Vec<String> {
  let bytes = source.as_bytes();
  let mut chain: Vec<String> = Vec::new();
  let mut k = end;
  loop {
    // Skip trailing whitespace.
    while k > 0 && bytes[k - 1].is_ascii_whitespace() {
      k -= 1;
    }
    // Need a closing `'`.
    if k == 0 || bytes[k - 1] != b'\'' {
      break;
    }
    let close = k - 1;
    // Walk back to the opening `'`.
    let mut open = close;
    while open > 0 && bytes[open - 1] != b'\'' {
      open -= 1;
    }
    if open == 0 {
      break;
    }
    let key = &source[open..close];
    let preceded_by_arrow = {
      let mut p = open.saturating_sub(1); // points at the opening `'`
      while p > 0 && bytes[p - 1].is_ascii_whitespace() {
        p -= 1;
      }
      let double = p >= 3 && bytes[p - 1] == b'>' && bytes[p - 2] == b'>' && bytes[p - 3] == b'-';
      let single = p >= 2 && bytes[p - 1] == b'>' && bytes[p - 2] == b'-';
      if double || single { Some(if double { p - 3 } else { p - 2 }) } else { None }
    };
    if let Some(next_end) = preceded_by_arrow {
      chain.push(key.to_string());
      k = next_end;
    } else {
      break;
    }
  }
  chain.reverse();
  chain
}

/// Given a JSON object literal (no surrounding quotes), navigate down
/// the `keys` path and return a sub-blob that the caller can re-scan
/// for keys. Returns None when the path doesn't resolve.
fn navigate_json<'a>(blob: &'a str, keys: &[String]) -> Option<&'a str> {
  if keys.is_empty() {
    return Some(blob);
  }
  let mut current = blob;
  for key in keys {
    current = find_value_for_key(current, key)?;
  }
  Some(current)
}

/// Locate `"key":` in `blob` and return the slice starting at the
/// value's first byte and ending at the matching close (`}` for
/// objects, `]` for arrays, or the next top-level `,`).
fn find_value_for_key<'a>(blob: &'a str, key: &str) -> Option<&'a str> {
  let needle = format!("\"{key}\"");
  let bytes = blob.as_bytes();
  let n = bytes.len();
  let mut i = 0;
  while i + needle.len() <= n {
    if blob[i..i + needle.len()] == needle {
      let mut j = i + needle.len();
      while j < n && bytes[j].is_ascii_whitespace() {
        j += 1;
      }
      if j >= n || bytes[j] != b':' {
        i += 1;
        continue;
      }
      j += 1;
      while j < n && bytes[j].is_ascii_whitespace() {
        j += 1;
      }
      if j >= n {
        return None;
      }
      let value_start = j;
      let value_end = scan_value_end(bytes, value_start);
      return Some(&blob[value_start..value_end]);
    }
    i += 1;
  }
  None
}

fn scan_value_end(bytes: &[u8], start: usize) -> usize {
  let n = bytes.len();
  if start >= n {
    return n;
  }
  match bytes[start] {
    b'{' | b'[' => {
      let open = bytes[start];
      let close = if open == b'{' { b'}' } else { b']' };
      let mut depth = 1i32;
      let mut i = start + 1;
      while i < n && depth > 0 {
        match bytes[i] {
          b'"' => {
            i += 1;
            while i < n && bytes[i] != b'"' {
              if bytes[i] == b'\\' && i + 1 < n {
                i += 2;
              } else {
                i += 1;
              }
            }
          },
          c if c == open => depth += 1,
          c if c == close => depth -= 1,
          _ => {},
        }
        i += 1;
      }
      i.min(n)
    },
    b'"' => {
      let mut i = start + 1;
      while i < n && bytes[i] != b'"' {
        if bytes[i] == b'\\' && i + 1 < n {
          i += 2;
        } else {
          i += 1;
        }
      }
      (i + 1).min(n)
    },
    _ => {
      let mut i = start;
      while i < n && bytes[i] != b',' && bytes[i] != b'}' && bytes[i] != b']' {
        i += 1;
      }
      i
    },
  }
}

fn harvest_json_keys(blob: &str, out: &mut std::collections::BTreeSet<String>) {
  // Cheap key-scanner: find each `"<key>":` pair without a real JSON
  // parser. Good enough for completion -- if the blob is malformed,
  // we miss a key or two, no harm done.
  let b = blob.as_bytes();
  let n = b.len();
  let mut i = 0;
  while i < n {
    if b[i] == b'"' {
      let key_start = i + 1;
      let mut j = key_start;
      while j < n && b[j] != b'"' {
        j += 1;
      }
      if j >= n {
        return;
      }
      let key = &blob[key_start..j];
      // Look forward for `:`.
      let mut k = j + 1;
      while k < n && b[k].is_ascii_whitespace() {
        k += 1;
      }
      if k < n && b[k] == b':' && !key.is_empty() && key.chars().all(|c| c.is_alphanumeric() || c == '_' || c == '-') {
        out.insert(key.to_string());
      }
      i = j + 1;
      continue;
    }
    i += 1;
  }
}
