//! sql017: SELECT mixes aggregates with bare column references but the
//! bare columns are not all listed in GROUP BY. Postgres treats this as
//! an error at execution time; we surface it at edit time.
//!
//! Heuristic on the statement text:
//!   1. Find the projection slice (between SELECT and FROM).
//!   2. Collect aggregate calls (`count(`, `sum(`, `avg(`, `min(`, `max(`,
//!      `array_agg(`, `string_agg(`, `json_agg(`, `bool_or(`, `bool_and(`).
//!   3. Collect bare column references in the projection (identifiers
//!      not followed by `(`, not inside an aggregate).
//!   4. If aggregates exist and any bare column is not present in the
//!      GROUP BY column list (case-insensitive whole-word match), flag.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::{Statement, StatementKind};
use dsl_resolve::Scope;

pub struct Rule;

const AGG_FNS: &[&str] = &[
    "count", "sum", "avg", "min", "max",
    "array_agg", "string_agg", "json_agg", "jsonb_agg",
    "bool_or", "bool_and", "every",
    "stddev", "stddev_pop", "stddev_samp",
    "variance", "var_pop", "var_samp",
];

impl LintRule for Rule {
    fn code(&self) -> &'static str { "sql017" }
    fn default_severity(&self) -> Severity { Severity::Warning }

    fn check(
        &self,
        source: &str,
        stmt: &Statement,
        _scope: &Scope,
        _catalog: &Catalog,
        out: &mut Vec<Diagnostic>,
    ) {
        if !matches!(stmt.kind, StatementKind::Select(_)) { return; }
        let start: u32 = stmt.range.start().into();
        let end: u32 = stmt.range.end().into();
        let end = (end as usize).min(source.len());
        let slice = &source[start as usize..end];

        let (proj, group_by) = match split_projection_and_group(slice) {
            Some(v) => v, None => return,
        };

        let (aggregates, bare_cols) = scan_projection(proj);
        if aggregates == 0 || bare_cols.is_empty() { return; }
        let group_idents = collect_group_idents(group_by);
        let missing: Vec<&String> = bare_cols
            .iter()
            .filter(|c| !group_idents.iter().any(|g| g.eq_ignore_ascii_case(c)))
            .collect();
        if missing.is_empty() { return; }

        let names: Vec<&str> = missing.iter().map(|s| s.as_str()).collect();
        // Narrow the diagnostic to the first missing column in source.
        let body = &source[start as usize..end];
        let range = names.first().and_then(|n| {
            // Find the bare column reference in the projection text.
            let body_lower = body.to_ascii_lowercase();
            let needle = n.to_ascii_lowercase();
            body_lower.find(&needle).map(|r| {
                let abs_start = start as usize + r;
                let abs_end = abs_start + needle.len();
                text_size::TextRange::new(
                    (abs_start as u32).into(),
                    (abs_end as u32).into(),
                )
            })
        }).unwrap_or(stmt.range);
        out.push(Diagnostic {
            code: "sql017",
            severity: Severity::Warning,
            message: format!(
                "column{} {} appear{} in SELECT alongside aggregates but are not in GROUP BY",
                if missing.len() == 1 { "" } else { "s" },
                names.join(", "),
                if missing.len() == 1 { "s" } else { "" },
            ),
            range,
        });
    }
}

/// Slice `text` into (projection, group_by_clause). Returns None when
/// the slice is not a SELECT we can parse.
fn split_projection_and_group(text: &str) -> Option<(&str, &str)> {
    let upper = text.to_ascii_uppercase();
    let select_pos = upper.find("SELECT")?;
    let from_pos = find_top_keyword(text, &upper, select_pos + 6, "FROM")?;
    let proj = &text[select_pos + 6..from_pos];
    let group_pos = find_top_keyword(text, &upper, from_pos, "GROUP");
    let group_by = match group_pos {
        Some(p) => {
            let after = p + "GROUP".len();
            // skip "BY"
            let mut j = after;
            while j < text.len() && (text.as_bytes()[j] as char).is_whitespace() { j += 1; }
            if upper[j..].starts_with("BY") { j += 2; }
            let end = find_top_keyword(text, &upper, j, "ORDER")
                .or_else(|| find_top_keyword(text, &upper, j, "LIMIT"))
                .or_else(|| find_top_keyword(text, &upper, j, "HAVING"))
                .unwrap_or(text.len());
            &text[j..end]
        }
        None => "",
    };
    Some((proj, group_by))
}

