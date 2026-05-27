//! sql225: `COMMENT ON ... IS NULL` (or `IS ''`) when the target
//! already has a non-empty catalog comment. PG accepts this -- it
//! deletes the comment silently -- but it's almost never intentional.
//! Suggest making the intent explicit (drop-then-recreate) or remove
//! the statement.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql225"
  }
  fn default_severity(&self) -> Severity {
    Severity::Hint
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let start: usize = u32::from(stmt.range.start()) as usize;
    let end: usize = (u32::from(stmt.range.end()) as usize).min(source.len());
    let raw = &source[start..end];
    let body_owned = crate::textutil::strip_noise_full(raw);
    let body = body_owned.as_str();
    let upper = body.to_ascii_uppercase();
    if !upper.trim_start().starts_with("COMMENT ON") {
      return;
    }
    let Some(is_at) = upper.find(" IS ") else { return };
    let after = is_at + " IS ".len();
    let val = body[after..].trim_start();
    let is_null = val.to_ascii_uppercase().starts_with("NULL");
    let is_empty = val.starts_with("''") || val.starts_with("\"\"");
    if !is_null && !is_empty {
      return;
    }
    // What target?
    let kinds: &[(&str, &str)] =
      &[("COMMENT ON TABLE ", "table"), ("COMMENT ON COLUMN ", "column"), ("COMMENT ON FUNCTION ", "function")];
    let Some((needle, kind)) = kinds.iter().find(|(n, _)| upper.contains(n)) else { return };
    let after_kind = upper.find(needle).unwrap() + needle.len();
    let rest = body[after_kind..].trim_start();
    let id_end =
      rest.find(|c: char| !c.is_ascii_alphanumeric() && c != '_' && c != '.' && c != '"').unwrap_or(rest.len());
    let id = rest[..id_end].to_string();
    let bare = id.rsplit('.').next().unwrap_or(&id).trim_matches('"').to_string();
    let parts: Vec<&str> = id.split('.').collect();
    let existing = match *kind {
      "table" => catalog.find_table(None, &bare).and_then(|t| t.comment.clone()),
      "column" => {
        if parts.len() < 2 {
          None
        } else {
          let tbl = parts[parts.len() - 2].trim_matches('"');
          let col = parts[parts.len() - 1].trim_matches('"');
          catalog
            .find_table(None, tbl)
            .and_then(|t| t.columns.iter().find(|c| c.name.eq_ignore_ascii_case(col)).cloned())
            .and_then(|c| c.comment)
        }
      },
      "function" => {
        catalog.functions.iter().find(|f| f.name.eq_ignore_ascii_case(&bare)).and_then(|f| f.comment.clone())
      },
      _ => None,
    };
    let Some(existing) = existing else { return };
    if existing.trim().is_empty() {
      return;
    }
    let abs_s = start;
    let abs_e = start + body.find(';').unwrap_or(body.len());
    out.push(Diagnostic {
      code: "sql225",
      severity: Severity::Hint,
      message: format!(
        "COMMENT ON {kind} `{bare}` IS NULL / '' will clear existing comment ({} chars) silently -- make the intent explicit",
        existing.len(),
      ),
      range: text_size::TextRange::new((abs_s as u32).into(), (abs_e as u32).into()),
    });
  }
}
