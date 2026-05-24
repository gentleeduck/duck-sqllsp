//! Item producers, one per kind of source data.
//!
//! Each emitter pushes onto an output vector; the engine decides which
//! emitters to call based on the detected [`Context`](crate::context::Context).

use crate::item::{Item, ItemKind};
use crate::render;
use dsl_catalog::{Catalog, Column, Table, TableKind};
use dsl_knowledge::{self as kb, Entry, Kind};
use dsl_resolve::Scope;

pub fn keywords(out: &mut Vec<Item>) {
    for (label, e) in kb::keywords() {
        out.push(from_entry(label, e, ItemKind::Keyword));
    }
}

pub fn types(out: &mut Vec<Item>) {
    for (label, e) in kb::types() {
        out.push(from_entry(label, e, ItemKind::Type));
    }
}

pub fn functions(out: &mut Vec<Item>) {
    for (label, e) in kb::functions() {
        out.push(from_entry(label, e, ItemKind::Function));
    }
}

/// User-defined functions from the catalog. Surfaced alongside the
/// built-ins so completion shows app code next to standard SQL.
/// User-defined types from the live catalog (enum / domain / composite).
/// Emitted after `sources::types()` in CastType phase so built-in PG
/// types still sort first.
pub fn db_types(cat: &Catalog, out: &mut Vec<Item>) {
    for t in &cat.types {
        let kind_label = match t.kind {
            dsl_catalog::TypeKind::Enum => "enum",
            dsl_catalog::TypeKind::Domain => "domain",
            dsl_catalog::TypeKind::Composite => "composite",
        };
        let doc = format!(
            "**User-defined {kind_label}** `{}.{}`\n\n_From catalog._\n",
            t.schema, t.name
        );
        out.push(Item {
            label: t.name.clone(),
            kind: ItemKind::Type,
            detail: Some(format!("{kind_label} type")),
            description: Some(format!("{} (db)", t.schema)),
            documentation_md: Some(doc),
            insert_text: t.name.clone(),
            is_snippet: false,
            sort_priority: 5,
        });
    }
}

pub fn db_functions(cat: &Catalog, out: &mut Vec<Item>) {
    for f in &cat.functions {
        let args = f
            .arguments
            .iter()
            .map(|a| match &a.name {
                Some(n) => format!("{n} {}", a.data_type),
                None => a.data_type.clone(),
            })
            .collect::<Vec<_>>()
            .join(", ");
        let signature = format!("{}({}) -> {}", f.name, args, f.return_type);
        let doc = format!(
            "**DB function** `{}.{}`\n\n```sql\n{}\n```\n",
            f.schema, f.name, signature
        );
        let insert_text = if f.arguments.is_empty() {
            format!("{}()", f.name)
        } else {
            format!("{}($0)", f.name)
        };
        out.push(Item {
            label: f.name.clone(),
            kind: ItemKind::Function,
            detail: Some(signature),
            description: Some(format!("{} (db)", f.schema)),
            documentation_md: Some(doc),
            insert_text,
            is_snippet: !f.arguments.is_empty(),
            sort_priority: 5,
        });
    }
}

/// Subset of [`functions`] safe inside a SELECT projection / expression:
/// aggregates, common scalar helpers, conditionals, window functions.
/// Excludes the long tail (JSON/array helpers) that would be noise.
pub fn projection_functions(out: &mut Vec<Item>) {
    const KEEP: &[&str] = &[
        "count", "sum", "avg", "min", "max",
        "array_agg", "string_agg", "json_agg", "jsonb_agg",
        "coalesce", "nullif", "greatest", "least",
        "lower", "upper", "length", "substring", "trim", "concat",
        "replace", "split_part",
        "now", "current_date", "age", "date_trunc", "extract", "to_char",
        "abs", "round",
        "gen_random_uuid",
        "row_number", "rank", "dense_rank", "lag", "lead",
    ];
    let table = kb::functions();
    for name in KEEP {
        if let Some(e) = table.get(*name) {
            out.push(from_entry(name, e, ItemKind::Function));
        }
    }
}

