//! sql089: two `RAISE EXCEPTION` calls back-to-back -- the second is
//! unreachable because the first aborts the transaction.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
    fn code(&self) -> &'static str { "sql089" }
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
        let bytes = upper.as_bytes();
        let n = bytes.len();
        // Find the first RAISE EXCEPTION, then look for the next RAISE
        // EXCEPTION before any IF / ELSE / WHEN / END / RETURN.
        let mut first: Option<usize> = None;
        let mut i = 0;
        while i < n {
            if i + 5 <= n && &upper[i..i + 5] == "RAISE"
                && (i == 0 || !is_word(bytes[i - 1] as char))
            {
                let mut j = i + 5;
                while j < n && bytes[j].is_ascii_whitespace() { j += 1; }
                if j + 9 <= n && &upper[j..j + 9] == "EXCEPTION" {
                    let next_ok = j + 9 == n || !is_word(bytes[j + 9] as char);
                    if next_ok {
                        match first {
                            None => {
                                first = Some(i);
                                i = j + 9;
                                continue;
                            }
                            Some(_) => {
                                let abs_start = start + i;
                                let abs_end = start + j + 9;
                                out.push(Diagnostic {
                                    code: "sql089",
                                    severity: Severity::Hint,
                                    message: "RAISE EXCEPTION is unreachable -- previous RAISE EXCEPTION already aborted".into(),
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
            // Reset on control-flow keywords (any of these between two
            // RAISEs means the second may be reachable). Note: do NOT
            // include EXCEPTION here -- the inner check already skips
            // past `RAISE EXCEPTION` so we don't double-count.
            if first.is_some() {
                if word_at(&upper, bytes, i, "IF") || word_at(&upper, bytes, i, "ELSE")
                    || word_at(&upper, bytes, i, "ELSIF") || word_at(&upper, bytes, i, "WHEN")
                    || word_at(&upper, bytes, i, "END") || word_at(&upper, bytes, i, "LOOP")
                    || word_at(&upper, bytes, i, "EXCEPTIONS") {
                    first = None;
                }
            }
            i += 1;
        }
    }
}

fn is_word(c: char) -> bool { c.is_alphanumeric() || c == '_' }

fn word_at(upper: &str, bytes: &[u8], i: usize, word: &str) -> bool {
    let n = bytes.len();
    let w = word.len();
    if i + w > n { return false; }
    if &upper[i..i + w] != word { return false; }
    let prev_ok = i == 0 || !is_word(bytes[i - 1] as char);
    let next_ok = i + w == n || !is_word(bytes[i + w] as char);
    prev_ok && next_ok
}
