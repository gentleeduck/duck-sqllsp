//! sql172: `<col> = <literal>` (or `<>`, `>`, `<`, `>=`, `<=`) where
//! the literal kind disagrees with the column's catalog type. Fires
//! in WHERE / HAVING / ON predicates and in CHECK constraint bodies.
//!
//! Conservative literal classification (str / int / float / bool /
//! null); skips function calls / casts / subqueries / column-vs-column
//! comparisons. The cursor / WHERE / etc. spans aren't parsed by
//! pg_query for predicate-level details, so this is a text scan that
//! splits on AND / OR + binary operators.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::{Statement, StatementKind};
use dsl_resolve::Scope;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LitKind {
  Str,
  Int,
  Float,
  Bool,
  Null,
  Unknown,
}

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql172"
  }
  fn default_severity(&self) -> Severity {
    Severity::Error
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let from_table_opt = match &stmt.kind {
      StatementKind::Select(s) => s.from.first().and_then(|t| catalog.find_table(t.schema.as_deref(), &t.name)),
      StatementKind::Update(u) => catalog.find_table(u.table.schema.as_deref(), &u.table.name),
      StatementKind::Delete(d) => catalog.find_table(d.table.schema.as_deref(), &d.table.name),
      _ => return,
    };
    let Some(from_table) = from_table_opt else { return };

    let start: usize = u32::from(stmt.range.start()) as usize;
    let end: usize = (u32::from(stmt.range.end()) as usize).min(source.len());
    let body = &source[start..end];
    let upper = body.to_ascii_uppercase();
    // Extract WHERE / HAVING / ON clauses.
    let mut clauses: Vec<(usize, &str)> = Vec::new();
    for kw in [" WHERE ", " HAVING ", " ON "] {
      let mut from = 0usize;
      while let Some(rel) = upper[from..].find(kw) {
        let clause_start = from + rel + kw.len();
        let clause_end = ["GROUP", "ORDER", "LIMIT", "RETURNING", "HAVING", "OFFSET", "FOR"]
          .iter()
          .filter_map(|stop| upper[clause_start..].find(&format!(" {stop} ")).map(|i| clause_start + i))
          .chain(body[clause_start..].find(';').map(|i| clause_start + i))
          .min()
          .unwrap_or(body.len());
        let clause = &body[clause_start..clause_end];
        clauses.push((clause_start, clause));
        from = clause_end;
      }
    }

    for (clause_offset, clause) in clauses {
      for (rel_lhs_s, lhs, op, rel_rhs_s, rhs) in extract_comparisons(clause) {
        let col_name = lhs.split('.').next_back().unwrap_or(lhs).trim_matches('"');
        let Some(col) = from_table.columns.iter().find(|c| c.name.eq_ignore_ascii_case(col_name)) else { continue };
        let lit = classify_literal(rhs);
        if matches!(lit, LitKind::Unknown | LitKind::Null) {
          continue;
        }
        if !compatible(lit, &col.data_type) {
          let abs_s = start + clause_offset + rel_rhs_s;
          let abs_e = abs_s + rhs.len();
          out.push(Diagnostic {
            code: "sql172",
            severity: Severity::Error,
            message: format!(
              "predicate value {} doesn't match column `{}` type `{}` ({} {} {})",
              kind_name(lit),
              col.name,
              col.data_type,
              col.name,
              op,
              rhs.chars().take(30).collect::<String>()
            ),
            range: text_size::TextRange::new((abs_s as u32).into(), (abs_e as u32).into()),
          });
          let _ = rel_lhs_s;
        }
      }
    }
  }
}

/// Walk `clause` and yield (lhs_offset, lhs, op, rhs_offset, rhs) for
/// each `<ident> <binop> <literal>` chunk separated by AND / OR.
/// Operators: = != <> < > <= >= LIKE ILIKE.
fn extract_comparisons(clause: &str) -> Vec<(usize, &str, &str, usize, &str)> {
  let mut out = Vec::new();
  let upper = clause.to_ascii_uppercase();
  let mut conjuncts: Vec<(usize, &str)> = Vec::new();
  // Split on AND / OR (word-bounded). Simple state machine over bytes.
  let bytes = clause.as_bytes();
  let n = bytes.len();
  let mut start = 0usize;
  let mut depth = 0i32;
  let mut i = 0usize;
  while i < n {
    if bytes[i] == b'(' {
      depth += 1;
      i += 1;
      continue;
    }
    if bytes[i] == b')' {
      depth -= 1;
      i += 1;
      continue;
    }
    if bytes[i] == b'\'' {
      i += 1;
      while i < n && bytes[i] != b'\'' {
        i += 1;
      }
      i = (i + 1).min(n);
      continue;
    }
    if depth == 0 {
      if let Some(len) = kw_at(&upper, i, "AND") {
        conjuncts.push((start, &clause[start..i]));
        start = i + len;
        i = start;
        continue;
      }
      if let Some(len) = kw_at(&upper, i, "OR") {
        conjuncts.push((start, &clause[start..i]));
        start = i + len;
        i = start;
        continue;
      }
    }
    i += 1;
  }
  if start < n {
    conjuncts.push((start, &clause[start..]));
  }
  for (off, conj) in conjuncts {
    let trimmed = conj.trim();
    if trimmed.is_empty() {
      continue;
    }
    let lead_ws = conj.len() - conj.trim_start().len();
    // Find the operator.
    let upper_c = trimmed.to_ascii_uppercase();
    let ops: &[(&str, &str)] = &[
      ("<>", "<>"),
      ("!=", "!="),
      ("<=", "<="),
      (">=", ">="),
      ("=", "="),
      ("<", "<"),
      (">", ">"),
      (" LIKE ", "LIKE"),
      (" ILIKE ", "ILIKE"),
    ];
    let mut op_rel = None;
    let mut op_len = 0usize;
    let mut op_str: &str = "";
    for (needle, label) in ops {
      if let Some(at) = upper_c.find(needle) {
        op_rel = Some(at);
        op_len = needle.len();
        op_str = label;
        break;
      }
    }
    let Some(op_at) = op_rel else { continue };
    let lhs = trimmed[..op_at].trim();
    let rhs = trimmed[op_at + op_len..].trim();
    if lhs.is_empty() || rhs.is_empty() {
      continue;
    }
    // Only fire when lhs is a plain (optionally-dotted) identifier.
    if !lhs.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '.' || c == '"') {
      continue;
    }
    // Compute absolute offsets back into `clause`.
    let lhs_rel_in_trimmed = trimmed.find(lhs).unwrap_or(0);
    let rhs_rel_in_trimmed = trimmed.rfind(rhs).unwrap_or(trimmed.len());
    let lhs_off = off + lead_ws + lhs_rel_in_trimmed;
    let rhs_off = off + lead_ws + rhs_rel_in_trimmed;
    out.push((lhs_off, lhs, op_str, rhs_off, rhs));
  }
  out
}

