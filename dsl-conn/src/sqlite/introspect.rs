//! SQLite schema introspection.
//!
//! SQLite has no `information_schema` worth using. We scan `sqlite_master`
//! for objects then `PRAGMA table_info` / `index_list` / `foreign_key_list`
//! to fill columns, indexes, and FK constraints.

use crate::driver::DriverError;
use crate::spec::ConnectionSpec;
use dsl_catalog::{
  CATALOG_VERSION, Catalog, Column, Constraint, ConstraintKind, ConstraintRef, IndexDef, Schema, Table, TableKind,
  Trigger,
};
use sqlx::{Row, SqlitePool};

const SCHEMA_NAME: &str = "main";

pub async fn run(pool: &SqlitePool, spec: &ConnectionSpec) -> Result<Catalog, DriverError> {
  let mut tables: Vec<Table> = Vec::new();
  let mut triggers_by_table: std::collections::BTreeMap<String, Vec<Trigger>> = Default::default();

  // Collect objects. `sql` carries the original CREATE text -- the only
  // place SQLite records generated-column expressions.
  let objs = sqlx::query("SELECT type, name, tbl_name, sql FROM sqlite_master WHERE type IN ('table','view','trigger') AND name NOT LIKE 'sqlite_%' ORDER BY type, name")
        .fetch_all(pool)
        .await
        .map_err(io_err)?;

  for row in &objs {
    let kind: String = row.try_get("type").map_err(io_err)?;
    let name: String = row.try_get("name").map_err(io_err)?;
    let tbl_name: String = row.try_get("tbl_name").map_err(io_err)?;
    let create_sql: Option<String> = row.try_get("sql").ok();
    match kind.as_str() {
      "table" | "view" => {
        let table_kind = if kind == "view" {
          TableKind::View
        } else if create_sql.as_deref().is_some_and(is_without_rowid) {
          TableKind::WithoutRowid
        } else {
          TableKind::Table
        };
        let mut columns = fetch_columns(pool, &name).await?;
        // `PRAGMA table_info` omits the generation expression, so fold it
        // in from the CREATE TABLE text. Stored bare (no outer parens) to
        // match the offline scanner / PG / MySQL convention.
        if let Some(sql) = &create_sql {
          let gen_map = parse_generated_columns(sql);
          if !gen_map.is_empty() {
            for col in columns.iter_mut() {
              if let Some(expr) = gen_map.get(&col.name.to_ascii_lowercase()) {
                col.generated = Some(expr.clone());
              }
            }
          }
        }
        let indexes = fetch_indexes(pool, &name).await?;
        let mut constraints = fetch_key_constraints(pool, &name).await?;
        constraints.extend(fetch_fks(pool, &name).await?);
        // CHECK constraints aren't exposed by any PRAGMA -- parse them out of
        // the CREATE TABLE text (the same source the generated-column and
        // WITHOUT ROWID detection use).
        if let Some(sql) = &create_sql {
          constraints.extend(parse_check_constraints(sql, &name));
        }
        // For views, pull the SELECT body out of the CREATE VIEW text so it
        // matches the bare-query form PG/MySQL store in `definition`.
        let definition = if kind == "view" { create_sql.as_deref().and_then(view_body) } else { None };
        let strict = kind == "table" && create_sql.as_deref().is_some_and(is_strict);
        tables.push(Table {
          schema: SCHEMA_NAME.into(),
          name,
          kind: table_kind,
          columns,
          constraints,
          indexes,
          triggers: Vec::new(),
          policies: Vec::new(),
          comment: None,
          row_estimate: None,
          owner: None,
          definition,
          strict,
          options: None,
        });
      },
      "trigger" => {
        // Trigger metadata in sqlite_master is sparse; we record name + parent.
        triggers_by_table.entry(tbl_name).or_default().push(Trigger {
          name,
          timing: String::new(),
          event: String::new(),
          granularity: String::new(),
          function: String::new(),
        });
      },
      _ => {},
    }
  }

  // Attach triggers to their tables.
  for t in tables.iter_mut() {
    if let Some(trs) = triggers_by_table.remove(&t.name) {
      t.triggers = trs;
    }
  }

  let schema = Schema { name: SCHEMA_NAME.into(), tables };

  Ok(Catalog {
    version: CATALOG_VERSION,
    connection_id: spec.name.clone(),
    schemas: vec![schema],
    functions: Vec::new(),
    types: Vec::new(),
    roles: Vec::new(),
    sequences: Vec::new(), // SQLite uses sqlite_sequence rows, not first-class sequence objects.
    extensions: Vec::new(),
  })
}

