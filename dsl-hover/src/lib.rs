//! Rich SQL hover.
//!
//! Resolution order (first match wins):
//!   1. Column declaration when cursor sits on the column NAME inside
//!      a CREATE TABLE in the current buffer.
//!   2. Constraint / index / trigger identifier (pk_/fk_/idx_/...) -- catalog
//!      lookup or fallback convention card.
//!   3. Catalog table / column / DB function.
//!   4. Multi-word SQL keyword (sliding window).
//!   5. Single-word knowledge entry (keyword / type / built-in function).

pub mod constraint_id;
pub mod ddl;
pub mod implicit;
pub mod md_table;
pub mod render;
pub mod resolver;
pub mod token;

use dsl_catalog::Catalog;
use text_size::TextSize;

/// How keywords are cased in synthesized DDL fragments shown in hover.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeywordCase { Upper, Lower, Preserve }

impl Default for KeywordCase {
    fn default() -> Self { KeywordCase::Upper }
}

impl KeywordCase {
    pub fn apply(self, s: &str) -> String {
        match self {
            KeywordCase::Upper    => s.to_ascii_uppercase(),
            KeywordCase::Lower    => s.to_ascii_lowercase(),
            KeywordCase::Preserve => s.to_string(),
        }
    }
}

// Thread-local current keyword case so the render fns don't need a new
// parameter on every call site. Set by `hover_with`. Default Upper.
thread_local! {
    static KW_CASE: std::cell::Cell<KeywordCase> = std::cell::Cell::new(KeywordCase::Upper);
}

pub fn current_keyword_case() -> KeywordCase {
    KW_CASE.with(|c| c.get())
}

pub fn hover(source: &str, offset: TextSize, catalog: &Catalog) -> Option<String> {
    hover_with(source, offset, catalog, KeywordCase::Upper)
}

/// Like `hover` but applies the caller's preferred keyword casing to
/// every synthesised DDL fragment.
pub fn hover_with(
    source: &str,
    offset: TextSize,
    catalog: &Catalog,
    case: KeywordCase,
) -> Option<String> {
    KW_CASE.with(|c| c.set(case));
    let parsed = dsl_parse::parse(source, dsl_parse::Dialect::Postgres);
    if let Some(md) = ddl::column_decl_at(&parsed, source, offset) {
        return Some(md);
    }

    if let Some(tok) = token::token_at(source, offset) {
        // Dotted token `a.b` -- narrow to the side under the cursor.
        // Cursor on the alias side ⇒ table card. Cursor on the column
        // side ⇒ single-column card resolved through the alias.
        if tok.contains('.') {
            if let Some(part) = dotted_part_under_cursor(source, offset, &tok) {
                let last_seg = tok.rsplit('.').next().unwrap_or("");
                let on_right = part == last_seg;
                if on_right {
                    if let Some(md) = scope_column_lookup(source, offset, &tok, catalog) {
                        return Some(md);
                    }
                    if let Some(md) = catalog_lookup(&part, catalog) {
                        return Some(md);
                    }
                } else {
                    if let Some(md) = alias_lookup(source, offset, &part, catalog) {
                        return Some(md);
                    }
                    if let Some(md) = catalog_lookup(&part, catalog) {
                        return Some(md);
                    }
                }
                if let Some(entry) = dsl_knowledge::lookup(&part) {
                    return Some(dsl_knowledge::render_markdown(entry));
                }
            }
        }
        // NEW / OLD trigger row aliases. Inside a CREATE TRIGGER body
        // they bind to the trigger's target table; standalone we still
        // explain what they are.
        let upper_tok = tok.to_ascii_uppercase();
        if upper_tok == "NEW" || upper_tok == "OLD" {
            return Some(new_old_hover(&upper_tok, source));
        }
        // PL/pgSQL function parameter / DECLARE'd local. Cheap text scan
        // of the enclosing CREATE FUNCTION header + DECLARE block.
        if let Some(md) = plpgsql_local_hover(source, offset, &tok) {
            return Some(md);
        }
        // Buffer-defined CREATE FUNCTION / CREATE INDEX / CREATE TRIGGER
        // first -- if the user just wrote it, that's exactly what they
        // want to see, not a generic "no match in catalog" card.
        if let Some(md) = buffer_object(source, &tok) {
            return Some(md);
        }
        // Alias resolution -- if the cursor sits on `u` in `FROM users u`
        // or `JOIN orders AS o`, hover the underlying table.
        if let Some(md) = alias_lookup(source, offset, &tok, catalog) {
            return Some(md);
        }
        // Cursor inside a CREATE TABLE body? Resolve the column against
        // just that table to avoid the "this column lives in 11 tables"
        // ambiguity card -- inside `pk_users_id PRIMARY KEY (id)` the
        // `id` means `users.id`, full stop.
        if let Some(md) = enclosing_table_column(source, offset, &tok, catalog) {
            return Some(md);
        }
        // Same scoping for CREATE INDEX ... ON <table> (col), UPDATE
        // <table> SET col, DELETE FROM <table> WHERE col, INSERT INTO
        // <table> (col, ...) -- if we can name the single target table
        // from the surrounding text, narrow the hover to that table.
        if let Some(md) = scoped_column_in_text(source, offset, &tok, catalog) {
            return Some(md);
        }
        if let Some(md) = constraint_id::render_for(&tok, catalog) {
            return Some(md);
        }
        // Scope-aware column hover: when the cursor sits on `id` or
        // `ur.id` inside a SELECT / UPDATE / DELETE, resolve via the
        // statement's FROM/JOIN bindings so the hover names the actual
        // origin table instead of a "lives in N tables" card.
        if let Some(md) = scope_column_lookup(source, offset, &tok, catalog) {
            return Some(md);
        }
        if let Some(md) = catalog_lookup(&tok, catalog) {
            return Some(md);
        }
        if let Some(md) = db_function(&tok, catalog) {
            return Some(md);
        }
    }

    let window = token::window_at(source, offset);
    if let Some(md) = resolver::from_window(&window) {
        return Some(md);
    }

    if let Some(tok) = token::token_at(source, offset) {
        if let Some(entry) = dsl_knowledge::lookup(&tok) {
            return Some(dsl_knowledge::render_markdown(entry));
        }
    }
    None
}