/// Keywords valid inside expression position. Excludes DDL/DML statement
/// keywords so SELECT projection doesn't get polluted with CREATE TABLE etc.
pub fn expression_keywords(out: &mut Vec<Item>) {
    const KEEP: &[&str] = &[
        "AS", "DISTINCT", "ALL",
        "CASE", "WHEN", "THEN", "ELSE", "END",
        "AND", "OR", "NOT",
        "IS NULL", "IS NOT NULL", "NULL", "IS",
        "IN", "EXISTS", "BETWEEN", "LIKE", "ILIKE",
        "OVER", "PARTITION BY",
    ];
    emit_keyword_subset(out, KEEP);
}

/// Keywords valid at the start of a statement.
pub fn statement_keywords(out: &mut Vec<Item>) {
    const KEEP: &[&str] = &[
        "SELECT", "WITH", "INSERT INTO", "UPDATE", "DELETE FROM",
        "CREATE TABLE", "CREATE INDEX", "CREATE UNIQUE INDEX", "CREATE VIEW",
        "CREATE SCHEMA", "CREATE SEQUENCE",
        "ALTER TABLE", "DROP TABLE",
        "TRUNCATE", "EXPLAIN", "EXPLAIN ANALYZE",
        "BEGIN", "COMMIT", "ROLLBACK", "SAVEPOINT",
        "GRANT", "REVOKE", "MERGE", "REFRESH", "REINDEX", "VACUUM", "ANALYZE",
        "SET", "SHOW", "COMMENT", "COPY", "CALL",
    ];
    emit_keyword_subset(out, KEEP);
    statement_snippets(out);
}

