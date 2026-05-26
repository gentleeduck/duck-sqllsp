//! sql001: table referenced by FROM / JOIN / UPDATE / DELETE / INSERT INTO
//! does not exist in the catalog.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::{Statement, StatementKind, TableRef};
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql001"
  }
  fn default_severity(&self) -> Severity {
    Severity::Error
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let refs = collect_tables(&stmt.kind);
    if refs.is_empty() {
      return;
    }

    // CTE names declared in the same statement count as virtual tables.
    let start: u32 = stmt.range.start().into();
    let end: u32 = stmt.range.end().into();
    let raw_slice = &source[start as usize..(end as usize).min(source.len())];
    // Strip line/block comments + strings before CTE walk so a
    // leading header comment like `-- WITH RECURSIVE foo` doesn't
    // hijack the "WITH" anchor and make the rule see zero CTEs.
    let slice_owned = strip_noise(raw_slice);
    let slice = slice_owned.as_str();
    let ctes = collect_cte_names(slice);

    for r in refs {
      if r.name.is_empty() {
        continue;
      }
      if ctes.iter().any(|c| c.eq_ignore_ascii_case(&r.name)) {
        continue;
      }
      if catalog.find_table(r.schema.as_deref(), &r.name).is_some() {
        continue;
      }
      // Catalog might be empty when the user hasn't connected yet;
      // suppress the false positive in that case.
      if catalog.tables().next().is_none() {
        continue;
      }
      // Narrow the diagnostic to the table reference itself when
      // the parser exposed a real range. Falls back to the whole
      // statement only when the backend didn't populate the
      // TableRef range (rare).
      let range = if r.range.len() > text_size::TextSize::from(0) { r.range } else { stmt.range };
      let suggestion = nearest_match(&r.name, catalog);
      let msg = match suggestion {
        Some(s) => format!("unresolved table `{}` — did you mean `{}`?", fq(&r), s,),
        None => format!("unresolved table `{}` (no match in catalog)", fq(&r)),
      };
      out.push(Diagnostic { code: "sql001", severity: Severity::Error, message: msg, range });
    }
  }
}

/// Find every CTE name in a WITH clause. Handles `WITH foo AS (...), bar AS (...)`
/// and `WITH RECURSIVE foo AS (...)`. The body of each CTE may itself
/// contain commas, so we count paren depth.
fn collect_cte_names(src: &str) -> Vec<String> {
  let bytes = src.as_bytes();
  let n = bytes.len();
  let upper = src.to_ascii_uppercase();
  let with_pos = match upper.find("WITH") {
    Some(p) if p == 0 || !is_word(bytes[p - 1] as char) => p,
    _ => return Vec::new(),
  };
  let mut i = with_pos + "WITH".len();
  // Skip optional RECURSIVE
  while i < n && (bytes[i] as char).is_whitespace() {
    i += 1;
  }
  if upper[i..].starts_with("RECURSIVE") && (i + 9 == n || !is_word(bytes[i + 9] as char)) {
    i += "RECURSIVE".len();
  }
  let mut out = Vec::new();
  loop {
    while i < n && (bytes[i] as char).is_whitespace() {
      i += 1;
    }
    // Read an identifier (CTE name).
    let name_start = i;
    while i < n && is_word(bytes[i] as char) {
      i += 1;
    }
    if i == name_start {
      break;
    }
    let name = src[name_start..i].to_string();
    // Optional column list (...) after the name.
    while i < n && (bytes[i] as char).is_whitespace() {
      i += 1;
    }
    if i < n && bytes[i] == b'(' {
      i = skip_parens(bytes, i);
    }
    // Expect AS
    while i < n && (bytes[i] as char).is_whitespace() {
      i += 1;
    }
    if !upper[i..].starts_with("AS") {
      break;
    }
    i += 2;
    // Skip MATERIALIZED / NOT MATERIALIZED keywords
    while i < n && (bytes[i] as char).is_whitespace() {
      i += 1;
    }
    if upper[i..].starts_with("NOT") {
      i += 3;
    }
    while i < n && (bytes[i] as char).is_whitespace() {
      i += 1;
    }
    if upper[i..].starts_with("MATERIALIZED") {
      i += "MATERIALIZED".len();
    }
    // Skip body in `(...)`.
    while i < n && (bytes[i] as char).is_whitespace() {
      i += 1;
    }
    if i >= n || bytes[i] != b'(' {
      break;
    }
    i = skip_parens(bytes, i);
    out.push(name);
    // Comma -> another CTE; anything else (or SELECT/INSERT keyword) ends the WITH.
    while i < n && (bytes[i] as char).is_whitespace() {
      i += 1;
    }
    if i < n && bytes[i] == b',' {
      i += 1;
      continue;
    }
    break;
  }
  out
}

