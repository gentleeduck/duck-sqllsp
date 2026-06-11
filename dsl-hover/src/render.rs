//! Markdown renderers for hover bodies.
//!
//! Table hover ships both a column-list table AND a synthesised
//! `CREATE TABLE` block so the user sees the source-level shape (types,
//! NOT NULL, DEFAULT, FK targets) in one glance. Column / function /
//! column-decl hovers each have their own renderer.

use dsl_catalog::{Column, Constraint, ConstraintKind, Table, TableKind, Type, TypeKind, display_type};

/// Render the table card + every catalog-wide attachment found via
/// `cat` (inbound FKs from other tables, sequences owned by this
/// table's columns). Used by the hover handler which has the full
/// catalog in scope; resolver.rs (which doesn't) keeps calling
/// `table()` for the bare card.
pub fn table_with_catalog(t: &Table, cat: &dsl_catalog::Catalog) -> String {
  let mut s = table(t);
  let inbound = inbound_fks(t, cat);
  if !inbound.is_empty() {
    s.push_str("\n**Referenced by**\n\n");
    s.push_str("```sql\n");
    for line in inbound {
      s.push_str(&line);
      s.push('\n');
    }
    s.push_str("```\n");
  }
  let owned: Vec<&dsl_catalog::Sequence> = cat
    .sequences
    .iter()
    .filter(|seq| {
      let Some(owner) = &seq.owned_by_column else { return false };
      let parts: Vec<&str> = owner.split('.').collect();
      parts.len() >= 2 && parts[parts.len() - 2].eq_ignore_ascii_case(&t.name)
    })
    .collect();
  if !owned.is_empty() {
    s.push_str("\n**Sequences**\n\n");
    s.push_str("```sql\n");
    for seq in owned {
      let cycle = if seq.cycle { " CYCLE" } else { "" };
      s.push_str(&format!(
        "CREATE SEQUENCE {}.{} AS {} START {} MIN {} MAX {} INCREMENT {}{};\n",
        seq.schema, seq.name, seq.data_type, seq.start_value, seq.min_value, seq.max_value, seq.increment_by, cycle
      ));
      if let Some(o) = &seq.owned_by_column {
        s.push_str(&format!("ALTER SEQUENCE {}.{} OWNED BY {};\n", seq.schema, seq.name, o));
      }
    }
    s.push_str("```\n");
  }
  s
}

fn inbound_fks(t: &Table, cat: &dsl_catalog::Catalog) -> Vec<String> {
  let mut out = Vec::new();
  for other in cat.tables() {
    if other.name.eq_ignore_ascii_case(&t.name) && other.schema.eq_ignore_ascii_case(&t.schema) {
      continue;
    }
    for c in &other.constraints {
      if !matches!(c.kind, ConstraintKind::ForeignKey) {
        continue;
      }
      let Some(refs) = &c.references else { continue };
      if !refs.table.eq_ignore_ascii_case(&t.name) {
        continue;
      }
      let local = c.columns.join(", ");
      let target = refs.columns.join(", ");
      out.push(format!("-- FK: {}.{} ({}) -> {}.{} ({})", other.schema, other.name, local, t.schema, t.name, target));
    }
  }
  out
}