/// Common multi-line scaffolds. These ride above the plain statement
/// keywords because typing `ctable` / `fn` / `trig` is faster than
/// hand-rolling the full skeleton every time.
fn statement_snippets(out: &mut Vec<Item>) {
    out.push(Item {
        label: "ctable".into(),
        kind: ItemKind::Keyword,
        detail: Some("CREATE TABLE skeleton".into()),
        description: Some("snippet".into()),
        documentation_md: Some(
            "Expands to a CREATE TABLE with id PK, created_at, and a tab-stop \
             for the table name and additional columns.".into()
        ),
        insert_text: "CREATE TABLE ${1:name} (\n    id uuid NOT NULL DEFAULT gen_random_uuid() PRIMARY KEY,\n    ${2:col} ${3:type} NOT NULL,\n    created_at timestamptz NOT NULL DEFAULT now()\n);$0".into(),
        is_snippet: true,
        sort_priority: 3,
    });
    out.push(Item {
        label: "fn".into(),
        kind: ItemKind::Keyword,
        detail: Some("CREATE OR REPLACE FUNCTION skeleton".into()),
        description: Some("snippet".into()),
        documentation_md: Some(
            "Expands to a CREATE OR REPLACE FUNCTION with plpgsql body.".into()
        ),
        insert_text: "CREATE OR REPLACE FUNCTION ${1:name}(${2:args})\n    RETURNS ${3:void}\n    LANGUAGE plpgsql\nAS $$\nBEGIN\n    $0\nEND;\n$$;".into(),
        is_snippet: true,
        sort_priority: 3,
    });
    out.push(Item {
        label: "trig".into(),
        kind: ItemKind::Keyword,
        detail: Some("CREATE TRIGGER + handler function skeleton".into()),
        description: Some("snippet".into()),
        documentation_md: Some(
            "Expands to a row-level AFTER trigger + matching plpgsql handler.".into()
        ),
        insert_text: "CREATE OR REPLACE FUNCTION ${1:handler}()\n    RETURNS TRIGGER\n    LANGUAGE plpgsql\nAS $$\nBEGIN\n    $0\n    RETURN NEW;\nEND;\n$$;\n\nCREATE TRIGGER ${2:trg_name}\n    AFTER INSERT OR UPDATE ON ${3:table}\n    FOR EACH ROW\n    EXECUTE FUNCTION ${1:handler}();".into(),
        is_snippet: true,
        sort_priority: 3,
    });
    out.push(Item {
        label: "idx".into(),
        kind: ItemKind::Keyword,
        detail: Some("CREATE INDEX skeleton".into()),
        description: Some("snippet".into()),
        documentation_md: Some("Index on a single column.".into()),
        insert_text: "CREATE INDEX ${1:idx_name} ON ${2:table} (${3:col});$0".into(),
        is_snippet: true,
        sort_priority: 3,
    });
    out.push(Item {
        label: "view".into(),
        kind: ItemKind::Keyword,
        detail: Some("CREATE VIEW skeleton".into()),
        description: Some("snippet".into()),
        documentation_md: Some("CREATE OR REPLACE VIEW with a body.".into()),
        insert_text: "CREATE OR REPLACE VIEW ${1:name} AS\nSELECT $0\nFROM ${2:table};".into(),
        is_snippet: true,
        sort_priority: 3,
    });
    out.push(Item {
        label: "mat".into(),
        kind: ItemKind::Keyword,
        detail: Some("CREATE MATERIALIZED VIEW skeleton".into()),
        description: Some("snippet".into()),
        documentation_md: Some("Materialised view + REFRESH hint.".into()),
        insert_text: "CREATE MATERIALIZED VIEW ${1:name} AS\nSELECT $0\nFROM ${2:table}\nWITH DATA;".into(),
        is_snippet: true,
        sort_priority: 3,
    });
    out.push(Item {
        label: "enum".into(),
        kind: ItemKind::Keyword,
        detail: Some("CREATE TYPE ... AS ENUM skeleton".into()),
        description: Some("snippet".into()),
        documentation_md: Some("Enum type with a couple of starter values.".into()),
        insert_text: "CREATE TYPE ${1:name} AS ENUM (\n    '${2:value_a}',\n    '${3:value_b}'\n);$0".into(),
        is_snippet: true,
        sort_priority: 3,
    });
    out.push(Item {
        label: "dom".into(),
        kind: ItemKind::Keyword,
        detail: Some("CREATE DOMAIN skeleton".into()),
        description: Some("snippet".into()),
        documentation_md: Some("Domain over an underlying type with optional CHECK.".into()),
        insert_text: "CREATE DOMAIN ${1:name} AS ${2:text}\n    CHECK (VALUE ${3:~ '^.+$'});$0".into(),
        is_snippet: true,
        sort_priority: 3,
    });
    out.push(Item {
        label: "pol".into(),
        kind: ItemKind::Keyword,
        detail: Some("CREATE POLICY skeleton".into()),
        description: Some("snippet".into()),
        documentation_md: Some("Row-level security policy.".into()),
        insert_text: "CREATE POLICY ${1:name} ON ${2:table}\n    FOR ${3|ALL,SELECT,INSERT,UPDATE,DELETE|}\n    TO ${4:role}\n    USING (${5:true});$0".into(),
        is_snippet: true,
        sort_priority: 3,
    });
    out.push(Item {
        label: "do".into(),
        kind: ItemKind::Keyword,
        detail: Some("DO $$ ... $$ anonymous block".into()),
        description: Some("snippet".into()),
        documentation_md: Some("Anonymous plpgsql block (no function declaration).".into()),
        insert_text: "DO $$\nBEGIN\n    $0\nEND;\n$$;".into(),
        is_snippet: true,
        sort_priority: 3,
    });
    out.push(Item {
        label: "addcol".into(),
        kind: ItemKind::Keyword,
        detail: Some("ALTER TABLE ADD COLUMN".into()),
        description: Some("snippet".into()),
        documentation_md: Some("Add a new column.".into()),
        insert_text: "ALTER TABLE ${1:table} ADD COLUMN ${2:name} ${3:type} ${4|NOT NULL,NULL|}${5: DEFAULT ${6:expr}};$0".into(),
        is_snippet: true,
        sort_priority: 3,
    });
    out.push(Item {
        label: "rencol".into(),
        kind: ItemKind::Keyword,
        detail: Some("ALTER TABLE RENAME COLUMN".into()),
        description: Some("snippet".into()),
        documentation_md: Some("Rename a column.".into()),
        insert_text: "ALTER TABLE ${1:table} RENAME COLUMN ${2:old_name} TO ${3:new_name};$0".into(),
        is_snippet: true,
        sort_priority: 3,
    });
    out.push(Item {
        label: "rentab".into(),
        kind: ItemKind::Keyword,
        detail: Some("ALTER TABLE RENAME TO".into()),
        description: Some("snippet".into()),
        documentation_md: Some("Rename a table.".into()),
        insert_text: "ALTER TABLE ${1:old_name} RENAME TO ${2:new_name};$0".into(),
        is_snippet: true,
        sort_priority: 3,
    });
    out.push(Item {
        label: "copyin".into(),
        kind: ItemKind::Keyword,
        detail: Some("COPY ... FROM CSV".into()),
        description: Some("snippet".into()),
        documentation_md: Some("Bulk load from a CSV file with HEADER + DELIMITER.".into()),
        insert_text: "COPY ${1:table} (${2:col1, col2})\nFROM '${3:/path/to/file.csv}'\nWITH (FORMAT csv, HEADER true, DELIMITER ',');$0".into(),
        is_snippet: true,
        sort_priority: 3,
    });
    out.push(Item {
        label: "copyout".into(),
        kind: ItemKind::Keyword,
        detail: Some("COPY ... TO CSV".into()),
        description: Some("snippet".into()),
        documentation_md: Some("Export to a CSV file with HEADER.".into()),
        insert_text: "COPY (SELECT ${1:*} FROM ${2:table})\nTO '${3:/path/to/file.csv}'\nWITH (FORMAT csv, HEADER true);$0".into(),
        is_snippet: true,
        sort_priority: 3,
    });
    out.push(Item {
        label: "listen".into(),
        kind: ItemKind::Keyword,
        detail: Some("LISTEN / UNLISTEN pair".into()),
        description: Some("snippet".into()),
        documentation_md: Some("LISTEN to a channel; remember to UNLISTEN on session end.".into()),
        insert_text: "LISTEN ${1:channel};\n-- ... do work ...\nUNLISTEN ${1:channel};$0".into(),
        is_snippet: true,
        sort_priority: 3,
    });
    out.push(Item {
        label: "notify".into(),
        kind: ItemKind::Keyword,
        detail: Some("NOTIFY <channel>, '<payload>'".into()),
        description: Some("snippet".into()),
        documentation_md: Some("Send a notification on a channel.".into()),
        insert_text: "NOTIFY ${1:channel}, '${2:payload}';$0".into(),
        is_snippet: true,
        sort_priority: 3,
    });
    out.push(Item {
        label: "upsert".into(),
        kind: ItemKind::Keyword,
        detail: Some("INSERT ... ON CONFLICT DO UPDATE".into()),
        description: Some("snippet".into()),
        documentation_md: Some("Insert or update on conflict (PG-native upsert).".into()),
        insert_text: "INSERT INTO ${1:table} (${2:cols})\nVALUES (${3:vals})\nON CONFLICT (${4:conflict_col})\nDO UPDATE SET ${5:col} = EXCLUDED.${5:col};$0".into(),
        is_snippet: true,
        sort_priority: 3,
    });
}

