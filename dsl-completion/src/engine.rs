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

pub fn complete(source: &str, file: &ParsedFile, scopes: &[Scope], catalog: &Catalog, offset: TextSize) -> Vec<Item> {
  // Normalise offset to the nearest valid UTF-8 char boundary so
  // downstream slicing can't panic on multi-byte characters.
  let raw_off: usize = offset.into();
  let off = floor_char_boundary(source, raw_off.min(source.len()));
  let offset = TextSize::from(off as u32);

  // Hard-suppress completion when the cursor sits at the "fresh
  // name" slot after a `CREATE [OR REPLACE] <KIND>` keyword. The
  // user is naming a brand-new object; no existing catalog symbol or
  // keyword is a sensible suggestion there.
  if at_fresh_name_slot(source, offset) {
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
    }
    // Filter columns already used in the same clause -- even in dot
    // context, typing `SELECT u.id, u.|` should not re-offer `id`.
    let used = used_columns_in_clause(source, offset);
    if !used.is_empty() {
      out.retain(|it| !is_column_listed(it, &used));
    }
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
      // Just typed `*` or `AS alias`. Next legal tokens: FROM
      // (continue the query) or `,` (more projection). Emit only
      // the small after-projection keyword set.
      sources::after_projection_keywords(&mut out);
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
        return dedup_items(out);
      }
      // `SELECT ... <table> TABLESAMPLE <cursor>` -- sampling method
      // slot (BERNOULLI / SYSTEM). The generic AfterTable handler
      // would wrongly offer JOIN keywords.
      if tablesample_expects_method(source, offset) {
        for (kw, doc) in [
          ("BERNOULLI", "TABLESAMPLE BERNOULLI (<percent>) -- row-level uniform sample"),
          ("SYSTEM", "TABLESAMPLE SYSTEM (<percent>) -- page-level random sample"),
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
      push_scope_columns_or_all(file, scopes, source, cat, offset, &mut out);
      push_aliases(file, scopes, source, offset, &mut out);
      push_all_functions(cat, &mut out);
    },
    Phase::OrderByList => {
      push_scope_columns_or_all(file, scopes, source, cat, offset, &mut out);
      push_aliases(file, scopes, source, offset, &mut out);
      push_all_functions(cat, &mut out);
      sources::order_modifiers(&mut out);
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
      if on_conflict_expects_target_column(source, offset) {
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
      // INSERT / UPDATE / DELETE ... RETURNING <cursor> -- columns of
      // the DML target table (the one named in INTO / UPDATE / DELETE
      // FROM). Falls back to in-scope columns when the target can't
      // be pinned down. Filters out columns the user already listed.
      if let Some(target) = dml_target_table(source, offset) {
        sources::columns_of_table(cat, None, &target, &mut out);
      } else {
        push_scope_columns_or_all(file, scopes, source, cat, offset, &mut out);
      }
      let used = used_columns_in_clause(source, offset);
      if !used.is_empty() {
        out.retain(|it| !is_column_listed(it, &used));
      }
    },

    // CREATE TABLE sub-phases ---------------------------------------
    Phase::CtlExpectTableName => {
      // Fresh name; nothing useful to suggest.
    },
    Phase::CtlBodyStart => {
      // User could be starting a column declaration (no completion
      // for the name itself) or a table-level constraint line.
      sources::create_table_entry_starters(&mut out);
    },
    Phase::CtlExpectType => {
      sources::types_only(&mut out);
    },
    Phase::CtlExpectColumnConstraint => {
      // `... DEFAULT <cursor>` -- the slot is an expression context,
      // NOT a constraint slot. The full column-constraint menu
      // (NOT NULL / PRIMARY KEY / CHECK / ...) here is a category
      // mistake; emit the curated DEFAULT-expression menu instead.
      if ctl_column_constraint_after_default(source, offset) {
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
      } else if alter_column_after_set_subkeyword_expects_silence(source, offset) {
        // `ALTER COLUMN <name> SET STATISTICS|STORAGE|COMPRESSION
        // <cursor>` -- these slots take literals / specific tokens
        // that have no useful catalog completion. Stay silent rather
        // than dumping the action menu. SET DEFAULT also falls into
        // this branch (the DEFAULT expression is freeform; a wide
        // function dump would be more noise than help here).
        // Emit nothing.
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
      } else if let Some(kws) = alter_table_subaction_at(source, offset) {
        // `ALTER TABLE <t> ADD <cursor>` / `... DROP <cursor>` etc --
        // the user has already picked the top-level action; narrow
        // to the sub-keywords (COLUMN / CONSTRAINT / etc) instead
        // of re-listing the entire 18-item action menu.
        for (kw, doc) in kws {
          out.push(crate::item::Item {
            label: (*kw).into(),
            kind: crate::item::ItemKind::Keyword,
            detail: Some((*doc).into()),
            insert_text: (*kw).into(),
            sort_priority: 0,
            ..Default::default()
          });
        }
      } else {
        sources::alter_table_actions(&mut out);
      }
    },

    Phase::AfterGrantOrRevoke => {
      sources::grant_privileges(&mut out);
    },
    Phase::AfterGrantOn => {
      // Object class keywords (TABLE/SEQUENCE/FUNCTION/SCHEMA/...)
      // plus the actual catalog targets so the user can either pick
      // the explicit class keyword or jump straight to a name.
      sources::grant_object_classes(&mut out);
      sources::tables(cat, &mut out);
    },
    Phase::AfterGrantTo => {
      sources::grant_roles(cat, &mut out);
    },

    Phase::AfterCreate | Phase::AfterAlter | Phase::AfterDrop | Phase::Unknown => {
      // `DROP TABLE [IF EXISTS]` / `DROP VIEW [IF EXISTS]` /
      // `TRUNCATE [TABLE]` all expect a table-class name. Skip the
      // generic catch-all dump and emit only matching catalog targets.
      if vacuum_paren_expects_option(source, offset) {
        for (kw, doc) in [
          ("FULL", "FULL -- rewrite the table (locks it)"),
          ("FREEZE", "FREEZE -- mark tuples as committed eagerly"),
          ("VERBOSE", "VERBOSE -- per-relation progress"),
          ("ANALYZE", "ANALYZE -- update planner stats too"),
          ("SKIP_LOCKED", "SKIP_LOCKED -- don't wait for locks"),
          ("INDEX_CLEANUP", "INDEX_CLEANUP AUTO|ON|OFF"),
          ("PROCESS_TOAST", "PROCESS_TOAST [true|false]"),
          ("TRUNCATE", "TRUNCATE [true|false] -- shrink the table file"),
          ("DISABLE_PAGE_SKIPPING", "DISABLE_PAGE_SKIPPING [true|false]"),
          ("BUFFER_USAGE_LIMIT", "BUFFER_USAGE_LIMIT '<size>' -- ring-buffer cap"),
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
        // ...) instead of the catch-all dump.
        sources::statement_keywords(&mut out);
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
      } else if let Some(kws) = after_top_level_drop_keyword(source, offset) {
        // `DROP <cursor>` -- narrow to the object-type keywords PG
        // accepts after DROP. The catch-all fallback below would dump
        // 642 keywords/tables/columns which is useless here.
        for (kw, doc) in kws {
          out.push(crate::item::Item {
            label: (*kw).into(),
            kind: crate::item::ItemKind::Keyword,
            detail: Some((*doc).into()),
            insert_text: (*kw).into(),
            sort_priority: 0,
            ..Default::default()
          });
        }
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
  let pos: usize = u32::from(offset) as usize;
  let pos = pos.min(source.len());
  let stmt_start = source[..pos].rfind(';').map(|p| p + 1).unwrap_or(0);
  let stmt = &source[stmt_start..pos];
  let upper = stmt.to_ascii_uppercase();
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
  let pos: usize = u32::from(offset) as usize;
  let pos = pos.min(source.len());
  let stmt_start = source[..pos].rfind(';').map(|p| p + 1).unwrap_or(0);
  let stmt = &source[stmt_start..pos];
  let upper = stmt.trim_start().to_ascii_uppercase();
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
  let pos: usize = u32::from(offset) as usize;
  let pos = pos.min(source.len());
  let stmt_start = source[..pos].rfind(';').map(|p| p + 1).unwrap_or(0);
  let stmt = &source[stmt_start..pos];
  let upper = stmt.trim_start().to_ascii_uppercase();
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
  let last_is_not = {
    let upper = trimmed.to_ascii_uppercase();
    upper.ends_with(" IS NOT") || upper == "IS NOT"
  };
  if last_is_not {
    // After `IS NOT`: NULL / TRUE / FALSE / UNKNOWN / DISTINCT FROM.
    const IS_NOT: &[(&str, &str)] = &[
      ("NULL", "IS NOT NULL"),
      ("TRUE", "IS NOT TRUE"),
      ("FALSE", "IS NOT FALSE"),
      ("UNKNOWN", "IS NOT UNKNOWN"),
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
    ];
    return Some(IS);
  }
  None
}

/// True when the cursor sits at `DISCARD <cursor>` -- the next token
/// is one of ALL / PLANS / SEQUENCES / TEMP / TEMPORARY.
fn discard_expects_subkeyword(source: &str, offset: TextSize) -> bool {
  let pos: usize = u32::from(offset) as usize;
  let pos = pos.min(source.len());
  let stmt_start = source[..pos].rfind(';').map(|p| p + 1).unwrap_or(0);
  let stmt = &source[stmt_start..pos];
  let upper = stmt.trim_start().to_ascii_uppercase();
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
  let pos: usize = u32::from(offset) as usize;
  let pos = pos.min(source.len());
  let stmt_start = source[..pos].rfind(';').map(|p| p + 1).unwrap_or(0);
  let stmt = &source[stmt_start..pos];
  let upper = stmt.trim_start().to_ascii_uppercase();
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
  let pos: usize = u32::from(offset) as usize;
  let pos = pos.min(source.len());
  let stmt_start = source[..pos].rfind(';').map(|p| p + 1).unwrap_or(0);
  let stmt = &source[stmt_start..pos];
  let upper = stmt.trim_start().to_ascii_uppercase();
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
  let pos: usize = u32::from(offset) as usize;
  let pos = pos.min(source.len());
  let stmt_start = source[..pos].rfind(';').map(|p| p + 1).unwrap_or(0);
  let stmt = &source[stmt_start..pos];
  let upper = stmt.trim_start().to_ascii_uppercase();
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
  let pos: usize = u32::from(offset) as usize;
  let pos = pos.min(source.len());
  let stmt_start = source[..pos].rfind(';').map(|p| p + 1).unwrap_or(0);
  let stmt = &source[stmt_start..pos];
  let upper = stmt.trim_start().to_ascii_uppercase();
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
  let pos: usize = u32::from(offset) as usize;
  let pos = pos.min(source.len());
  let stmt_start = source[..pos].rfind(';').map(|p| p + 1).unwrap_or(0);
  let stmt = &source[stmt_start..pos];
  let upper = stmt.trim_start().to_ascii_uppercase();
  let kinds: &[(&[&str], &[&str])] = &[
    (&["ALTER", "ROLE"], &["IF", "EXISTS"]),
    (&["ALTER", "USER"], &["IF", "EXISTS"]),
    (&["ALTER", "GROUP"], &["IF", "EXISTS"]),
    (&["DROP", "ROLE"], &["IF", "EXISTS"]),
    (&["DROP", "USER"], &["IF", "EXISTS"]),
    (&["DROP", "GROUP"], &["IF", "EXISTS"]),
    (&["REASSIGN", "OWNED", "BY"], &[]),
    (&["DROP", "OWNED", "BY"], &[]),
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
  let pos: usize = u32::from(offset) as usize;
  let pos = pos.min(source.len());
  let stmt_start = source[..pos].rfind(';').map(|p| p + 1).unwrap_or(0);
  let stmt = &source[stmt_start..pos];
  let upper = stmt.trim_start().to_ascii_uppercase();
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
  const EMPTY: &[(&str, &str)] = &[];
  let words: Vec<&str> = upper.split_ascii_whitespace().collect();
  // `SET <cursor>` or `SET <partial-word-or-LOCAL/SESSION-or-name>`.
  if words.len() == 1 && words[0] == "SET" && stmt.ends_with(char::is_whitespace) {
    return Some(SET_MODS);
  }
  // `SET LOCAL ` / `SET SESSION ` -- GUC name slot.
  if words.len() == 2
    && words[0] == "SET"
    && (words[1] == "LOCAL" || words[1] == "SESSION")
    && stmt.ends_with(char::is_whitespace)
  {
    return Some(EMPTY);
  }
  None
}

/// Detect a transaction-control statement slot. Returns the keyword
/// list to emit (empty for COMMIT/ROLLBACK/END/ABORT/SAVEPOINT which
/// take no further token or a fresh identifier). None when the cursor
/// isn't in a recognised transaction-control slot.
fn transaction_control_completion(source: &str, offset: TextSize) -> Option<&'static [(&'static str, &'static str)]> {
  let pos: usize = u32::from(offset) as usize;
  let pos = pos.min(source.len());
  let stmt_start = source[..pos].rfind(';').map(|p| p + 1).unwrap_or(0);
  let stmt = &source[stmt_start..pos];
  let upper = stmt.trim_start().to_ascii_uppercase();
  // Slots that take transaction modifiers (ISOLATION LEVEL, READ ONLY,
  // READ WRITE, DEFERRABLE, NOT DEFERRABLE).
  const TXN_MODIFIERS: &[(&str, &str)] = &[
    ("ISOLATION LEVEL", "ISOLATION LEVEL SERIALIZABLE|REPEATABLE READ|READ COMMITTED|READ UNCOMMITTED"),
    ("READ ONLY", "READ ONLY"),
    ("READ WRITE", "READ WRITE"),
    ("DEFERRABLE", "DEFERRABLE"),
    ("NOT DEFERRABLE", "NOT DEFERRABLE"),
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

/// Object classes that COMMENT ON accepts. Subset most users actually
/// type; PG accepts more (POLICY, RULE, OPERATOR FAMILY, ...).
const COMMENT_ON_CLASSES: &[(&str, &str)] = &[
  ("TABLE", "COMMENT ON TABLE <name> IS '...'"),
  ("COLUMN", "COMMENT ON COLUMN <table>.<column> IS '...'"),
  ("SCHEMA", "COMMENT ON SCHEMA <name> IS '...'"),
  ("DATABASE", "COMMENT ON DATABASE <name> IS '...'"),
  ("FUNCTION", "COMMENT ON FUNCTION <name>(...) IS '...'"),
  ("PROCEDURE", "COMMENT ON PROCEDURE <name>(...) IS '...'"),
  ("INDEX", "COMMENT ON INDEX <name> IS '...'"),
  ("VIEW", "COMMENT ON VIEW <name> IS '...'"),
  ("MATERIALIZED VIEW", "COMMENT ON MATERIALIZED VIEW <name> IS '...'"),
  ("SEQUENCE", "COMMENT ON SEQUENCE <name> IS '...'"),
  ("TYPE", "COMMENT ON TYPE <name> IS '...'"),
  ("DOMAIN", "COMMENT ON DOMAIN <name> IS '...'"),
  ("EXTENSION", "COMMENT ON EXTENSION <name> IS '...'"),
  ("ROLE", "COMMENT ON ROLE <name> IS '...'"),
  ("TRIGGER", "COMMENT ON TRIGGER <name> ON <table> IS '...'"),
  ("CONSTRAINT", "COMMENT ON CONSTRAINT <name> ON <table> IS '...'"),
  ("POLICY", "COMMENT ON POLICY <name> ON <table> IS '...'"),
];

/// True when the cursor sits right after `COMMENT ON` with no class
/// keyword typed yet -- the user must pick a class before naming the
/// target object.
fn comment_on_expects_class_keyword(source: &str, offset: TextSize) -> bool {
  let pos: usize = u32::from(offset) as usize;
  let pos = pos.min(source.len());
  let stmt_start = source[..pos].rfind(';').map(|p| p + 1).unwrap_or(0);
  let stmt = &source[stmt_start..pos];
  let upper = stmt.trim_start().to_ascii_uppercase();
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
  let pos: usize = u32::from(offset) as usize;
  let pos = pos.min(source.len());
  let stmt_start = source[..pos].rfind(';').map(|p| p + 1).unwrap_or(0);
  let stmt = &source[stmt_start..pos];
  let upper = stmt.trim_start().to_ascii_uppercase();
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
  let pos: usize = u32::from(offset) as usize;
  let pos = pos.min(source.len());
  let stmt_start = source[..pos].rfind(';').map(|p| p + 1).unwrap_or(0);
  let stmt = &source[stmt_start..pos];
  let upper = stmt.trim_start().to_ascii_uppercase();
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
  let pos: usize = u32::from(offset) as usize;
  let pos = pos.min(source.len());
  let stmt_start = source[..pos].rfind(';').map(|p| p + 1).unwrap_or(0);
  let stmt = &source[stmt_start..pos];
  let upper = stmt.trim_start().to_ascii_uppercase();
  let words: Vec<&str> = upper.split_ascii_whitespace().collect();
  words.len() == 1 && words[0] == "DO" && stmt.ends_with(char::is_whitespace)
}

/// True when the cursor sits at the option slot of
/// `CREATE [TEMP|TEMPORARY|UNLOGGED] SEQUENCE [IF NOT EXISTS] <name>
/// <cursor>` -- the next token is one of the sequence-option keywords.
fn create_sequence_expects_option(source: &str, offset: TextSize) -> bool {
  let pos: usize = u32::from(offset) as usize;
  let pos = pos.min(source.len());
  let stmt_start = source[..pos].rfind(';').map(|p| p + 1).unwrap_or(0);
  let stmt = &source[stmt_start..pos];
  let upper = stmt.trim_start().to_ascii_uppercase();
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
  let pos: usize = (u32::from(offset) as usize).min(source.len());
  let stmt_start = source[..pos].rfind(';').map(|p| p + 1).unwrap_or(0);
  let stmt = &source[stmt_start..pos];
  let upper = stmt.trim_start().to_ascii_uppercase();
  if !upper.starts_with("CREATE TYPE") {
    return false;
  }
  // Need either ` AS ENUM (` or ` AS RANGE (` with the cursor inside.
  let body_open = upper.find(" AS ENUM (").map(|p| p + " AS ENUM (".len()).or_else(|| {
    upper.find(" AS RANGE (").map(|p| p + " AS RANGE (".len())
  });
  let Some(open_at) = body_open else {
    return false;
  };
  // Track paren depth from `open_at` to end -- still > 0 means cursor is inside.
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
  let pos: usize = u32::from(offset) as usize;
  let pos = pos.min(source.len());
  let stmt_start = source[..pos].rfind(';').map(|p| p + 1).unwrap_or(0);
  let stmt = &source[stmt_start..pos];
  let upper = stmt.trim_start().to_ascii_uppercase();
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
  let pos: usize = u32::from(offset) as usize;
  let pos = pos.min(source.len());
  let stmt_start = source[..pos].rfind(';').map(|p| p + 1).unwrap_or(0);
  let stmt = &source[stmt_start..pos];
  let upper = stmt.trim_start().to_ascii_uppercase();
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

/// True when the cursor sits inside the `EXPLAIN (...)` options paren
/// at a fresh option-name slot (start of paren, or right after `,`).
fn explain_paren_expects_option(source: &str, offset: TextSize) -> bool {
  let pos: usize = u32::from(offset) as usize;
  let pos = pos.min(source.len());
  let stmt_start = source[..pos].rfind(';').map(|p| p + 1).unwrap_or(0);
  let stmt = &source[stmt_start..pos];
  let upper = stmt.trim_start().to_ascii_uppercase();
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
  let pos: usize = u32::from(offset) as usize;
  let pos = pos.min(source.len());
  let stmt_start = source[..pos].rfind(';').map(|p| p + 1).unwrap_or(0);
  let stmt = &source[stmt_start..pos];
  let upper = stmt.trim_start().to_ascii_uppercase();
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
  let pos: usize = u32::from(offset) as usize;
  let pos = pos.min(source.len());
  let stmt_start = source[..pos].rfind(';').map(|p| p + 1).unwrap_or(0);
  let stmt = &source[stmt_start..pos];
  let upper = stmt.to_ascii_uppercase();
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
  let pos: usize = (u32::from(offset) as usize).min(source.len());
  let stmt_start = source[..pos].rfind(';').map(|p| p + 1).unwrap_or(0);
  let trimmed = source[stmt_start..pos].trim();
  let upper = trimmed.to_ascii_uppercase();
  if !upper.contains("VIEW") || !upper.ends_with("AS") {
    return false;
  }
  // Quick CREATE VIEW shape check -- the create_view_body_start path
  // in phase.rs validates the full prefix; here we just need to be
  // sure we're at the AS boundary with no body content yet.
  let bytes = upper.as_bytes();
  if !bytes.starts_with(b"CREATE") {
    return false;
  }
  true
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
  let pos: usize = (u32::from(offset) as usize).min(source.len());
  let stmt_start = source[..pos].rfind(';').map(|p| p + 1).unwrap_or(0);
  let slice = &source[stmt_start..pos];
  // Must end with whitespace -- otherwise the cursor is mid-name.
  if !slice.ends_with(char::is_whitespace) {
    return false;
  }
  let trimmed = slice.trim();
  let upper = trimmed.to_ascii_uppercase();
  if !upper.starts_with("DROP") {
    return false;
  }
  // Tokenize trimmed body on whitespace; quick filter.
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
  let pos: usize = (u32::from(offset) as usize).min(source.len());
  let stmt_start = source[..pos].rfind(';').map(|p| p + 1).unwrap_or(0);
  let slice = &source[stmt_start..pos];
  let upper = slice.to_ascii_uppercase();
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
  let pos: usize = (u32::from(offset) as usize).min(source.len());
  let stmt_start = source[..pos].rfind(';').map(|p| p + 1).unwrap_or(0);
  let slice = &source[stmt_start..pos];
  let upper = slice.to_ascii_uppercase();
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

/// True when the cursor sits directly after a leading `DROP` keyword
/// at the start of a statement (no other meaningful tokens between
/// DROP and the cursor). Returns the static (keyword, doc) list of
/// object types the user can drop. None when DROP isn't the leading
/// keyword or another token has already been typed after it.
fn after_top_level_drop_keyword(source: &str, offset: TextSize) -> Option<&'static [(&'static str, &'static str)]> {
  let pos: usize = (u32::from(offset) as usize).min(source.len());
  let bytes = source.as_bytes();
  // Walk back to the statement start (last `;` or 0).
  let stmt_start = source[..pos].rfind(';').map(|p| p + 1).unwrap_or(0);
  let slice = source[stmt_start..pos].trim();
  let upper = slice.to_ascii_uppercase();
  // Must be exactly `DROP` (no other tokens).
  if upper != "DROP" {
    return None;
  }
  // Cursor must be at end of input or right after `DROP` with trailing
  // whitespace.
  if pos < bytes.len() && !bytes[pos].is_ascii_whitespace() {
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
    ("SERVER", "DROP SERVER [IF EXISTS] <name>"),
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
    _ => None,
  }
}

fn alter_column_after_set_subkeyword_expects_silence(source: &str, offset: TextSize) -> bool {
  let pos: usize = u32::from(offset) as usize;
  let pos = pos.min(source.len());
  let stmt_start = source[..pos].rfind(';').map(|p| p + 1).unwrap_or(0);
  let stmt = &source[stmt_start..pos];
  let upper = stmt.trim_end().to_ascii_uppercase();
  if !upper.contains("ALTER COLUMN") {
    return false;
  }
  // The most recent SET sub-keyword in the trimmed upper text.
  for kw in &[" SET DEFAULT", " SET STATISTICS", " SET STORAGE", " SET COMPRESSION"] {
    if upper.ends_with(kw) && stmt.ends_with(char::is_whitespace) {
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
  let pos: usize = u32::from(offset) as usize;
  let pos = pos.min(source.len());
  let stmt_start = source[..pos].rfind(';').map(|p| p + 1).unwrap_or(0);
  let stmt = &source[stmt_start..pos];
  let upper = stmt.to_ascii_uppercase();
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
  let pos: usize = u32::from(offset) as usize;
  let pos = pos.min(source.len());
  let stmt_start = source[..pos].rfind(';').map(|p| p + 1).unwrap_or(0);
  let stmt = &source[stmt_start..pos];
  let upper = stmt.to_ascii_uppercase();
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
  let pos: usize = u32::from(offset) as usize;
  let pos = pos.min(source.len());
  let stmt_start = source[..pos].rfind(';').map(|p| p + 1).unwrap_or(0);
  let stmt = &source[stmt_start..pos];
  let upper = stmt.to_ascii_uppercase();
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
    (&["CLUSTER"], &["VERBOSE"]),
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
  let pos: usize = u32::from(offset) as usize;
  let pos = pos.min(source.len());
  let stmt_start = source[..pos].rfind(';').map(|p| p + 1).unwrap_or(0);
  let stmt = &source[stmt_start..pos];
  let upper = stmt.to_ascii_uppercase();
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
  let pos: usize = u32::from(offset) as usize;
  let pos = pos.min(source.len());
  let stmt_start = source[..pos].rfind(';').map(|p| p + 1).unwrap_or(0);
  let stmt = &source[stmt_start..pos];
  let upper = stmt.to_ascii_uppercase();
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
fn insert_target_table(source: &str, offset: TextSize) -> Option<String> {
  let pos: usize = u32::from(offset) as usize;
  let pos = pos.min(source.len());
  // Statement boundary.
  let stmt_start = source[..pos].rfind(';').map(|p| p + 1).unwrap_or(0);
  let stmt = &source[stmt_start..pos];
  let upper = stmt.to_ascii_uppercase();
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
  let idx = upper.find("CREATE TRIGGER")?;
  let rest_upper = &upper[idx..];
  let on_idx = rest_upper.find(" ON ")?;
  let after = &source[idx + on_idx + 4..];
  let tok = after
    .trim_start()
    .split(|c: char| c.is_whitespace() || c == '(' || c == ';' || c == ',')
    .find(|s| !s.is_empty())?;
  // Strip schema prefix if any.
  Some(tok.split('.').next_back().unwrap_or(tok).to_string())
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
fn at_fresh_name_slot(source: &str, offset: TextSize) -> bool {
  let pos: usize = u32::from(offset) as usize;
  let pos = pos.min(source.len());
  // Look backwards across the current statement only (last `;`).
  let stmt_start = source[..pos].rfind(';').map(|i| i + 1).unwrap_or(0);
  let before = &source[stmt_start..pos];
  let upper = before.to_ascii_uppercase();
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
    // `SHOW <name>` -- GUC name. No catalog completion is useful.
    "SHOW",
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