pub fn table(t: &Table) -> String {
  // Views render their defining query rather than a synthesised CREATE TABLE
  // -- the SELECT is what a user hovering a view actually wants to see.
  if matches!(t.kind, TableKind::View | TableKind::MaterializedView)
    && let Some(def) = &t.definition
    && !def.trim().is_empty()
  {
    return render_view(t, def);
  }

  // Compact view: skip the `# Table public.<name>` markdown title and
  // emit the DDL block straight up. CREATE TABLE / CREATE VIEW already
  // declares the kind + fully-qualified name on its first line, so the
  // title was redundant noise.
  if t.columns.is_empty() {
    let fq = format!("{}.{}", t.schema, t.name);
    let base = match t.kind {
      TableKind::View => "View",
      TableKind::MaterializedView => "Materialised View",
      _ => "Table",
    };
    // WITHOUT ROWID and STRICT are independent SQLite table options.
    let mut opts = Vec::new();
    if matches!(t.kind, TableKind::WithoutRowid) {
      opts.push("WITHOUT ROWID");
    }
    if t.strict {
      opts.push("STRICT");
    }
    let kind = if opts.is_empty() { base.to_string() } else { format!("{base} ({})", opts.join(", ")) };
    return format!("_{kind} `{fq}` -- no columns cached; run `:DBRefresh` after connecting._\n");
  }

  let mut s = String::new();
  s.push_str("```sql\n");
  s.push_str(&table_ddl(t));
  s.push('\n');

  // Owner: surface ownership on its own line right under the CREATE TABLE
  // so a quick hover answers "who owns this." Mirrors how PG's
  // `\d <table>` shows the owner in the metadata block.
  if let Some(owner) = &t.owner
    && !owner.is_empty()
  {
    s.push_str(&format!("ALTER TABLE {}.{} OWNER TO {};\n", t.schema, t.name, owner));
  }

  if !t.indexes.is_empty() {
    s.push_str("\n-- Indexes\n");
    for i in &t.indexes {
      if let Some(def) = &i.definition {
        s.push_str(def);
        if !def.trim_end().ends_with(';') {
          s.push(';');
        }
        s.push('\n');
      } else {
        let unique = if i.unique { "UNIQUE " } else { "" };
        s.push_str(&format!(
          "CREATE {unique}INDEX {} ON {}.{} ({});\n",
          i.name,
          t.schema,
          t.name,
          i.columns.join(", ")
        ));
      }
    }
  }

  if !t.triggers.is_empty() {
    s.push_str("\n-- Triggers\n");
    for tg in &t.triggers {
      s.push_str(&format!(
        "CREATE TRIGGER {} {} {} ON {}.{} FOR EACH {} EXECUTE FUNCTION {}();\n",
        tg.name, tg.timing, tg.event, t.schema, t.name, tg.granularity, tg.function
      ));
    }
  }

  if !t.policies.is_empty() {
    s.push_str("\n-- Policies (Row-Level Security)\n");
    for p in &t.policies {
      s.push_str(&format!(
        "CREATE POLICY {} ON {}.{} AS {} FOR {} TO {}",
        p.name, t.schema, t.name, p.permissive, p.command, p.roles
      ));
      if let Some(u) = &p.using_expr {
        s.push_str(&format!(" USING ({u})"));
      }
      if let Some(c) = &p.check_expr {
        s.push_str(&format!(" WITH CHECK ({c})"));
      }
      s.push_str(";\n");
    }
  }

  if let Some(comment) = &t.comment
    && !comment.trim().is_empty()
  {
    s.push_str("\n-- Comment\n");
    s.push_str("COMMENT ON TABLE ");
    s.push_str(&t.schema);
    s.push('.');
    s.push_str(&t.name);
    s.push_str(" IS ");
    s.push('\'');
    s.push_str(&comment.replace('\'', "''"));
    s.push_str("';\n");
  }

  s.push_str("```\n");
  s
}

/// Render a view / materialized view as its defining query, with the output
/// columns listed as a trailing comment block. Keywords follow the active
/// hover keyword case.
fn render_view(t: &Table, def: &str) -> String {
  let case = crate::current_keyword_case();
  let kw = |k: &str| case.apply(k);
  let create = if matches!(t.kind, TableKind::MaterializedView) {
    kw("CREATE MATERIALIZED VIEW")
  } else {
    kw("CREATE VIEW")
  };

  let mut s = String::new();
  s.push_str("```sql\n");
  s.push_str(&format!("{} {}.{} {}\n", create, t.schema, t.name, kw("AS")));
  s.push_str(def.trim_end().trim_end_matches(';').trim_end());
  s.push_str(";\n");

  if !t.columns.is_empty() {
    s.push_str("\n-- Columns\n");
    for c in &t.columns {
      // Offline-scanned views carry column names without types.
      if c.data_type.is_empty() {
        s.push_str(&format!("--   {}\n", c.name));
      } else {
        s.push_str(&format!("--   {} {}\n", c.name, case.apply(display_type(&c.data_type))));
      }
    }
  }

  if let Some(comment) = &t.comment
    && !comment.trim().is_empty()
  {
    s.push_str(&format!("\n-- {}\n", comment.trim()));
  }

  s.push_str("```\n");
  s
}

