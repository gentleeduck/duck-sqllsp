//! sql045: unreachable code after an unconditional `RETURN` or `RAISE
//! EXCEPTION`.
//!
//! Postgres won't error on dead code but it's almost always a bug --
//! either the author forgot to remove obsolete code or guarded the
//! return wrongly. Hint severity.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::{Statement, StatementKind};
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
    fn code(&self) -> &'static str { "sql045" }
    fn default_severity(&self) -> Severity { Severity::Hint }

    fn check(
        &self,
        source: &str,
        stmt: &Statement,
        _scope: &Scope,
        _catalog: &Catalog,
        out: &mut Vec<Diagnostic>,
    ) {
        if !matches!(stmt.kind, StatementKind::Unknown { .. }) { return; }
        let start: usize = u32::from(stmt.range.start()) as usize;
        let end: usize = (u32::from(stmt.range.end()) as usize).min(source.len());
        let body = &source[start..end];
        let upper = body.to_ascii_uppercase();
        if !upper.contains("CREATE") || !upper.contains("FUNCTION") { return; }
        let Some(body_text) = dollar_body(body) else { return };

        // For each top-level RETURN / RAISE EXCEPTION statement (not
        // nested inside IF/LOOP), check whether any statement follows
        // it before the matching END. We approximate "top-level" by
        // tracking depth on IF / LOOP / FOR / WHILE / BEGIN openers.
        let upper_body = body_text.to_ascii_uppercase();
        let stripped = strip_comments(&upper_body);
        let bytes = stripped.as_bytes();
        let n = bytes.len();
        let mut depth = 0i32;
        let mut last_unconditional: Option<usize> = None;
        let mut last_was_terminator = false;
        let mut i = 0;
        while i < n {
            // Skip whitespace.
            while i < n && bytes[i].is_ascii_whitespace() { i += 1; }
            if i >= n { break; }
            // Read next token (word) or punctuation.
            if bytes[i].is_ascii_alphabetic() || bytes[i] == b'_' {
                let s = i;
                while i < n && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_') { i += 1; }
                let tok = &stripped[s..i];
                match tok {
                    "IF" | "LOOP" | "FOR" | "WHILE" | "BEGIN" => {
                        depth += 1;
                        last_was_terminator = false;
                    }
                    "END" => {
                        depth -= 1;
                        last_was_terminator = false;
                    }
                    "RETURN" | "RAISE" => {
                        if depth == 1 {
                            last_unconditional = Some(s);
                        }
                        last_was_terminator = true;
                    }
                    _ => {
                        if last_was_terminator
                            && depth == 1
                            && tok != "END"
                            && tok != "EXCEPTION" // RAISE EXCEPTION continuation
                            && tok != "NOTICE" && tok != "WARNING"
                            && tok != "INFO" && tok != "LOG" && tok != "DEBUG"
                            && tok != "USING"
                        {
                            if let Some(_) = last_unconditional.take() {
                                let base = source.find(body_text).unwrap_or(start);
                                let abs_start = base + s;
                                let abs_end = base + i;
                                out.push(Diagnostic {
                                    code: "sql045",
                                    severity: Severity::Hint,
                                    message: format!(
                                        "unreachable: this code follows an unconditional RETURN/RAISE EXCEPTION"
                                    ),
                                    range: text_size::TextRange::new(
                                        (abs_start as u32).into(),
                                        (abs_end as u32).into(),
                                    ),
                                });
                                last_was_terminator = false;
                                return; // one is plenty per function
                            }
                        }
                    }
                }
            } else {
                // Punctuation; reset terminator on `;` boundary tracker.
                if bytes[i] == b';' {
                    // statement terminator -- next token continues at top level
                }
                if bytes[i] == b'\'' {
                    i += 1;
                    while i < n && bytes[i] != b'\'' { i += 1; }
                }
                i += 1;
            }
        }
    }
}

fn dollar_body(text: &str) -> Option<&str> {
    let start = text.find("$$")?;
    let after = start + 2;
    let end_rel = text[after..].find("$$")?;
    Some(&text[after..after + end_rel])
}

fn strip_comments(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let bytes = s.as_bytes();
    let n = bytes.len();
    let mut i = 0;
    while i < n {
        if i + 1 < n && bytes[i] == b'-' && bytes[i + 1] == b'-' {
            while i < n && bytes[i] != b'\n' { i += 1; }
        } else if i + 1 < n && bytes[i] == b'/' && bytes[i + 1] == b'*' {
            i += 2;
            while i + 1 < n && !(bytes[i] == b'*' && bytes[i + 1] == b'/') { i += 1; }
            i = (i + 2).min(n);
        } else {
            out.push(bytes[i] as char);
            i += 1;
        }
    }
    out
}
