//! Every accepted shape of `.duck-sqllsp.toml` / `.duck-sqllsp.json` /
//! `initializationOptions` must actually apply.
//!
//! Two of these used to silently fall back to defaults. Both paths
//! guessed "wrapped or bare?" by trying the wrapped parse and seeing
//! whether it produced anything interesting -- which is not a test of
//! shape at all, because serde skips unknown fields and happily returns
//! an all-defaults value. The user got no error, just settings that did
//! nothing.
//!
//! The values below are all deliberately *different from the defaults*.
//! An earlier version of this test used `keywordCase = "upper"`, which
//! is the default, so it passed against a config that was being thrown
//! away entirely.

use dsl_server::config::{self, Case};

fn json_of(toml_src: &str) -> serde_json::Value {
  let v: toml::Value = toml::from_str(toml_src).expect("valid toml");
  serde_json::to_value(&v).expect("json-able")
}

/// Non-default settings, in wrapped and bare form.
const WRAPPED_STYLE_ONLY: &str = r#"
[duck_sqllsp.style]
keywordCase = "lower"
[duck_sqllsp.style.formatter]
singleLine = true
tabWidth = 8
"#;

const BARE_STYLE_ONLY: &str = r#"
[style]
keywordCase = "lower"
[style.formatter]
singleLine = true
tabWidth = 8
"#;

const WRAPPED_WITH_CONNECTION: &str = r#"
[duck_sqllsp]
activeConnection = "local"
[duck_sqllsp.style]
keywordCase = "lower"
[duck_sqllsp.style.formatter]
singleLine = true
tabWidth = 8
"#;

fn assert_applied(tag: &str, cfg: &config::DuckSqllspConfig) {
  assert_eq!(cfg.style.keyword, Case::Lower, "{tag}: keywordCase was dropped");
  assert!(cfg.style.formatter.single_line, "{tag}: singleLine was dropped");
  assert_eq!(cfg.style.formatter.tab_width, 8, "{tag}: tabWidth was dropped");
}

#[test]
fn json_wrapped_style_only_is_applied() {
  // The regression: no connection, no activeConnection, no scope.
  assert_applied("json wrapped style-only", &config::parse(json_of(WRAPPED_STYLE_ONLY)).duck_sqllsp);
}

#[test]
fn json_bare_style_only_is_applied() {
  assert_applied("json bare style-only", &config::parse(json_of(BARE_STYLE_ONLY)).duck_sqllsp);
}

#[test]
fn json_wrapped_with_a_connection_is_applied() {
  let cfg = config::parse(json_of(WRAPPED_WITH_CONNECTION)).duck_sqllsp;
  assert_applied("json wrapped + connection", &cfg);
  assert_eq!(cfg.active_connection.as_deref(), Some("local"));
}

/// `.duck-sqllsp.toml` in a scratch directory, read the way the server
/// reads it.
fn load_toml(tag: &str, body: &str) -> config::DuckSqllspConfig {
  let dir = std::env::temp_dir().join(format!("duck-sqllsp-cfg-{tag}-{}", std::process::id()));
  let _ = std::fs::remove_dir_all(&dir);
  std::fs::create_dir_all(&dir).expect("scratch dir");
  std::fs::write(dir.join(".duck-sqllsp.toml"), body).expect("write config");
  let cfg = config::load_project_config(&dir).unwrap_or_else(|| panic!("{tag}: no config loaded"));
  let _ = std::fs::remove_dir_all(&dir);
  cfg
}

#[test]
fn toml_wrapped_style_only_is_applied() {
  assert_applied("toml wrapped style-only", &load_toml("wrapped", WRAPPED_STYLE_ONLY));
}

#[test]
fn toml_bare_style_only_is_applied() {
  // The other regression: a bare file parsed "successfully" as a
  // wrapped one whose only field defaulted, so nothing applied.
  assert_applied("toml bare style-only", &load_toml("bare", BARE_STYLE_ONLY));
}

#[test]
fn snake_case_aliases_still_work() {
  // Both spellings are documented; neither may quietly stop working.
  let cfg = config::parse(json_of(
    r#"
[duck_sqllsp]
active_connection = "local"
require_connection = false
[duck_sqllsp.style]
keyword_case = "lower"
[duck_sqllsp.style.formatter]
tab_width = 8
"#,
  ))
  .duck_sqllsp;
  assert_eq!(cfg.style.keyword, Case::Lower, "keyword_case alias");
  assert_eq!(cfg.style.formatter.tab_width, 8, "tab_width alias");
  assert_eq!(cfg.active_connection.as_deref(), Some("local"));
  assert!(!cfg.require_connection, "require_connection alias");
}

