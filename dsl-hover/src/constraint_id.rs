//! Hover for constraint / index / trigger identifiers.
//!
//! Conventional Postgres style prefixes object names with a 2-3 letter
//! tag indicating their kind:
//!
//!   pk_   primary key
//!   fk_   foreign key
//!   uq_   unique constraint
//!   uidx_ unique index
//!   idx_  index
//!   ix_   index (alt)
//!   ch_   check constraint
//!   chk_  check (alt)
//!   tg_   trigger
//!   tr_   trigger (alt)
//!   trg_  trigger (alt)
//!
//! We use the prefix as a quick classifier, then try to look up the
//! actual object in the catalog (constraints + indexes + triggers
//! attached to each table). When we find a match we render its real
//! definition; otherwise we still render an "expected to be a <kind>"
//! card so the user gets useful context.

use dsl_catalog::{Catalog, Constraint, ConstraintKind, IndexDef, Table, Trigger};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
  PrimaryKey,
  ForeignKey,
  Unique,
  UniqueIndex,
  Index,
  Check,
  Trigger,
}

impl Kind {
  fn label(self) -> &'static str {
    match self {
      Kind::PrimaryKey => "Primary key",
      Kind::ForeignKey => "Foreign key",
      Kind::Unique => "Unique constraint",
      Kind::UniqueIndex => "Unique index",
      Kind::Index => "Index",
      Kind::Check => "Check constraint",
      Kind::Trigger => "Trigger",
    }
  }
}

/// Classify a token by its conventional prefix. Order matters --
/// `uidx_` must be checked before `idx_`.
pub fn classify(token: &str) -> Option<Kind> {
  let t = token.to_ascii_lowercase();
  for (prefix, kind) in [
    ("uidx_", Kind::UniqueIndex),
    ("idx_", Kind::Index),
    ("ix_", Kind::Index),
    ("pk_", Kind::PrimaryKey),
    ("fk_", Kind::ForeignKey),
    ("uq_", Kind::Unique),
    ("ch_", Kind::Check),
    ("chk_", Kind::Check),
    ("tg_", Kind::Trigger),
    ("tr_", Kind::Trigger),
    ("trg_", Kind::Trigger),
  ] {
    if t.starts_with(prefix) {
      return Some(kind);
    }
  }
  None
}

/// Look the identifier up in the catalog and render a card. Returns None
/// when the token does not match the constraint-identifier convention.
pub fn render_for(token: &str, catalog: &Catalog) -> Option<String> {
  let kind = classify(token)?;
  if let Some(found) = find_in_catalog(token, catalog) {
    return Some(render_known(token, kind, found));
  }
  Some(render_unknown(token, kind))
}

enum Found<'a> {
  Constraint(&'a Table, &'a Constraint),
  Index(&'a Table, &'a IndexDef),
  Trigger(&'a Table, &'a Trigger),
}

fn find_in_catalog<'a>(token: &str, catalog: &'a Catalog) -> Option<Found<'a>> {
  for t in catalog.tables() {
    for c in &t.constraints {
      if c.name.eq_ignore_ascii_case(token) {
        return Some(Found::Constraint(t, c));
      }
    }
    for i in &t.indexes {
      if i.name.eq_ignore_ascii_case(token) {
        return Some(Found::Index(t, i));
      }
    }
    for tg in &t.triggers {
      if tg.name.eq_ignore_ascii_case(token) {
        return Some(Found::Trigger(t, tg));
      }
    }
  }
  None
}

fn render_known(token: &str, kind: Kind, found: Found<'_>) -> String {
  // Pure-SQL output: one fenced code block, header lines as SQL
  // comments. The server splits this at the fence and ships the
  // body as `LanguageString { language: "sql", value: ... }` so
  // nvim's stock hover handler colors EVERY line with sql.vim
  // syntax -- including the comment header. No markdown italic /
  // bold / heading styling can leak through.
  let mut s = String::new();
  s.push_str("```sql\n");
  s.push_str(&format!("-- {}: {}\n", kind.label(), token));
  match found {
    Found::Constraint(t, c) => {
      s.push_str(&format!("-- on {}.{}\n", t.schema, t.name));
      s.push('\n');
      s.push_str(&render_constraint(c));
    },
    Found::Index(t, i) => {
      s.push_str(&format!("-- on {}.{}\n", t.schema, t.name));
      s.push('\n');
      if let Some(def) = &i.definition {
        s.push_str(def);
        if !def.trim_end().ends_with(';') {
          s.push(';');
        }
      } else {
        let unique = if i.unique { "UNIQUE " } else { "" };
        s.push_str(&format!("CREATE {unique}INDEX {} ON {}.{} ({});", i.name, t.schema, t.name, i.columns.join(", ")));
      }
    },
    Found::Trigger(t, tg) => {
      s.push_str(&format!("-- on {}.{}\n", t.schema, t.name));
      s.push('\n');
      s.push_str(&format!(
        "CREATE TRIGGER {} {} {} ON {}.{} FOR EACH {} EXECUTE FUNCTION {}();",
        tg.name, tg.timing, tg.event, t.schema, t.name, tg.granularity, tg.function
      ));
    },
  }
  s.push_str("\n```\n");
  s
}

fn render_unknown(token: &str, kind: Kind) -> String {
  let hint = match kind {
    Kind::PrimaryKey => "pk_<table>_<columns>",
    Kind::ForeignKey => "fk_<table>_<column>_<ref_table>",
    Kind::Unique => "uq_<table>_<columns>",
    Kind::UniqueIndex => "uidx_<table>_<columns>",
    Kind::Index => "idx_<table>_<columns>",
    Kind::Check => "ch_<table>_<rule>",
    Kind::Trigger => "tg_<table>_<event>",
  };
  format!(
    "```sql\n-- {kind_label} (identifier): {token}\n-- No matching object in the catalog yet.\n-- Convention: {hint}\n```\n",
    kind_label = kind.label(),
  )
}

fn render_constraint(c: &Constraint) -> String {
  if let Some(def) = &c.definition {
    return format!("CONSTRAINT {} {def}", c.name);
  }
  let mut s = format!("CONSTRAINT {} ", c.name);
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