/// Keywords expected after a finished table reference: JOIN / WHERE /
/// GROUP BY / HAVING / ORDER BY / LIMIT / set ops.
pub fn after_table_keywords(out: &mut Vec<Item>) {
    const KEEP: &[&str] = &[
        "INNER JOIN", "LEFT JOIN", "RIGHT JOIN", "FULL OUTER JOIN", "CROSS JOIN",
        "JOIN", "ON", "USING", "AS", "LATERAL",
        "WHERE", "GROUP BY", "HAVING", "ORDER BY", "LIMIT", "OFFSET",
        "UNION", "INTERSECT", "EXCEPT",
    ];
    emit_keyword_subset(out, KEEP);
}

/// Keywords valid at the end of a projection item (between SELECT and FROM).
pub fn after_projection_keywords(out: &mut Vec<Item>) {
    const KEEP: &[&str] = &["FROM", "AS", "INTO"];
    emit_keyword_subset(out, KEEP);
}

/// Sort modifiers used inside ORDER BY.
pub fn order_modifiers(out: &mut Vec<Item>) {
    const KEEP: &[&str] = &["ASC", "DESC", "NULLS FIRST", "NULLS LAST"];
    emit_keyword_subset(out, KEEP);
}

/// Column-constraint keywords valid right after a column type inside a
/// CREATE TABLE definition.
pub fn column_constraint_keywords(out: &mut Vec<Item>) {
    const KEEP: &[&str] = &[
        "NOT NULL", "NULL", "DEFAULT", "PRIMARY KEY", "UNIQUE", "CHECK",
        "REFERENCES", "GENERATED", "COLLATE",
    ];
    emit_keyword_subset(out, KEEP);
}

