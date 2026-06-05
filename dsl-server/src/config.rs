//! LSP configuration with three layered sources, last wins:
//!
//!   1. `initializationOptions` (editor pushes known connections).
//!   2. `workspace/didChangeConfiguration` mid-session.
//!   3. Project-level `.duck-sqllsp.toml` or `.duck-sqllsp.json` walked
//!      upward from the workspace root or any open SQL file.
//!
//! All three feed the same [`DuckSqllspConfig`].

use dsl_conn::ConnectionSpec;
use serde::Deserialize;
use std::path::{Path, PathBuf};

// Re-export so external callers can still write `config::CreateTableStyle`
// without caring that the type physically lives in `dsl-format`.
pub use dsl_format::{CreateTableStyle, FormatterStyle};

#[derive(Debug, Clone, Default, Deserialize)]
pub struct RootConfig {
  #[serde(rename = "duckSqllsp", alias = "duck_sqllsp", default)]
  pub duck_sqllsp: DuckSqllspConfig,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct DuckSqllspConfig {
  #[serde(default)]
  pub connections: Vec<ConnectionSpec>,
  #[serde(default, rename = "activeConnection", alias = "active_connection")]
  pub active_connection: Option<String>,
  #[serde(default)]
  pub scope: Option<String>,
  /// SQL dialect for completion + diagnostics. Auto-derived from the
  /// active connection's `driver` field when omitted; defaults to
  /// `postgresql` when no connection is set.
  #[serde(default)]
  pub dialect: Option<Dialect>,
  /// When true, suppress catalog-dependent diagnostics (sql001 unresolved
  /// table, sql002 unknown column) if no connection is active. Default
  /// true -- keeps things quiet during offline editing.
  #[serde(default = "yes", rename = "requireConnection", alias = "require_connection")]
  pub require_connection: bool,
  #[serde(default)]
  pub style: Style,
  /// Per-rule severity overrides. Keys are diagnostic codes (e.g.
  /// `sql001`); values are `"error"` / `"warning"` / `"info"` /
  /// `"hint"` / `"off"`. The server applies overrides post-rule so
  /// rule implementations don't need to know about config.
  #[serde(default)]
  pub rules: std::collections::HashMap<String, String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
#[derive(Default)]
pub enum Dialect {
  #[default]
  #[serde(alias = "postgres", alias = "pg")]
  Postgresql,
  #[serde(alias = "mariadb")]
  Mysql,
  #[serde(alias = "sqlite3")]
  Sqlite,
  #[serde(alias = "sqlserver", alias = "tsql", alias = "transactsql")]
  Mssql,
}

impl DuckSqllspConfig {
  /// Resolve the effective dialect: explicit config field beats the
  /// active connection's driver beats the postgresql default.
  pub fn effective_dialect(&self) -> Dialect {
    if let Some(d) = self.dialect {
      return d;
    }
    if let Some(active) = self.active() {
      return match active.driver() {
        "postgres" => Dialect::Postgresql,
        "mysql" => Dialect::Mysql,
        "sqlite" => Dialect::Sqlite,
        _ => Dialect::Postgresql,
      };
    }
    Dialect::Postgresql
  }
}

/// Casing for completion item labels and inserts. Defaults follow the
/// most common Postgres style guide: keywords UPPER, types UPPER,
/// functions lower, identifiers preserve.
#[derive(Debug, Clone, Deserialize)]
pub struct Style {
  #[serde(rename = "keywordCase", alias = "keyword_case", default = "default_keyword")]
  pub keyword: Case,
  #[serde(rename = "functionCase", alias = "function_case", default = "default_function")]
  pub function: Case,
  #[serde(rename = "typeCase", alias = "type_case", default = "default_type")]
  pub type_: Case,
  #[serde(rename = "identifierCase", alias = "identifier_case", default = "default_identifier")]
  pub identifier: Case,
  #[serde(rename = "createTable", alias = "create_table", default)]
  pub create_table: CreateTableStyle,
  #[serde(rename = "formatter", default)]
  pub formatter: FormatterStyle,
}

impl Default for Style {
  fn default() -> Self {
    Self {
      keyword: default_keyword(),
      function: default_function(),
      type_: default_type(),
      identifier: default_identifier(),
      create_table: CreateTableStyle::default(),
      formatter: FormatterStyle::default(),
    }
  }
}

// `CreateTableStyle` and `FormatterStyle` live in `dsl-format::style` so
// the formatter API has no hidden dependency back into this crate. They
// are re-exported above via `pub use dsl_format::{...}`.

fn yes() -> bool {
  true
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Case {
  Upper,
  Lower,
  Preserve,
}

fn default_keyword() -> Case {
  Case::Upper
}
fn default_function() -> Case {
  Case::Lower
}
fn default_type() -> Case {
  Case::Upper
}
fn default_identifier() -> Case {
  Case::Preserve
}

impl Case {
  pub fn apply(self, s: &str) -> String {
    match self {
      Case::Upper => s.to_ascii_uppercase(),
      Case::Lower => s.to_ascii_lowercase(),
      Case::Preserve => s.to_string(),
    }
  }
}

impl DuckSqllspConfig {
  pub fn active(&self) -> Option<&ConnectionSpec> {
    let name = self.active_connection.as_deref()?;
    self.connections.iter().find(|c| c.name == name)
  }

  /// Merge another config into self; `other` wins on every field it sets.
  pub fn merge_from(&mut self, other: DuckSqllspConfig) {
    if !other.connections.is_empty() {
      self.connections = other.connections;
    }
    if other.active_connection.is_some() {
      self.active_connection = other.active_connection;
    }
    if other.scope.is_some() {
      self.scope = other.scope;
    }
    if other.dialect.is_some() {
      self.dialect = other.dialect;
    }
    // require_connection default is true; only override when other
    // explicitly disables it. Preserve self otherwise.
    if !other.require_connection {
      self.require_connection = false;
    }
    // Style overrides only the fields that diverge from the default.
    let default = Style::default();
    if other.style.keyword != default.keyword {
      self.style.keyword = other.style.keyword;
    }
    if other.style.function != default.function {
      self.style.function = other.style.function;
    }
    if other.style.type_ != default.type_ {
      self.style.type_ = other.style.type_;
    }
    if other.style.identifier != default.identifier {
      self.style.identifier = other.style.identifier;
    }
    // Formatter + CreateTable: overlay wholesale only when the
    // incoming block differs from defaults. Field-level merging
    // would require knowing whether each scalar was explicitly
    // set (serde::default zeroes that signal); the wholesale
    // overlay matches the "last source wins" intent of the
    // didChangeConfiguration / project-file pipeline.
    if other.style.create_table != default.create_table {
      self.style.create_table = other.style.create_table;
    }
    if other.style.formatter != default.formatter {
      self.style.formatter = other.style.formatter;
    }
  }
}

/// Parse JSON values from initialize / didChangeConfiguration / project file.
/// Accepts both wrapped (`duckSqllsp: { ... }`) and bare root forms.
pub fn parse(value: serde_json::Value) -> RootConfig {
  if let Ok(cfg) = serde_json::from_value::<RootConfig>(value.clone())
    && (!cfg.duck_sqllsp.connections.is_empty()
      || cfg.duck_sqllsp.active_connection.is_some()
      || cfg.duck_sqllsp.scope.is_some())
  {
    return cfg;
  }
  let inner: DuckSqllspConfig = serde_json::from_value(value).unwrap_or_default();
  RootConfig { duck_sqllsp: inner }
}

/// Walk upward from `start` looking for a project config. Prefers
/// `.duck-sqllsp.toml` then `.duck-sqllsp.json`. Stops at filesystem root.
pub fn load_project_config(start: &Path) -> Option<DuckSqllspConfig> {
  let mut dir: PathBuf = if start.is_dir() { start.to_path_buf() } else { start.parent()?.to_path_buf() };
  loop {
    let toml_path = dir.join(".duck-sqllsp.toml");
    if toml_path.is_file()
      && let Ok(text) = std::fs::read_to_string(&toml_path)
    {
      if let Ok(parsed) = toml::from_str::<RootConfig>(&text) {
        tracing::info!(path = %toml_path.display(), single_line = parsed.duck_sqllsp.style.formatter.single_line, "loaded .duck-sqllsp.toml");
        return Some(parsed.duck_sqllsp);
      }
      if let Ok(inner) = toml::from_str::<DuckSqllspConfig>(&text) {
        tracing::info!(path = %toml_path.display(), single_line = inner.style.formatter.single_line, "loaded .duck-sqllsp.toml (bare)");
        return Some(inner);
      }
    }
    let json_path = dir.join(".duck-sqllsp.json");
    if json_path.is_file() {
      let bytes = std::fs::read(&json_path).ok()?;
      let value: serde_json::Value = serde_json::from_slice(&bytes).ok()?;
      return Some(parse(value).duck_sqllsp);
    }
    if !dir.pop() {
      return None;
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn merge_overlays_formatter_when_nondefault() {
    let mut base = DuckSqllspConfig::default();
    let mut other = DuckSqllspConfig::default();
    other.style.formatter.tab_width = 8;
    other.style.formatter.keyword_case = "lower".into();
    base.merge_from(other);
    assert_eq!(base.style.formatter.tab_width, 8);
    assert_eq!(base.style.formatter.keyword_case, "lower");
  }

  #[test]
  fn merge_leaves_formatter_alone_when_other_is_default() {
    let mut base = DuckSqllspConfig::default();
    base.style.formatter.tab_width = 2;
    let other = DuckSqllspConfig::default();
    base.merge_from(other);
    assert_eq!(base.style.formatter.tab_width, 2, "default `other` must not clobber base");
  }

  #[test]
  fn merge_overlays_create_table_block() {
    let mut base = DuckSqllspConfig::default();
    let mut other = DuckSqllspConfig::default();
    other.style.create_table.column_gap = 1;
    other.style.create_table.align_columns = false;
    base.merge_from(other);
    assert_eq!(base.style.create_table.column_gap, 1);
    assert!(!base.style.create_table.align_columns);
  }

  #[test]
  fn parse_accepts_nested_formatter_block() {
    let json = serde_json::json!({
        "style": {
            "formatter": { "tabWidth": 8, "keywordCase": "lower" },
            "createTable": { "columnGap": 1 }
        }
    });
    let cfg = parse(json).duck_sqllsp;
    assert_eq!(cfg.style.formatter.tab_width, 8);
    assert_eq!(cfg.style.formatter.keyword_case, "lower");
    assert_eq!(cfg.style.create_table.column_gap, 1);
  }
}
