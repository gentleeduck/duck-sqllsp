//! sql249: `INSERT INTO t DEFAULT VALUES` -- requires every column
//! to be NOT NULL with a DEFAULT, GENERATED, or nullable. Catches
//! the common case where the catalog table has a NOT NULL column
//! without DEFAULT (and not a serial / generated identity), which
//! PG raises 23502 at runtime.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::{Statement, StatementKind};
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql249"
  }
  fn default_severity(&self) -> Severity {
    Severity::Error
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let StatementKind::Insert(ins) = &stmt.kind else { return };
    let start: usize = u32::from(stmt.range.start()) as usize;
    let end: usize = (u32::from(stmt.range.end()) as usize).min(source.len());
    let raw = &source[start..end];
    // Strip comments / strings so `-- INSERT DEFAULT VALUES` in a
    // header comment doesn't trigger the rule on the next INSERT
    // (which may itself be a normal `... VALUES (...)`).
    let body_owned = strip_comments_and_strings(raw);
    let body = body_owned.as_str();
    let upper = body.to_ascii_uppercase();
    if !upper.contains("DEFAULT VALUES") {
      return;
    }
    let Some(t) = catalog.find_table(ins.table.schema.as_deref(), &ins.table.name) else { return };
    // The catalog's `Column.default` is populated from `ConstraintExpr`
    // cooked_expr -- which is empty for client-side parses. So for
    // offline buffers a NOT NULL column with `DEFAULT 0` looks
    // identical to a NOT NULL column without a default. Scan the
    // source text for the CREATE TABLE that matches and extract per-
    // column DEFAULT presence as a fallback.
    let source_defaults: std::collections::HashSet<String> = scan_defaults_for(source, &t.name);
    let bad: Vec<&str> = t
      .columns
      .iter()
      .filter(|c| {
        !c.nullable
          && c.default.is_none()
          && c.generated.is_none()
          && !is_implicit_serial(&c.data_type)
          && !source_defaults.contains(&c.name.to_ascii_lowercase())
      })
      .map(|c| c.name.as_str())
      .collect();
    if bad.is_empty() {
      return;
    }
    let abs_s = start;
    let abs_e = start + body.find(';').unwrap_or(body.len());
    out.push(Diagnostic {
      code: "sql249",
      severity: Severity::Error,
      message: format!(
        "INSERT DEFAULT VALUES into `{}` -- NOT NULL columns without DEFAULT: {} -- PG raises 23502",
        t.name,
        bad.join(", "),
      ),
      range: text_size::TextRange::new((abs_s as u32).into(), (abs_e as u32).into()),
    });
  }
}

/// Find `CREATE TABLE <table>` (case-insensitive) in source and return
/// the lowercase names of columns whose definition contains a DEFAULT
/// clause. Best-effort and lenient: depth-tracked, comment-stripped,
/// string-stripped scan of the body inside the outer `(...)`.
fn scan_defaults_for(source: &str, table: &str) -> std::collections::HashSet<String> {
  let mut out: std::collections::HashSet<String> = std::collections::HashSet::new();
  let cleaned = strip_comments_and_strings(source);
  let upper = cleaned.to_ascii_uppercase();
  let bytes = cleaned.as_bytes();
  let n = bytes.len();
  let needle = "CREATE TABLE";
  let mut from = 0usize;
  while let Some(rel) = upper[from..].find(needle) {
    let at = from + rel;
    // Skip past keyword + optional IF NOT EXISTS + identifier.
    let mut k = at + needle.len();
    while k < n && bytes[k].is_ascii_whitespace() {
      k += 1
    }
    // Optional IF NOT EXISTS.
    if upper.get(k..).is_some_and(|s| s.starts_with("IF NOT EXISTS")) {
      k += "IF NOT EXISTS".len();
      while k < n && bytes[k].is_ascii_whitespace() {
        k += 1
      }
    }
    let id_start = k;
    while k < n && (bytes[k].is_ascii_alphanumeric() || bytes[k] == b'_' || bytes[k] == b'.' || bytes[k] == b'"') {
      k += 1
    }
    let id_end = k;
    let id = cleaned[id_start..id_end].trim_matches('"').to_ascii_lowercase();
    let bare = id.rsplit('.').next().unwrap_or(&id).to_string();
    from = id_end;
    if bare != table.to_ascii_lowercase() {
      continue;
    }
    // Find matching `(` and balance until `)`.
    while k < n && bytes[k] != b'(' {
      k += 1
    }
    if k >= n {
      continue;
    }
    let body_start = k + 1;
    let mut depth = 1i32;
    let mut j = body_start;
    while j < n && depth > 0 {
      match bytes[j] {
        b'(' => depth += 1,
        b')' => depth -= 1,
        _ => {},
      }
      if depth == 0 {
        break;
      }
      j += 1;
    }
    let body = &cleaned[body_start..j];
    // Split body on top-level commas, then for each piece check first
    // word is identifier and body contains DEFAULT.
    let mut piece_start = 0usize;
    let mut d = 0i32;
    let bb = body.as_bytes();
    let mut idx = 0usize;
    while idx <= bb.len() {
      let ch = if idx < bb.len() { bb[idx] } else { b',' };
      match ch {
        b'(' => d += 1,
        b')' => d -= 1,
        b',' if d == 0 => {
          let piece = &body[piece_start..idx];
          handle_piece(piece, &mut out);
          piece_start = idx + 1;
        },
        _ => {},
      }
      idx += 1;
    }
    break;
  }
  out
}