/// Starter keywords that may begin an entry inside a CREATE TABLE body
/// in lieu of a fresh column name.
pub fn create_table_entry_starters(out: &mut Vec<Item>) {
    const KEEP: &[&str] = &[
        "PRIMARY KEY", "FOREIGN KEY", "UNIQUE", "CHECK", "LIKE",
    ];
    emit_keyword_subset(out, KEEP);
    // CONSTRAINT gets a snippet template so the user only types the
    // constraint name, picks the kind, and types the column list.
    out.push(Item {
        label: "CONSTRAINT".into(),
        kind: ItemKind::Keyword,
        detail: Some("CONSTRAINT <name> <kind> (<cols>)".into()),
        description: Some("named constraint".into()),
        documentation_md: Some(
            "Insert a named constraint. After the snippet expands you fill in:\n\n\
             1. constraint name\n\
             2. constraint kind (`PRIMARY KEY` / `UNIQUE` / `FOREIGN KEY` / `CHECK`)\n\
             3. column list".into()
        ),
        insert_text: "CONSTRAINT ${1:name} ${2|PRIMARY KEY,UNIQUE,FOREIGN KEY,CHECK|} (${3:col})$0".into(),
        is_snippet: true,
        sort_priority: 4,
    });
}

/// Constraint-kind keywords that follow `CONSTRAINT <name>`.
pub fn constraint_kinds(out: &mut Vec<Item>) {
    const KEEP: &[&str] = &["PRIMARY KEY", "FOREIGN KEY", "UNIQUE", "CHECK"];
    emit_keyword_subset(out, KEEP);
}

/// All known SQL types -- used inside CREATE TABLE column-type position.
pub fn types_only(out: &mut Vec<Item>) {
    for (label, e) in kb::types() {
        out.push(from_entry(label, e, ItemKind::Type));
    }
}

/// Columns of a specific catalog table only.
pub fn columns_of_table(
    cat: &Catalog, schema: Option<&str>, table_name: &str, out: &mut Vec<Item>,
) {
    if let Some(t) = cat.find_table(schema, table_name) {
        for c in &t.columns {
            out.push(column_item(t, c));
        }
    }
}

/// PL/pgSQL flow-control + body essentials.
pub fn plpgsql_keywords(out: &mut Vec<Item>) {
    const KEEP: &[&str] = &[
        // Control flow + block structure.
        "DECLARE", "BEGIN", "END", "IF", "ELSIF", "ELSE", "THEN",
        "LOOP", "WHILE", "FOR", "EXIT", "CONTINUE", "PERFORM",
        // Return + exception handling.
        "RETURN", "RETURNS", "RAISE", "NOTICE", "WARNING", "EXCEPTION",
        "FOUND", "STRICT", "INTO STRICT",
        // SQL inside the body.
        "EXECUTE", "SELECT", "INSERT INTO", "UPDATE", "DELETE FROM",
        "WHERE", "AND", "OR", "NOT", "IS NULL", "IS NOT NULL",
        "AS", "FROM", "JOIN", "ON", "GROUP BY", "ORDER BY", "LIMIT",
        "VALUES", "SET", "RETURNING", "USING", "IN", "EXISTS",
    ];
    emit_keyword_subset(out, KEEP);
}

