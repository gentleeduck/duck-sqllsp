//! Context-aware special completions.
//!
//! Catches situations the phase state machine doesn't model:
//!   * `CREATE INDEX ... USING <method>`
//!   * `CREATE INDEX ... ON t (col <opclass>)`
//!   * `CREATE TRIGGER ... BEFORE|AFTER|INSTEAD OF <event>`
//!   * `CREATE TRIGGER ... ON <table>`
//!   * `CREATE TRIGGER ... EXECUTE [FUNCTION|PROCEDURE] <fn>`
//!   * `CALL <proc>`
//!   * `CREATE POLICY ... FOR <cmd>`
//!   * `CREATE POLICY ... TO <role>`
//!   * `ALTER TABLE t ALTER COLUMN c TYPE <type>`
//!
//! Runs before the phase state machine: returns `Some(items)` to
//! short-circuit; `None` to fall through.

use crate::item::{Item, ItemKind};
use crate::sources;
use dsl_catalog::Catalog;
use text_size::TextSize;

pub fn detect(source: &str, offset: TextSize, cat: &Catalog) -> Option<Vec<Item>> {
  let pos: usize = u32::from(offset) as usize;
  let pos = pos.min(source.len());
  // Walk back to the nearest statement boundary so we don't match
  // tokens from a previous statement.
  let stmt_start = source[..pos].rfind(';').map(|p| p + 1).unwrap_or(0);
  let stmt = &source[stmt_start..pos];
  let upper = stmt.to_ascii_uppercase();
  let _ = source;

  // ---- CREATE INDEX USING <method> ----
  if upper.contains("CREATE INDEX") || upper.contains("CREATE UNIQUE INDEX") {
    if let Some(items) = index_using_method(&upper) {
      return Some(items);
    }
    if let Some(items) = index_opclass(&upper, stmt) {
      return Some(items);
    }
  }

  // ---- CREATE TRIGGER ----
  if upper.contains("CREATE TRIGGER") || upper.contains("CREATE OR REPLACE TRIGGER") || upper.contains("CREATE CONSTRAINT TRIGGER") {
    if let Some(items) = trigger_event(&upper) {
      return Some(items);
    }
    if let Some(items) = trigger_on_table(&upper, cat) {
      return Some(items);
    }
    if let Some(items) = trigger_execute_function(&upper, source, cat) {
      return Some(items);
    }
  }

  // ---- CALL <procedure> ----
  if let Some(items) = call_procedure(&upper, cat) {
    return Some(items);
  }

  // ---- CREATE POLICY ----
  if upper.contains("CREATE POLICY") || upper.contains("CREATE OR REPLACE POLICY") || upper.contains("ALTER POLICY") {
    if let Some(items) = policy_for_command(&upper) {
      return Some(items);
    }
    if let Some(items) = policy_to_role(&upper, cat) {
      return Some(items);
    }
  }

  // ---- ALTER TABLE ALTER COLUMN ... TYPE <type> ----
  if upper.contains("ALTER TABLE") && upper.contains("ALTER COLUMN") {
    if let Some(items) = alter_column_type(&upper) {
      return Some(items);
    }
  }

  None
}

fn index_using_method(upper: &str) -> Option<Vec<Item>> {
  // Cursor right after `USING ` (or with partial method already typed).
  let using_at = upper.rfind("USING ")?;
  let after = &upper[using_at + 6..];
  // Bail if there's a paren already -- we're past the method.
  if after.contains('(') { return None }
  // Bail if there's a space after a non-whitespace token (method already complete).
  let trimmed = after.trim_start();
  if trimmed.split_whitespace().count() > 1 { return None }
  let mut out = Vec::new();
  for (m, doc) in [
    ("btree", "default index, supports = < > BETWEEN ORDER BY"),
    ("hash", "equality-only, smaller than btree"),
    ("gist", "geometry, full-text, range types"),
    ("gin", "jsonb, arrays, tsvector"),
    ("brin", "huge append-only tables (logs, time series)"),
    ("spgist", "non-balanced data: trees, points"),
  ] {
    out.push(Item {
      label: m.into(),
      kind: ItemKind::Keyword,
      detail: Some(doc.into()),
      description: Some("index method".into()),
      documentation_md: None,
      insert_text: m.into(),
      is_snippet: false,
      sort_priority: 1,
    });
  }
  Some(out)
}