async fn fetch_columns(pool: &SqlitePool, table: &str) -> Result<Vec<Column>, DriverError> {
  let q = format!("PRAGMA table_info({})", quote_ident(table));
  let rows = sqlx::query(&q).fetch_all(pool).await.map_err(io_err)?;
  let mut out = Vec::with_capacity(rows.len());
  for row in rows {
    let name: String = row.try_get("name").map_err(io_err)?;
    let data_type: String = row.try_get("type").map_err(io_err)?;
    let notnull: i64 = row.try_get("notnull").map_err(io_err)?;
    let default: Option<String> = row.try_get("dflt_value").ok();
    out.push(Column {
      name,
      data_type,
      nullable: notnull == 0,
      default,
      comment: None,
      generated: None,
      json_keys: None,
    });
  }
  Ok(out)
}

async fn fetch_indexes(pool: &SqlitePool, table: &str) -> Result<Vec<IndexDef>, DriverError> {
  let q = format!("PRAGMA index_list({})", quote_ident(table));
  let list = sqlx::query(&q).fetch_all(pool).await.map_err(io_err)?;
  let mut out = Vec::with_capacity(list.len());
  for row in list {
    let iname: String = row.try_get("name").map_err(io_err)?;
    let unique: i64 = row.try_get("unique").map_err(io_err)?;
    let info_q = format!("PRAGMA index_info({})", quote_ident(&iname));
    let info = sqlx::query(&info_q).fetch_all(pool).await.map_err(io_err)?;
    let cols: Vec<String> = info.into_iter().filter_map(|r| r.try_get::<String, _>("name").ok()).collect();
    out.push(IndexDef { name: iname, columns: cols, unique: unique != 0, definition: None });
  }
  Ok(out)
}

/// PRIMARY KEY and UNIQUE constraints. SQLite exposes neither through a
/// constraint catalog, so we reconstruct them: the PK from the `pk` column
/// of `PRAGMA table_info` (non-zero = 1-based position in the key), and the
/// UNIQUE set from `PRAGMA index_list` rows whose `origin` is `'u'` (a
/// `UNIQUE` constraint) -- as opposed to `'pk'` (the implicit PK index) or
/// `'c'` (a plain `CREATE INDEX`).
async fn fetch_key_constraints(pool: &SqlitePool, table: &str) -> Result<Vec<Constraint>, DriverError> {
  let mut out = Vec::new();

  // PRIMARY KEY -- collect (position, column) where pk > 0, ordered by position.
  let info_q = format!("PRAGMA table_info({})", quote_ident(table));
  let mut pk: Vec<(i64, String)> = Vec::new();
  for row in sqlx::query(&info_q).fetch_all(pool).await.map_err(io_err)? {
    let pos: i64 = row.try_get("pk").map_err(io_err)?;
    if pos > 0 {
      let name: String = row.try_get("name").map_err(io_err)?;
      pk.push((pos, name));
    }
  }
  if !pk.is_empty() {
    pk.sort_by_key(|(pos, _)| *pos);
    out.push(Constraint {
      name: format!("{}_pkey", table),
      kind: ConstraintKind::PrimaryKey,
      columns: pk.into_iter().map(|(_, c)| c).collect(),
      references: None,
      definition: None,
      inline: false,
    });
  }

  // UNIQUE -- one constraint per index whose origin is a UNIQUE clause.
  let list_q = format!("PRAGMA index_list({})", quote_ident(table));
  for row in sqlx::query(&list_q).fetch_all(pool).await.map_err(io_err)? {
    let origin: String = row.try_get("origin").unwrap_or_default();
    if origin != "u" {
      continue;
    }
    let iname: String = row.try_get("name").map_err(io_err)?;
    let info_q = format!("PRAGMA index_info({})", quote_ident(&iname));
    let cols: Vec<String> = sqlx::query(&info_q)
      .fetch_all(pool)
      .await
      .map_err(io_err)?
      .into_iter()
      .filter_map(|r| r.try_get::<String, _>("name").ok())
      .collect();
    out.push(Constraint {
      name: iname,
      kind: ConstraintKind::Unique,
      columns: cols,
      references: None,
      definition: None,
      inline: false,
    });
  }

  Ok(out)
}

