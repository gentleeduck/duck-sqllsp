//! sql202: PL/pgSQL trigger function body references `OLD.*` inside
//! an INSERT trigger or `NEW.*` inside a DELETE trigger. PG raises
//! "record `old` has no field `xyz`" -- the row alias is undefined.
//!
//! Heuristic: each CREATE TRIGGER statement names the function it
//! invokes plus the event(s) (INSERT/UPDATE/DELETE). We map the
//! trigger fn -> events, then re-scan every CREATE FUNCTION body to
//! flag forbidden NEW/OLD references for its registered events.
//!
//! Two-phase pass: this rule only fires when both CREATE TRIGGER and
//! CREATE FUNCTION appear in the same buffer, which is the common
//! workspace layout.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;
use std::collections::{HashMap, HashSet};

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql202"
  }
  fn default_severity(&self) -> Severity {
    Severity::Error
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let start: usize = u32::from(stmt.range.start()) as usize;
    let end: usize = (u32::from(stmt.range.end()) as usize).min(source.len());
    let body = &source[start..end];
    let upper = body.to_ascii_uppercase();
    // Only inspect CREATE FUNCTION ... AS $$ ... $$ bodies.
    if !upper.contains("CREATE") || !upper.contains("FUNCTION") { return }
    let Some(name) = function_name(body) else { return };
    // Scan the entire file for CREATE TRIGGER ... EXECUTE [FUNCTION|PROCEDURE]
    // <name> to collect this fn's events.
    let mut events: HashSet<&'static str> = HashSet::new();
    collect_trigger_events(source, &name, &mut events);
    if events.is_empty() { return }
    let Some(body_start) = body.find("$$").map(|p| p + 2) else { return };
    let body_end = body[body_start..].find("$$").map(|p| body_start + p).unwrap_or(body.len());
    let fbody = &body[body_start..body_end];
    let fbody_upper = fbody.to_ascii_uppercase();

    let forbid_old = events.contains("INSERT") && !events.contains("UPDATE") && !events.contains("DELETE");
    let forbid_new = events.contains("DELETE") && !events.contains("INSERT") && !events.contains("UPDATE");

    if forbid_old {
      if let Some(at) = first_token(&fbody_upper, "OLD.") {
        let abs_s = start + body_start + at;
        let abs_e = abs_s + 4;
        out.push(Diagnostic {
          code: "sql202",
          severity: Severity::Error,
          message: "INSERT-only trigger references `OLD` -- OLD undefined on INSERT".into(),
          range: text_size::TextRange::new((abs_s as u32).into(), (abs_e as u32).into()),
        });
      }
    }
    if forbid_new {
      if let Some(at) = first_token(&fbody_upper, "NEW.") {
        let abs_s = start + body_start + at;
        let abs_e = abs_s + 4;
        out.push(Diagnostic {
          code: "sql202",
          severity: Severity::Error,
          message: "DELETE-only trigger references `NEW` -- NEW undefined on DELETE".into(),
          range: text_size::TextRange::new((abs_s as u32).into(), (abs_e as u32).into()),
        });
      }
    }
    let _ = HashMap::<(), ()>::new();
  }
}

fn function_name(body: &str) -> Option<String> {
  let upper = body.to_ascii_uppercase();
  let at = upper.find("CREATE")?;
  let after = at + "CREATE".len();
  let rest_upper = &upper[after..];
  // skip OR REPLACE / FUNCTION
  let after_fn = rest_upper.find("FUNCTION")? + after + "FUNCTION".len();
  let tail = body[after_fn..].trim_start();
  let off = after_fn + (body[after_fn..].len() - tail.len());
  let bytes = body.as_bytes();
  let mut j = off;
  while j < bytes.len() && (bytes[j].is_ascii_alphanumeric() || bytes[j] == b'_' || bytes[j] == b'.' || bytes[j] == b'"') {
    j += 1;
  }
  let name = body[off..j].rsplit('.').next().unwrap_or(&body[off..j]).trim_matches('"').to_string();
  if name.is_empty() { None } else { Some(name) }
}

fn collect_trigger_events(source: &str, fn_name: &str, out: &mut HashSet<&'static str>) {
  let lower_src = source.to_ascii_lowercase();
  let needle_a = format!("execute function {}", fn_name.to_ascii_lowercase());
  let needle_b = format!("execute procedure {}", fn_name.to_ascii_lowercase());
  let mut from = 0usize;
  loop {
    let pos = lower_src[from..]
      .find(&needle_a)
      .map(|p| (p, needle_a.len()))
      .or_else(|| lower_src[from..].find(&needle_b).map(|p| (p, needle_b.len())));
    let Some((rel, _len)) = pos else { break };
    let abs = from + rel;
    // Walk backwards to find the preceding CREATE TRIGGER block.
    let trig_at = lower_src[..abs].rfind("create trigger");
    let Some(trig_at) = trig_at else { break };
    let window = &lower_src[trig_at..abs];
    if window.contains("insert") { out.insert("INSERT"); }
    if window.contains("update") { out.insert("UPDATE"); }
    if window.contains("delete") { out.insert("DELETE"); }
    if window.contains("truncate") { out.insert("TRUNCATE"); }
    from = abs + 1;
  }
}

fn first_token(haystack_upper: &str, needle: &str) -> Option<usize> {
  let bytes = haystack_upper.as_bytes();
  let mut from = 0usize;
  while let Some(rel) = haystack_upper[from..].find(needle) {
    let at = from + rel;
    if at > 0 {
      let prev = bytes[at - 1] as char;
      if prev.is_ascii_alphanumeric() || prev == '_' { from = at + needle.len(); continue }
    }
    return Some(at);
  }
  None
}