fn catalog_lookup(token: &str, catalog: &Catalog) -> Option<String> {
    // Schema-qualified 3-segment path: `schema.table.column`.
    let segs: Vec<&str> = token.split('.').collect();
    if segs.len() == 3 {
        let (schema, table, column) = (segs[0], segs[1], segs[2]);
        if let Some(t) = catalog.find_table(Some(schema), table) {
            if let Some(c) = t.columns.iter().find(|c| c.name.eq_ignore_ascii_case(column)) {
                return Some(render::column(t, c));
            }
            // 3rd segment is not a column -- still a valid table card.
            return Some(render::table(t));
        }
    }
    if let Some((left, right)) = token.split_once('.') {
        if let Some(t) = catalog.find_table(Some(left), right) {
            return Some(render::table(t));
        }
        if let Some(t) = catalog.find_table(None, left) {
            if let Some(c) = t.columns.iter().find(|c| c.name == right) {
                return Some(render::column(t, c));
            }
        }
    }
    if let Some(t) = catalog.find_table(None, token) {
        return Some(render::table(t));
    }
    if let Some(ty) = catalog.find_type(None, token) {
        return Some(render::user_type(ty));
    }
    let cols = catalog.columns_named(token);
    if !cols.is_empty() {
        return Some(render::column_in_tables(&cols));
    }
    None
}

/// Find the CREATE TABLE body that encloses `offset` and resolve the
/// column reference against it (using either the live catalog row when
/// available, or the parsed buffer ColumnDef when not).
fn enclosing_table_column(
    source: &str,
    offset: TextSize,
    token: &str,
    catalog: &Catalog,
) -> Option<String> {
    let pos: usize = u32::from(offset) as usize;
    let parsed = dsl_parse::parse(source, dsl_parse::Dialect::Postgres);
    for stmt in &parsed.statements {
        let s: u32 = stmt.range.start().into();
        let e: u32 = stmt.range.end().into();
        if pos < s as usize || pos > e as usize { continue; }
        if let dsl_parse::StatementKind::CreateTable(ct) = &stmt.kind {
            // Prefer the live catalog row so we get types / nullability /
            // FK info. Fall back to the buffer ColumnDef when the table
            // isn't yet in the catalog.
            if let Some(t) = catalog.find_table(None, &ct.table.name) {
                if let Some(c) = t.columns.iter().find(|c| c.name.eq_ignore_ascii_case(token)) {
                    return Some(render::column(t, c));
                }
            }
            if let Some(col) = ct.columns.iter().find(|c| c.name.eq_ignore_ascii_case(token)) {
                return Some(render::column_decl(&ct.table.name, col));
            }
        }
    }
    None
}