/// Synthesise a CREATE TABLE that mirrors the catalog. Columns first,
/// then constraints. Aligned name + type columns for readability.
/// Keywords are cased according to the active hover keyword case (set
/// by `dsl_hover::hover_with`).
pub fn table_ddl(t: &Table) -> String {
  let case = crate::current_keyword_case();
  let kw = |k: &str| case.apply(k);
  let fq = format!("{}.{}", t.schema, t.name);
  let mut lines: Vec<String> = Vec::new();
  lines.push(format!("{} {} {fq} (", kw("CREATE"), kw("TABLE")));

  // Inline-origin single-column constraints fold back onto the column
  // row (`id int PRIMARY KEY`, `email text UNIQUE`,
  // `parent_id int REFERENCES users(id)`). Anything table-level renders
  // as a separate `CONSTRAINT ...` line below the columns.
  let inline_by_col: std::collections::HashMap<String, Vec<&Constraint>> = {
    let mut m: std::collections::HashMap<String, Vec<&Constraint>> = std::collections::HashMap::new();
    for con in &t.constraints {
      if con.inline && con.columns.len() == 1 {
        m.entry(con.columns[0].to_ascii_lowercase()).or_default().push(con);
      }
    }
    m
  };

  let name_w = t.columns.iter().map(|c| c.name.len()).max().unwrap_or(0);
  let type_w = t.columns.iter().map(|c| display_type(&c.data_type).len()).max().unwrap_or(0);

  let mut members: Vec<String> = Vec::new();
  for c in &t.columns {
    let inlines = inline_by_col.get(&c.name.to_ascii_lowercase());
    let has_inline_pk = inlines.is_some_and(|v| v.iter().any(|c| matches!(c.kind, ConstraintKind::PrimaryKey)));
    let mut row =
      format!("    {:<nw$} {:<tw$}", c.name, case.apply(display_type(&c.data_type)), nw = name_w, tw = type_w);
    // PRIMARY KEY already implies NOT NULL -- don't double up.
    if !c.nullable && !has_inline_pk {
      row.push(' ');
      row.push_str(&kw("NOT NULL"));
    }
    if let Some(d) = &c.default {
      row.push(' ');
      row.push_str(&kw("DEFAULT"));
      row.push(' ');
      row.push_str(d);
    }
    if let Some(cons) = inlines {
      // Stable order: PK, UNIQUE, FK, CHECK.
      let order = |k: &ConstraintKind| match k {
        ConstraintKind::PrimaryKey => 0,
        ConstraintKind::Unique => 1,
        ConstraintKind::ForeignKey => 2,
        ConstraintKind::Check => 3,
      };
      let mut sorted: Vec<&&Constraint> = cons.iter().collect();
      sorted.sort_by_key(|c| order(&c.kind));
      for con in sorted {
        row.push(' ');
        match con.kind {
          ConstraintKind::PrimaryKey => row.push_str(&kw("PRIMARY KEY")),
          ConstraintKind::Unique => row.push_str(&kw("UNIQUE")),
          ConstraintKind::ForeignKey => {
            row.push_str(&kw("REFERENCES"));
            if let Some(r) = &con.references {
              let target_cols = if r.columns.is_empty() { String::new() } else { format!(" ({})", r.columns.join(", ")) };
              row.push(' ');
              if r.schema.is_empty() || r.schema.eq_ignore_ascii_case("public") {
                row.push_str(&format!("{}{target_cols}", r.table));
              } else {
                row.push_str(&format!("{}.{}{target_cols}", r.schema, r.table));
              }
            }
          },
          ConstraintKind::Check => {
            row.push_str(&kw("CHECK"));
            if let Some(def) = &con.definition {
              row.push(' ');
              row.push_str(def);
            }
          },
        }
      }
    }
    members.push(row);
  }
  // Table-level constraints get a blank line then a `CONSTRAINT ...` row each.
  // Inline-origin constraints are already folded onto the column rows above.
  let top_level: Vec<&Constraint> = t.constraints.iter().filter(|c| !c.inline).collect();
  let inject_gap = !t.columns.is_empty() && !top_level.is_empty();
  let columns_end_idx = members.len();
  for con in top_level {
    members.push(render_constraint(con));
  }

  let last_non_blank = members.len().saturating_sub(1);
  for (i, m) in members.into_iter().enumerate() {
    if inject_gap && i == columns_end_idx {
      lines.push(String::new());
    }
    if i < last_non_blank {
      lines.push(format!("{m},"));
    } else {
      lines.push(m);
    }
  }
  // Trailing table-option clause after the column list. SQLite carries
  // WITHOUT ROWID / STRICT (comma-joined, possibly both); MySQL carries a
  // pre-rendered `ENGINE=... DEFAULT CHARSET=...` string in `options`. The
  // two dialects never co-occur on one table.
  let mut sqlite_opts: Vec<String> = Vec::new();
  if matches!(t.kind, TableKind::WithoutRowid) {
    sqlite_opts.push(kw("WITHOUT ROWID"));
  }
  if t.strict {
    sqlite_opts.push(kw("STRICT"));
  }
  if !sqlite_opts.is_empty() {
    lines.push(format!(") {};", sqlite_opts.join(", ")));
  } else if let Some(opts) = t.options.as_deref().map(str::trim).filter(|o| !o.is_empty()) {
    lines.push(format!(") {opts};"));
  } else {
    lines.push(");".into());
  }
  lines.join("\n")
}

