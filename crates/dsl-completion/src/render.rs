//! Markdown renderers for table and column completion items.
//!
//! Keyword / type / function documentation is delegated to
//! [`dsl_knowledge::render_markdown`] so we have one canonical source of
//! truth for those bodies.

use dsl_catalog::{Column, Table};

pub fn table(t: &Table) -> String {
    let mut s = format!("**table** `{}.{}`\n\n", t.schema, t.name);
    if t.columns.is_empty() {
        s.push_str("_no columns cached -- run :DBRefresh after switching connections_\n");
        return s;
    }
    s.push_str("| column | type |\n");
    s.push_str("|--------|------|\n");
    for c in &t.columns {
        s.push_str(&format!("| `{}` | `{}` |\n", c.name, c.data_type));
    }
    s
}

pub fn column(t: &Table, c: &Column) -> String {
    // Blank lines between fields so markdown renders them as separate
    // paragraphs instead of collapsing onto one line.
    format!(
        "**column** `{}.{}.{}`\n\n\
         - **type:** `{}`\n\
         - **nullable:** `{}`\n",
        t.schema, t.name, c.name, c.data_type, c.nullable
    )
}
