//! sql118: `SELECT ... INTO foo FROM t` at the top level is **DDL** --
//! it creates a new table `foo`. Usually the user meant PL/pgSQL
//! variable assignment (which only works inside `$$ ... $$`).

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::{Statement, StatementKind};
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql118"
  }
  fn default_severity(&self) -> Severity {
    Severity::Hint
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    // Only run on top-level SELECT statements -- pg_query produces
    // one opaque Statement for a CREATE FUNCTION / DO block, so its
    // kind is never `Select` and the rule skips it automatically.
    if !matches!(stmt.kind, StatementKind::Select(_)) {
      return;
    }
    let start: usize = u32::from(stmt.range.start()) as usize;
    let end: usize = (u32::from(stmt.range.end()) as usize).min(source.len());
    let raw = &source[start..end];
    let body_owned = crate::textutil::strip_noise_full(raw);
    let body = body_owned.as_str();
    let upper = body.to_ascii_uppercase();
    // Belt-and-suspenders: if a future backend ever exposes inner
    // PL/pgSQL stmts as `Select`, the context-aware checks below
    // still catch them.
    let before_upper = source[..start].to_ascii_uppercase();
    if inside_dollar_block(&before_upper) {
      return;
    }
    if before_upper.contains("LANGUAGE PLPGSQL") {
      return;
    }
    if before_upper.contains("LANGUAGE 'PLPGSQL'") {
      return;
    }
    if before_upper.contains("DO $$") {
      return;
    }
    if unmatched_begin(&before_upper) {
      return;
    }
    // Stmt must contain `SELECT ... INTO <target> ... FROM`.
    let bytes = upper.as_bytes();
    let n = bytes.len();
    // Find SELECT keyword.
    let Some(sel) = upper.find("SELECT") else { return };
    if !is_keyword_at(&upper, bytes, sel, "SELECT") {
      return;
    }
    // After SELECT, projection ends at FROM. INTO must appear before FROM.
    let after_sel = sel + 6;
    let from_at = match upper[after_sel..].find(" FROM ") {
      Some(p) => after_sel + p + 1,
      None => return,
    };
    let into_at = match upper[after_sel..from_at].find(" INTO ") {
      Some(p) => after_sel + p + 1,
      None => return,
    };
    if !is_keyword_at(&upper, bytes, into_at, "INTO") {
      return;
    }
    let _ = n;
    let abs_start = start + into_at;
    let abs_end = start + into_at + 4;
    out.push(Diagnostic {
            code: "sql118",
            severity: Severity::Hint,
            message: "top-level `SELECT INTO` creates a new table -- inside PL/pgSQL it assigns variables, but at the top level it's DDL".into(),
            range: text_size::TextRange::new(
                (abs_start as u32).into(),
                (abs_end as u32).into(),
            ),
        });
  }
}

fn is_keyword_at(upper: &str, bytes: &[u8], i: usize, word: &str) -> bool {
  let w = word.len();
  if i + w > bytes.len() {
    return false;
  }
  if &upper[i..i + w] != word {
    return false;
  }
  let prev_ok = i == 0 || !is_word(bytes[i - 1] as char);
  let next_ok = i + w == bytes.len() || !is_word(bytes[i + w] as char);
  prev_ok && next_ok
}

fn is_word(c: char) -> bool {
  c.is_alphanumeric() || c == '_'
}

/// Count `BEGIN` minus `END` word tokens. Positive means we're inside
/// an unmatched BEGIN ... END block (PL/pgSQL function body or a
/// nested PL/pgSQL block) -- in which case `SELECT INTO` is variable
/// assignment, not DDL.
fn unmatched_begin(before: &str) -> bool {
  let b = before.as_bytes();
  let n = b.len();
  let mut begins: i32 = 0;
  let mut ends: i32 = 0;
  let mut i = 0;
  while i < n {
    if b[i] == b'\'' {
      i += 1;
      while i < n && b[i] != b'\'' {
        i += 1;
      }
      if i < n {
        i += 1;
      }
      continue;
    }
    if i + 2 <= n && &before[i..i + 2] == "--" {
      while i < n && b[i] != b'\n' {
        i += 1;
      }
      continue;
    }
    if i + 5 <= n && &before[i..i + 5] == "BEGIN" {
      let prev_ok = i == 0 || !is_word(b[i - 1] as char);
      let next_ok = i + 5 == n || !is_word(b[i + 5] as char);
      if prev_ok && next_ok {
        begins += 1;
        i += 5;
        continue;
      }
    }
    if i + 3 <= n && &before[i..i + 3] == "END" {
      let prev_ok = i == 0 || !is_word(b[i - 1] as char);
      let next_ok = i + 3 == n || !is_word(b[i + 3] as char);
      if prev_ok && next_ok {
        ends += 1;
        i += 3;
        continue;
      }
    }
    i += 1;
  }
  begins > ends
}

/// Cheap check: are we currently inside an open `$$ ... $$` block?
/// Counts opening / closing dollar tags in `before` and returns true
/// when there's an unmatched opener.
fn inside_dollar_block(before: &str) -> bool {
  let bytes = before.as_bytes();
  let n = bytes.len();
  let mut i = 0;
  let mut open = false;
  while i < n {
    if bytes[i] == b'$' {
      // Read tag like `$$` or `$tag$`.
      let tag_start = i;
      let mut j = i + 1;
      while j < n && (bytes[j].is_ascii_alphanumeric() || bytes[j] == b'_') {
        j += 1;
      }
      if j < n && bytes[j] == b'$' {
        let _tag = &before[tag_start..=j];
        open = !open;
        i = j + 1;
        continue;
      }
    }
    i += 1;
  }
  open
}