fn render_constraint(c: &Constraint) -> String {
  // Prefer the live `pg_get_constraintdef` output when introspection
  // captured it -- it's the authoritative form straight from Postgres.
  if let Some(def) = &c.definition {
    return format!("    CONSTRAINT {} {def}", c.name);
  }
  let mut s = format!("    CONSTRAINT {} ", c.name);
  match c.kind {
    ConstraintKind::PrimaryKey => s.push_str(&format!("PRIMARY KEY ({})", c.columns.join(", "))),
    ConstraintKind::Unique => s.push_str(&format!("UNIQUE ({})", c.columns.join(", "))),
    ConstraintKind::Check => s.push_str("CHECK (...)"),
    ConstraintKind::ForeignKey => {
      s.push_str(&format!("FOREIGN KEY ({})", c.columns.join(", ")));
      if let Some(r) = &c.references {
        if r.schema.is_empty() || r.schema.eq_ignore_ascii_case("public") {
          s.push_str(&format!(" REFERENCES {} ({})", r.table, r.columns.join(", ")));
        } else {
          s.push_str(&format!(" REFERENCES {}.{} ({})", r.schema, r.table, r.columns.join(", ")));
        }
      }
    },
  }
  s
}

pub fn column(t: &Table, c: &Column) -> String {
  // Blank lines around every paragraph and a bullet list so markdown
  // renders the spec stack instead of collapsing onto one line. Long
  // default expressions get wrapped at 72 cols so the hover float
  // stays narrow.
  let mut s = format!("# Column `{}.{}.{}`\n\n", t.schema, t.name, c.name);
  s.push_str(&format!("- **type:** `{}`\n", display_type(&c.data_type)));
  s.push_str(&format!("- **nullable:** `{}`\n", c.nullable));
  if let Some(g) = &c.generated {
    let wrapped = dsl_knowledge::wrap_paragraphs(g, 64);
    s.push_str(&format!("- **generated:** `GENERATED ALWAYS AS ({wrapped}) STORED`\n"));
  }
  if let Some(d) = &c.default {
    let wrapped = dsl_knowledge::wrap_paragraphs(d, 64);
    s.push_str(&format!("- **default:** `{wrapped}`\n"));
  }
  // Mention every constraint on this column from its table.
  let constraints_for_col: Vec<&Constraint> =
    t.constraints.iter().filter(|con| con.columns.iter().any(|cn| cn.eq_ignore_ascii_case(&c.name))).collect();
  if !constraints_for_col.is_empty() {
    s.push_str("- **constraints:**\n");
    for con in constraints_for_col {
      let kind = match con.kind {
        ConstraintKind::PrimaryKey => "PRIMARY KEY",
        ConstraintKind::ForeignKey => "FOREIGN KEY",
        ConstraintKind::Unique => "UNIQUE",
        ConstraintKind::Check => "CHECK",
      };
      s.push_str(&format!("  - {kind} (`{}`)\n", con.name));
    }
  }
  if let Some(cm) = &c.comment
    && !cm.trim().is_empty()
  {
    s.push_str(&format!("\n{}\n", dsl_knowledge::wrap_paragraphs(cm, 72)));
  }
  s.push_str(&format!("\n_From table `{}.{}`_\n", t.schema, t.name));
  s
}