fn index_opclass(upper: &str, stmt: &str) -> Option<Vec<Item>> {
  // Cursor inside `CREATE INDEX ... USING gin (col <here>)` -- need to be
  // past the column name with at least one whitespace.
  let using_at = upper.rfind("USING ")?;
  let after = &upper[using_at + 6..];
  let method: String = after.chars().take_while(|c| c.is_ascii_alphanumeric() || *c == '_').collect();
  let method_lc = method.to_ascii_lowercase();
  let Some(paren_rel) = after.find('(') else { return None };
  let paren_body = &after[paren_rel + 1..];
  // Find the last comma at depth 0 -- everything after is the current column entry.
  let mut depth = 0i32;
  let mut entry_start = 0usize;
  let bytes = paren_body.as_bytes();
  for (i, b) in bytes.iter().enumerate() {
    match b {
      b'(' => depth += 1,
      b')' => depth -= 1,
      b',' if depth == 0 => entry_start = i + 1,
      _ => {},
    }
  }
  let entry = &paren_body[entry_start..];
  let tokens: Vec<&str> = entry.split_whitespace().collect();
  if tokens.len() < 2 { return None }
  // After the column token, suggest opclasses appropriate to the method.
  let opclasses: Vec<(&str, &str)> = match method_lc.as_str() {
    "btree" => vec![
      ("text_pattern_ops", "LIKE 'prefix%' on text"),
      ("varchar_pattern_ops", "LIKE 'prefix%' on varchar"),
      ("bpchar_pattern_ops", "LIKE 'prefix%' on char(n)"),
    ],
    "gin" => vec![
      ("jsonb_path_ops", "smaller than jsonb_ops, only @> queries"),
      ("jsonb_ops", "default jsonb operator class"),
      ("array_ops", "default for ARRAY columns"),
      ("gin_trgm_ops", "pg_trgm: LIKE/ILIKE with wildcards"),
    ],
    "gist" => vec![
      ("gist_trgm_ops", "pg_trgm GIST variant"),
      ("range_ops", "range types"),
      ("inet_ops", "IP address range queries"),
    ],
    "hash" => vec![("array_ops", "array equality")],
    _ => return None,
  };
  let _ = stmt;
  let mut out = Vec::new();
  for (op, doc) in opclasses {
    out.push(Item {
      label: op.into(),
      kind: ItemKind::Keyword,
      detail: Some(doc.into()),
      description: Some(format!("{method_lc} operator class")),
      documentation_md: None,
      insert_text: op.into(),
      is_snippet: false,
      sort_priority: 1,
    });
  }
  Some(out)
}

fn trigger_event(upper: &str) -> Option<Vec<Item>> {
  // Match the most recent BEFORE / AFTER / INSTEAD OF in the statement.
  let kw = ["BEFORE ", "AFTER ", "INSTEAD OF "].into_iter().filter_map(|k| upper.rfind(k).map(|p| (p, k))).max_by_key(|x| x.0)?;
  let after = &upper[kw.0 + kw.1.len()..];
  // Bail if we already past the event (an OR / ON appears).
  if after.contains(" ON ") { return None }
  let toks: Vec<&str> = after.split_whitespace().collect();
  if toks.len() > 2 { return None }
  let mut out = Vec::new();
  for (ev, doc) in [
    ("INSERT", "row insertion"),
    ("UPDATE", "row update"),
    ("DELETE", "row deletion"),
    ("TRUNCATE", "table truncate (statement-level only)"),
  ] {
    out.push(Item {
      label: ev.into(),
      kind: ItemKind::Keyword,
      detail: Some(doc.into()),
      description: Some("trigger event".into()),
      documentation_md: None,
      insert_text: ev.into(),
      is_snippet: false,
      sort_priority: 1,
    });
  }
  Some(out)
}

fn trigger_on_table(upper: &str, cat: &Catalog) -> Option<Vec<Item>> {
  // Cursor immediately after `ON ` in a CREATE TRIGGER statement.
  let on_at = upper.rfind(" ON ")?;
  let after = &upper[on_at + 4..];
  // Bail if we're past the table (FOR / EXECUTE / WHEN keyword present).
  let post_upper = after.to_ascii_uppercase();
  for stop in ["FOR ", "EXECUTE", "WHEN", "REFERENCING"] {
    if post_upper.contains(stop) { return None }
  }
  let toks: Vec<&str> = after.split_whitespace().collect();
  if toks.len() > 1 { return None }
  let mut out = Vec::new();
  sources::tables(cat, &mut out);
  Some(out)
}

fn trigger_execute_function(upper: &str, stmt: &str, cat: &Catalog) -> Option<Vec<Item>> {
  let exec_at = upper.rfind("EXECUTE FUNCTION ").map(|p| p + "EXECUTE FUNCTION ".len())
    .or_else(|| upper.rfind("EXECUTE PROCEDURE ").map(|p| p + "EXECUTE PROCEDURE ".len()))?;
  let after = &upper[exec_at..];
  if after.contains('(') { return None }
  let toks: Vec<&str> = after.split_whitespace().collect();
  if toks.len() > 1 { return None }
  let mut out = Vec::new();
  // Catalog functions returning `trigger`.
  for f in &cat.functions {
    if !f.return_type.eq_ignore_ascii_case("trigger") { continue }
    out.push(Item {
      label: f.name.clone(),
      kind: ItemKind::Function,
      detail: Some(format!("returns trigger ({} args)", f.arguments.len())),
      description: Some(format!("{} (db)", f.schema)),
      documentation_md: None,
      insert_text: format!("{}()", f.name),
      is_snippet: false,
      sort_priority: 1,
    });
  }
  // Also harvest CREATE FUNCTION ... RETURNS trigger from the buffer.
  let upper_src = stmt.to_ascii_uppercase();
  for needle in ["CREATE OR REPLACE FUNCTION ", "CREATE FUNCTION "] {
    let mut from = 0;
    while let Some(rel) = upper_src[from..].find(needle) {
      let at = from + rel + needle.len();
      let after_kw = &stmt[at..];
      let name_end = after_kw.find(|c: char| !(c.is_ascii_alphanumeric() || c == '_' || c == '.')).unwrap_or(after_kw.len());
      let name = after_kw[..name_end].rsplit('.').next().unwrap_or(&after_kw[..name_end]).to_string();
      let tail_upper = upper_src[at..].to_string();
      if tail_upper.contains("RETURNS TRIGGER") {
        if !out.iter().any(|i| i.label == name) {
          out.push(Item {
            label: name.clone(),
            kind: ItemKind::Function,
            detail: Some("returns trigger (defined in this buffer)".into()),
            description: Some("source".into()),
            documentation_md: None,
            insert_text: format!("{name}()"),
            is_snippet: false,
            sort_priority: 1,
          });
        }
      }
      from = at + name_end;
    }
  }
  Some(out)
}

