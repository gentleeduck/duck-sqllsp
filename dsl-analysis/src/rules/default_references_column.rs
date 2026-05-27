//! sql199: `<col> <type> DEFAULT <expr>` where `<expr>` references
//! another column on the same table. PG raises 0A000 "cannot use
//! column reference in DEFAULT expression" at CREATE TABLE.
//!
//! Conservative scan: walks the column list of CREATE TABLE, locates
//! each column's `DEFAULT` clause, then checks bare identifiers in
//! the expression against the set of sibling column names.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::{Statement, StatementKind};
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql199"
  }
  fn default_severity(&self) -> Severity {
    Severity::Error
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let StatementKind::CreateTable(ct) = &stmt.kind else { return };
    let start: usize = u32::from(stmt.range.start()) as usize;
    let end: usize = (u32::from(stmt.range.end()) as usize).min(source.len());
    let raw = &source[start..end];
    let body_owned = crate::textutil::strip_noise_full(raw);
    let body = body_owned.as_str();
    let Some(paren_at) = body.find('(') else { return };
    let Some(close_rel) = find_matching_paren(body, paren_at) else { return };
    let cols_text = &body[paren_at + 1..close_rel];

    let sibling_names: Vec<String> = ct.columns.iter().map(|c| c.name.to_ascii_lowercase()).collect();

    for span in split_top_level(cols_text) {
      let frag = &cols_text[span.start..span.end];
      let frag_upper = frag.to_ascii_uppercase();
      if !frag_upper.contains("DEFAULT") {
        continue;
      }
      let Some(def_at) = frag_upper.find("DEFAULT") else { continue };
      // Ensure DEFAULT is a separate token (not part of an identifier).
      if def_at > 0 {
        let prev = frag.as_bytes()[def_at - 1] as char;
        if prev.is_ascii_alphanumeric() || prev == '_' {
          continue;
        }
      }
      let after = def_at + "DEFAULT".len();
      let expr_text = frag[after..].trim();
      let col_name = frag.split_whitespace().next().unwrap_or("").trim_matches('"').to_ascii_lowercase();
      if col_name.is_empty() {
        continue;
      }
      let bytes = expr_text.as_bytes();
      let mut i = 0usize;
      while i < bytes.len() {
        if bytes[i] == b'\'' {
          i += 1;
          while i < bytes.len() && bytes[i] != b'\'' {
            i += 1
          }
          if i < bytes.len() {
            i += 1
          }
          continue;
        }
        if bytes[i].is_ascii_alphabetic() || bytes[i] == b'_' {
          let s = i;
          while i < bytes.len() && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_') {
            i += 1
          }
          let id_raw = &expr_text[s..i];
          // Function call? skip.
          let mut k = i;
          while k < bytes.len() && bytes[k].is_ascii_whitespace() {
            k += 1
          }
          if k < bytes.len() && bytes[k] == b'(' {
            continue;
          }
          let lc = id_raw.to_ascii_lowercase();
          if is_reserved(&id_raw.to_ascii_uppercase()) {
            continue;
          }
          if sibling_names.contains(&lc) && lc != col_name {
            let abs_s = start + paren_at + 1 + span.start;
            let abs_e = start + paren_at + 1 + span.end;
            out.push(Diagnostic {
              code: "sql199",
              severity: Severity::Error,
              message: format!(
                "DEFAULT for `{col_name}` references sibling column `{id_raw}` -- PG raises 0A000; use a trigger or table-level constraint instead"
              ),
              range: text_size::TextRange::new((abs_s as u32).into(), (abs_e as u32).into()),
            });
            break;
          }
        } else {
          i += 1;
        }
      }
    }
  }
}

struct Span {
  start: usize,
  end: usize,
}

fn split_top_level(text: &str) -> Vec<Span> {
  let mut out = Vec::new();
  let bytes = text.as_bytes();
  let mut depth = 0i32;
  let mut start = 0usize;
  let mut i = 0usize;
  while i < bytes.len() {
    match bytes[i] {
      b'(' => depth += 1,
      b')' => depth -= 1,
      b',' if depth == 0 => {
        out.push(Span { start, end: i });
        start = i + 1
      },
      b'\'' => {
        i += 1;
        while i < bytes.len() && bytes[i] != b'\'' {
          i += 1
        }
      },
      _ => {},
    }
    i += 1;
  }
  out.push(Span { start, end: bytes.len() });
  out
}

fn find_matching_paren(s: &str, open: usize) -> Option<usize> {
  let bytes = s.as_bytes();
  let mut depth = 0i32;
  let mut i = open;
  while i < bytes.len() {
    match bytes[i] {
      b'(' => depth += 1,
      b')' => {
        depth -= 1;
        if depth == 0 {
          return Some(i);
        }
      },
      b'\'' => {
        i += 1;
        while i < bytes.len() && bytes[i] != b'\'' {
          i += 1
        }
      },
      _ => {},
    }
    i += 1;
  }
  None
}

fn is_reserved(up: &str) -> bool {
  matches!(
    up,
    "NULL"
      | "TRUE"
      | "FALSE"
      | "CURRENT_DATE"
      | "CURRENT_TIME"
      | "CURRENT_TIMESTAMP"
      | "NOW"
      | "LOCALTIME"
      | "LOCALTIMESTAMP"
      | "CURRENT_USER"
      | "SESSION_USER"
      | "USER"
      | "CAST"
      | "AS"
      | "ARRAY"
  )
}