/// Variant of [`column`] that also lists every FK in the catalog
/// pointing at this column. The hover handler uses this when the
/// catalog is in scope.
pub fn column_with_catalog(t: &Table, c: &Column, cat: &dsl_catalog::Catalog) -> String {
  let mut s = column(t, c);
  let inbound: Vec<String> = cat
    .tables()
    .flat_map(|other| {
      other.constraints.iter().filter_map(move |con| {
        if !matches!(con.kind, ConstraintKind::ForeignKey) {
          return None;
        }
        let refs = con.references.as_ref()?;
        if !refs.table.eq_ignore_ascii_case(&t.name) {
          return None;
        }
        if !refs.columns.iter().any(|x| x.eq_ignore_ascii_case(&c.name)) {
          return None;
        }
        let local = con.columns.join(", ");
        Some(format!("- `{}.{}` ({})", other.schema, other.name, local))
      })
    })
    .collect();
  if !inbound.is_empty() {
    s.push_str("\n**Referenced by**\n\n");
    for line in inbound {
      s.push_str(&line);
      s.push('\n');
    }
  }
  s
}

pub fn column_in_tables(items: &[(&Table, &Column)]) -> String {
  if items.len() == 1 {
    return column(items[0].0, items[0].1);
  }
  let mut s = format!("# Column `{}` (in {} tables)\n\n", items[0].1.name, items.len());
  let rows: Vec<Vec<String>> = items
    .iter()
    .map(|(t, c)| {
      vec![format!("{}.{}", t.schema, t.name), display_type(&c.data_type).to_string(), if c.nullable { "YES" } else { "NO" }.into()]
    })
    .collect();
  s.push_str(&crate::md_table::render(&["table", "type", "nullable"], &rows));
  s
}

/// Render a column declaration with no implicit-spec data.
pub fn column_decl(table_name: &str, col: &dsl_parse::ColumnDef) -> String {
  column_decl_with_implicit(table_name, col, &crate::implicit::Implicit::default())
}

