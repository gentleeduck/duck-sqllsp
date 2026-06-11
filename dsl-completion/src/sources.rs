//! Item producers, one per kind of source data.
//!
//! Each emitter pushes onto an output vector; the engine decides which
//! emitters to call based on the detected [`Context`](crate::context::Context).

use crate::item::{Item, ItemKind};
use crate::render;
use dsl_catalog::{Catalog, Column, Table, TableKind, display_type};
use dsl_knowledge::{self as kb, Entry};
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
    let doc = format!("**User-defined {kind_label}** `{}.{}`\n\n_From catalog._\n", t.schema, t.name);
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
        Some(n) => format!("{n} {}", display_type(&a.data_type)),
        None => display_type(&a.data_type).to_string(),
      })
      .collect::<Vec<_>>()
      .join(", ");
    let signature = format!("{}({}) -> {}", f.name, args, display_type(&f.return_type));
    // Include the full DDL body when we have it (CREATE FUNCTION ...).
    // Without the fenced code block, editors render the body as plain
    // monospace text -- with it, treesitter/markdown highlighters
    // color the SQL.
    let body_block = f
      .comment
      .as_ref()
      .filter(|c| !c.trim().is_empty())
      .map(|c| {
        let t = c.trim();
        if t.starts_with("```") { format!("\n\n{t}") } else { format!("\n\n```sql\n{t}\n```") }
      })
      .unwrap_or_default();
    let doc = format!(
      "**DB function** `{}.{}`\n\n```sql\n{}\n```{}\n",
      f.schema, f.name, signature, body_block
    );
    let insert_text = if f.arguments.is_empty() { format!("{}()", f.name) } else { format!("{}($0)", f.name) };
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
    "count",
    "sum",
    "avg",
    "min",
    "max",
    "array_agg",
    "string_agg",
    "json_agg",
    "jsonb_agg",
    "coalesce",
    "nullif",
    "greatest",
    "least",
    "lower",
    "upper",
    "length",
    "substring",
    "trim",
    "concat",
    "replace",
    "split_part",
    "now",
    "current_date",
    "age",
    "date_trunc",
    "extract",
    "to_char",
    "abs",
    "round",
    "gen_random_uuid",
    "row_number",
    "rank",
    "dense_rank",
    "lag",
    "lead",
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
    "AS",
    "DISTINCT",
    "ALL",
    "CASE",
    "WHEN",
    "THEN",
    "ELSE",
    "END",
    "AND",
    "OR",
    "NOT",
    "IS NULL",
    "IS NOT NULL",
    "NULL",
    "IS",
    "IN",
    "EXISTS",
    "BETWEEN",
    "LIKE",
    "ILIKE",
    "OVER",
    "PARTITION BY",
  ];
  emit_keyword_subset(out, KEEP);
}

