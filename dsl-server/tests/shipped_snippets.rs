//! The snippets the VS Code extension ships must be valid SQL.
//!
//! They are user-facing in the most direct way possible: accept one, tab
//! through without renaming anything, and whatever it produced is now in
//! your buffer being analysed by this very server. Two of them used to
//! expand to a syntax error, because `table` and `column` are reserved
//! words in PostgreSQL:
//!
//!     SELECT * FROM table WHERE condition;      -- sql000
//!     CREATE INDEX ix_name ON table_name (column);  -- sql000
//!
//! Nothing checked them, so nothing caught it. This lints every shipped
//! snippet with the engine that would flag it in the editor.
//!
//! A third check -- "no placeholder default is a reserved word" -- was
//! tried and dropped. `dsl_knowledge::keywords()` does not distinguish
//! reserved from unreserved, and PostgreSQL's unreserved keywords are
//! legal identifiers, so it flagged `${1:name}` and `${3:value}`, which
//! are fine. Parsing the expansion is the empirical version of the same
//! question and does not need the distinction.

use dsl_server::handlers::completion::render_snippet_preview;
use std::path::{Path, PathBuf};

fn snippets_path() -> PathBuf {
  Path::new(env!("CARGO_MANIFEST_DIR")).parent().expect("workspace root").join("vscode-extension/snippets/sql.json")
}

/// `(name, prefix, rendered SQL)` for every shipped snippet.
fn shipped() -> Vec<(String, String, String)> {
  let text = std::fs::read_to_string(snippets_path()).expect("snippets file");
  let json: serde_json::Value = serde_json::from_str(&text).expect("valid JSON");
  let obj = json.as_object().expect("snippet map");
  obj
    .iter()
    .map(|(name, v)| {
      let body = match &v["body"] {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Array(a) => a.iter().map(|l| l.as_str().unwrap_or_default()).collect::<Vec<_>>().join("\n"),
        other => panic!("{name}: unexpected body shape {other:?}"),
      };
      let prefix = v["prefix"].as_str().unwrap_or_default().to_string();
      (name.clone(), prefix, render_snippet_preview(&body))
    })
    .collect()
}

fn diagnostics_for(sql: &str) -> Vec<String> {
  let file = dsl_parse::parse(sql, dsl_parse::Dialect::Postgres);
  let scopes = dsl_resolve::resolve_with_source(&file.statements, sql);
  let catalog = dsl_catalog::Catalog::default();
  dsl_analysis::run(sql, &file, &scopes, &catalog).into_iter().map(|d| format!("{} {}", d.code, d.message)).collect()
}

#[test]
fn every_shipped_snippet_parses() {
  let snippets = shipped();
  assert!(snippets.len() >= 10, "expected the full snippet set, found {}", snippets.len());

  let mut broken = Vec::new();
  for (name, prefix, sql) in &snippets {
    let all = diagnostics_for(sql);
    let syntax: Vec<&String> = all.iter().filter(|d| d.starts_with("sql000")).collect();
    if let Some(first) = syntax.first() {
      broken.push(format!("{name} (prefix `{prefix}`):\n{sql}\n  -> {first}"));
    }
  }
  assert!(broken.is_empty(), "snippets that expand to invalid SQL:\n\n{}", broken.join("\n\n"));
}

/// Beyond parsing: a shipped snippet should not trip our own lint rules
/// at their default placeholders. Being told off by the same server that
/// offered the snippet is a bad first impression.
#[test]
fn no_shipped_snippet_trips_an_error_level_rule() {
  let mut offenders = Vec::new();
  for (name, prefix, sql) in shipped() {
    let errors: Vec<String> = diagnostics_for(&sql);
    if !errors.is_empty() {
      offenders.push(format!("{name} (prefix `{prefix}`) -> {}", errors.join(", ")));
    }
  }
  assert!(offenders.is_empty(), "snippets flagged by our own rules:\n{}", offenders.join("\n"));
}
