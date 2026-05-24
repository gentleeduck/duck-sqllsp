//! Configuration shapes for the two formatting layers.
//!
//! These types are deserialised by `dsl-server::config` from LSP
//! `initializationOptions`, `workspace/didChangeConfiguration` and
//! project-level config files. They live here -- not in the server crate
//! -- so the formatter API has no hidden dependency back into the LSP
//! wire layer.

use serde::Deserialize;

/// Column-aligned CREATE TABLE output. Mirrors the DataGrip-style block
/// the user prefers: `(` on its own line, name / type / NOT NULL /
/// DEFAULT all padded to column width, constraints last.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct CreateTableStyle {
    #[serde(rename = "alignColumns", alias = "align_columns", default = "yes")]
    pub align_columns: bool,
    #[serde(rename = "openParenOnNewLine", alias = "open_paren_on_new_line", default = "yes")]
    pub open_paren_on_new_line: bool,
    #[serde(rename = "constraintsAtEnd", alias = "constraints_at_end", default = "yes")]
    pub constraints_at_end: bool,
    /// Visual gap between consecutive columns. 1 keeps things tight,
    /// 2-4 is more readable. Default 4.
    #[serde(rename = "columnGap", alias = "column_gap", default = "default_column_gap")]
    pub column_gap: usize,
    /// Tightly pack consecutive CREATE INDEX statements (no blank
    /// separators between them). Defaults on.
    #[serde(rename = "groupIndexes", alias = "group_indexes", default = "yes")]
    pub group_indexes: bool,
}

impl Default for CreateTableStyle {
    fn default() -> Self {
        Self {
            align_columns: true,
            open_paren_on_new_line: true,
            constraints_at_end: true,
            column_gap: default_column_gap(),
            group_indexes: true,
        }
    }
}

/// External SQL formatter (`sql-formatter` v15+) tuning. Every key maps
/// directly onto a JSON option understood by the binary; we serialise
/// them at handler time. Add a field, get a knob -- no string literals.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct FormatterStyle {
    /// SQL dialect passed via `-l` (postgresql, mysql, sqlite, ...).
    #[serde(rename = "language", alias = "language", default = "default_lang")]
    pub language: String,
    /// `--tab-width` equivalent (sql-formatter v15 JSON key `tabWidth`).
    #[serde(rename = "tabWidth", alias = "tab_width", default = "default_tab_width")]
    pub tab_width: usize,
    /// `keywordCase` JSON key. Accepts "upper" / "lower" / "preserve".
    #[serde(rename = "keywordCase", alias = "keyword_case", default = "default_kw_case")]
    pub keyword_case: String,
    /// Blank lines between statements. Default 1.
    #[serde(rename = "linesBetweenQueries", alias = "lines_between_queries", default = "default_lines")]
    pub lines_between_queries: usize,
    /// `dataTypeCase` -- upper / lower / preserve.
    #[serde(rename = "dataTypeCase", alias = "data_type_case", default = "default_data_case")]
    pub data_type_case: String,
    /// `functionCase` -- upper / lower / preserve.
    #[serde(rename = "functionCase", alias = "function_case", default = "default_fn_case")]
    pub function_case: String,
    /// `denseOperators` -- collapse spaces around `=`, `<>` etc.
    #[serde(rename = "denseOperators", alias = "dense_operators", default)]
    pub dense_operators: bool,
    /// `newlineBeforeSemicolon` -- newline before trailing `;`.
    #[serde(rename = "newlineBeforeSemicolon", alias = "newline_before_semicolon", default)]
    pub newline_before_semicolon: bool,
    /// `expressionWidth` -- max line width before sql-formatter wraps
    /// long expressions (SELECT projections, WHERE/ON conjunctions,
    /// VALUES lists). Default 80 keeps lines under the standard PG/SQL
    /// review column. Set lower to break more aggressively.
    #[serde(rename = "expressionWidth", alias = "expression_width", default = "default_expr_width")]
    pub expression_width: usize,
    /// `logicalOperatorNewline` -- "before" puts `AND`/`OR` at the start
    /// of the new line (PG / DataGrip default), "after" leaves them at
    /// the end of the previous line.
    #[serde(rename = "logicalOperatorNewline", alias = "logical_operator_newline", default = "default_lon")]
    pub logical_operator_newline: String,
}

impl Default for FormatterStyle {
    fn default() -> Self {
        Self {
            language: default_lang(),
            tab_width: default_tab_width(),
            keyword_case: default_kw_case(),
            lines_between_queries: default_lines(),
            data_type_case: default_data_case(),
            function_case: default_fn_case(),
            dense_operators: false,
            newline_before_semicolon: false,
            expression_width: default_expr_width(),
            logical_operator_newline: default_lon(),
        }
    }
}

impl FormatterStyle {
    /// Serialise to the JSON string sql-formatter expects via `-c`.
    pub fn to_json(&self) -> String {
        // Hand-rolled so we keep zero JSON dependencies in the hot path
        // and the key names match sql-formatter v15 verbatim.
        format!(
            r#"{{"tabWidth":{},"keywordCase":"{}","linesBetweenQueries":{},"dataTypeCase":"{}","functionCase":"{}","denseOperators":{},"newlineBeforeSemicolon":{},"expressionWidth":{},"logicalOperatorNewline":"{}"}}"#,
            self.tab_width,
            escape(&self.keyword_case),
            self.lines_between_queries,
            escape(&self.data_type_case),
            escape(&self.function_case),
            self.dense_operators,
            self.newline_before_semicolon,
            self.expression_width,
            escape(&self.logical_operator_newline),
        )
    }
}

fn escape(s: &str) -> String { s.replace('"', "\\\"") }

fn yes() -> bool { true }
fn default_column_gap() -> usize { 4 }
fn default_lang()      -> String { "postgresql".into() }
fn default_tab_width() -> usize { 4 }
fn default_kw_case()   -> String { "upper".into() }
fn default_lines()     -> usize { 1 }
fn default_data_case() -> String { "preserve".into() }
fn default_fn_case()   -> String { "lower".into() }
fn default_expr_width() -> usize { 80 }
fn default_lon()       -> String { "before".into() }