/// If `token` is a FROM / JOIN alias in the enclosing statement, render
/// the hover for the actual underlying table instead of "no match". When
/// the cursor sits inside a `$$ ... $$` function body, the body text is
/// re-parsed standalone so the alias resolves against its inner SELECT.
fn alias_lookup(
    source: &str,
    offset: TextSize,
    token: &str,
    catalog: &Catalog,
) -> Option<String> {
    let pos: usize = u32::from(offset) as usize;
    // First try resolving in the top-level statement at this offset.
    if let Some(t) = resolve_alias_in(source, pos, token, catalog) {
        return Some(t);
    }
    // Fallback: re-parse the dollar-quoted body the cursor sits in.
    if let Some((body_start, body_end)) = enclosing_dollar_body(source, pos) {
        let body = &source[body_start..body_end];
        let body_pos = pos.saturating_sub(body_start);
        if let Some(t) = resolve_alias_in(body, body_pos, token, catalog) {
            return Some(t);
        }
    }
    None
}

/// When `tok` is a dotted identifier (`a.b` or longer), return only the
/// segment the cursor sits in. Returns `None` when `tok` has no `.`.
/// Cursor exactly on the dot snaps to the left side.
fn dotted_part_under_cursor(source: &str, offset: TextSize, tok: &str) -> Option<String> {
    if !tok.contains('.') { return None; }
    // Re-derive the token's byte range from the source so the cursor
    // offset is interpreted in source coordinates, not token-local.
    let pos: usize = u32::from(offset) as usize;
    let bytes = source.as_bytes();
    let mut start = pos;
    while start > 0 {
        let c = bytes[start - 1] as char;
        if c.is_alphanumeric() || c == '_' || c == '.' { start -= 1; } else { break; }
    }
    let mut end = pos;
    while end < bytes.len() {
        let c = bytes[end] as char;
        if c.is_alphanumeric() || c == '_' || c == '.' { end += 1; } else { break; }
    }
    if pos < start || pos > end { return None; }
    // Walk segments left-to-right; return the one containing the cursor.
    let mut seg_start = start;
    for (i, b) in source[start..end].bytes().enumerate() {
        if b == b'.' {
            let abs = start + i;
            if pos <= abs {
                return Some(source[seg_start..abs].to_string());
            }
            seg_start = abs + 1;
        }
    }
    Some(source[seg_start..end].to_string())
}

/// Scope-aware column hover for `tok` (bare column or `alias.col`).
///
/// Parses + resolves the statement enclosing `offset`, looks up `tok`
/// against its FROM/JOIN bindings, and returns a single-column hover
/// when the column belongs to exactly one in-scope table. Falls back to
/// `None` so the catalog-wide hover (which shows the "in N tables"
/// table when ambiguous) still runs.
fn scope_column_lookup(
    source: &str,
    offset: TextSize,
    tok: &str,
    catalog: &Catalog,
) -> Option<String> {
    let pos: usize = u32::from(offset) as usize;
    let parsed = dsl_parse::parse(source, dsl_parse::Dialect::Postgres);
    let scopes = dsl_resolve::resolve_with_source(&parsed.statements, source);
    let idx = parsed.statements.iter().position(|s| {
        let lo: u32 = s.range.start().into();
        let hi: u32 = s.range.end().into();
        pos >= lo as usize && pos <= hi as usize
    })?;
    let scope = scopes.get(idx)?;

    // Qualified form: `alias.col` -- resolve alias, then look up col.
    if let Some((left, right)) = tok.split_once('.') {
        if right.is_empty() { return None; }
        // CTE-qualified column? Render a CTE card showing the alias and
        // the projected column name.
        if let Some(cte_cols) = scope.cte_columns_of(left) {
            if cte_cols.iter().any(|c| c.eq_ignore_ascii_case(right)) {
                let cols_md = if cte_cols.is_empty() {
                    String::from("_columns not parsed_")
                } else {
                    cte_cols.iter()
                        .map(|c| format!("- `{c}`"))
                        .collect::<Vec<_>>()
                        .join("\n")
                };
                return Some(format!(
                    "# `{left}.{right}`\n_CTE column_\n\n**`{left}`** projects:\n\n{cols_md}\n"
                ));
            }
        }
        if let Some(binding) = scope.bindings.iter().find_map(|(k, v)| {
            if v.alias.eq_ignore_ascii_case(left) || k.eq_ignore_ascii_case(left) {
                Some(v)
            } else { None }
        }) {
            if let Some(t) = catalog.find_table(binding.table.schema.as_deref(), &binding.table.name) {
                if let Some(c) = t.columns.iter().find(|c| c.name.eq_ignore_ascii_case(right)) {
                    return Some(render::column(t, c));
                }
            }
        }
        return None;
    }

    // Bare column: scan every in-scope table for a unique match.
    let mut hit: Option<(&dsl_catalog::Table, &dsl_catalog::Column)> = None;
    for b in scope.tables() {
        if let Some(t) = catalog.find_table(b.table.schema.as_deref(), &b.table.name) {
            if let Some(c) = t.columns.iter().find(|c| c.name.eq_ignore_ascii_case(tok)) {
                if hit.is_some() {
                    // Ambiguous within scope -- let catalog_lookup render
                    // the multi-table disambiguation card.
                    return None;
                }
                hit = Some((t, c));
            }
        }
    }
    hit.map(|(t, c)| render::column(t, c))
}