async fn fetch_fks(pool: &SqlitePool, table: &str) -> Result<Vec<Constraint>, DriverError> {
  let q = format!("PRAGMA foreign_key_list({})", quote_ident(table));
  let rows = sqlx::query(&q).fetch_all(pool).await.map_err(io_err)?;
  use std::collections::BTreeMap;
  let mut acc: BTreeMap<i64, (Vec<String>, String, Vec<String>)> = BTreeMap::new();
  for row in rows {
    let id: i64 = row.try_get("id").map_err(io_err)?;
    let from: String = row.try_get("from").map_err(io_err)?;
    let to: String = row.try_get("to").map_err(io_err)?;
    let ref_table: String = row.try_get("table").map_err(io_err)?;
    let entry = acc.entry(id).or_insert((Vec::new(), ref_table, Vec::new()));
    entry.0.push(from);
    entry.2.push(to);
  }
  Ok(
    acc
      .into_iter()
      .map(|(id, (cols, ref_table, ref_cols))| Constraint {
        name: format!("fk_{}_{}", table, id),
        kind: ConstraintKind::ForeignKey,
        columns: cols,
        references: Some(ConstraintRef { schema: SCHEMA_NAME.into(), table: ref_table, columns: ref_cols }),
        definition: None,
        inline: false,
      })
      .collect(),
  )
}

fn quote_ident(s: &str) -> String {
  format!("\"{}\"", s.replace('"', "\"\""))
}

/// Parse generated-column expressions out of a `CREATE TABLE` statement.
/// Returns a map of lowercased column name -> bare expression (no outer
/// parens). Handles both `GENERATED ALWAYS AS (expr)` and the shorthand
/// `col type AS (expr)`, with `STORED` / `VIRTUAL` trailing either form.
///
/// SQLite exposes generated columns through `PRAGMA table_info` but not the
/// expression, so the CREATE text is the only source.
fn parse_generated_columns(create_sql: &str) -> std::collections::HashMap<String, String> {
  let mut out = std::collections::HashMap::new();
  let bytes = create_sql.as_bytes();

  // Body = the column list between the first top-level '(' and its match.
  let Some(open) = bytes.iter().position(|&b| b == b'(') else { return out };
  let Some(close) = match_paren(bytes, open) else { return out };
  let body = &create_sql[open + 1..close];

  for part in split_top_level(body) {
    let def = part.trim();
    // First token is the column name (a constraint def starts with a
    // keyword like PRIMARY / UNIQUE / CHECK / FOREIGN / CONSTRAINT -- those
    // never carry a generated expression so a missing match just skips).
    let (col_name, rest) = match split_first_ident(def) {
      Some(v) => v,
      None => continue,
    };
    // Find a top-level `AS` keyword followed by `(`. `GENERATED ALWAYS AS`
    // reduces to the same `AS (` once we scan word-by-word.
    if let Some(expr) = find_generated_expr(rest) {
      out.insert(col_name.to_ascii_lowercase(), expr);
    }
  }
  out
}

