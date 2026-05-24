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

pub fn complete(
    source: &str,
    file: &ParsedFile,
    scopes: &[Scope],
    catalog: &Catalog,
    offset: TextSize,
) -> Vec<Item> {
    // Normalise offset to the nearest valid UTF-8 char boundary so
    // downstream slicing can't panic on multi-byte characters.
    let raw_off: usize = offset.into();
    let off = floor_char_boundary(source, raw_off.min(source.len()));
    let offset = TextSize::from(off as u32);

    // Merge live catalog with in-file CREATE TABLE definitions.
    let derived = source_tables::from_file(file);
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
            let target = trigger_target_table(source)
                .or_else(|| enclosing_fn_trigger_table(source, pos, &cat));
            if let Some(t) = target {
                sources::columns_of_table(&cat, None, &t, &mut out);
                return out;
            }
            // No table known -- emit nothing so the user doesn't get a
            // misleading global column dump.
            return out;
        }
        let stmt_scope = scope_for_offset(file, scopes, offset);
        let count = stmt_scope
            .map(|s| sources::columns_of_alias(&cat, s, &alias, &mut out))
            .unwrap_or(0);
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
                let bare = ty
                    .split('%')
                    .next()
                    .unwrap_or(&ty)
                    .trim()
                    .trim_end_matches(';')
                    .trim();
                if cat.find_table(None, bare).is_some() {
                    sources::columns_of_table(&cat, None, bare, &mut out);
                }
            }
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
        }

        Phase::SelectProjection
        | Phase::InProjection
        | Phase::NextProjection => {
            push_scope_columns_or_all(file, scopes, source, cat, offset, &mut out);
            push_aliases(file, scopes, source, offset, &mut out);
            push_all_functions(cat, &mut out);
            sources::expression_keywords(&mut out);
        }

        Phase::AfterStar | Phase::ProjectionAlias => {
            // Just typed `*` or `AS alias`. Next legal tokens: FROM
            // (continue the query) or `,` (more projection). Emit only
            // the small after-projection keyword set.
            sources::after_projection_keywords(&mut out);
        }

        Phase::ExpectTable => {
            sources::tables(cat, &mut out);
        }

        Phase::AfterTable | Phase::JoinModifier | Phase::JoinComplete => {
            push_aliases(file, scopes, source, offset, &mut out);
            sources::after_table_keywords(&mut out);
        }

        Phase::OnClause | Phase::WhereClause | Phase::InPredicate | Phase::HavingClause => {
            push_scope_columns_or_all(file, scopes, source, cat, offset, &mut out);
            push_aliases(file, scopes, source, offset, &mut out);
            push_all_functions(cat, &mut out);
            sources::expression_keywords(&mut out);
        }

        Phase::UsingClause => {
            push_scope_columns_or_all(file, scopes, source, cat, offset, &mut out);
            push_aliases(file, scopes, source, offset, &mut out);
        }

        Phase::AfterGroup | Phase::AfterOrder => {
            // Just typed GROUP / ORDER, next is "BY".
            sources::after_table_keywords(&mut out);
        }
        Phase::GroupByList => {
            push_scope_columns_or_all(file, scopes, source, cat, offset, &mut out);
            push_aliases(file, scopes, source, offset, &mut out);
            push_all_functions(cat, &mut out);
        }
        Phase::OrderByList => {
            push_scope_columns_or_all(file, scopes, source, cat, offset, &mut out);
            push_aliases(file, scopes, source, offset, &mut out);
            push_all_functions(cat, &mut out);
            sources::order_modifiers(&mut out);
        }

        Phase::LimitClause | Phase::OffsetClause => {
            // Numbers only; we don't suggest those. Just emit OFFSET as
            // a follow-up keyword.
            sources::after_table_keywords(&mut out);
        }

        Phase::AfterInsert => {
            sources::after_projection_keywords(&mut out);
        }
        Phase::AfterInsertTable => {
            sources::tables(cat, &mut out);
        }
        Phase::InsertColumnList => {
            push_scope_columns_or_all(file, scopes, source, cat, offset, &mut out);
            push_aliases(file, scopes, source, offset, &mut out);
        }
        Phase::InsertExpectValues | Phase::InsertValuesList => {
            push_aliases(file, scopes, source, offset, &mut out);
            push_all_functions(cat, &mut out);
            sources::expression_keywords(&mut out);
        }

        Phase::AfterUpdate => {
            sources::tables(cat, &mut out);
        }
        Phase::AfterUpdateTable => {
            push_aliases(file, scopes, source, offset, &mut out);
            sources::after_table_keywords(&mut out);
        }
        Phase::UpdateAssignment => {
            push_scope_columns_or_all(file, scopes, source, cat, offset, &mut out);
            push_aliases(file, scopes, source, offset, &mut out);
            push_all_functions(cat, &mut out);
            sources::expression_keywords(&mut out);
        }

        Phase::AfterDelete => {
            sources::after_projection_keywords(&mut out);
        }

        // CREATE TABLE sub-phases ---------------------------------------
        Phase::CtlExpectTableName => {
            // Fresh name; nothing useful to suggest.
        }
        Phase::CtlBodyStart => {
            // User could be starting a column declaration (no completion
            // for the name itself) or a table-level constraint line.
            sources::create_table_entry_starters(&mut out);
        }
        Phase::CtlExpectType => {
            sources::types_only(&mut out);
        }
        Phase::CtlExpectColumnConstraint => {
            sources::column_constraint_keywords(&mut out);
        }
        Phase::CtlExpectConstraintName => {
            // Fresh constraint name; nothing useful.
        }
        Phase::CtlExpectConstraintKind => {
            sources::constraint_kinds(&mut out);
        }
        Phase::CtlExpectFkTable {} => {
            sources::tables(cat, &mut out);
        }
        Phase::CtlCheckExpr { table } => {
            if let Some(t) = table {
                sources::columns_of_table(cat, None, &t, &mut out);
                if out.is_empty() {
                    for name in crate::source_tables::buffer_column_names(source, &t) {
                        out.push(crate::item::Item {
                            label: name.clone(),
                            kind: crate::item::ItemKind::Column,
                            detail: Some(format!("column of `{t}` (buffer)")),
                            description: None,
                            documentation_md: None,
                            insert_text: name,
                            sort_priority: 0,
                        });
                    }
                }
            }
            push_all_functions(cat, &mut out);
            sources::expression_keywords(&mut out);
        }
        Phase::CtlExpectFkColumn { table } => {
            sources::columns_of_table(cat, None, &table, &mut out);
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
            sort_priority: 5,
                    });
                }
            }
        }

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
        }
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
        }

        // After PG `::` cast operator -- emit types only. Built-ins
        // first, then user-defined enums/domains/composites from the
        // live catalog.
        Phase::CastType => {
            sources::types(&mut out);
            sources::db_types(cat, &mut out);
        }

        Phase::AfterCreate | Phase::AfterAlter | Phase::AfterDrop | Phase::Unknown => {
            // Broad fallback: keywords + tables + columns + types + funcs.
            sources::keywords(&mut out);
            sources::types(&mut out);
            sources::functions(&mut out);
            sources::tables(cat, &mut out);
            sources::columns(cat, &mut out);
        }
    }
    dedup_items(out)
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
    for needle in ["CREATE OR REPLACE FUNCTION ", "CREATE FUNCTION ",
                   "CREATE OR REPLACE PROCEDURE ", "CREATE PROCEDURE "] {
        let mut from = 0usize;
        while let Some(rel) = upper[from..].find(needle) {
            let p = from + rel;
            if p > pos { break; }
            latest = Some(p + needle.len());
            from = p + needle.len();
        }
    }
    let after = source[latest?..].trim_start();
    let fn_name: String = after
        .chars()
        .take_while(|c| c.is_alphanumeric() || *c == '_' || *c == '.')
        .collect();
    let fn_short = fn_name.rsplit('.').next().unwrap_or(&fn_name);
    if fn_short.is_empty() { return None; }

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
                let tok: String = after
                    .trim_start()
                    .chars()
                    .take_while(|c| c.is_alphanumeric() || *c == '_' || *c == '.')
                    .collect();
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
    if !after_dot.chars().all(|c| c.is_alphanumeric() || c == '_') { return None; }
    let pre_dot = &before[..dot_idx];
    let alias: String = pre_dot
        .chars()
        .rev()
        .take_while(|c| c.is_alphanumeric() || *c == '_')
        .collect::<String>()
        .chars()
        .rev()
        .collect();
    if alias.is_empty() { None } else { Some(alias) }
}

