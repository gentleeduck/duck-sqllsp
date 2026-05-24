//! sql137: bare `LISTEN <channel>` in a session that never `UNLISTEN`s
//! -- the backend accumulates queued notifications indefinitely.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
    fn code(&self) -> &'static str { "sql137" }
    fn default_severity(&self) -> Severity { Severity::Warning }

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
        let trimmed = upper.trim_start();
        if !trimmed.starts_with("LISTEN ") { return; }
        // Pull the channel name (next identifier after LISTEN).
        let after_kw = trimmed[7..].trim_start();
        let chan: String = after_kw.chars()
            .take_while(|c| c.is_alphanumeric() || *c == '_' || *c == '"')
            .collect();
        if chan.is_empty() { return; }
        let chan_clean = chan.trim_matches('"');
        // Does any later statement in this source contain
        // `UNLISTEN <chan>` or `UNLISTEN *`?
        let after = &source[end..].to_ascii_uppercase();
        let bare_un = format!("UNLISTEN {}", chan_clean.to_ascii_uppercase());
        if after.contains(&bare_un) || after.contains("UNLISTEN *") { return; }
        let leading = upper.len() - trimmed.len();
        let abs_start = start + leading;
        let abs_end = abs_start + 6;
        out.push(Diagnostic {
            code: "sql137",
            severity: Severity::Warning,
            message: format!("LISTEN `{chan_clean}` without a matching UNLISTEN -- queued notifications accumulate for the lifetime of the session"),
            range: text_size::TextRange::new(
                (abs_start as u32).into(),
                (abs_end as u32).into(),
            ),
        });
    }
}