/// Extract the SELECT body from a `CREATE VIEW name [(cols)] AS <select>`
/// statement -- everything after the first top-level `AS`, with any trailing
/// `;` trimmed. The optional column list is parenthesised, so its contents
/// (and any `AS` in the SELECT's own column aliases) sit at depth > 0 / after
/// the view's `AS` and never match first. Returns `None` if no `AS` is found.
fn view_body(create_sql: &str) -> Option<String> {
  let bytes = create_sql.as_bytes();
  let is_ident = |b: u8| b.is_ascii_alphanumeric() || b == b'_';
  let mut i = 0usize;
  let mut depth = 0i32;
  while i < bytes.len() {
    match bytes[i] {
      b'\'' | b'"' | b'`' => {
        let q = bytes[i];
        i += 1;
        while i < bytes.len() && bytes[i] != q {
          i += 1;
        }
        i += 1;
      },
      b'(' => {
        depth += 1;
        i += 1;
      },
      b')' => {
        depth -= 1;
        i += 1;
      },
      b'a' | b'A' if depth == 0 => {
        let prev_boundary = i == 0 || !is_ident(bytes[i - 1]);
        let is_as = bytes.get(i + 1).is_some_and(|b| b.eq_ignore_ascii_case(&b's'));
        let next_boundary = bytes.get(i + 2).is_none_or(|&b| !is_ident(b));
        if prev_boundary && is_as && next_boundary {
          let body = create_sql[i + 2..].trim();
          let body = body.trim_end_matches(';').trim();
          return if body.is_empty() { None } else { Some(body.to_string()) };
        }
        i += 1;
      },
      _ => i += 1,
    }
  }
  None
}

/// The trailing table-options clause of a `CREATE TABLE`, upper-cased and
/// single-spaced. These options (`WITHOUT ROWID`, `STRICT`) live *after* the
/// balanced column-list parens, so we skip the column list first -- this is
/// what stops a column literally named `rowid` or `strict`, or a `WITHOUT`
/// inside the body, from being mistaken for a table option.
fn table_options(create_sql: &str) -> String {
  let bytes = create_sql.as_bytes();
  let Some(open) = bytes.iter().position(|&b| b == b'(') else { return String::new() };
  let Some(close) = match_paren(bytes, open) else { return String::new() };
  create_sql[close + 1..].to_ascii_uppercase().split_whitespace().collect::<Vec<_>>().join(" ")
}

/// True when a `CREATE TABLE` carries the `WITHOUT ROWID` option.
fn is_without_rowid(create_sql: &str) -> bool {
  table_options(create_sql).contains("WITHOUT ROWID")
}

/// True when a `CREATE TABLE` carries the `STRICT` option. Tokenised on
/// whitespace / commas so `STRICT` matches as a whole word and never as a
/// fragment of another option.
fn is_strict(create_sql: &str) -> bool {
  table_options(create_sql).split([' ', ',']).any(|t| t == "STRICT")
}

/// Pull the column name off the front of a column definition, returning
/// (name, remainder). Handles `"x"`, `[x]`, `` `x` ``, and bare identifiers.
fn split_first_ident(def: &str) -> Option<(String, &str)> {
  let bytes = def.as_bytes();
  let first = *bytes.first()?;
  let (close_ch, start) = match first {
    b'"' => (b'"', 1),
    b'`' => (b'`', 1),
    b'[' => (b']', 1),
    _ => {
      // Bare identifier: up to first whitespace.
      let end = def.find(|c: char| c.is_whitespace()).unwrap_or(def.len());
      if end == 0 {
        return None;
      }
      return Some((def[..end].to_string(), &def[end..]));
    },
  };
  let rel = def[start..].find(close_ch as char)?;
  let name = def[start..start + rel].to_string();
  Some((name, &def[start + rel + 1..]))
}

