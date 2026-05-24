//! A single FROM / JOIN binding inside a statement.

use dsl_parse::TableRef;
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct Binding {
    /// The alias the user typed, or the unaliased table name if no alias.
    pub alias: String,
    /// The fully-qualified table being referred to.
    pub table: TableRef,
}