fn call_procedure(upper: &str, cat: &Catalog) -> Option<Vec<Item>> {
  // CALL is a top-level keyword introducing a procedure call.
  let trimmed = upper.trim_start();
  if !trimmed.starts_with("CALL ") { return None }
  let after = &trimmed[5..];
  if after.contains('(') { return None }
  let toks: Vec<&str> = after.split_whitespace().collect();
  if toks.len() > 1 { return None }
  // Catalog functions with no return type (typical of procedures);
  // also accept anything with `void` return.
  let mut out = Vec::new();
  for f in &cat.functions {
    if !(f.return_type.is_empty() || f.return_type.eq_ignore_ascii_case("void")) { continue }
    out.push(Item {
      label: f.name.clone(),
      kind: ItemKind::Function,
      detail: Some(format!("procedure ({} args)", f.arguments.len())),
      description: Some(format!("{} (db)", f.schema)),
      documentation_md: None,
      insert_text: if f.arguments.is_empty() { format!("{}()", f.name) } else { format!("{}($0)", f.name) },
      is_snippet: !f.arguments.is_empty(),
      sort_priority: 1,
    });
  }
  Some(out)
}

fn policy_for_command(upper: &str) -> Option<Vec<Item>> {
  let for_at = upper.rfind(" FOR ")?;
  let after = &upper[for_at + 5..];
  let toks: Vec<&str> = after.split_whitespace().collect();
  if toks.len() > 1 { return None }
  let mut out = Vec::new();
  for cmd in ["ALL", "SELECT", "INSERT", "UPDATE", "DELETE"] {
    out.push(Item {
      label: cmd.into(),
      kind: ItemKind::Keyword,
      detail: Some(format!("policy applies to {cmd}")),
      description: Some("policy command".into()),
      documentation_md: None,
      insert_text: cmd.into(),
      is_snippet: false,
      sort_priority: 1,
    });
  }
  Some(out)
}

fn policy_to_role(upper: &str, cat: &Catalog) -> Option<Vec<Item>> {
  let to_at = upper.rfind(" TO ")?;
  let after = &upper[to_at + 4..];
  if after.contains("USING") || after.contains("WITH CHECK") { return None }
  let toks: Vec<&str> = after.split_whitespace().collect();
  if toks.len() > 1 { return None }
  let mut out = Vec::new();
  out.push(Item {
    label: "PUBLIC".into(),
    kind: ItemKind::Keyword,
    detail: Some("every role".into()),
    description: Some("policy role".into()),
    documentation_md: None,
    insert_text: "PUBLIC".into(),
    is_snippet: false,
    sort_priority: 1,
  });
  for role in &cat.roles {
    out.push(Item {
      label: role.clone(),
      kind: ItemKind::Keyword,
      detail: Some("role".into()),
      description: Some("policy role".into()),
      documentation_md: None,
      insert_text: role.clone(),
      is_snippet: false,
      sort_priority: 2,
    });
  }
  Some(out)
}

fn alter_column_type(upper: &str) -> Option<Vec<Item>> {
  // `ALTER COLUMN c TYPE <here>` (with or without `SET DATA`).
  let after = if let Some(at) = upper.rfind(" SET DATA TYPE ") {
    &upper[at + 15..]
  } else if let Some(at) = upper.rfind(" TYPE ") {
    &upper[at + 6..]
  } else {
    return None;
  };
  if after.contains("USING") || after.contains(';') { return None }
  let toks: Vec<&str> = after.split_whitespace().collect();
  if toks.len() > 1 { return None }
  let mut out = Vec::new();
  for ty in [
    "text", "varchar", "char(1)", "integer", "bigint", "smallint", "serial", "bigserial",
    "numeric", "real", "double precision", "boolean", "uuid", "jsonb", "json",
    "date", "time", "timestamp", "timestamptz", "interval", "bytea", "inet", "cidr",
  ] {
    out.push(Item {
      label: ty.into(),
      kind: ItemKind::Type,
      detail: Some("data type".into()),
      description: Some("ALTER COLUMN target".into()),
      documentation_md: None,
      insert_text: ty.into(),
      is_snippet: false,
      sort_priority: 1,
    });
  }
  Some(out)
}