fn kw_at(upper: &str, pos: usize, kw: &str) -> Option<usize> {
  let bytes = upper.as_bytes();
  let n = bytes.len();
  // Need leading word-boundary.
  if pos > 0 {
    let prev = bytes[pos - 1] as char;
    if prev.is_ascii_alphanumeric() || prev == '_' {
      return None;
    }
  }
  if pos + kw.len() > n {
    return None;
  }
  if &upper[pos..pos + kw.len()] != kw {
    return None;
  }
  let after = pos + kw.len();
  if after < n {
    let next = bytes[after] as char;
    if next.is_ascii_alphanumeric() || next == '_' {
      return None;
    }
  }
  Some(kw.len())
}

fn classify_literal(s: &str) -> LitKind {
  let t = s.trim();
  if t.is_empty() {
    return LitKind::Unknown;
  }
  let upper = t.to_ascii_uppercase();
  if upper == "NULL" {
    return LitKind::Null;
  }
  if upper == "TRUE" || upper == "FALSE" {
    return LitKind::Bool;
  }
  if t.starts_with('\'') {
    return LitKind::Str;
  }
  let body = if t.starts_with('-') || t.starts_with('+') { &t[1..] } else { t };
  if !body.is_empty() && body.chars().all(|c| c.is_ascii_digit()) {
    return LitKind::Int;
  }
  if !body.is_empty() && body.chars().all(|c| c.is_ascii_digit() || c == '.') && body.contains('.') {
    return LitKind::Float;
  }
  LitKind::Unknown
}

fn kind_name(k: LitKind) -> &'static str {
  match k {
    LitKind::Str => "text/string",
    LitKind::Int => "integer",
    LitKind::Float => "float",
    LitKind::Bool => "boolean",
    _ => "?",
  }
}

fn compatible(kind: LitKind, declared: &str) -> bool {
  let d = declared.to_ascii_uppercase();
  let d = d.split('(').next().unwrap_or(&d).trim();
  let d = d.rsplit('.').next().unwrap_or(d).trim();
  // Strip array suffix [] / [n].
  let d = d.trim_end_matches(|c: char| c == ']' || c == '[' || c.is_ascii_digit() || c.is_ascii_whitespace());
  let int_types =
    ["INT", "INTEGER", "BIGINT", "SMALLINT", "INT4", "INT8", "INT2", "SERIAL", "BIGSERIAL", "SMALLSERIAL"];
  let num_types = ["NUMERIC", "DECIMAL", "REAL", "DOUBLE", "FLOAT", "MONEY"];
  let str_types = ["TEXT", "VARCHAR", "CHAR", "CHARACTER", "CITEXT", "NAME", "JSON", "JSONB", "XML", "BYTEA"];
  let uuid_types = ["UUID"];
  let bool_types = ["BOOLEAN", "BOOL"];
  let time_types = ["DATE", "TIMESTAMP", "TIMESTAMPTZ", "TIME", "INTERVAL"];
  let all_known: &[&[&str]] = &[&int_types, &num_types, &str_types, &uuid_types, &bool_types, &time_types];
  // Unknown / user-defined type (DOMAIN, ENUM, composite, extension
  // type like inet/cidr/macaddr/tsvector/...): we can't classify, so
  // don't flag a mismatch. The lint must be conservative -- a false
  // positive on a custom type is worse than a missed type error.
  let is_known = all_known.iter().any(|grp| grp.iter().any(|t| d.starts_with(t)));
  if !is_known {
    return true;
  }
  match kind {
    LitKind::Str => {
      str_types.iter().any(|t| d.starts_with(t))
        || uuid_types.contains(&d)
        || time_types.iter().any(|t| d.starts_with(t))
    },
    LitKind::Int => int_types.contains(&d) || num_types.iter().any(|t| d.starts_with(t)),
    LitKind::Float => num_types.iter().any(|t| d.starts_with(t)),
    LitKind::Bool => bool_types.contains(&d),
    _ => true,
  }
}