/// Locate a word-boundaried keyword `kw` at top level (outside any parens or
/// quoted run) that is immediately followed by a parenthesised group, and
/// return the byte range of that group *including* its parens
/// (`s[open..=close]`). Used to pull the expression off `AS (...)` (generated
/// columns) and `CHECK (...)` (check constraints) without tripping on the
/// keyword appearing inside a string literal, an identifier, or a nested
/// expression.
fn find_kw_parens(s: &str, kw: &str) -> Option<(usize, usize)> {
  let bytes = s.as_bytes();
  let kw = kw.as_bytes();
  let is_ident = |b: u8| b.is_ascii_alphanumeric() || b == b'_';
  let mut i = 0usize;
  let mut depth = 0i32;
  while i < bytes.len() {
    match bytes[i] {
      b'\'' | b'"' | b'`' => {
        // Skip quoted runs so the keyword inside a literal can't match.
        let q = bytes[i];
        i += 1;
        while i < bytes.len() && bytes[i] != q {
          i += 1;
        }
        i += 1;
      },
      b'(' => {
        depth += 1;
        i += 1;
      },
      b')' => {
        depth -= 1;
        i += 1;
      },
      _ if depth == 0 => {
        let matches_kw = i + kw.len() <= bytes.len()
          && bytes[i..i + kw.len()].iter().zip(kw).all(|(a, b)| a.eq_ignore_ascii_case(b));
        let prev_boundary = i == 0 || !is_ident(bytes[i - 1]);
        let next_boundary = bytes.get(i + kw.len()).is_none_or(|&b| !is_ident(b));
        if matches_kw && prev_boundary && next_boundary {
          let mut j = i + kw.len();
          while j < bytes.len() && bytes[j].is_ascii_whitespace() {
            j += 1;
          }
          if bytes.get(j) == Some(&b'(') {
            let close = match_paren(bytes, j)?;
            return Some((j, close));
          }
        }
        i += 1;
      },
      _ => i += 1,
    }
  }
  None
}

/// Scan a column definition tail for a generated expression: a word-boundaried
/// `AS` keyword immediately followed by a parenthesised expression. Returns
/// the bare inner expression (no surrounding parens). `GENERATED ALWAYS AS`
/// reduces to the same `AS (...)` match.
fn find_generated_expr(s: &str) -> Option<String> {
  let (open, close) = find_kw_parens(s, "AS")?;
  Some(s[open + 1..close].trim().to_string())
}

/// CHECK constraints declared in a `CREATE TABLE` body -- both table-level
/// (`CHECK (...)` / `CONSTRAINT name CHECK (...)`) and inline column-level
/// (`col TYPE CHECK (...)`). SQLite keeps no constraint catalog, so the CREATE
/// text is the only source. `definition` matches the convention used by the
/// offline source scanner: the full `CHECK (...)` fragment for table-level
/// constraints, and the bare `(...)` body for inline ones (the hover renderer
/// prepends `CHECK` itself when folding inline constraints onto the column).
fn parse_check_constraints(create_sql: &str, table: &str) -> Vec<Constraint> {
  let mut out = Vec::new();
  let bytes = create_sql.as_bytes();
  let Some(open) = bytes.iter().position(|&b| b == b'(') else { return out };
  let Some(close) = match_paren(bytes, open) else { return out };
  let body = &create_sql[open + 1..close];

  let mut anon = 0u32;
  for part in split_top_level(body) {
    let def = part.trim();
    if def.is_empty() {
      continue;
    }
    let upper = def.to_ascii_uppercase();

    // Named constraint: `CONSTRAINT <name> CHECK (...)`.
    if upper.starts_with("CONSTRAINT")
      && let Some((cname, rest)) = split_first_ident(def["CONSTRAINT".len()..].trim_start())
      && let Some((o, c)) = find_kw_parens(rest, "CHECK")
      // Guard against `CONSTRAINT name PRIMARY KEY (col CHECK ...)` style:
      // the CHECK must be the constraint keyword, i.e. start the body.
      && rest.trim_start().to_ascii_uppercase().starts_with("CHECK")
    {
      out.push(check_constraint(cname, &rest[o..=c], false, Vec::new()));
      continue;
    }

    // Anonymous table-level: `CHECK (...)`.
    if upper.starts_with("CHECK")
      && let Some((o, c)) = find_kw_parens(def, "CHECK")
    {
      anon += 1;
      out.push(check_constraint(format!("ck_{table}_{anon}"), &def[o..=c], false, Vec::new()));
      continue;
    }

    // Skip the other table-level constraint forms outright.
    if matches!(upper.split_whitespace().next(), Some("PRIMARY" | "UNIQUE" | "FOREIGN")) {
      continue;
    }

    // Inline column-level: `<col> <type> ... CHECK (...)`.
    if let Some((col, rest)) = split_first_ident(def)
      && let Some((o, c)) = find_kw_parens(rest, "CHECK")
    {
      // Inline `definition` is the bare `(...)` body; the renderer adds CHECK.
      out.push(check_constraint(format!("ck_{col}"), &rest[o..=c], true, vec![col]));
    }
  }
  out
}

