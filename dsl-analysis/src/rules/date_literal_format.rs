//! sql182: `INSERT INTO t (d) VALUES ('garbage')` where `d` is
//! DATE / TIMESTAMP / TIMESTAMPTZ / TIME and the string literal
//! doesn't parse as that type. Lightweight regex check at edit
//! time; PG raises 22007 / 22008 at runtime.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::{Statement, StatementKind};
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql182"
  }
  fn default_severity(&self) -> Severity {
    Severity::Error
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let StatementKind::Insert(ins) = &stmt.kind else { return };
    if ins.columns.is_empty() {
      return;
    }
    let Some(t) = catalog.find_table(ins.table.schema.as_deref(), &ins.table.name) else { return };

    let start: usize = u32::from(stmt.range.start()) as usize;
    let end: usize = (u32::from(stmt.range.end()) as usize).min(source.len());
    let body = &source[start..end];
    let upper = body.to_ascii_uppercase();
    let Some(values_at) = upper.find("VALUES") else { return };
    let bytes = body.as_bytes();
    let n = bytes.len();
    let mut k = values_at + 6;
    while k < n && bytes[k].is_ascii_whitespace() {
      k += 1;
    }
    if k >= n || bytes[k] != b'(' {
      return;
    }
    let Some(close) = match_paren(bytes, k) else { return };
    let tuple = &body[k + 1..close];
    let values = split_top_commas(tuple);
    if values.len() != ins.columns.len() {
      return;
    }

    for (col_name, raw_val) in ins.columns.iter().zip(values.iter()) {
      let trimmed = raw_val.trim();
      if !trimmed.starts_with('\'') || !trimmed.ends_with('\'') {
        continue;
      }
      let lit = &trimmed[1..trimmed.len() - 1];
      let Some(col) = t.columns.iter().find(|c| c.name.eq_ignore_ascii_case(col_name)) else { continue };
      let ty = col.data_type.to_ascii_uppercase();
      let ty = ty.rsplit('.').next().unwrap_or(&ty).trim();
      let expected = if ty.starts_with("DATE") { Some(TemporalKind::Date) }
        else if ty.starts_with("TIMESTAMP") { Some(TemporalKind::Timestamp) }
        else if ty.starts_with("TIME") { Some(TemporalKind::Time) }
        else { None };
      let Some(kind) = expected else { continue };
      if validates(lit, kind) {
        continue;
      }
      let rel = raw_val.as_ptr() as usize - body.as_ptr() as usize;
      let lead = raw_val.len() - raw_val.trim_start().len();
      let abs_s = start + rel + lead;
      let abs_e = abs_s + trimmed.len();
      out.push(Diagnostic {
        code: "sql182",
        severity: Severity::Error,
        message: format!(
          "literal `'{}'` not a valid {} -- PG raises 22007/22008 at exec",
          lit.chars().take(40).collect::<String>(),
          kind.name()
        ),
        range: text_size::TextRange::new((abs_s as u32).into(), (abs_e as u32).into()),
      });
    }
  }
}

#[derive(Clone, Copy)]
enum TemporalKind {
  Date,
  Time,
  Timestamp,
}

impl TemporalKind {
  fn name(&self) -> &'static str {
    match self {
      TemporalKind::Date => "DATE",
      TemporalKind::Time => "TIME",
      TemporalKind::Timestamp => "TIMESTAMP",
    }
  }
}

fn validates(lit: &str, kind: TemporalKind) -> bool {
  let s = lit.trim();
  if s.is_empty() { return false; }
  // PG accepts many forms; allow these high-confidence shapes:
  //   DATE          -- YYYY-MM-DD
  //   TIME          -- HH:MM[:SS[.fraction]]
  //   TIMESTAMP     -- YYYY-MM-DD HH:MM[:SS][.fraction][+TZ]
  //   plus 'now', 'today', 'tomorrow', 'yesterday', 'infinity', '-infinity', 'epoch'
  let lower = s.to_ascii_lowercase();
  if matches!(lower.as_str(), "now" | "today" | "tomorrow" | "yesterday" | "infinity" | "-infinity" | "epoch") {
    return true;
  }
  match kind {
    TemporalKind::Date => looks_like_date(s),
    TemporalKind::Time => looks_like_time(s),
    TemporalKind::Timestamp => looks_like_date(s.split_whitespace().next().unwrap_or(""))
      || looks_like_iso_ts(s),
  }
}

fn looks_like_date(s: &str) -> bool {
  let parts: Vec<&str> = s.split('-').collect();
  if parts.len() != 3 { return false; }
  parts.iter().enumerate().all(|(i, p)| {
    let want = if i == 0 { 4 } else { 2 };
    p.len() == want && p.chars().all(|c| c.is_ascii_digit())
  })
}

fn looks_like_time(s: &str) -> bool {
  let parts: Vec<&str> = s.split(':').collect();
  if parts.len() < 2 || parts.len() > 3 { return false; }
  parts.iter().enumerate().all(|(i, p)| {
    let bare = p.split('.').next().unwrap_or(p);
    bare.chars().all(|c| c.is_ascii_digit()) && (bare.len() == 2 || (i == 2 && bare.len() <= 2))
  })
}

fn looks_like_iso_ts(s: &str) -> bool {
  // YYYY-MM-DD[T ]HH:MM[:SS][.fraction][TZ]
  let mid = s.find('T').or_else(|| s.find(' '));
  let Some(mid) = mid else { return false };
  let (date_part, rest) = s.split_at(mid);
  if !looks_like_date(date_part) { return false; }
  let time_part = rest.trim_start_matches(|c| c == 'T' || c == ' ');
  // Trim trailing timezone marker (+HH / -HH / Z / +HH:MM).
  let time_only = time_part
    .trim_end_matches(|c: char| c.is_ascii_digit() || c == ':' || c == '+' || c == '-' || c == 'Z' || c == ' ' || c == '.');
  let _ = time_only;
  let trimmed = time_part.trim_end_matches(['Z', ' ']);
  let parts: Vec<&str> = trimmed.splitn(2, |c| c == '+' || c == '-').collect();
  looks_like_time(parts[0])
}

fn match_paren(bytes: &[u8], open: usize) -> Option<usize> {
  let n = bytes.len();
  let mut depth = 0i32;
  let mut i = open;
  while i < n {
    match bytes[i] {
      b'(' => depth += 1,
      b')' => {
        depth -= 1;
        if depth == 0 {
          return Some(i);
        }
      }
      b'\'' => {
        i += 1;
        while i < n && bytes[i] != b'\'' { i += 1; }
      }
      _ => {}
    }
    i += 1;
  }
  None
}

fn split_top_commas(s: &str) -> Vec<&str> {
  let bytes = s.as_bytes();
  let n = bytes.len();
  let mut out = Vec::new();
  let mut depth = 0i32;
  let mut start = 0usize;
  let mut i = 0usize;
  while i < n {
    match bytes[i] {
      b'\'' => {
        i += 1;
        while i < n && bytes[i] != b'\'' { i += 1; }
      }
      b'(' => depth += 1,
      b')' => depth -= 1,
      b',' if depth == 0 => {
        out.push(&s[start..i]);
        start = i + 1;
      }
      _ => {}
    }
    i += 1;
  }
  if start < n {
    out.push(&s[start..]);
  }
  out
}
