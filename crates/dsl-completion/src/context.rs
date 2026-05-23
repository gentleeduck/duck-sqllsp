//! Detect what kinds of completion items make sense at the cursor.
//!
//! Token-only classifier: reads the text to the left of the cursor and
//! recognises the high-value cases. Doesn't depend on the parser
//! succeeding so we keep working on half-written SQL.
//!
//! | Cursor position             | Context        |
//! |-----------------------------|----------------|
//! | `<alias>.<partial>`         | `DotOf`        |
//! | after FROM / JOIN / INTO / UPDATE / ALTER TABLE | `Table` |
//! | after ON / WHERE / HAVING / AND / OR  | `Predicate` (columns + funcs + keywords) |
//! | start of statement          | `Statement` (keywords-focused) |
//! | inside SELECT list (after SELECT, before FROM) | `Projection` (columns of FROM + funcs) |
//! | anywhere else               | `General` (everything merged) |

use text_size::TextSize;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Context {
    DotOf { alias: String },
    Table,
    Projection,
    Predicate,
    Statement,
    General,
}

pub fn detect(source: &str, offset: TextSize) -> Context {
    let pos: usize = offset.into();
    let before = &source[..pos.min(source.len())];

    if let Some(ctx) = dot_context(before) {
        return ctx;
    }
    if is_after_keyword(before, &["FROM", "JOIN", "INTO", "UPDATE", "TABLE", "INSERT INTO", "ALTER TABLE"]) {
        return Context::Table;
    }
    if is_after_keyword(before, &["ON", "WHERE", "HAVING", "AND", "OR"]) {
        return Context::Predicate;
    }
    // Inside SELECT projection list: between SELECT and FROM on the same
    // statement.
    if in_projection_list(before) {
        return Context::Projection;
    }
    // Start of a statement (only whitespace / semicolon before cursor's word).
    if at_statement_start(before) {
        return Context::Statement;
    }
    Context::General
}

fn dot_context(before: &str) -> Option<Context> {
    let dot_idx = before.rfind('.')?;
    let after_dot = &before[dot_idx + 1..];
    if !after_dot.chars().all(|c| c.is_alphanumeric() || c == '_') {
        return None;
    }
    let pre_dot = &before[..dot_idx];
    let alias: String = pre_dot
        .chars()
        .rev()
        .take_while(|c| c.is_alphanumeric() || *c == '_')
        .collect::<String>()
        .chars()
        .rev()
        .collect();
    if alias.is_empty() { return None; }
    Some(Context::DotOf { alias })
}

/// Strip the partial token under the cursor, then check whether the
/// preceding tokens end with any of `keywords` (case-insensitive).
fn is_after_keyword(before: &str, keywords: &[&str]) -> bool {
    let cur = before
        .chars()
        .rev()
        .take_while(|c| c.is_alphanumeric() || *c == '_')
        .collect::<String>();
    let cut = before.len() - cur.len();
    let trimmed_upper = before[..cut].trim_end().to_uppercase();
    keywords.iter().any(|kw| trimmed_upper.ends_with(kw))
}

/// Heuristic: between the most recent SELECT and the next FROM (or end of
/// buffer), we are likely in a projection list.
fn in_projection_list(before: &str) -> bool {
    let upper = before.to_uppercase();
    let select_idx = upper.rfind("SELECT");
    let Some(sel) = select_idx else { return false; };
    // If there is a FROM after the SELECT we just found, we're past it.
    if let Some(from) = upper[sel + 6..].find("FROM") {
        // We're past FROM only if there's no newer SELECT after it.
        let from_abs = sel + 6 + from;
        if upper[from_abs..].rfind("SELECT").is_none() {
            return false;
        }
    }
    // Need to actually be inside whitespace or after a comma in the list.
    let last = before.chars().rev().find(|c| !c.is_whitespace());
    matches!(last, Some(',') | Some('(') | None) || true
}

fn at_statement_start(before: &str) -> bool {
    let cur = before
        .chars()
        .rev()
        .take_while(|c| c.is_alphanumeric() || *c == '_')
        .collect::<String>();
    let cut = before.len() - cur.len();
    let head = before[..cut].trim_end();
    head.is_empty() || head.ends_with(';')
}
