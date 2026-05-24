//! sql118: `SELECT ... INTO foo FROM t` at the top level is **DDL** --
//! it creates a new table `foo`. Usually the user meant PL/pgSQL
//! variable assignment (which only works inside `$$ ... $$`).

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
    fn code(&self) -> &'static str { "sql118" }
    fn default_severity(&self) -> Severity { Severity::Hint }

    fn check(
        &self,
        source: &str,
        stmt: &Statement,
        _scope: &Scope,
        _catalog: &Catalog,
        out: &mut Vec<Diagnostic>,
    ) {
        let start: usize = u32::from(stmt.range.start()) as usize;
        let end: usize = (u32::from(stmt.range.end()) as usize).min(source.len());
        let body = &source[start..end];
        let upper = body.to_ascii_uppercase();
        // Inside a function body? Skip -- there `SELECT INTO` is
        // assignment.
        let before_upper = source[..start].to_ascii_uppercase();
        if inside_dollar_block(&before_upper) { return; }
        // Stmt must contain `SELECT ... INTO <target> ... FROM`.
        let bytes = upper.as_bytes();
        let n = bytes.len();
        // Find SELECT keyword.
        let Some(sel) = upper.find("SELECT") else { return };
        if !is_keyword_at(&upper, bytes, sel, "SELECT") { return; }
        // After SELECT, projection ends at FROM. INTO must appear before FROM.
        let after_sel = sel + 6;
        let from_at = match upper[after_sel..].find(" FROM ") {
            Some(p) => after_sel + p + 1,
            None => return,
        };
        let into_at = match upper[after_sel..from_at].find(" INTO ") {
            Some(p) => after_sel + p + 1,
            None => return,
        };
        if !is_keyword_at(&upper, bytes, into_at, "INTO") { return; }
        let _ = n;
        let abs_start = start + into_at;
        let abs_end = start + into_at + 4;
        out.push(Diagnostic {
            code: "sql118",
            severity: Severity::Hint,
            message: "top-level `SELECT INTO` creates a new table -- inside PL/pgSQL it assigns variables, but at the top level it's DDL".into(),
            range: text_size::TextRange::new(
                (abs_start as u32).into(),
                (abs_end as u32).into(),
            ),
        });
    }
}

fn is_keyword_at(upper: &str, bytes: &[u8], i: usize, word: &str) -> bool {
    let w = word.len();
    if i + w > bytes.len() { return false; }
    if &upper[i..i + w] != word { return false; }
    let prev_ok = i == 0 || !is_word(bytes[i - 1] as char);
    let next_ok = i + w == bytes.len() || !is_word(bytes[i + w] as char);
    prev_ok && next_ok
}

fn is_word(c: char) -> bool { c.is_alphanumeric() || c == '_' }

/// Cheap check: are we currently inside an open `$$ ... $$` block?
/// Counts opening / closing dollar tags in `before` and returns true
/// when there's an unmatched opener.
fn inside_dollar_block(before: &str) -> bool {
    let bytes = before.as_bytes();
    let n = bytes.len();
    let mut i = 0;
    let mut open = false;
    while i < n {
        if bytes[i] == b'$' {
            // Read tag like `$$` or `$tag$`.
            let tag_start = i;
            let mut j = i + 1;
            while j < n && (bytes[j].is_ascii_alphanumeric() || bytes[j] == b'_') { j += 1; }
            if j < n && bytes[j] == b'$' {
                let _tag = &before[tag_start..=j];
                open = !open;
                i = j + 1;
                continue;
            }
        }
        i += 1;
    }
    open
}