/// Keywords valid at the start of a statement.
pub fn statement_keywords(out: &mut Vec<Item>) {
  const KEEP: &[&str] = &[
    "SELECT",
    "WITH",
    "INSERT INTO",
    "UPDATE",
    "DELETE FROM",
    "CREATE TABLE",
    "CREATE INDEX",
    "CREATE UNIQUE INDEX",
    "CREATE VIEW",
    "CREATE SCHEMA",
    "CREATE SEQUENCE",
    "ALTER TABLE",
    "DROP TABLE",
    "TRUNCATE",
    "EXPLAIN",
    "EXPLAIN ANALYZE",
    "BEGIN",
    "COMMIT",
    "ROLLBACK",
    "SAVEPOINT",
    "GRANT",
    "REVOKE",
    "MERGE",
    "REFRESH",
    "REINDEX",
    "VACUUM",
    "ANALYZE",
    "SET",
    "SHOW",
    "COMMENT",
    "COPY",
    "CALL",
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
    insert_text: "ALTER TABLE ${1:table} ADD COLUMN ${2:name} ${3:type} ${4|NOT NULL,NULL|}${5: DEFAULT ${6:expr}};$0"
      .into(),
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
    insert_text: "COPY (SELECT ${1:*} FROM ${2:table})\nTO '${3:/path/to/file.csv}'\nWITH (FORMAT csv, HEADER true);$0"
      .into(),
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
    "INNER JOIN",
    "LEFT JOIN",
    "RIGHT JOIN",
    "FULL OUTER JOIN",
    "CROSS JOIN",
    "JOIN",
    "ON",
    "USING",
    "AS",
    "LATERAL",
    "WHERE",
    "GROUP BY",
    "HAVING",
    "ORDER BY",
    "LIMIT",
    "OFFSET",
    "UNION",
    "INTERSECT",
    "EXCEPT",
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
  const KEEP: &[&str] =
    &["NOT NULL", "NULL", "DEFAULT", "PRIMARY KEY", "UNIQUE", "CHECK", "REFERENCES", "GENERATED", "COLLATE"];
  emit_keyword_subset(out, KEEP);
}

/// Starter keywords that may begin an entry inside a CREATE TABLE body
/// in lieu of a fresh column name.
pub fn create_table_entry_starters(out: &mut Vec<Item>) {
  const KEEP: &[&str] = &["PRIMARY KEY", "FOREIGN KEY", "UNIQUE", "CHECK", "LIKE"];
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
             3. column list"
        .into(),
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

/// Privilege keywords valid after `GRANT` / `REVOKE`. Mirrors the
/// PG GRANT grammar. ALL [PRIVILEGES] short-hand is offered first
/// so a quick "GRANT ALL ..." stays one keystroke away.
pub fn grant_privileges(out: &mut Vec<Item>) {
  const PRIVS: &[(&str, &str)] = &[
    ("ALL PRIVILEGES", "every privilege the role is allowed to grant"),
    ("SELECT", "read rows / call function / read sequence value"),
    ("INSERT", "add new rows"),
    ("UPDATE", "modify existing rows"),
    ("DELETE", "remove rows"),
    ("TRUNCATE", "TRUNCATE table"),
    ("REFERENCES", "create FK referencing the table/column"),
    ("TRIGGER", "create trigger on the table"),
    ("USAGE", "use schema / sequence / language / type"),
    ("EXECUTE", "call a function or procedure"),
    ("CREATE", "create objects in the database/schema"),
    ("CONNECT", "connect to the database"),
    ("TEMPORARY", "create temporary tables in the database"),
    ("MAINTAIN", "PG17+ -- VACUUM/ANALYZE/REINDEX/REFRESH MV/CLUSTER/LOCK TABLE without owning the table"),
  ];
  for (label, doc) in PRIVS {
    out.push(Item {
      label: (*label).into(),
      kind: ItemKind::Keyword,
      detail: Some((*doc).into()),
      description: Some("GRANT / REVOKE".into()),
      documentation_md: None,
      insert_text: (*label).into(),
      is_snippet: false,
      sort_priority: 0,
    });
  }
  out.push(Item {
    label: "ON".into(),
    kind: ItemKind::Keyword,
    detail: Some("introduces the target object list".into()),
    description: Some("GRANT / REVOKE".into()),
    documentation_md: None,
    insert_text: "ON".into(),
    is_snippet: false,
    sort_priority: 1,
  });
}

/// Object-class keywords valid after `GRANT ... ON`. Real catalog
/// targets are emitted alongside by the engine.
pub fn grant_object_classes(out: &mut Vec<Item>) {
  const CLASSES: &[(&str, &str)] = &[
    ("TABLE", "ON TABLE <name>[, ...]"),
    ("ALL TABLES IN SCHEMA", "ON ALL TABLES IN SCHEMA <schema>[, ...]"),
    ("SEQUENCE", "ON SEQUENCE <name>[, ...]"),
    ("ALL SEQUENCES IN SCHEMA", "ON ALL SEQUENCES IN SCHEMA <schema>[, ...]"),
    ("FUNCTION", "ON FUNCTION <name>(<args>)[, ...]"),
    ("ALL FUNCTIONS IN SCHEMA", "ON ALL FUNCTIONS IN SCHEMA <schema>[, ...]"),
    ("PROCEDURE", "ON PROCEDURE <name>[, ...]"),
    ("SCHEMA", "ON SCHEMA <name>[, ...]"),
    ("DATABASE", "ON DATABASE <name>[, ...]"),
    ("LANGUAGE", "ON LANGUAGE <name>[, ...]"),
    ("DOMAIN", "ON DOMAIN <name>[, ...]"),
    ("TYPE", "ON TYPE <name>[, ...]"),
    ("FOREIGN DATA WRAPPER", "ON FOREIGN DATA WRAPPER <name>"),
    ("FOREIGN SERVER", "ON FOREIGN SERVER <name>"),
  ];
  for (label, detail) in CLASSES {
    out.push(Item {
      label: (*label).into(),
      kind: ItemKind::Keyword,
      detail: Some((*detail).into()),
      description: Some("GRANT ... ON".into()),
      documentation_md: None,
      insert_text: (*label).into(),
      is_snippet: false,
      sort_priority: 0,
    });
  }
}

/// Role names valid after `GRANT ... TO` / `REVOKE ... FROM`. Pulled
/// from the live catalog plus the `PUBLIC` pseudo-role. Empty live
/// catalog -> only the four built-in / convention names so the menu
/// still surfaces something useful offline.
pub fn grant_roles(cat: &Catalog, out: &mut Vec<Item>) {
  let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
  out.push(Item {
    label: "PUBLIC".into(),
    kind: ItemKind::Keyword,
    detail: Some("every existing role and every future role".into()),
    description: Some("pseudo-role".into()),
    documentation_md: None,
    insert_text: "PUBLIC".into(),
    is_snippet: false,
    sort_priority: 1,
  });
  seen.insert("PUBLIC".into());
  for r in &cat.roles {
    if seen.insert(r.to_ascii_uppercase()) {
      out.push(Item {
        label: r.clone(),
        kind: ItemKind::Variable,
        detail: Some("role".into()),
        description: Some("catalog".into()),
        documentation_md: None,
        insert_text: r.clone(),
        is_snippet: false,
        sort_priority: 0,
      });
    }
  }
  // Offline / freshly-introspected DB: surface common defaults so
  // the menu isn't a one-item PUBLIC stub.
  for fallback in ["postgres", "pg_read_all_data", "pg_write_all_data"] {
    if seen.insert(fallback.to_ascii_uppercase()) {
      out.push(Item {
        label: fallback.into(),
        kind: ItemKind::Variable,
        detail: Some("built-in role".into()),
        description: Some("default".into()),
        documentation_md: None,
        insert_text: fallback.into(),
        is_snippet: false,
        sort_priority: 5,
      });
    }
  }
}

/// Sub-actions valid after `ALTER TABLE <name> `. Each item is a
/// snippet so the user lands inside the relevant placeholder. List
/// is curated from the PG13+ ALTER TABLE grammar -- the cardinality
/// is too high to dump every variation, so we ship the high-value
/// ones and let users fall back to free-form typing for the rest.
pub fn alter_table_actions(out: &mut Vec<Item>) {
  let actions: &[(&str, &str, &str)] = &[
    ("ADD COLUMN", "add a column", "ADD COLUMN ${1:name} ${2:type}${3| NOT NULL,|}${4: DEFAULT ${5:expr}}"),
    ("DROP COLUMN", "drop a column", "DROP COLUMN ${1:name}${2| CASCADE,|}"),
    ("RENAME COLUMN", "rename a column", "RENAME COLUMN ${1:old} TO ${2:new}"),
    (
      "ALTER COLUMN",
      "modify a column",
      "ALTER COLUMN ${1:name} ${2|TYPE ${3:type},SET DEFAULT ${4:expr},DROP DEFAULT,SET NOT NULL,DROP NOT NULL|}",
    ),
    ("RENAME TO", "rename the table", "RENAME TO ${1:new_name}"),
    ("SET SCHEMA", "move to a different schema", "SET SCHEMA ${1:schema}"),
    (
      "ADD CONSTRAINT",
      "add a named constraint",
      "ADD CONSTRAINT ${1:name} ${2|PRIMARY KEY (${3:cols}),UNIQUE (${3:cols}),FOREIGN KEY (${4:col}) REFERENCES ${5:tbl}(${6:col}),CHECK (${7:expr})|}",
    ),
    ("DROP CONSTRAINT", "drop a constraint", "DROP CONSTRAINT ${1:name}${2| CASCADE,|}"),
    ("OWNER TO", "change owner", "OWNER TO ${1:role}"),
    ("ENABLE", "enable trigger / RLS", "ENABLE ${1|TRIGGER ${2:name},ROW LEVEL SECURITY|}"),
    ("DISABLE", "disable trigger / RLS", "DISABLE ${1|TRIGGER ${2:name},ROW LEVEL SECURITY|}"),
    (
      "ATTACH PARTITION",
      "attach a partition",
      "ATTACH PARTITION ${1:partition} ${2|FOR VALUES IN (${3:list}),FOR VALUES FROM (${4:lo}) TO (${5:hi}),DEFAULT|}",
    ),
    ("DETACH PARTITION", "detach a partition", "DETACH PARTITION ${1:partition}"),
    ("INHERIT", "add parent table", "INHERIT ${1:parent}"),
    ("NO INHERIT", "remove parent table", "NO INHERIT ${1:parent}"),
    ("SET TABLESPACE", "move table to tablespace", "SET TABLESPACE ${1:tablespace}"),
    ("CLUSTER ON", "set CLUSTER index", "CLUSTER ON ${1:index_name}"),
    ("SET WITHOUT CLUSTER", "remove CLUSTER", "SET WITHOUT CLUSTER"),
    ("ENABLE TRIGGER", "enable a trigger by name (or ALL / USER)", "ENABLE TRIGGER ${1:name}"),
    ("DISABLE TRIGGER", "disable a trigger by name (or ALL / USER)", "DISABLE TRIGGER ${1:name}"),
    ("ENABLE ROW LEVEL SECURITY", "turn RLS on for the table", "ENABLE ROW LEVEL SECURITY"),
    ("DISABLE ROW LEVEL SECURITY", "turn RLS off for the table", "DISABLE ROW LEVEL SECURITY"),
    ("FORCE ROW LEVEL SECURITY", "apply RLS even to the table owner", "FORCE ROW LEVEL SECURITY"),
    ("NO FORCE ROW LEVEL SECURITY", "revert FORCE RLS (default)", "NO FORCE ROW LEVEL SECURITY"),
    ("ENABLE REPLICA TRIGGER", "ENABLE REPLICA TRIGGER -- fire only when in replica role", "ENABLE REPLICA TRIGGER ${1:name}"),
    ("ENABLE ALWAYS TRIGGER", "ENABLE ALWAYS TRIGGER -- fire even in replica role", "ENABLE ALWAYS TRIGGER ${1:name}"),
    ("REPLICA IDENTITY", "REPLICA IDENTITY {DEFAULT|FULL|USING INDEX <ix>|NOTHING}", "REPLICA IDENTITY ${1|DEFAULT,FULL,NOTHING|}"),
    ("RESET", "RESET (<storage_param>[, ...])", "RESET (${1:storage_param})"),
    ("SET", "SET (<storage_param> = <value>[, ...])", "SET (${1:storage_param} = ${2:value})"),
    ("OF", "OF <type> -- bind table to a composite type", "OF ${1:type_name}"),
    ("NOT OF", "NOT OF -- detach composite type binding", "NOT OF"),
    ("VALIDATE CONSTRAINT", "validate a previously NOT VALID constraint", "VALIDATE CONSTRAINT ${1:name}"),
    ("OPTIONS", "FOREIGN TABLE OPTIONS only -- key/value list edit", "OPTIONS (${1:key} '${2:value}')"),
    ("SET ACCESS METHOD", "switch table access method (PG12+)", "SET ACCESS METHOD ${1:method}"),
    ("SET LOGGED", "make an UNLOGGED table logged", "SET LOGGED"),
    ("SET UNLOGGED", "make a table UNLOGGED", "SET UNLOGGED"),
  ];
  for (label, detail, snippet) in actions {
    out.push(Item {
      label: (*label).into(),
      kind: ItemKind::Keyword,
      detail: Some((*detail).into()),
      description: Some("ALTER TABLE".into()),
      documentation_md: None,
      insert_text: (*snippet).into(),
      is_snippet: true,
      sort_priority: 0,
    });
  }
}

/// All known SQL types -- used inside CREATE TABLE column-type position.
pub fn types_only(out: &mut Vec<Item>) {
  for (label, e) in kb::types() {
    out.push(from_entry(label, e, ItemKind::Type));
  }
}

/// Columns of a specific catalog table only.
pub fn columns_of_table(cat: &Catalog, schema: Option<&str>, table_name: &str, out: &mut Vec<Item>) {
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
    "DECLARE",
    "BEGIN",
    "END",
    "IF",
    "ELSIF",
    "ELSE",
    "THEN",
    "LOOP",
    "WHILE",
    "FOR",
    "EXIT",
    "CONTINUE",
    "PERFORM",
    // Return + exception handling.
    "RETURN",
    "RETURNS",
    "RAISE",
    "NOTICE",
    "WARNING",
    "EXCEPTION",
    "FOUND",
    "STRICT",
    "INTO STRICT",
    // SQL inside the body.
    "EXECUTE",
    "SELECT",
    "INSERT INTO",
    "UPDATE",
    "DELETE FROM",
    "WHERE",
    "AND",
    "OR",
    "NOT",
    "IS NULL",
    "IS NOT NULL",
    "AS",
    "FROM",
    "JOIN",
    "ON",
    "GROUP BY",
    "ORDER BY",
    "LIMIT",
    "VALUES",
    "SET",
    "RETURNING",
    "USING",
    "IN",
    "EXISTS",
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

/// Emit tables/views belonging to the named schema. Used by dot
/// completion when the user types `<schema>.` and we don't have a
/// matching alias in scope -- the next valid token in that position is
/// a relation name in that schema. Returns the number of items emitted.
pub fn tables_in_schema(cat: &Catalog, schema_name: &str, out: &mut Vec<Item>) -> usize {
  let start = out.len();
  for s in &cat.schemas {
    if s.name.eq_ignore_ascii_case(schema_name) {
      for t in &s.tables {
        out.push(table_item(t));
      }
    }
  }
  out.len() - start
}

/// Emit every function declared in the given schema. Mirror of
/// [`tables_in_schema`] but for the function namespace: invoked when
/// the user types `schema.<cursor>` and the dot handler wants to
/// surface `schema.fn(...)` candidates alongside `schema.table` ones.
pub fn functions_in_schema(cat: &Catalog, schema_name: &str, out: &mut Vec<Item>) -> usize {
  let start = out.len();
  for f in &cat.functions {
    if f.schema.eq_ignore_ascii_case(schema_name) {
      let detail = if f.return_type.is_empty() {
        "function".to_string()
      } else {
        format!("function -> {}", f.return_type)
      };
      // Wrap the raw DDL in a fenced sql code block so the editor's
      // markdown renderer picks it up for syntax-highlighted display.
      // Without the fence the body shows as plain monospace text.
      let documentation_md = f.comment.as_ref().map(|c| {
        let trimmed = c.trim();
        if trimmed.starts_with("```") {
          c.clone()
        } else {
          format!("```sql\n{trimmed}\n```")
        }
      });
      out.push(Item {
        label: f.name.clone(),
        kind: ItemKind::Function,
        detail: Some(detail),
        description: if f.return_type.is_empty() { None } else { Some(f.return_type.clone()) },
        documentation_md,
        insert_text: format!("{}(", f.name),
        is_snippet: false,
        sort_priority: 0,
      });
    }
  }
  out.len() - start
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
    if b.alias.eq_ignore_ascii_case(&b.table.name) {
      continue;
    }
    out.push(Item {
      label: b.alias.clone(),
      kind: ItemKind::Table,
      detail: Some(format!("alias of {}", b.table.name)),
      description: Some(format!("→ {}", b.table.name)),
      documentation_md: Some(format!("**Alias** `{}` → table `{}`\n", b.alias, b.table.name,)),
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
      if !seen.insert(c.name.to_ascii_lowercase()) {
        continue;
      }
      out.push(column_item(t, c));
    }
  }
}

/// Catalog-wide columns when the user has no FROM yet. Dedups by column
/// name -- a column that appears in N tables produces one item whose
/// `detail` lists every owner so the user still sees the origins.
pub fn columns_all(cat: &Catalog, out: &mut Vec<Item>) {
  use std::collections::BTreeMap;
  #[allow(clippy::type_complexity)]
  let mut by_name: BTreeMap<String, (Vec<(&Table, &Column)>, &Table, &Column)> = BTreeMap::new();
  for t in cat.tables() {
    for c in &t.columns {
      let key = c.name.to_ascii_lowercase();
      by_name.entry(key).and_modify(|e| e.0.push((t, c))).or_insert_with(|| (vec![(t, c)], t, c));
    }
  }
  for (_, (all_owners, first_t, first_c)) in by_name {
    let owners: Vec<String> = all_owners.iter().map(|(t, _)| t.name.clone()).collect();
    let detail = if owners.len() == 1 {
      format!("{}  {}", display_type(&first_c.data_type), owners[0])
    } else {
      format!("{}  in {} tables: {}", display_type(&first_c.data_type), owners.len(), owners.join(", "))
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
pub fn columns_of_alias(cat: &Catalog, scope: &Scope, alias: &str, out: &mut Vec<Item>) -> usize {
  let Some(b) = scope.get(alias) else {
    return 0;
  };
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
  let base = match t.kind {
    TableKind::View => "view",
    TableKind::MaterializedView => "matview",
    TableKind::WithoutRowid => "table (no rowid)",
    TableKind::Table => "table",
  };
  // STRICT composes with any table kind; surface it as a trailing tag.
  let label = if t.strict { format!("{base}, strict") } else { base.to_string() };
  let detail = format!("{label}  {}.{}", t.schema, t.name);
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
    detail: Some(format!("{}  {}", display_type(&c.data_type), t.name)),
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
  let detail = e.signature.map(|s| s.to_string()).or_else(|| {
    let first_line = e.doc.lines().next().unwrap_or("").trim();
    if first_line.is_empty() { None } else { Some(first_line.to_string()) }
  });
  // For built-in functions, emit a snippet template like `length($0)`
  // so the editor inserts `length(<cursor>)` and the user only types
  // the argument. Zero-arg functions still get bare `name()`.
  let (insert_text, is_snippet) = if kind == ItemKind::Function {
    // Empty-paren check: if signature ends in `()` -> no args.
    let zero_args = e
      .signature
      .map(|s| {
        let trimmed = s.trim_end_matches([' ', '\t']);
        trimmed.contains("()")
          || trimmed.ends_with("() -> ")
          || trimmed.split_once("(").map(|(_, rest)| rest.trim_start().starts_with(')')).unwrap_or(false)
      })
      .unwrap_or(false);
    if zero_args { (format!("{label}()"), false) } else { (format!("{label}($0)"), true) }
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
