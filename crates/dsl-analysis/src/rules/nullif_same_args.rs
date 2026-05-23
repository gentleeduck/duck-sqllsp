//! sql085: `NULLIF(x, x)` always returns NULL -- pointless. Error.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
    fn code(&self) -> &'static str { "sql085" }
    fn default_severity(&self) -> Severity { Severity::Error }

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
        let bytes = body.as_bytes();
        let n = bytes.len();
        let mut i = 0;
        while i + 6 <= n {
            if &upper[i..i + 6] == "NULLIF" {
                let prev_ok = i == 0 || !is_word(bytes[i - 1] as char);
                if prev_ok {
                    let mut j = i + 6;
                    while j < n && bytes[j].is_ascii_whitespace() { j += 1; }
                    if j < n && bytes[j] == b'(' {
                        if let Some(close) = match_paren(bytes, j) {
                            let inner = &body[j + 1..close];
                            let parts = split_top_level_commas(inner);
                            if parts.len() == 2 && parts[0].trim() == parts[1].trim() {
                                let abs_start = start + i;
                                let abs_end = start + close + 1;
                                out.push(Diagnostic {
                                    code: "sql085",
                                    severity: Severity::Error,
                                    message: format!("NULLIF({}, {}) always returns NULL", parts[0].trim(), parts[1].trim()),
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
            }
            i += 1;
        }
    }
}

fn match_paren(bytes: &[u8], open: usize) -> Option<usize> {
    let n = bytes.len();
    let mut depth = 0i32;
    let mut i = open;
    while i < n {
        match bytes[i] {
            b'(' => depth += 1,
            b')' => { depth -= 1; if depth == 0 { return Some(i); } }
            b'\'' => {
                i += 1;
                while i < n && bytes[i] != b'\'' { i += 1; }
            }
            _ => {}
        }
        i += 1;
    }
    None
}

fn split_top_level_commas(s: &str) -> Vec<String> {
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
            b'\'' => {
                i += 1;
                while i < n && bytes[i] != b'\'' { i += 1; }
            }
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

fn is_word(c: char) -> bool { c.is_alphanumeric() || c == '_' }
