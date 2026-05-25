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
    if count == 0 {
      if let Some(fb) = fallback::scope_from_text(source) {
        sources::columns_of_alias(&cat, &fb, &alias, &mut out);
      }
    }
    // CTE alias: surface columns the resolver extracted from the
    // CTE body projection. `cte_columns_of(alias)` returns
    // `Some(empty)` when the CTE is declared but the body was not
    // parsed -- in that case we have nothing useful to add.
    if out.is_empty() {
      if let Some(s) = stmt_scope {
        if let Some(cols) = s.cte_columns_of(&alias) {
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
    // Filter columns already used in the same clause -- even in dot
    // context, typing `SELECT u.id, u.|` should not re-offer `id`.
    let used = used_columns_in_clause(source, offset);
    if !used.is_empty() {
      out.retain(|it| !is_column_listed(it, &used));
    }
    return out;
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
      sources::statement_keywords(&mut out);
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
    },

    Phase::AfterTable | Phase::JoinModifier | Phase::JoinComplete => {
      push_aliases(file, scopes, source, offset, &mut out);
      sources::after_table_keywords(&mut out);
    },

    Phase::OnClause | Phase::WhereClause | Phase::InPredicate | Phase::HavingClause => {
      push_scope_columns_or_all(file, scopes, source, cat, offset, &mut out);
      push_aliases(file, scopes, source, offset, &mut out);
      push_all_functions(cat, &mut out);
      sources::expression_keywords(&mut out);
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

    Phase::LimitClause | Phase::OffsetClause => {
      // Numbers only; we don't suggest those. Just emit OFFSET as
      // a follow-up keyword.
      sources::after_table_keywords(&mut out);
    },

    Phase::AfterInsert => {
      sources::after_projection_keywords(&mut out);
    },
    Phase::AfterInsertTable => {
      sources::tables(cat, &mut out);
    },
    Phase::InsertColumnList => {
      push_scope_columns_or_all(file, scopes, source, cat, offset, &mut out);
      push_aliases(file, scopes, source, offset, &mut out);
    },
    Phase::InsertExpectValues | Phase::InsertValuesList => {
      push_aliases(file, scopes, source, offset, &mut out);
      push_all_functions(cat, &mut out);
      sources::expression_keywords(&mut out);
    },

    Phase::AfterUpdate => {
      sources::tables(cat, &mut out);
    },
    Phase::AfterUpdateTable => {
      push_aliases(file, scopes, source, offset, &mut out);
      sources::after_table_keywords(&mut out);
    },
    Phase::UpdateAssignment => {
      push_scope_columns_or_all(file, scopes, source, cat, offset, &mut out);
      push_aliases(file, scopes, source, offset, &mut out);
      push_all_functions(cat, &mut out);
      sources::expression_keywords(&mut out);
    },

    Phase::AfterDelete => {
      sources::after_projection_keywords(&mut out);
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
      sources::column_constraint_keywords(&mut out);
      // Constraint keywords like DEFAULT / CHECK introduce
      // expression contexts. Surface functions + expression
      // keywords here too so `col text DEFAULT now()` /
      // `col text CHECK (length(col) > 0)` autocompletes the
      // function names without forcing a new phase.
      push_all_functions(cat, &mut out);
      sources::expression_keywords(&mut out);
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
        for name in crate::source_tables::buffer_column_names(source, &table) {
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
      sources::alter_table_actions(&mut out);
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
      // Broad fallback: keywords + tables + columns + types + funcs.
      sources::keywords(&mut out);
      sources::types(&mut out);
      sources::functions(&mut out);
      sources::tables(cat, &mut out);
      sources::columns(cat, &mut out);
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
  let region = if source.is_char_boundary(anchor) && source.is_char_boundary(pos) {
    &source[anchor..pos]
  } else {
    ""
  };
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
  let head_end = item
    .find(|c: char| !c.is_ascii_alphanumeric() && c != '_' && c != '.' && c != '"')
    .unwrap_or(item.len());
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
    if mentions {
      if let Some(on_pos) = stmt_upper.find(" ON ") {
        let after = &source[p + on_pos + 4..stmt_end];
        let tok: String =
          after.trim_start().chars().take_while(|c| c.is_alphanumeric() || *c == '_' || *c == '.').collect();
        if !tok.is_empty() {
          return Some(tok.rsplit('.').next().unwrap_or(&tok).to_string());
        }
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
  ];
  PATTERNS.iter().any(|p| lead.ends_with(p))
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
fn push_aliases(file: &ParsedFile, scopes: &[Scope], source: &str, offset: TextSize, out: &mut Vec<Item>) {
  let start = out.len();
  if let Some(scope) = scope_for_offset(file, scopes, offset) {
    sources::aliases_in_scope(scope, out);
  }
  if out.len() == start {
    if let Some(scope) = fallback::scope_from_text(source) {
      sources::aliases_in_scope(&scope, out);
    }
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
  // Whether ANY in-scope binding was found, even if all of them were
  // aliased (and so contributed zero bare columns). This is what
  // decides whether to bury the menu under a catalog-wide column dump.
  let mut had_scope = false;
  if let Some(scope) = scope_for_offset(file, scopes, offset) {
    if scope.tables().next().is_some() {
      had_scope = true;
    }
    push_scope_columns(scope, cat, out);
  }
  if !had_scope {
    if let Some(fb) = fallback::scope_from_text(source) {
      had_scope = fb.tables().next().is_some();
      push_scope_columns(&fb, cat, out);
    }
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
  if pos > bytes.len() { return None }
  let mut s = pos;
  while s > 0 && bytes[s - 1] != b'\'' { s -= 1 }
  if s == 0 { return None }
  let mut k = s - 1;
  while k > 0 && bytes[k - 1].is_ascii_whitespace() { k -= 1 }
  let has_double = k >= 3 && bytes[k - 1] == b'>' && bytes[k - 2] == b'>' && bytes[k - 3] == b'-';
  let has_single = bytes[k - 1] == b'>' && bytes[k - 2] == b'-';
  if !has_double && !has_single { return None }
  let arrow_at = if has_double { k - 3 } else { k - 2 };
  // Identifier just before the first arrow.
  let mut j = arrow_at;
  while j > 0 && bytes[j - 1].is_ascii_whitespace() { j -= 1 }
  let id_end = j;
  while j > 0 && (bytes[j - 1].is_ascii_alphanumeric() || bytes[j - 1] == b'_' || bytes[j - 1] == b'.' || bytes[j - 1] == b'"') {
    j -= 1;
  }
  if j == id_end { return None }
  let col_full = &source[j..id_end];
  let col_bare = col_full.rsplit('.').next().unwrap_or(col_full).trim_matches('"');
  for t in catalog.tables() {
    if let Some(c) = t.columns.iter().find(|c| c.name.eq_ignore_ascii_case(col_bare)) {
      if let Some(keys) = &c.json_keys {
        if !keys.is_empty() {
          return Some(keys.clone());
        }
      }
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
      if double || single {
        Some(if double { p - 3 } else { p - 2 })
      } else {
        None
      }
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
    if &blob[i..i + needle.len()] == needle {
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
              if bytes[i] == b'\\' && i + 1 < n { i += 2; } else { i += 1; }
            }
          }
          c if c == open => depth += 1,
          c if c == close => depth -= 1,
          _ => {}
        }
        i += 1;
      }
      i.min(n)
    }
    b'"' => {
      let mut i = start + 1;
      while i < n && bytes[i] != b'"' {
        if bytes[i] == b'\\' && i + 1 < n { i += 2; } else { i += 1; }
      }
      (i + 1).min(n)
    }
    _ => {
      let mut i = start;
      while i < n && bytes[i] != b',' && bytes[i] != b'}' && bytes[i] != b']' {
        i += 1;
      }
      i
    }
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
      if k < n && b[k] == b':' {
        if !key.is_empty() && key.chars().all(|c| c.is_alphanumeric() || c == '_' || c == '-') {
          out.insert(key.to_string());
        }
      }
      i = j + 1;
      continue;
    }
    i += 1;
  }
}
