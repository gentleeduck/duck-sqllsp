//! sql065: `GROUP BY 1, 2` -- positional grouping is brittle. A
//! projection-list edit silently changes the grouping. Hint: use
//! the column expression (or its alias) instead.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::{Statement, StatementKind};
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
    fn code(&self) -> &'static str { "sql065" }
    fn default_severity(&self) -> Severity { Severity::Hint }

    fn check(
        &self,
        source: &str,
        stmt: &Statement,
        _scope: &Scope,
        _catalog: &Catalog,
        out: &mut Vec<Diagnostic>,
    ) {
        if !matches!(stmt.kind, StatementKind::Select(_)) { return; }
        let start: usize = u32::from(stmt.range.start()) as usize;
        let end: usize = (u32::from(stmt.range.end()) as usize).min(source.len());
        let body = &source[start..end];
        let stripped = strip_quoted_and_comments(body);
        let upper = stripped.to_ascii_uppercase();
        let bytes = upper.as_bytes();
        let n = bytes.len();

        // Find `GROUP BY` then read items until clause-terminator.
        let Some(g_pos) = find_word(&upper, "GROUP") else { return };
        let after_group = g_pos + 5;
        if after_group + 2 > n { return; }
        let mut i = after_group;
        while i < n && bytes[i].is_ascii_whitespace() { i += 1; }
        if !upper[i..].starts_with("BY") { return; }
        i += 2;
        let items_end = find_clause_end(&upper, i);
        let items_start = i;
        let items = &stripped[items_start..items_end];
        for (item_off, item) in split_top_level_pos(items) {
            let leading = item.len() - item.trim_start().len();
            let t = item.trim();
            if !t.is_empty() && t.chars().all(|c| c.is_ascii_digit()) {
                let abs_start = start + items_start + item_off + leading;
                let abs_end = abs_start + t.len();
                out.push(Diagnostic {
                    code: "sql065",
                    severity: Severity::Hint,
                    message: format!(
                        "GROUP BY position `{t}` -- prefer the column name/expression so projection edits stay safe"
                    ),
                    range: text_size::TextRange::new(
                        (abs_start as u32).into(),
                        (abs_end as u32).into(),
                    ),
                });
                return;
            }
        }
    }
}

fn split_top_level_pos(s: &str) -> Vec<(usize, &str)> {
    let bytes = s.as_bytes();
    let n = bytes.len();
    let mut out = Vec::new();
    let mut start = 0usize;
    let mut depth = 0i32;
    let mut i = 0;
    while i < n {
        match bytes[i] {
            b'(' => depth += 1,
            b')' => depth -= 1,
            b',' if depth == 0 => {
                out.push((start, &s[start..i]));
                start = i + 1;
            }
            _ => {}
        }
        i += 1;
    }
    out.push((start, &s[start..n]));
    out
}

fn find_word(s: &str, w: &str) -> Option<usize> {
    let bytes = s.as_bytes();
    let nb = w.as_bytes();
    let n = bytes.len();
    let mut i = 0;
    while i + nb.len() <= n {
        if &bytes[i..i + nb.len()] == nb {
            let prev_ok = i == 0 || !is_word(bytes[i - 1] as char);
            let next_ok = i + nb.len() == n || !is_word(bytes[i + nb.len()] as char);
            if prev_ok && next_ok { return Some(i); }
        }
        i += 1;
    }
    None
}

fn find_clause_end(s: &str, from: usize) -> usize {
    let bytes = s.as_bytes();
    let n = bytes.len();
    let mut i = from;
    let mut depth = 0i32;
    while i < n {
        match bytes[i] {
            b'(' => depth += 1,
            b')' => depth -= 1,
            b';' if depth == 0 => return i,
            _ => {}
        }
        if depth == 0 && bytes[i].is_ascii_alphabetic() {
            let start = i;
            while i < n && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_') { i += 1; }
            let word = &s[start..i];
            if matches!(word, "HAVING" | "ORDER" | "LIMIT" | "OFFSET" | "UNION" | "EXCEPT" | "INTERSECT" | "RETURNING" | "WINDOW") {
                return start;
            }
            continue;
        }
        i += 1;
    }
    n
}

fn split_top_level(s: &str) -> Vec<String> {
    let bytes = s.as_bytes();
    let n = bytes.len();
    let mut out = Vec::new();
    let mut start = 0;
    let mut depth = 0i32;
    let mut i = 0;
    while i < n {
        match bytes[i] {
            b'(' => depth += 1,
            b')' => depth -= 1,
            b',' if depth == 0 => {
                out.push(s[start..i].to_string());
                start = i + 1;
            }
            _ => {}
        }
        i += 1;
    }
    out.push(s[start..].to_string());
    out
}

/// Space-preserving: keeps output indices 1:1 with input.
fn strip_quoted_and_comments(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let bytes = s.as_bytes();
    let n = bytes.len();
    let mut i = 0;
    while i < n {
        if i + 1 < n && bytes[i] == b'-' && bytes[i + 1] == b'-' {
            while i < n && bytes[i] != b'\n' { out.push(' '); i += 1; }
        } else if i + 1 < n && bytes[i] == b'/' && bytes[i + 1] == b'*' {
            out.push(' '); out.push(' '); i += 2;
            while i + 1 < n && !(bytes[i] == b'*' && bytes[i + 1] == b'/') { out.push(' '); i += 1; }
            if i + 1 < n { out.push(' '); out.push(' '); i += 2; }
            else { while i < n { out.push(' '); i += 1; } }
        } else if bytes[i] == b'\'' {
            out.push(' '); i += 1;
            while i < n && bytes[i] != b'\'' { out.push(' '); i += 1; }
            if i < n { out.push(' '); i += 1; }
        } else if bytes[i].is_ascii() {
            out.push(bytes[i] as char);
            i += 1;
        } else {
            out.push(' ');
            i += 1;
        }
    }
    out
}

fn is_word(c: char) -> bool { c.is_alphanumeric() || c == '_' }