fn check_constraint(name: String, paren_group: &str, inline: bool, columns: Vec<String>) -> Constraint {
  let definition = if inline { paren_group.to_string() } else { format!("CHECK {paren_group}") };
  Constraint {
    name,
    kind: ConstraintKind::Check,
    columns,
    references: None,
    definition: Some(definition),
    inline,
  }
}

/// Index of the ')' matching the '(' at `open`, respecting nested parens and
/// quoted strings. Returns None if unbalanced.
fn match_paren(bytes: &[u8], open: usize) -> Option<usize> {
  let mut depth = 0i32;
  let mut i = open;
  while i < bytes.len() {
    match bytes[i] {
      b'\'' | b'"' | b'`' => {
        let q = bytes[i];
        i += 1;
        while i < bytes.len() && bytes[i] != q {
          i += 1;
        }
      },
      b'(' => depth += 1,
      b')' => {
        depth -= 1;
        if depth == 0 {
          return Some(i);
        }
      },
      _ => {},
    }
    i += 1;
  }
  None
}

/// Split a comma-separated list at top level only (commas inside parens or
/// quotes are ignored), used to break a column list into its definitions.
fn split_top_level(s: &str) -> Vec<&str> {
  let bytes = s.as_bytes();
  let mut out = Vec::new();
  let mut depth = 0i32;
  let mut start = 0usize;
  let mut i = 0usize;
  while i < bytes.len() {
    match bytes[i] {
      b'\'' | b'"' | b'`' => {
        let q = bytes[i];
        i += 1;
        while i < bytes.len() && bytes[i] != q {
          i += 1;
        }
      },
      b'(' => depth += 1,
      b')' => depth -= 1,
      b',' if depth == 0 => {
        out.push(&s[start..i]);
        start = i + 1;
      },
      _ => {},
    }
    i += 1;
  }
  if start < s.len() {
    out.push(&s[start..]);
  }
  out
}

fn io_err(e: sqlx::Error) -> DriverError {
  DriverError::Introspect(e.to_string())
}

#[cfg(test)]
mod tests {
  use super::{is_strict, is_without_rowid, parse_check_constraints, parse_generated_columns, view_body};
  use dsl_catalog::ConstraintKind;

  #[test]
  fn extracts_view_select_body() {
    assert_eq!(view_body("CREATE VIEW v AS SELECT a, b FROM t;").as_deref(), Some("SELECT a, b FROM t"));
    // Column-list before AS, and an AS alias inside the SELECT, don't fool it.
    assert_eq!(
      view_body("CREATE VIEW v (x, y) AS SELECT a AS x, b AS y FROM t").as_deref(),
      Some("SELECT a AS x, b AS y FROM t")
    );
    assert_eq!(view_body("CREATE VIEW v AS").as_deref(), None);
  }

  #[test]
  fn parses_inline_table_level_and_named_checks() {
    let sql = "CREATE TABLE t (\n  age INT CHECK (age >= 0),\n  status TEXT,\n  CHECK (status IN ('a','b')),\n  CONSTRAINT positive CHECK (age < 200)\n)";
    let cs = parse_check_constraints(sql, "t");
    assert_eq!(cs.len(), 3);

    let inline = cs.iter().find(|c| c.inline).unwrap();
    assert_eq!(inline.kind, ConstraintKind::Check);
    assert_eq!(inline.name, "ck_age");
    assert_eq!(inline.columns, vec!["age".to_string()]);
    assert_eq!(inline.definition.as_deref(), Some("(age >= 0)"));

    let anon = cs.iter().find(|c| c.name == "ck_t_1").unwrap();
    assert!(!anon.inline);
    assert_eq!(anon.definition.as_deref(), Some("CHECK (status IN ('a','b'))"));

    let named = cs.iter().find(|c| c.name == "positive").unwrap();
    assert_eq!(named.definition.as_deref(), Some("CHECK (age < 200)"));
  }