fn is_word(c: char) -> bool {
  c.is_alphanumeric() || c == '_'
}

fn strip_noise(s: &str) -> String {
  let mut out: Vec<u8> = s.as_bytes().to_vec();
  let n = out.len();
  let mut i = 0usize;
  while i < n {
    if i + 1 < n && out[i] == b'-' && out[i + 1] == b'-' {
      while i < n && out[i] != b'\n' { out[i] = b' '; i += 1 }
      continue;
    }
    if i + 1 < n && out[i] == b'/' && out[i + 1] == b'*' {
      let mut depth = 1u32;
      out[i] = b' '; out[i + 1] = b' '; i += 2;
      while i + 1 < n && depth > 0 {
        if out[i] == b'/' && out[i + 1] == b'*' { depth += 1; out[i] = b' '; out[i + 1] = b' '; i += 2; }
        else if out[i] == b'*' && out[i + 1] == b'/' { depth -= 1; out[i] = b' '; out[i + 1] = b' '; i += 2; }
        else { out[i] = b' '; i += 1; }
      }
      continue;
    }
    if out[i] == b'\'' {
      out[i] = b' '; i += 1;
      while i < n && out[i] != b'\'' { out[i] = b' '; i += 1 }
      if i < n { out[i] = b' '; i += 1 }
      continue;
    }
    i += 1;
  }
  String::from_utf8(out).unwrap_or_else(|_| s.to_string())
}

fn skip_parens(bytes: &[u8], i: usize) -> usize {
  if i >= bytes.len() || bytes[i] != b'(' {
    return i;
  }
  let mut depth = 0i32;
  let mut j = i;
  let n = bytes.len();
  while j < n {
    match bytes[j] {
      b'(' => {
        depth += 1;
        j += 1;
      },
      b')' => {
        depth -= 1;
        j += 1;
        if depth == 0 {
          return j;
        }
      },
      b'\'' => {
        j += 1;
        while j < n {
          if bytes[j] == b'\'' {
            j += 1;
            break;
          }
          j += 1;
        }
      },
      _ => j += 1,
    }
  }
  j
}

fn fq(r: &TableRef) -> String {
  match &r.schema {
    Some(s) => format!("{s}.{}", r.name),
    None => r.name.clone(),
  }
}

/// Find the closest catalog table name to `wanted` by case-insensitive
/// prefix or substring match. Returns `None` when nothing is similar.
fn nearest_match(wanted: &str, catalog: &Catalog) -> Option<String> {
  let lower = wanted.to_ascii_lowercase();
  let mut best: Option<(usize, String)> = None;
  for t in catalog.tables() {
    let name_l = t.name.to_ascii_lowercase();
    let score = if name_l == lower {
      return Some(t.name.clone());
    } else if name_l.starts_with(&lower) || lower.starts_with(&name_l) {
      1
    } else if name_l.contains(&lower) || lower.contains(&name_l) {
      2
    } else {
      // Levenshtein-ish: count shared characters.
      let shared = name_l.chars().filter(|c| lower.contains(*c)).count();
      if shared * 2 < lower.len() {
        continue;
      }
      3
    };
    match &best {
      None => best = Some((score, t.name.clone())),
      Some((s, _)) if score < *s => best = Some((score, t.name.clone())),
      _ => {},
    }
  }
  best.map(|(_, n)| n)
}

fn collect_tables(kind: &StatementKind) -> Vec<TableRef> {
  let mut out = Vec::new();
  match kind {
    StatementKind::Select(s) => {
      for t in &s.from {
        if is_synthetic(t) { continue }
        out.push(t.clone());
      }
      for j in &s.joins {
        if is_synthetic(&j.table) { continue }
        out.push(j.table.clone());
      }
    },
    StatementKind::Update(u) => { if !is_synthetic(&u.table) { out.push(u.table.clone()); } },
    StatementKind::Delete(d) => { if !is_synthetic(&d.table) { out.push(d.table.clone()); } },
    StatementKind::Insert(i) => { if !is_synthetic(&i.table) { out.push(i.table.clone()); } },
    _ => {},
  }
  out
}

/// Sentinel-schema markers set by the parser backends to flag bindings
/// that are not real catalog tables: function-call FROM (`<func>`),
/// subquery alias (`<subq>`), and CTE refs. sql001 / sql002 / sql349 /
/// etc. should skip catalog lookups for these.
fn is_synthetic(t: &TableRef) -> bool {
  t.schema.as_deref().map_or(false, |s| s.starts_with('<'))
}