/// NEW and OLD virtual row aliases for trigger function bodies.
pub fn new_old_aliases(out: &mut Vec<Item>) {
    out.push(Item {
        label: "NEW".into(),
        kind: ItemKind::Keyword,
        detail: Some("trigger row (post-change)".into()),
        description: Some("trigger".into()),
        documentation_md: Some(
            "**NEW**\n\nTrigger row variable: the row being inserted or updated. \
             Use `NEW.<column>` to access its fields."
                .into(),
        ),
        insert_text: "NEW".into(),
            is_snippet: false,
            sort_priority: 5,
    });
    out.push(Item {
        label: "OLD".into(),
        kind: ItemKind::Keyword,
        detail: Some("trigger row (pre-change)".into()),
        description: Some("trigger".into()),
        documentation_md: Some(
            "**OLD**\n\nTrigger row variable: the row before UPDATE / DELETE. \
             Use `OLD.<column>` to access its fields."
                .into(),
        ),
        insert_text: "OLD".into(),
            is_snippet: false,
            sort_priority: 5,
    });
}

fn emit_keyword_subset(out: &mut Vec<Item>, names: &[&str]) {
    let table = kb::keywords();
    for name in names {
        if let Some(e) = table.get(*name) {
            out.push(from_entry(name, e, ItemKind::Keyword));
        }
    }
}

pub fn tables(cat: &Catalog, out: &mut Vec<Item>) {
    for t in cat.tables() {
        out.push(table_item(t));
    }
}

/// In-scope FROM/JOIN aliases declared in the current statement. Lets
/// the user complete `u` after writing `FROM users AS u, orders o ...`
/// without having to remember every alias. Each alias item carries the
/// resolved table in its `description`. Items get a high priority bump
/// (1) by the engine so they appear right under in-scope columns.
pub fn aliases_in_scope(scope: &Scope, out: &mut Vec<Item>) {
    for b in scope.tables() {
        // Skip auto-bindings keyed by the bare table name; they aren't
        // really aliases the user typed.
        if b.alias.eq_ignore_ascii_case(&b.table.name) { continue; }
        out.push(Item {
            label: b.alias.clone(),
            kind: ItemKind::Table,
            detail: Some(format!("alias of {}", b.table.name)),
            description: Some(format!("→ {}", b.table.name)),
            documentation_md: Some(format!(
                "**Alias** `{}` → table `{}`\n",
                b.alias, b.table.name,
            )),
            insert_text: b.alias.clone(),
            is_snippet: false,
            sort_priority: 1,
        });
    }
}

/// Emit each column NAME exactly once -- even when the same column
/// (`updated_at`, `created_at`, `id`) appears in many tables, the user
/// only sees one suggestion. The detail field still names the originating
/// table so they can tell where the type came from.
pub fn columns(cat: &Catalog, out: &mut Vec<Item>) {
    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
    for t in cat.tables() {
        for c in &t.columns {
            if !seen.insert(c.name.to_ascii_lowercase()) { continue; }
            out.push(column_item(t, c));
        }
    }
}

/// Catalog-wide columns when the user has no FROM yet. Dedups by column
/// name -- a column that appears in N tables produces one item whose
/// `detail` lists every owner so the user still sees the origins.
pub fn columns_all(cat: &Catalog, out: &mut Vec<Item>) {
    use std::collections::BTreeMap;
    let mut by_name: BTreeMap<String, (Vec<(&Table, &Column)>, &Table, &Column)> = BTreeMap::new();
    for t in cat.tables() {
        for c in &t.columns {
            let key = c.name.to_ascii_lowercase();
            by_name
                .entry(key)
                .and_modify(|e| e.0.push((t, c)))
                .or_insert_with(|| (vec![(t, c)], t, c));
        }
    }
    for (_, (all_owners, first_t, first_c)) in by_name {
        let owners: Vec<String> = all_owners.iter().map(|(t, _)| t.name.clone()).collect();
        let detail = if owners.len() == 1 {
            format!("{}  {}", first_c.data_type, owners[0])
        } else {
            format!("{}  in {} tables: {}", first_c.data_type, owners.len(),
                    owners.join(", "))
        };
        let mut item = column_item(first_t, first_c);
        item.detail = Some(detail);
        out.push(item);
    }
}