/// Find the byte offset of a top-level keyword starting at or after `from`,
/// ignoring parentheses, strings, and comments.
fn find_top_keyword(text: &str, upper: &str, from: usize, kw: &str) -> Option<usize> {
    let bytes = text.as_bytes();
    let n = bytes.len();
    let mut i = from;
    let mut depth = 0i32;
    while i < n {
        let c = bytes[i];
        if c == b'\'' {
            i += 1;
            while i < n {
                if bytes[i] == b'\'' {
                    if i + 1 < n && bytes[i + 1] == b'\'' { i += 2; continue; }
                    i += 1; break;
                }
                i += 1;
            }
            continue;
        }
        if c == b'(' { depth += 1; i += 1; continue; }
        if c == b')' { depth -= 1; i += 1; continue; }
        if depth == 0 && upper[i..].starts_with(kw) {
            let after = i + kw.len();
            let prev_ok = i == 0 || !is_word(bytes[i - 1] as char);
            let next_ok = after >= n || !is_word(bytes[after] as char);
            if prev_ok && next_ok { return Some(i); }
        }
        i += 1;
    }
    None
}

fn is_word(c: char) -> bool { c.is_alphanumeric() || c == '_' }

/// Count aggregate calls and collect bare column names within `proj`.
fn scan_projection(proj: &str) -> (usize, Vec<String>) {
    let bytes = proj.as_bytes();
    let n = bytes.len();
    let mut aggregates = 0usize;
    let mut bare = Vec::new();
    let mut i = 0usize;
    while i < n {
        let c = bytes[i];
        if c == b'\'' {
            i += 1;
            while i < n {
                if bytes[i] == b'\'' { i += 1; break; }
                i += 1;
            }
            continue;
        }
        if (c as char).is_alphabetic() || c == b'_' {
            let start = i;
            while i < n && is_word(bytes[i] as char) { i += 1; }
            let word = &proj[start..i];
            // Skip whitespace then test for call paren
            let mut k = i;
            while k < n && (bytes[k] as char).is_whitespace() { k += 1; }
            let is_call = k < n && bytes[k] == b'(';
            let lower = word.to_ascii_lowercase();
            if is_call {
                if AGG_FNS.iter().any(|f| *f == lower) {
                    aggregates += 1;
                    // Skip paren body so columns inside aggregate aren't "bare"
                    i = skip_parens(bytes, k);
                } else {
                    i = skip_parens(bytes, k);
                }
                continue;
            }
            // Bare identifier -- ignore keywords/types/aliasing
            if !is_noise(&lower) {
                // Strip table qualifier
                let name = word.rsplit('.').next().unwrap_or(word).to_string();
                if !bare.contains(&name) { bare.push(name); }
            }
            continue;
        }
        if c == b'.' { i += 1; continue; }
        i += 1;
    }
    (aggregates, bare)
}

/// Words inside a projection that aren't bare column references.
fn is_noise(lower: &str) -> bool {
    matches!(lower,
        "as" | "and" | "or" | "not" | "null" | "true" | "false" |
        "distinct" | "all" | "case" | "when" | "then" | "else" | "end" |
        "in" | "is" | "between" | "like" | "ilike"
    )
}

fn skip_parens(bytes: &[u8], i: usize) -> usize {
    if i >= bytes.len() || bytes[i] != b'(' { return i; }
    let mut depth = 0i32;
    let mut j = i;
    let n = bytes.len();
    while j < n {
        match bytes[j] {
            b'(' => { depth += 1; j += 1; }
            b')' => { depth -= 1; j += 1; if depth == 0 { return j; } }
            b'\'' => {
                j += 1;
                while j < n {
                    if bytes[j] == b'\'' { j += 1; break; }
                    j += 1;
                }
            }
            _ => j += 1,
        }
    }
    j
}

fn collect_group_idents(group_by: &str) -> Vec<String> {
    let mut out = Vec::new();
    let bytes = group_by.as_bytes();
    let n = bytes.len();
    let mut i = 0usize;
    while i < n {
        let c = bytes[i];
        if (c as char).is_alphabetic() || c == b'_' {
            let start = i;
            while i < n && is_word(bytes[i] as char) { i += 1; }
            let word = &group_by[start..i];
            // Strip qualifier `t.col` -> `col`
            let name = word.rsplit('.').next().unwrap_or(word).to_string();
            out.push(name);
            continue;
        }
        i += 1;
    }
    out
}