#[test]
fn an_empty_config_yields_defaults_rather_than_failing() {
  let cfg = config::parse(json_of("")).duck_sqllsp;
  assert_eq!(cfg.style.keyword, Case::Upper);
  assert!(cfg.connections.is_empty());
}

#[test]
fn per_rule_severity_overrides_are_read() {
  let cfg = config::parse(json_of(
    r#"
[duck_sqllsp.rules]
sql015 = "off"
sql001 = "hint"
"#,
  ))
  .duck_sqllsp;
  assert_eq!(cfg.rules.get("sql015").map(String::as_str), Some("off"));
  assert_eq!(cfg.rules.get("sql001").map(String::as_str), Some("hint"));
}

/// The "complete example" block in `dsl-server/docs/configuration.md` must parse,
/// and every value in it must survive into the config.
///
/// Documentation that silently stops matching the code is worse than no
/// documentation, and this file's whole subject is settings that looked
/// applied but weren't. Extracting the example from the doc itself means
/// the two cannot drift.
#[test]
fn the_documented_example_config_parses_and_applies() {
  let doc = include_str!("../docs/configuration.md");
  let example = doc
    .split("## A complete example")
    .nth(1)
    .expect("docs/configuration.md must keep its `## A complete example` section")
    .split("```toml")
    .nth(1)
    .expect("that section must contain a ```toml block")
    .split("```")
    .next()
    .expect("unterminated toml block");

  let cfg = config::parse(json_of(example)).duck_sqllsp;

  assert_eq!(cfg.active_connection.as_deref(), Some("local"));
  assert!(!cfg.require_connection);
  assert_eq!(cfg.rules.get("sql015").map(String::as_str), Some("off"));
  assert_eq!(cfg.style.keyword, Case::Upper);
  assert_eq!(cfg.style.function, Case::Lower);
  assert_eq!(cfg.style.identifier, Case::Preserve);
  assert_eq!(cfg.style.create_table.column_gap, 4);
  assert!(cfg.style.create_table.group_indexes);
  assert_eq!(cfg.style.formatter.tab_width, 2);
  assert_eq!(cfg.style.formatter.expression_width, 100);
  assert_eq!(cfg.style.formatter.logical_operator_newline, "before");
  assert!(!cfg.style.formatter.single_line);
  assert_eq!(cfg.connections.len(), 1);
  assert_eq!(cfg.connections[0].name, "local");
}

/// Every key the reference documents must be one the server reads.
///
/// Catches the reverse drift: a table row for a setting that was renamed
/// or removed, which sends people chasing a key that does nothing.
#[test]
fn every_documented_key_is_understood_by_the_parser() {
  let doc = include_str!("../docs/configuration.md");
  // Keys are the first cell of each table row, in backticks.
  let documented: Vec<String> = doc
    .lines()
    .filter(|l| l.starts_with("| `"))
    .filter_map(|l| l.split('`').nth(1).map(str::to_string))
    .filter(|k| !k.contains("://") && !k.contains(' '))
    .collect();
  assert!(documented.len() > 20, "expected a full table, found {documented:?}");

  let known: &[&str] = &[
    "activeConnection",
    "connections",
    "dialect",
    "requireConnection",
    "scope",
    "rules",
    "style",
    "keywordCase",
    "functionCase",
    "typeCase",
    "identifierCase",
    "alignColumns",
    "openParenOnNewLine",
    "constraintsAtEnd",
    "columnGap",
    "groupIndexes",
    "language",
    "tabWidth",
    "dataTypeCase",
    "linesBetweenQueries",
    "expressionWidth",
    "denseOperators",
    "newlineBeforeSemicolon",
    "logicalOperatorNewline",
    "singleLine",
    "compactClauses",
  ];
  for key in &documented {
    assert!(known.contains(&key.as_str()), "docs/configuration.md documents unknown key `{key}`");
  }
  for key in known {
    assert!(documented.iter().any(|d| d == key), "config key `{key}` is not documented");
  }
}
