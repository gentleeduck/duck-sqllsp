//! sql197: `array_length(col, ...)`, `unnest(col)`, `cardinality(col)`,
//! `array_to_string(col, ...)`, `array_position(col, ...)` where `col`
//! resolves to a scalar (non-array) catalog column. PG raises 42883
//! "function does not exist" at runtime (no array overload).
//!
//! Conservative: only flags bare column references inside the array
//! function's first argument. Subqueries and computed exprs are skipped.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

const FNS: &[&str] = &[
  "array_length",
  "array_lower",
  "array_upper",
  "array_ndims",
  "array_to_string",
  "array_position",
  "array_positions",
  "array_remove",
  "array_replace",
  "cardinality",
  "unnest",
];

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql197"
  }
  fn default_severity(&self) -> Severity {
    Severity::Warning
  }

  fn check(&self, source: &str, stmt: &Statement, scope: &Scope, catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, body) = crate::stmt_body(stmt, source);
    let lower = body.to_ascii_lowercase();
    for &fname in FNS {
      let needle = format!("{fname}(");
      let mut from = 0usize;
      while let Some(rel) = lower[from..].find(&needle) {
        let at = from + rel;
        if at > 0 {
          let prev = body.as_bytes()[at - 1] as char;
          if prev.is_ascii_alphanumeric() || prev == '_' {
            from = at + needle.len();
            continue;
          }
        }
        let open = at + needle.len();
        let Some(close_off) = body[open..].find([',', ')']) else {
          from = open;
          continue;
        };
        let arg = body[open..open + close_off].trim();
        if arg.is_empty() {
          from = open;
          continue;
        }
        let (alias, col) =
          if let Some((a, c)) = arg.split_once('.') { (Some(a.trim()), c.trim()) } else { (None, arg) };
        if col.contains(' ') || col.contains('(') {
          from = open;
          continue;
        }
        let col = col.trim_matches('"');
        let alias = alias.map(|a| a.trim_matches('"'));
        let col_type = resolve_column_type(scope, catalog, alias, col);
        let Some(ty) = col_type else {
          from = open;
          continue;
        };
        if is_array_type(&ty) {
          from = open;
          continue;
        }
        let abs_s = start + at;
        let abs_e = start + open + close_off;
        out.push(Diagnostic {
          code: "sql197",
          severity: Severity::Warning,
          message: format!("`{fname}({arg})` -- `{col}` is `{ty}` (not array) -- PG 42883 at runtime"),
          range: crate::range_at(abs_s, abs_e),
        });
        from = open + close_off;
      }
    }
  }
}

fn resolve_column_type(scope: &Scope, catalog: &Catalog, alias: Option<&str>, col: &str) -> Option<String> {
  if let Some(alias) = alias {
    for b in scope.bindings.values() {
      if b.alias.eq_ignore_ascii_case(alias) || b.table.name.eq_ignore_ascii_case(alias) {
        let t = catalog.find_table(b.table.schema.as_deref(), &b.table.name)?;
        return t.columns.iter().find(|c| c.name.eq_ignore_ascii_case(col)).map(|c| c.data_type.clone());
      }
    }
    return None;
  }
  for b in scope.bindings.values() {
    let Some(t) = catalog.find_table(b.table.schema.as_deref(), &b.table.name) else { continue };
    if let Some(c) = t.columns.iter().find(|c| c.name.eq_ignore_ascii_case(col)) {
      return Some(c.data_type.clone());
    }
  }
  None
}

fn is_array_type(ty: &str) -> bool {
  let t = ty.trim().to_ascii_lowercase();
  t.ends_with("[]") || t.starts_with("array") || t.contains(" array")
}