/// Run the parse / resolve pipeline against `src`, find the statement
/// containing `pos`, and return the underlying-table hover for `token`
/// when it's an alias in that statement.
fn resolve_alias_in(src: &str, pos: usize, token: &str, catalog: &Catalog) -> Option<String> {
    let parsed = dsl_parse::parse(src, dsl_parse::Dialect::Postgres);
    let scopes = dsl_resolve::resolve(&parsed.statements);
    let idx = parsed.statements.iter().position(|s| {
        let lo: u32 = s.range.start().into();
        let hi: u32 = s.range.end().into();
        pos >= lo as usize && pos <= hi as usize
    })?;
    let scope = scopes.get(idx)?;
    let binding = scope.bindings.iter().find_map(|(k, v)| {
        if v.alias.eq_ignore_ascii_case(token) || k.eq_ignore_ascii_case(token) {
            if v.table.name.eq_ignore_ascii_case(token) { return None; }
            Some(v)
        } else { None }
    })?;
    if let Some(t) = catalog.find_table(binding.table.schema.as_deref(), &binding.table.name) {
        return Some(render::table(t));
    }
    buffer_object(src, &binding.table.name)
}

/// Return (open_after, close_before) byte offsets when `pos` is inside a
/// dollar-quoted `$$ ... $$` block. Used to extract the body for
/// re-parsing.
fn enclosing_dollar_body(source: &str, pos: usize) -> Option<(usize, usize)> {
    let bytes = source.as_bytes();
    let n = bytes.len();
    let mut i = 0usize;
    while i < n {
        if bytes[i] == b'$' {
            // Read tag.
            let tag_start = i;
            let mut j = i + 1;
            while j < n && (bytes[j].is_ascii_alphanumeric() || bytes[j] == b'_') { j += 1; }
            if j < n && bytes[j] == b'$' {
                let tag = &source[tag_start..=j];
                let body_start = j + 1;
                // Find matching close.
                let mut k = body_start;
                while k + tag.len() <= n {
                    if &source.as_bytes()[k..k + tag.len()] == tag.as_bytes() {
                        if pos >= body_start && pos <= k {
                            return Some((body_start, k));
                        }
                        i = k + tag.len();
                        break;
                    }
                    k += 1;
                }
                if k + tag.len() > n {
                    if pos >= body_start {
                        return Some((body_start, n));
                    }
                    return None;
                }
                continue;
            }
        }
        i += 1;
    }
    None
}

