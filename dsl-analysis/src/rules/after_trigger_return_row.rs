//! sql236: `AFTER` trigger function returns NEW/OLD row -- PG
//! discards the value for AFTER triggers (only BEFORE / INSTEAD OF
//! can mutate the row via the RETURNed record). Suggest `RETURN
//! NULL` to clarify intent.
//!
//! Cross-references CREATE TRIGGER ... AFTER ... EXECUTE FUNCTION
//! <fn> with the CREATE FUNCTION body in the same buffer.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;
use std::collections::HashSet;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql236"
  }
  fn default_severity(&self) -> Severity {
    Severity::Hint
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, raw) = crate::stmt_body(stmt, source);
    let body_owned = crate::textutil::strip_noise_full(raw);
    let body = body_owned.as_str();
    let upper = body.to_ascii_uppercase();
    if !upper.contains("CREATE") || !upper.contains("FUNCTION") {
      return;
    }
    if !upper.contains("RETURNS TRIGGER") {
      return;
    }
    let Some(name) = function_name(body) else { return };
    let mut timings: HashSet<&'static str> = HashSet::new();
    collect_timings(source, &name, &mut timings);
    if !timings.contains("AFTER") {
      return;
    }
    if timings.contains("BEFORE") || timings.contains("INSTEAD") {
      return;
    } // skip if also BEFORE -- ambiguous
    let Some(body_start) = body.find("$$").map(|p| p + 2) else { return };
    let body_end = body[body_start..].find("$$").map(|p| body_start + p).unwrap_or(body.len());
    let fbody = &body[body_start..body_end];
    let fupper = fbody.to_ascii_uppercase();
    // Find any RETURN NEW/OLD; flag.
    let bytes = fupper.as_bytes();
    let mut from = 0usize;
    while let Some(rel) = fupper[from..].find("RETURN ") {
      let at = from + rel;
      if at > 0 {
        let prev = bytes[at - 1] as char;
        if prev.is_ascii_alphanumeric() || prev == '_' {
          from = at + 7;
          continue;
        }
      }
      let after = at + "RETURN ".len();
      let tail = &fupper[after..];
      if tail.starts_with("NEW") || tail.starts_with("OLD") {
        let kw = if tail.starts_with("NEW") { "NEW" } else { "OLD" };
        let abs_s = start + body_start + after;
        let abs_e = abs_s + kw.len();
        out.push(Diagnostic {
          code: "sql236",
          severity: Severity::Hint,
          message: format!(
            "AFTER trigger returns `{kw}` -- PG discards the row for AFTER triggers; `RETURN NULL` makes intent explicit"
          ),
          range: crate::range_at(abs_s, abs_e),
        });
      }
      from = after;
    }
  }
}

fn function_name(body: &str) -> Option<String> {
  let upper = body.to_ascii_uppercase();
  let at = upper.find("CREATE")?;
  let after = at + "CREATE".len();
  let rest_upper = &upper[after..];
  let after_fn = rest_upper.find("FUNCTION")? + after + "FUNCTION".len();
  let tail = body[after_fn..].trim_start();
  let off = after_fn + (body[after_fn..].len() - tail.len());
  let bytes = body.as_bytes();
  let mut j = off;
  while j < bytes.len()
    && (bytes[j].is_ascii_alphanumeric() || bytes[j] == b'_' || bytes[j] == b'.' || bytes[j] == b'"')
  {
    j += 1;
  }
  let name = body[off..j].rsplit('.').next().unwrap_or(&body[off..j]).trim_matches('"').to_string();
  if name.is_empty() { None } else { Some(name) }
}

fn collect_timings(source: &str, fn_name: &str, out: &mut HashSet<&'static str>) {
  let lower = source.to_ascii_lowercase();
  let n_a = format!("execute function {}", fn_name.to_ascii_lowercase());
  let n_b = format!("execute procedure {}", fn_name.to_ascii_lowercase());
  let mut from = 0usize;
  loop {
    let pos =
      lower[from..].find(&n_a).map(|p| (p, n_a.len())).or_else(|| lower[from..].find(&n_b).map(|p| (p, n_b.len())));
    let Some((rel, _)) = pos else { break };
    let abs = from + rel;
    let Some(trig_at) = lower[..abs].rfind("create trigger") else { break };
    let window = &lower[trig_at..abs];
    if window.contains("before") {
      out.insert("BEFORE");
    }
    if window.contains("after") {
      out.insert("AFTER");
    }
    if window.contains("instead of") {
      out.insert("INSTEAD");
    }
    from = abs + 1;
  }
}
