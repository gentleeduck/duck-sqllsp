//! sql205: `NOTIFY <channel>` where no `LISTEN <channel>` appears in
//! the same buffer. Dead channel -- subscriber side missing. Best-
//! effort: covers buffers that contain both producer + consumer SQL
//! (common in repo-managed schema dumps + migration files).

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;
use std::collections::HashSet;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql205"
  }
  fn default_severity(&self) -> Severity {
    Severity::Info
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let start: usize = u32::from(stmt.range.start()) as usize;
    let end: usize = (u32::from(stmt.range.end()) as usize).min(source.len());
    let raw = &source[start..end];
    let body_owned = crate::textutil::strip_noise_full(raw);
    let body = body_owned.as_str();
    let upper = body.to_ascii_uppercase();
    if !upper.trim_start().starts_with("NOTIFY") { return }
    let after = upper.find("NOTIFY ").map(|p| p + "NOTIFY ".len());
    let Some(after) = after else { return };
    let rest = &body[after..];
    let tok_end = rest.find(|c: char| c == ',' || c == ';' || c.is_whitespace()).unwrap_or(rest.len());
    let channel = rest[..tok_end].trim_matches('"');
    if channel.is_empty() { return }
    let known = collect_listeners(source);
    if known.iter().any(|l| l.eq_ignore_ascii_case(channel)) { return }
    let abs_s = start + after;
    let abs_e = abs_s + tok_end;
    out.push(Diagnostic {
      code: "sql205",
      severity: Severity::Info,
      message: format!(
        "NOTIFY `{channel}` -- no matching LISTEN in this buffer; subscriber may be missing or in another file"
      ),
      range: text_size::TextRange::new((abs_s as u32).into(), (abs_e as u32).into()),
    });
  }
}

fn collect_listeners(source: &str) -> HashSet<String> {
  let upper = source.to_ascii_uppercase();
  let mut out = HashSet::new();
  let mut from = 0usize;
  while let Some(rel) = upper[from..].find("LISTEN ") {
    let at = from + rel;
    // Word boundary -- avoid matching UNLISTEN.
    if at > 0 {
      let prev = upper.as_bytes()[at - 1] as char;
      if prev.is_ascii_alphanumeric() || prev == '_' { from = at + "LISTEN ".len(); continue }
    }
    let after = at + "LISTEN ".len();
    let rest = &source[after..];
    let tok_end = rest.find(|c: char| c == ',' || c == ';' || c.is_whitespace()).unwrap_or(rest.len());
    let chan = rest[..tok_end].trim_matches('"').to_string();
    if !chan.is_empty() { out.insert(chan); }
    from = after + tok_end;
  }
  out
}