/// Find the target table for the statement enclosing the cursor when
/// it's a CREATE INDEX / UPDATE / DELETE FROM / INSERT INTO -- and look
/// up `token` as a column of that single table.
fn scoped_column_in_text(
    source: &str,
    offset: TextSize,
    token: &str,
    catalog: &Catalog,
) -> Option<String> {
    let pos: usize = u32::from(offset) as usize;
    // Walk back to the last `;` (or start) to bound the current statement.
    let stmt_start = source[..pos]
        .rfind(';')
        .map(|i| i + 1)
        .unwrap_or(0);
    let slice = &source[stmt_start..pos];
    let upper = slice.to_ascii_uppercase();

    let table = if upper.contains("CREATE INDEX") || upper.contains("CREATE UNIQUE INDEX") {
        // After `ON `.
        let on_pos = find_kw(&upper, "ON")?;
        read_table_after(slice, on_pos + 2)
    } else if upper.starts_with("UPDATE") {
        let after = slice["UPDATE".len()..].trim_start();
        Some(read_ident(after))
    } else if upper.contains("DELETE FROM") {
        let p = upper.find("DELETE FROM")?;
        let after = slice[p + "DELETE FROM".len()..].trim_start();
        Some(read_ident(after))
    } else if upper.contains("INSERT INTO") {
        let p = upper.find("INSERT INTO")?;
        let after = slice[p + "INSERT INTO".len()..].trim_start();
        Some(read_ident(after))
    } else if upper.contains("ALTER TABLE") {
        let p = upper.find("ALTER TABLE")?;
        let after = slice[p + "ALTER TABLE".len()..].trim_start();
        Some(read_ident(after))
    } else { None }?;
    if table.is_empty() { return None; }

    if let Some(t) = catalog.find_table(None, &table) {
        if let Some(c) = t.columns.iter().find(|c| c.name.eq_ignore_ascii_case(token)) {
            return Some(render::column(t, c));
        }
    }
    // Buffer-defined fallback (table being declared in same file).
    let parsed = dsl_parse::parse(source, dsl_parse::Dialect::Postgres);
    for stmt in &parsed.statements {
        if let dsl_parse::StatementKind::CreateTable(ct) = &stmt.kind {
            if ct.table.name.eq_ignore_ascii_case(&table) {
                if let Some(col) = ct.columns.iter().find(|c| c.name.eq_ignore_ascii_case(token)) {
                    return Some(render::column_decl(&ct.table.name, col));
                }
            }
        }
    }
    None
}

/// Whole-word search for `kw` in `upper`. Returns the byte offset.
fn find_kw(upper: &str, kw: &str) -> Option<usize> {
    let bytes = upper.as_bytes();
    let n = bytes.len();
    let mut from = 0usize;
    while let Some(rel) = upper[from..].find(kw) {
        let i = from + rel;
        let after = i + kw.len();
        let prev_ok = i == 0 || !(bytes[i - 1].is_ascii_alphanumeric() || bytes[i - 1] == b'_');
        let next_ok = after >= n || !(bytes[after].is_ascii_alphanumeric() || bytes[after] == b'_');
        if prev_ok && next_ok { return Some(i); }
        from = after;
    }
    None
}

fn read_table_after(src: &str, from: usize) -> Option<String> {
    let rest = src[from..].trim_start();
    Some(read_ident(rest))
}

fn read_ident(s: &str) -> String {
    let name: String = s
        .chars()
        .take_while(|c| c.is_alphanumeric() || *c == '_' || *c == '.')
        .collect();
    name.rsplit('.').next().unwrap_or(&name).to_string()
}

/// Render hover for the NEW / OLD trigger row aliases. If the buffer
/// contains a `CREATE TRIGGER ... ON <table>` clause, mention the table
/// the alias resolves against; otherwise show generic docs.
fn new_old_hover(name: &str, source: &str) -> String {
    let target = {
        let upper = source.to_ascii_uppercase();
        let idx = upper.find("CREATE TRIGGER");
        idx.and_then(|i| {
            let rest_upper = &upper[i..];
            rest_upper.find(" ON ").map(|p| {
                let after = &source[i + p + 4..];
                let n: String = after
                    .chars()
                    .take_while(|c| c.is_alphanumeric() || *c == '_' || *c == '.')
                    .collect();
                n
            })
        })
    };
    let desc = if name == "NEW" {
        "The row being inserted or updated. Inside an INSERT trigger \
         it holds the new tuple; inside UPDATE it holds the new column \
         values. NULL inside a DELETE / statement-level trigger."
    } else {
        "The row before UPDATE / DELETE. NULL inside an INSERT trigger \
         or a statement-level trigger."
    };
    let mut s = format!("# `{name}`\n*Trigger row variable*\n{desc}\n");
    if let Some(t) = target.filter(|s| !s.is_empty()) {
        s.push_str(&format!("\nResolves to a row of `{t}` (from the enclosing CREATE TRIGGER).\n"));
        s.push_str(&format!("```sql\n{name}.<column>\n```\n"));
    } else {
        s.push_str("```sql\nNEW.<column>   -- access a field of the new row\nOLD.<column>   -- access a field of the old row\n```\n");
    }
    s
}

/// Hover for a PL/pgSQL function parameter or DECLARE'd local. Uses
/// the same extraction logic as completion so the two stay in sync.
fn plpgsql_local_hover(source: &str, offset: TextSize, token: &str) -> Option<String> {
    let pos: usize = u32::from(offset) as usize;
    let locals = dsl_completion::plpgsql_locals::extract(source, pos);
    if let Some((_, ty)) = locals.params.iter().find(|(n, _)| n.eq_ignore_ascii_case(token)) {
        return Some(format!(
            "# `{token}`\n*function parameter*\n```sql\n{token} {ty}\n```\n"
        ));
    }
    if let Some((_, ty)) = locals.decls.iter().find(|(n, _)| n.eq_ignore_ascii_case(token)) {
        return Some(format!(
            "# `{token}`\n*local (DECLARE)*\n```sql\n{token} {ty}\n```\n"
        ));
    }
    None
}