fn handle_piece(piece: &str, out: &mut std::collections::HashSet<String>) {
  let trimmed = piece.trim_start();
  let bytes = trimmed.as_bytes();
  if bytes.is_empty() {
    return;
  }
  // Skip table-level constraints (start with PRIMARY/UNIQUE/CHECK/FOREIGN/EXCLUDE/CONSTRAINT/LIKE).
  let upper_head: String = trimmed.chars().take(20).collect::<String>().to_ascii_uppercase();
  if upper_head.starts_with("PRIMARY ")
    || upper_head.starts_with("UNIQUE ")
    || upper_head.starts_with("UNIQUE(")
    || upper_head.starts_with("CHECK ")
    || upper_head.starts_with("CHECK(")
    || upper_head.starts_with("FOREIGN ")
    || upper_head.starts_with("EXCLUDE ")
    || upper_head.starts_with("CONSTRAINT ")
    || upper_head.starts_with("LIKE ")
  {
    return;
  }
  // First word = column name.
  let mut k = 0usize;
  while k < bytes.len() && (bytes[k].is_ascii_alphanumeric() || bytes[k] == b'_' || bytes[k] == b'"') {
    k += 1
  }
  if k == 0 {
    return;
  }
  let name = trimmed[..k].trim_matches('"').to_ascii_lowercase();
  // Check rest for DEFAULT keyword (whole word).
  let rest_upper = trimmed[k..].to_ascii_uppercase();
  let rb = rest_upper.as_bytes();
  let needle = "DEFAULT";
  let mut i = 0usize;
  while i + needle.len() <= rb.len() {
    if &rest_upper[i..i + needle.len()] == needle {
      let prev_ok = i == 0 || !(rb[i - 1].is_ascii_alphanumeric() || rb[i - 1] == b'_');
      let next_ok =
        i + needle.len() == rb.len() || !(rb[i + needle.len()].is_ascii_alphanumeric() || rb[i + needle.len()] == b'_');
      if prev_ok && next_ok {
        out.insert(name);
        return;
      }
    }
    i += 1;
  }
  // Also treat GENERATED ALWAYS AS / GENERATED BY DEFAULT AS as having a default.
  if rest_upper.contains("GENERATED ") {
    out.insert(name);
  }
}

/// `serial`, `bigserial`, `smallserial`, `serial2/4/8` -- PG implicitly
/// creates a sequence + DEFAULT nextval(...) for these. The AST
/// converter doesn't write the synthetic DEFAULT onto the column, so
/// the rule has to recognise the type-name itself.
fn is_implicit_serial(type_name: &str) -> bool {
  let t = type_name.trim().to_ascii_lowercase();
  matches!(t.as_str(), "serial" | "serial4" | "bigserial" | "serial8" | "smallserial" | "serial2")
}

fn strip_comments_and_strings(s: &str) -> String {
  let mut out = String::with_capacity(s.len());
  let bytes = s.as_bytes();
  let n = bytes.len();
  let mut i = 0usize;
  while i < n {
    if i + 1 < n && bytes[i] == b'-' && bytes[i + 1] == b'-' {
      while i < n && bytes[i] != b'\n' {
        out.push(' ');
        i += 1
      }
    } else if i + 1 < n && bytes[i] == b'/' && bytes[i + 1] == b'*' {
      let mut depth = 1u32;
      out.push(' ');
      out.push(' ');
      i += 2;
      while i + 1 < n && depth > 0 {
        if bytes[i] == b'/' && bytes[i + 1] == b'*' {
          depth += 1;
          out.push(' ');
          out.push(' ');
          i += 2;
        } else if bytes[i] == b'*' && bytes[i + 1] == b'/' {
          depth -= 1;
          out.push(' ');
          out.push(' ');
          i += 2;
        } else {
          out.push(' ');
          i += 1;
        }
      }
    } else if bytes[i] == b'\'' {
      out.push(' ');
      i += 1;
      while i < n && bytes[i] != b'\'' {
        out.push(' ');
        i += 1
      }
      if i < n {
        out.push(' ');
        i += 1
      }
    } else if bytes[i].is_ascii() {
      out.push(bytes[i] as char);
      i += 1;
    } else {
      out.push(' ');
      i += 1;
    }
  }
  out
}
