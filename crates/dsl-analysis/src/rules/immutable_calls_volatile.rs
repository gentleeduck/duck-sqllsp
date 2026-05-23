//! sql040: an `IMMUTABLE` function body calls a known `VOLATILE`
//! built-in.
//!
//! Postgres trusts the author's volatility annotation; planning + index
//! optimisations rely on it. Violating purity by calling `now()`,
//! `random()`, `gen_random_uuid()`, `clock_timestamp()`, etc. from an
//! IMMUTABLE function silently produces wrong query plans.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::{Statement, StatementKind};
use dsl_resolve::Scope;

const VOLATILE_BUILTINS: &[&str] = &[
    "NOW", "CLOCK_TIMESTAMP", "STATEMENT_TIMESTAMP",
    "RANDOM", "GEN_RANDOM_UUID", "UUID_GENERATE_V4",
    "NEXTVAL", "SETVAL",
    "CURRVAL", "LASTVAL",
    "PG_BACKEND_PID",
];

pub struct Rule;

impl LintRule for Rule {
    fn code(&self) -> &'static str { "sql040" }
    fn default_severity(&self) -> Severity { Severity::Warning }

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
        if !upper.contains("IMMUTABLE") { return; }
        let Some(body_text) = dollar_body(body) else { return };
        let upper_body = body_text.to_ascii_uppercase();
        let stripped = strip_quoted_and_comments(&upper_body);

        // Scan tokens. Detect `IDENT(` patterns where IDENT is in the
        // VOLATILE list. Emit one diagnostic per violation.
        let bytes = stripped.as_bytes();
        let n = bytes.len();
        let mut i = 0;
        while i < n {
            if bytes[i].is_ascii_alphabetic() || bytes[i] == b'_' {
                let s = i;
                while i < n && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_') { i += 1; }
                let tok = &stripped[s..i];
                // Followed by `(` (allow whitespace) -> function call.
                let mut j = i;
                while j < n && bytes[j].is_ascii_whitespace() { j += 1; }
                if j < n && bytes[j] == b'(' && VOLATILE_BUILTINS.contains(&tok) {
                    let base = source.find(body_text).unwrap_or(start);
                    let abs_start = base + s;
                    let abs_end = base + i;
                    out.push(Diagnostic {
                        code: "sql040",
                        severity: Severity::Warning,
                        message: format!(
                            "IMMUTABLE function calls VOLATILE builtin `{}` -- mark function VOLATILE or pick a pure alternative",
                            tok.to_ascii_lowercase()
                        ),
                        range: text_size::TextRange::new(
                            (abs_start as u32).into(),
                            (abs_end as u32).into(),
                        ),
                    });
                }
            } else {
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

fn strip_quoted_and_comments(s: &str) -> String {
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
        } else if bytes[i] == b'\'' {
            i += 1;
            while i < n && bytes[i] != b'\'' { i += 1; }
            if i < n { i += 1; }
        } else {
            out.push(bytes[i] as char);
            i += 1;
        }
    }
    out
}
