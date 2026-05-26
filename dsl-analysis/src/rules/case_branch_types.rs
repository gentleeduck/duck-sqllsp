//! sql218: `CASE WHEN ... THEN 1 ... WHEN ... THEN 'foo' ... END` --
//! branches return literals of incompatible families (integer +
//! string + boolean). PG raises 42804 at parse time. Local literal
//! sniff only -- expressions / column refs are accepted as unknown.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql218"
  }
  fn default_severity(&self) -> Severity {
    Severity::Warning
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let start: usize = u32::from(stmt.range.start()) as usize;
    let end: usize = (u32::from(stmt.range.end()) as usize).min(source.len());
    let body = &source[start..end];
    let upper = body.to_ascii_uppercase();
    let mut from = 0usize;
    while let Some(rel) = upper[from..].find("CASE") {
      let case_at = from + rel;
      if case_at > 0 {
        let prev = body.as_bytes()[case_at - 1] as char;
        if prev.is_ascii_alphanumeric() || prev == '_' { from = case_at + 4; continue }
      }
      // Find matching END for this CASE (handle nested CASE).
      let Some(end_at) = find_matching_case_end(&upper, case_at) else { from = case_at + 4; break };
      let case_body = &body[case_at..end_at];
      let mut types: Vec<&'static str> = Vec::new();
      let mut bytes_off = 0usize;
      let cb_upper = case_body.to_ascii_uppercase();
      while let Some(then_rel) = cb_upper[bytes_off..].find("THEN") {
        let then_at = bytes_off + then_rel;
        if then_at > 0 {
          let prev = case_body.as_bytes()[then_at - 1] as char;
          if prev.is_ascii_alphanumeric() || prev == '_' { bytes_off = then_at + 4; continue }
        }
        let after = then_at + "THEN".len();
        let rest = &case_body[after..];
        let stop = rest.to_ascii_uppercase()
          .find("WHEN")
          .or_else(|| rest.to_ascii_uppercase().find("ELSE"))
          .or_else(|| rest.to_ascii_uppercase().find("END"))
          .unwrap_or(rest.len());
        let expr = rest[..stop].trim();
        if let Some(ty) = literal_family(expr) {
          if !types.contains(&ty) { types.push(ty); }
        }
        bytes_off = after + stop;
      }
      // Also handle ELSE clause.
      if let Some(else_rel) = cb_upper.find("ELSE") {
        let after = else_rel + "ELSE".len();
        let rest = &case_body[after..];
        let stop = rest.to_ascii_uppercase().find("END").unwrap_or(rest.len());
        let expr = rest[..stop].trim();
        if let Some(ty) = literal_family(expr) {
          if !types.contains(&ty) { types.push(ty); }
        }
      }
      if types.len() >= 2 {
        out.push(Diagnostic {
          code: "sql218",
          severity: Severity::Warning,
          message: format!(
            "CASE branches return divergent literal types: {} -- PG raises 42804 unless all branches share a type family",
            types.join(", "),
          ),
          range: text_size::TextRange::new(((start + case_at) as u32).into(), ((start + end_at) as u32).into()),
        });
      }
      from = end_at;
    }
  }
}

fn find_matching_case_end(upper: &str, case_at: usize) -> Option<usize> {
  let bytes = upper.as_bytes();
  let mut depth = 1i32;
  let mut i = case_at + 4;
  while i + 3 <= bytes.len() {
    let prev_ok = i == 0 || !{ let p = bytes[i - 1] as char; p.is_ascii_alphanumeric() || p == '_' };
    if prev_ok && &upper[i..i + 4] == "CASE" {
      let after = i + 4;
      let after_ok = after >= bytes.len() || !{ let p = bytes[after] as char; p.is_ascii_alphanumeric() || p == '_' };
      if after_ok { depth += 1; i = after; continue }
    }
    if i + 3 <= bytes.len() && &upper[i..i + 3] == "END" {
      let prev = if i == 0 { ' ' } else { bytes[i - 1] as char };
      let after = i + 3;
      let prev_ok = !(prev.is_ascii_alphanumeric() || prev == '_');
      let after_ok = after >= bytes.len() || !{ let p = bytes[after] as char; p.is_ascii_alphanumeric() || p == '_' };
      if prev_ok && after_ok {
        depth -= 1;
        if depth == 0 { return Some(after); }
        i = after; continue;
      }
    }
    i += 1;
  }
  None
}

fn literal_family(s: &str) -> Option<&'static str> {
  let t = s.trim().trim_end_matches(',').trim();
  if t.is_empty() { return None }
  if t.eq_ignore_ascii_case("NULL") { return None }
  if t.eq_ignore_ascii_case("TRUE") || t.eq_ignore_ascii_case("FALSE") { return Some("boolean") }
  if let Some(stripped) = t.strip_prefix('\'') {
    if stripped.ends_with('\'') { return Some("text") }
  }
  // Integers and decimals are both numeric in PG -- they unify via
  // implicit promotion. Collapse to a single family so a CASE that
  // mixes `0` and `0.20` doesn't trip sql218.
  if t.parse::<i64>().is_ok() || t.parse::<f64>().is_ok() { return Some("numeric") }
  None
}