  #[test]
  fn check_ignores_strings_and_other_constraints() {
    // `CHECK` inside a default string literal must not register, and PK/FK
    // table-level clauses are skipped.
    let sql = "CREATE TABLE t (note TEXT DEFAULT 'CHECK (x)', a INT, PRIMARY KEY (a), FOREIGN KEY (a) REFERENCES u(id))";
    assert!(parse_check_constraints(sql, "t").is_empty());
  }

  #[test]
  fn check_handles_nested_parens() {
    let sql = "CREATE TABLE t (x INT CHECK (x > abs(min(0, -1))))";
    let cs = parse_check_constraints(sql, "t");
    assert_eq!(cs.len(), 1);
    assert_eq!(cs[0].definition.as_deref(), Some("(x > abs(min(0, -1)))"));
  }

  #[test]
  fn detects_without_rowid() {
    assert!(is_without_rowid("CREATE TABLE t (a INT PRIMARY KEY, b TEXT) WITHOUT ROWID"));
    assert!(is_without_rowid("CREATE TABLE t (a INT PRIMARY KEY) WITHOUT  ROWID"));
    assert!(is_without_rowid("CREATE TABLE t (a INT PRIMARY KEY) STRICT, WITHOUT ROWID"));
  }

  #[test]
  fn ignores_rowid_column_and_plain_tables() {
    assert!(!is_without_rowid("CREATE TABLE t (rowid INT, name TEXT)"));
    assert!(!is_without_rowid("CREATE TABLE t (a INT, b TEXT)"));
    // A `without` appearing inside the body must not trigger it.
    assert!(!is_without_rowid("CREATE TABLE t (note TEXT DEFAULT 'without rowid')"));
  }

  #[test]
  fn detects_strict_in_any_combination() {
    assert!(is_strict("CREATE TABLE t (a INT) STRICT"));
    assert!(is_strict("CREATE TABLE t (a INT PRIMARY KEY) WITHOUT ROWID, STRICT"));
    assert!(is_strict("CREATE TABLE t (a INT PRIMARY KEY) STRICT, WITHOUT ROWID"));
    // Both options surface independently.
    let combo = "CREATE TABLE t (a INT PRIMARY KEY) STRICT, WITHOUT ROWID";
    assert!(is_strict(combo) && is_without_rowid(combo));
  }

  #[test]
  fn ignores_strict_inside_body() {
    assert!(!is_strict("CREATE TABLE t (a INT, b TEXT)"));
    // A column named `strict` or a string default must not trigger it.
    assert!(!is_strict("CREATE TABLE t (strict INT, note TEXT DEFAULT 'strict')"));
  }

  #[test]
  fn parses_generated_always_and_shorthand() {
    let sql = "CREATE TABLE t (\n  a INTEGER,\n  b INTEGER GENERATED ALWAYS AS (a * 2) STORED,\n  c AS (a + 1) VIRTUAL\n)";
    let g = parse_generated_columns(sql);
    assert_eq!(g.get("b").map(String::as_str), Some("a * 2"));
    assert_eq!(g.get("c").map(String::as_str), Some("a + 1"));
    assert_eq!(g.get("a"), None);
  }

  #[test]
  fn ignores_check_and_quoted_idents() {
    let sql = "CREATE TABLE t (\"x y\" INT CHECK (\"x y\" > 0), z TEXT GENERATED ALWAYS AS (upper(\"x y\")) STORED)";
    let g = parse_generated_columns(sql);
    assert_eq!(g.get("x y"), None);
    assert_eq!(g.get("z").map(String::as_str), Some("upper(\"x y\")"));
  }

  #[test]
  fn no_generated_columns_yields_empty() {
    let g = parse_generated_columns("CREATE TABLE t (a INT, b TEXT, PRIMARY KEY (a))");
    assert!(g.is_empty());
  }
}