/// Render a column declaration plus every implicit spec derived from
/// the parent table (PRIMARY KEY -> NOT NULL + UNIQUE, FOREIGN KEY,
/// SERIAL / IDENTITY -> NOT NULL + auto-increment). The result is the
/// "full spec view" -- declared + implicit, in one table.
pub fn column_decl_with_implicit(
  table_name: &str,
  col: &dsl_parse::ColumnDef,
  implicit: &crate::implicit::Implicit,
) -> String {
  // Compute effective values.
  let nullable_effective = col.nullable && !implicit.primary_key && !implicit.auto_increment;
  let unique_effective = implicit.unique || implicit.primary_key;

  let mut s = format!("# Column `{}.{}`\n\n_DDL definition (current buffer)_\n\n", table_name, col.name);

  let mut rows: Vec<Vec<String>> = Vec::new();
  let push = |rows: &mut Vec<Vec<String>>, spec: &str, value: &str, declared: bool| {
    rows.push(vec![spec.into(), value.into(), (if declared { "declared" } else { "implicit" }).into()]);
  };
  push(&mut rows, "type", &col.type_name, true);
  push(
    &mut rows,
    "nullable",
    if nullable_effective { "YES" } else { "NO" },
    !implicit.primary_key && !implicit.auto_increment,
  );
  if implicit.primary_key {
    push(&mut rows, "primary key", "yes", false);
  }
  if unique_effective && !implicit.primary_key {
    push(&mut rows, "unique", "yes", !implicit.unique);
  }
  if implicit.auto_increment {
    push(&mut rows, "auto-increment", "yes (SERIAL / IDENTITY)", false);
  }
  if let Some(fk) = &implicit.foreign_key {
    push(&mut rows, "references", &fk.references, false);
  }
  if implicit.check_count > 0 {
    push(&mut rows, "check constraints", &format!("{}", implicit.check_count), false);
  }
  if let Some(d) = &col.default {
    push(&mut rows, "default", d, true);
  } else if implicit.auto_increment {
    push(&mut rows, "default", "nextval(...) (implicit via SERIAL / IDENTITY)", false);
  }
  s.push_str(&crate::md_table::render(&["spec", "value", "source"], &rows));

  s.push_str("\n```sql\n");
  s.push_str(&col.name);
  s.push(' ');
  s.push_str(&col.type_name);
  if !nullable_effective {
    s.push_str(" NOT NULL");
  }
  if let Some(d) = &col.default {
    s.push_str(" DEFAULT ");
    s.push_str(d);
  }
  if implicit.primary_key {
    s.push_str(" PRIMARY KEY");
  }
  s.push_str("\n```\n");

  if implicit.primary_key || implicit.auto_increment || implicit.foreign_key.is_some() || implicit.unique {
    s.push_str(
      "\n_Implicit specs come from table-level constraints \
             (PRIMARY KEY, UNIQUE, FOREIGN KEY) or the column type \
             (SERIAL, IDENTITY)._\n",
    );
  }
  s
}

/// Render a DB-side function with signature + return type + body when
/// available. Body is taken from the `comment` field, where introspection
/// stashes the `pg_get_functiondef()` output.
pub fn function_full(f: &dsl_catalog::Function) -> String {
  let args = f
    .arguments
    .iter()
    .map(|a| match &a.name {
      Some(n) => format!("{n} {}", display_type(&a.data_type)),
      None => display_type(&a.data_type).to_string(),
    })
    .collect::<Vec<_>>()
    .join(", ");
  let mut s = format!("# Function `{}.{}`\n\n_DB function_\n\n", f.schema, f.name);
  s.push_str("```sql\n");
  // Break the signature across lines when it grows past 72 cols --
  // long argument lists are otherwise unreadable inside the hover.
  let sig = format!("{}.{}({}) -> {}", f.schema, f.name, args, display_type(&f.return_type));
  if sig.len() > 72 && !args.is_empty() {
    s.push_str(&format!("{}.{} (\n", f.schema, f.name));
    for (i, a) in f.arguments.iter().enumerate() {
      let arg_str = match &a.name {
        Some(n) => format!("    {n} {}", display_type(&a.data_type)),
        None => format!("    {}", display_type(&a.data_type)),
      };
      s.push_str(&arg_str);
      if i + 1 < f.arguments.len() {
        s.push(',');
      }
      s.push('\n');
    }
    s.push_str(&format!(") -> {}", display_type(&f.return_type)));
  } else {
    s.push_str(&sig);
  }
  s.push_str("\n```\n\n");
  if let Some(body) = function_body(f) {
    s.push_str("**Source**\n\n```sql\n");
    s.push_str(&body);
    s.push_str("\n```\n");
  }
  s
}

fn function_body(f: &dsl_catalog::Function) -> Option<String> {
  let c = f.comment.as_ref()?;
  let trimmed = c.trim_start().to_ascii_uppercase();
  if trimmed.starts_with("CREATE") { Some(c.clone()) } else { None }
}

