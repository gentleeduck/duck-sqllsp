//! Markdown renderers for hover bodies.
//!
//! Table hover ships both a column-list table AND a synthesised
//! `CREATE TABLE` block so the user sees the source-level shape (types,
//! NOT NULL, DEFAULT, FK targets) in one glance. Column / function /
//! column-decl hovers each have their own renderer.

use dsl_catalog::{Column, Constraint, ConstraintKind, Table, TableKind, Type, TypeKind};

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
      if !matches!(c.kind, ConstraintKind::ForeignKey) { continue; }
      let Some(refs) = &c.references else { continue };
      if !refs.table.eq_ignore_ascii_case(&t.name) { continue; }
      let local = c.columns.join(", ");
      let target = refs.columns.join(", ");
      out.push(format!(
        "-- FK: {}.{} ({}) -> {}.{} ({})",
        other.schema, other.name, local, t.schema, t.name, target
      ));
    }
  }
  out
}

pub fn table(t: &Table) -> String {
  let fq = format!("{}.{}", t.schema, t.name);
  let kind = match t.kind {
    TableKind::View => "View",
    TableKind::MaterializedView => "Materialised View",
    _ => "Table",
  };
  let mut s = format!("# {kind} `{fq}`\n\n");

  if t.columns.is_empty() {
    s.push_str("_(no columns cached -- run `:DBRefresh` after switching connections)_\n");
    return s;
  }

  // Full DDL block -- the canonical view. The user reads SQL faster
  // than markdown tables; constraint / index / trigger sections come
  // as commented sub-blocks after the CREATE TABLE.
  s.push_str("```sql\n");
  s.push_str(&table_ddl(t));
  s.push('\n');

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

  if let Some(comment) = &t.comment {
    if !comment.trim().is_empty() {
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

  let name_w = t.columns.iter().map(|c| c.name.len()).max().unwrap_or(0);
  let type_w = t.columns.iter().map(|c| c.data_type.len()).max().unwrap_or(0);

  let mut members: Vec<String> = Vec::new();
  for c in &t.columns {
    let mut row = format!("    {:<nw$} {:<tw$}", c.name, case.apply(&c.data_type), nw = name_w, tw = type_w);
    if !c.nullable {
      row.push(' ');
      row.push_str(&kw("NOT NULL"));
    }
    if let Some(d) = &c.default {
      row.push(' ');
      row.push_str(&kw("DEFAULT"));
      row.push(' ');
      row.push_str(d);
    }
    members.push(row);
  }
  // Visual gap between the column block and the table-level
  // constraints. Every constraint stays as a `CONSTRAINT <name> ...`
  // line at the bottom -- no inlining onto the column rows.
  let inject_gap = !t.columns.is_empty() && !t.constraints.is_empty();
  let columns_end_idx = members.len();
  for con in &t.constraints {
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
  lines.push(");".into());
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
        s.push_str(&format!(" REFERENCES {}.{} ({})", r.schema, r.table, r.columns.join(", ")));
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
  s.push_str(&format!("- **type:** `{}`\n", c.data_type));
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
  let constraints_for_col: Vec<&Constraint> = t
    .constraints
    .iter()
    .filter(|con| con.columns.iter().any(|cn| cn.eq_ignore_ascii_case(&c.name)))
    .collect();
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
  if let Some(cm) = &c.comment {
    if !cm.trim().is_empty() {
      s.push_str(&format!("\n{}\n", dsl_knowledge::wrap_paragraphs(cm, 72)));
    }
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
      other
        .constraints
        .iter()
        .filter_map(move |con| {
          if !matches!(con.kind, ConstraintKind::ForeignKey) { return None; }
          let refs = con.references.as_ref()?;
          if !refs.table.eq_ignore_ascii_case(&t.name) { return None; }
          if !refs.columns.iter().any(|x| x.eq_ignore_ascii_case(&c.name)) { return None; }
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
      vec![format!("{}.{}", t.schema, t.name), c.data_type.clone(), if c.nullable { "YES" } else { "NO" }.into()]
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

  if implicit.primary_key
    || implicit.auto_increment
    || implicit.foreign_key.is_some()
    || (implicit.unique && !implicit.primary_key)
  {
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
      Some(n) => format!("{n} {}", a.data_type),
      None => a.data_type.clone(),
    })
    .collect::<Vec<_>>()
    .join(", ");
  let mut s = format!("# Function `{}.{}`\n\n_DB function_\n\n", f.schema, f.name);
  s.push_str("```sql\n");
  // Break the signature across lines when it grows past 72 cols --
  // long argument lists are otherwise unreadable inside the hover.
  let sig = format!("{}.{}({}) -> {}", f.schema, f.name, args, f.return_type);
  if sig.len() > 72 && !args.is_empty() {
    s.push_str(&format!("{}.{} (\n", f.schema, f.name));
    for (i, a) in f.arguments.iter().enumerate() {
      let arg_str = match &a.name {
        Some(n) => format!("    {n} {}", a.data_type),
        None => format!("    {}", a.data_type),
      };
      s.push_str(&arg_str);
      if i + 1 < f.arguments.len() {
        s.push(',');
      }
      s.push('\n');
    }
    s.push_str(&format!(") -> {}", f.return_type));
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