fn db_function(token: &str, catalog: &Catalog) -> Option<String> {
    catalog
        .functions
        .iter()
        .find(|f| f.name.eq_ignore_ascii_case(token))
        .map(render::function_full)
}

/// Locate a CREATE FUNCTION / CREATE TRIGGER / CREATE INDEX defining
/// `token` in the buffer text and render it as a SQL code block. Lets
/// hover work before the DB schema has been refreshed.
fn buffer_object(source: &str, token: &str) -> Option<String> {
    for (heading, kind) in [
        ("CREATE OR REPLACE FUNCTION ", "Function"),
        ("CREATE FUNCTION ",            "Function"),
        ("CREATE OR REPLACE PROCEDURE ", "Procedure"),
        ("CREATE PROCEDURE ",            "Procedure"),
        ("CREATE TRIGGER ",              "Trigger"),
        ("CREATE INDEX ",                "Index"),
        ("CREATE UNIQUE INDEX ",         "Unique index"),
        ("CREATE MATERIALIZED VIEW ",    "Materialised view"),
        ("CREATE VIEW ",                 "View"),
        ("CREATE OR REPLACE VIEW ",     "View"),
        ("CREATE SEQUENCE ",             "Sequence"),
        ("CREATE TYPE ",                 "Type"),
        ("CREATE DOMAIN ",               "Domain"),
        ("CREATE EXTENSION ",            "Extension"),
        ("CREATE SCHEMA ",               "Schema"),
        ("CREATE POLICY ",               "Policy"),
    ] {
        if let Some(body) = find_def(source, heading, token) {
            return Some(format!(
                "# `{token}`\n_{kind} (current buffer)_\n\n```sql\n{}\n```\n",
                body.trim_end_matches(|c: char| c.is_whitespace())
            ));
        }
    }
    None
}

/// Walk `source` for `<heading> <token>` and, when found, return the
/// statement text up to the next top-level `;` (or EOF). Skips strings
/// and dollar-quoted bodies so the terminator is the real statement end.
fn find_def(source: &str, heading: &str, token: &str) -> Option<String> {
    let upper = source.to_ascii_uppercase();
    let mut from = 0usize;
    while let Some(rel) = upper[from..].find(heading) {
        let start = from + rel;
        let after = start + heading.len();
        let rest = &source[after..];
        let trim_lead = rest.len() - rest.trim_start().len();
        let name_start = after + trim_lead;
        let name_end = name_start
            + source[name_start..]
                .chars()
                .take_while(|c| c.is_alphanumeric() || *c == '_' || *c == '.')
                .map(|c| c.len_utf8())
                .sum::<usize>();
        let name = source[name_start..name_end].rsplit('.').next().unwrap_or("");
        if name.eq_ignore_ascii_case(token) {
            let end = stmt_end(source, name_end);
            return Some(source[start..end].to_string());
        }
        from = after;
    }
    None
}

/// Return the byte offset just past the next top-level `;`. Respects
/// `$$ ... $$` and single-quoted strings so we don't terminate mid-body.
fn stmt_end(source: &str, mut i: usize) -> usize {
    let bytes = source.as_bytes();
    let n = bytes.len();
    while i < n {
        match bytes[i] {
            b'\'' => {
                i += 1;
                while i < n {
                    if bytes[i] == b'\'' { i += 1; break; }
                    i += 1;
                }
            }
            b'$' => {
                // Dollar-quoted body. Read the tag.
                let tag_start = i;
                let mut j = i + 1;
                while j < n && (bytes[j].is_ascii_alphanumeric() || bytes[j] == b'_') { j += 1; }
                if j < n && bytes[j] == b'$' {
                    let tag = &source[tag_start..=j];
                    i = j + 1;
                    // Walk until matching tag.
                    while i + tag.len() <= n {
                        if &source.as_bytes()[i..i + tag.len()] == tag.as_bytes() {
                            i += tag.len();
                            break;
                        }
                        i += 1;
                    }
                } else {
                    i += 1;
                }
            }
            b';' => return i + 1,
            _ => i += 1,
        }
    }
    n
}