pub fn user_type(t: &Type) -> String {
  let fq = format!("{}.{}", t.schema, t.name);
  let kind = match t.kind {
    TypeKind::Enum => "enum",
    TypeKind::Domain => "domain",
    TypeKind::Composite => "composite type",
  };
  format!("# `{fq}`\n_{kind}_\n")
}

#[cfg(test)]
mod tests {
  use super::*;

  fn view(kind: TableKind, definition: Option<&str>, columns: Vec<Column>) -> Table {
    Table {
      schema: "public".into(),
      name: "v".into(),
      kind,
      columns,
      constraints: Vec::new(),
      indexes: Vec::new(),
      triggers: Vec::new(),
      policies: Vec::new(),
      comment: None,
      row_estimate: None,
      owner: None,
      definition: definition.map(str::to_string),
      strict: false, options: None,
    }
  }

  #[test]
  fn view_renders_defining_query() {
    let out = table(&view(TableKind::View, Some("SELECT id FROM t"), Vec::new()));
    assert!(out.contains("CREATE VIEW public.v AS"), "{out}");
    assert!(out.contains("SELECT id FROM t;"), "{out}");
  }

  #[test]
  fn materialized_view_label() {
    let out = table(&view(TableKind::MaterializedView, Some("SELECT 1"), Vec::new()));
    assert!(out.contains("CREATE MATERIALIZED VIEW public.v AS"), "{out}");
  }

  #[test]
  fn view_without_definition_falls_back() {
    // No definition cached -> keep the legacy "no columns" rendering path
    // rather than emitting an empty CREATE VIEW.
    let out = table(&view(TableKind::View, None, Vec::new()));
    assert!(out.contains("View `public.v`"), "{out}");
  }

  fn col(name: &str) -> Column {
    Column {
      name: name.into(),
      data_type: "int".into(),
      nullable: true,
      default: None,
      comment: None,
      generated: None,
      json_keys: None,
    }
  }

  fn sqlite_table(kind: TableKind, strict: bool) -> Table {
    let mut t = view(kind, None, vec![col("a")]);
    t.strict = strict;
    t
  }

  #[test]
  fn strict_table_ddl_trailer() {
    let out = table_ddl(&sqlite_table(TableKind::Table, true));
    assert!(out.trim_end().ends_with(") STRICT;"), "{out}");
  }

  #[test]
  fn without_rowid_and_strict_combine_in_ddl() {
    let out = table_ddl(&sqlite_table(TableKind::WithoutRowid, true));
    assert!(out.trim_end().ends_with(") WITHOUT ROWID, STRICT;"), "{out}");
  }

  #[test]
  fn mysql_options_render_in_ddl_trailer() {
    let mut t = sqlite_table(TableKind::Table, false);
    t.options = Some("ENGINE=InnoDB DEFAULT CHARSET=utf8mb4".into());
    let out = table_ddl(&t);
    assert!(out.trim_end().ends_with(") ENGINE=InnoDB DEFAULT CHARSET=utf8mb4;"), "{out}");
  }

  #[test]
  fn sqlite_options_take_precedence_over_generic_options() {
    // A STRICT SQLite table never also has MySQL `options`, but guard the
    // precedence anyway: the kind/flag trailer wins.
    let mut t = sqlite_table(TableKind::WithoutRowid, true);
    t.options = Some("ENGINE=InnoDB".into());
    let out = table_ddl(&t);
    assert!(out.trim_end().ends_with(") WITHOUT ROWID, STRICT;"), "{out}");
  }

  #[test]
  fn empty_strict_table_label() {
    // No columns -> the compact label reflects the option.
    let out = table(&sqlite_table(TableKind::Table, true));
    // sqlite_table has a column, so drop it to hit the empty-label path.
    let mut empty = sqlite_table(TableKind::Table, true);
    empty.columns.clear();
    let label = table(&empty);
    assert!(label.contains("Table (STRICT)"), "{label}");
    // With columns it renders the DDL trailer instead.
    assert!(out.contains("STRICT"), "{out}");
  }
}