pub fn schemas(cat: &Catalog, out: &mut Vec<Item>) {
    for s in &cat.schemas {
        out.push(Item {
            label: s.name.clone(),
            kind: ItemKind::Schema,
            detail: Some("schema".into()),
            description: Some("schema".into()),
            documentation_md: None,
            insert_text: s.name.clone(),
            is_snippet: false,
            sort_priority: 5,
        });
    }
}

/// Emit columns of the table resolved by `alias` in `scope`, if any.
/// Returns the number of items emitted so callers can detect "no match".
pub fn columns_of_alias(
    cat: &Catalog,
    scope: &Scope,
    alias: &str,
    out: &mut Vec<Item>,
) -> usize {
    let Some(b) = scope.get(alias) else { return 0; };
    let table = match cat.find_table(b.table.schema.as_deref(), &b.table.name) {
        Some(t) => t,
        None => return 0,
    };
    let start = out.len();
    for c in &table.columns {
        out.push(column_item(table, c));
    }
    out.len() - start
}

fn table_item(t: &Table) -> Item {
    let detail = format!(
        "{}  {}.{}",
        match t.kind {
            TableKind::View => "view",
            TableKind::MaterializedView => "matview",
            _ => "table",
        },
        t.schema, t.name
    );
    Item {
        label: t.name.clone(),
        kind: if matches!(t.kind, TableKind::View) { ItemKind::View } else { ItemKind::Table },
        detail: Some(detail),
        description: Some(t.schema.clone()),
        documentation_md: Some(render::table(t)),
        insert_text: t.name.clone(),
            is_snippet: false,
            sort_priority: 5,
    }
}

pub fn column_item(t: &Table, c: &Column) -> Item {
    Item {
        label: c.name.clone(),
        kind: ItemKind::Column,
        detail: Some(format!("{}  {}", c.data_type, t.name)),
        description: Some(t.name.clone()),
        documentation_md: Some(render::column(t, c)),
        insert_text: c.name.clone(),
            is_snippet: false,
            sort_priority: 5,
    }
}

fn from_entry(label: &str, e: &Entry, kind: ItemKind) -> Item {
    // For functions, `detail` is the signature (highly informative). For
    // keywords / types the signature is None -- fall back to a short doc
    // excerpt so detail isn't blank. The kind icon already conveys what
    // kind of item this is, so we don't repeat it in `description`.
    let detail = e
        .signature
        .map(|s| s.to_string())
        .or_else(|| {
            let first_line = e.doc.lines().next().unwrap_or("").trim();
            if first_line.is_empty() { None } else { Some(first_line.to_string()) }
        });
    // For built-in functions, emit a snippet template like `length($0)`
    // so the editor inserts `length(<cursor>)` and the user only types
    // the argument. Zero-arg functions still get bare `name()`.
    let (insert_text, is_snippet) = if kind == ItemKind::Function {
        // Empty-paren check: if signature ends in `()` -> no args.
        let zero_args = e.signature
            .map(|s| {
                let trimmed = s.trim_end_matches(|c: char| c == ' ' || c == '\t');
                trimmed.contains("()") || trimmed.ends_with("() -> ")
                    || trimmed.split_once("(")
                        .map(|(_, rest)| rest.trim_start().starts_with(')'))
                        .unwrap_or(false)
            })
            .unwrap_or(false);
        if zero_args {
            (format!("{label}()"), false)
        } else {
            (format!("{label}($0)"), true)
        }
    } else {
        (label.to_string(), false)
    };
    Item {
        label: label.to_string(),
        kind,
        detail,
        description: None,
        documentation_md: Some(kb::render_markdown(e)),
        insert_text,
        is_snippet,
        sort_priority: 5,
    }
}