/// Return the largest valid UTF-8 char boundary at or before `byte`.
/// Mirrors the (unstable) `str::floor_char_boundary` API.
fn floor_char_boundary(s: &str, byte: usize) -> usize {
    let mut b = byte.min(s.len());
    while b > 0 && !s.is_char_boundary(b) { b -= 1; }
    b
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
            if let (Some(existing_detail), Some(new_detail)) =
                (out[existing].detail.as_ref(), it.detail.as_ref())
            {
                if existing_detail != new_detail
                    && !existing_detail.contains(new_detail.as_str())
                {
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
fn push_aliases(
    file: &ParsedFile,
    scopes: &[Scope],
    source: &str,
    offset: TextSize,
    out: &mut Vec<Item>,
) {
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
        if scope.tables().next().is_some() { had_scope = true; }
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
        if b.alias != b.table.name { continue; }
        if !seen.insert(b.table.name.to_ascii_lowercase()) { continue; }
        let Some(t) = cat.find_table(b.table.schema.as_deref(), &b.table.name) else {
            continue;
        };
        for c in &t.columns {
            out.push(sources::column_item(t, c));
        }
    }
}

fn scope_for_offset<'a>(
    file: &ParsedFile,
    scopes: &'a [Scope],
    offset: TextSize,
) -> Option<&'a Scope> {
    let idx = file
        .statements
        .iter()
        .position(|s| s.range.contains_inclusive(offset))?;
    scopes.get(idx)
}
