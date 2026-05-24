//! SQL formatting business logic for duck-sqllsp.
//!
//! Two layers cooperate:
//!
//!   1. [`external::run_sql_formatter`] -- shells out to the `sql-formatter`
//!      npm CLI (when present) for the bulk reflow / keyword casing /
//!      expression-width wrapping work that a dedicated grammar-aware tool
//!      already solves well.
//!   2. [`align::rewrite`] -- DataGrip-style CREATE TABLE / FUNCTION /
//!      TRIGGER / INDEX post-pass that aligns columns into padded sub-
//!      columns and breaks long clause-chained headers onto their own
//!      lines.
//!
//! The composite [`format`] entry runs both in order. Server handlers stay
//! thin shims that read the document, call this, build a TextEdit.

pub mod align;
pub mod external;
pub mod style;

pub use align::rewrite;
pub use external::run_sql_formatter;
pub use style::{CreateTableStyle, FormatterStyle};

/// One-shot format pipeline: external sql-formatter first (when available),
/// DataGrip-style alignment second. Falls back to passing the input through
/// unchanged when the external binary is missing -- callers can then
/// decide whether to skip emitting a no-op TextEdit.
pub fn format(input: &str, fmt_style: &FormatterStyle, ct_style: &CreateTableStyle) -> String {
    let after_external = external::run_sql_formatter(input, fmt_style)
        .unwrap_or_else(|| input.to_string());
    align::rewrite(&after_external, ct_style)
}
