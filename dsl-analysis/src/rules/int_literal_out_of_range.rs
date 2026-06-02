//! sql456: `WHERE smallint_col = 100000` -- the literal exceeds the
//! column type's range. PG raises 22003 "smallint out of range" at
//! execution; the comparison can never match because the value
//! literally doesn't fit. Almost always a copy-paste mistake from a
//! wider-type context.
//!
//! Implementation note: our Expr AST exposes the WHERE clause as a
//! flat `Expr::List` of column references without operator/literal
//! pairs, so this rule does a text scan of the WHERE body looking
//! for `<col> <op> <intlit>` triples.

use crate::clause_scan::{find_clause, find_clause_end, is_word};
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;
use text_size::TextRange;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql456"
  }
  fn default_severity(&self) -> Severity {
    Severity::Error
  }

  fn check(&self, source: &str, stmt: &Statement, scope: &Scope, catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    if scope.is_empty() {
      return;
    }
    let (start, raw) = crate::stmt_body(stmt, source);
    let cleaned = crate::textutil::strip_noise_full(raw);
    let upper = cleaned.to_ascii_uppercase();
    let ub = upper.as_bytes();
    let stopwords =
      ["GROUP BY", "ORDER BY", "LIMIT", "OFFSET", "HAVING", "FOR", "FETCH", "WINDOW", "RETURNING", "UNION", "INTERSECT", "EXCEPT"];
    let Some(rel_where) = find_clause(ub, b"WHERE") else {
      return;
    };
    let pred_start = rel_where + 5;
    let pred_end = find_clause_end(ub, pred_start, &stopwords);
    let pred = &cleaned[pred_start..pred_end];
    scan_pairs(pred, start + pred_start, stmt.range, scope, catalog, out);
  }
}

fn scan_pairs(pred: &str, abs_off: usize, stmt_range: TextRange, scope: &Scope, catalog: &Catalog, out: &mut Vec<Diagnostic>) {
  let bytes = pred.as_bytes();
  let n = bytes.len();
  let mut i = 0usize;
  let mut emitted_at: std::collections::HashSet<usize> = std::collections::HashSet::new();
  while i < n {
    // Read an identifier (possibly qualified).
    if !is_ident_start(bytes[i]) {
      i += 1;
      continue;
    }
    let id_start = i;
    while i < n && (is_word(bytes[i] as char) || bytes[i] == b'.') {
      i += 1;
    }
    let id_end = i;
    let ident = &pred[id_start..id_end];
    if ident.is_empty() || ident.contains("..") || ident.starts_with('.') || ident.ends_with('.') {
      continue;
    }
    let (qualifier, name) = split_qualifier(ident);
    // Skip whitespace.
    while i < n && bytes[i].is_ascii_whitespace() {
      i += 1;
    }
    // Op?
    let op_len = peek_cmp_op(bytes, i);
    if op_len == 0 {
      continue;
    }
    i += op_len;
    while i < n && bytes[i].is_ascii_whitespace() {
      i += 1;
    }
    // Numeric literal? Read optional minus + digits.
    let lit_start = i;
    if i < n && (bytes[i] == b'-' || bytes[i] == b'+') {
      i += 1;
    }
    let digits_start = i;
    while i < n && bytes[i].is_ascii_digit() {
      i += 1;
    }
    if digits_start == i {
      continue;
    }
    // Must NOT be followed by `.` (decimal) or `e`/`E` (sci) -- only
    // pure integer literals.
    if i < n && (bytes[i] == b'.' || bytes[i] == b'e' || bytes[i] == b'E') {
      continue;
    }
    let lit = &pred[lit_start..i];
    let Ok(v) = lit.parse::<i128>() else {
      continue;
    };
    let Some(ty) = resolve_column_type(scope, catalog, qualifier, name) else {
      continue;
    };
    let Some((min, max, type_label)) = int_range(&ty) else {
      continue;
    };
    if (v < min || v > max) && emitted_at.insert(id_start) {
      let abs_s = abs_off + id_start;
      let abs_e = abs_off + i;
      let _ = (stmt_range,);
      out.push(Diagnostic {
        code: "sql456",
        severity: Severity::Error,
        message: format!(
          "literal `{v}` is outside the range of column `{name}` ({type_label}: {min}..{max}) -- PG raises 22003 \"{type_label} out of range\" at execution; the comparison can never match"
        ),
        range: TextRange::new((abs_s as u32).into(), (abs_e as u32).into()),
      });
    }
  }
}

fn is_ident_start(b: u8) -> bool {
  b.is_ascii_alphabetic() || b == b'_'
}

fn split_qualifier(ident: &str) -> (Option<&str>, &str) {
  if let Some(dot) = ident.rfind('.') {
    (Some(&ident[..dot]), &ident[dot + 1..])
  } else {
    (None, ident)
  }
}

fn peek_cmp_op(bytes: &[u8], i: usize) -> usize {
  let n = bytes.len();
  if i + 2 <= n {
    let two = &bytes[i..i + 2];
    if two == b"<=" || two == b">=" || two == b"<>" || two == b"!=" {
      return 2;
    }
  }
  if i < n && (bytes[i] == b'=' || bytes[i] == b'<' || bytes[i] == b'>') {
    return 1;
  }
  0
}

fn resolve_column_type(scope: &Scope, catalog: &Catalog, qualifier: Option<&str>, name: &str) -> Option<String> {
  let lname = name.to_ascii_lowercase();
  for binding in scope.tables() {
    if let Some(q) = qualifier {
      let key_matches = binding.alias.eq_ignore_ascii_case(q) || binding.table.name.eq_ignore_ascii_case(q);
      if !key_matches {
        continue;
      }
    }
    if let Some(t) = catalog.find_table(binding.table.schema.as_deref(), &binding.table.name) {
      for col in &t.columns {
        if col.name.eq_ignore_ascii_case(&lname) {
          return Some(col.data_type.clone());
        }
      }
    }
  }
  None
}

fn int_range(ty: &str) -> Option<(i128, i128, &'static str)> {
  let lower = ty.to_ascii_lowercase();
  let core = lower.rsplit('.').next().unwrap_or(&lower).trim();
  let core = core.split_whitespace().next().unwrap_or(core);
  match core {
    "smallint" | "int2" => Some((-32_768, 32_767, "smallint")),
    "integer" | "int" | "int4" => Some((-2_147_483_648, 2_147_483_647, "integer")),
    "bigint" | "int8" => Some((-9_223_372_036_854_775_808, 9_223_372_036_854_775_807, "bigint")),
    _ => None,
  }
}
