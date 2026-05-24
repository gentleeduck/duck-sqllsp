//! sql030: trigger function body has no RETURN.
//!
//! Any `CREATE FUNCTION ... RETURNS TRIGGER` must end every reachable
//! control-flow path with `RETURN NEW;`, `RETURN OLD;`, or `RETURN NULL;`.
//! Without it Postgres fires "control reached end of trigger procedure
//! without RETURN" at runtime.
//!
//! v1 is a text-level approximation: we treat the function as buggy
//! when the body contains no `RETURN ` keyword at all. Branch-aware
//! analysis (every IF/ELSE arm has a RETURN) comes in a follow-up.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::{Statement, StatementKind};
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql030"
  }
  fn default_severity(&self) -> Severity {
    Severity::Error
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    // Unknown / non-DDL statements only carry the function as text
    // inside `text` -- the parser hasn't typed it. Scan the source
    // bytes of this statement directly so we don't depend on a
    // structured CreateFunction node.
    if !matches!(stmt.kind, StatementKind::Unknown { .. }) {
      return;
    }
    let start: usize = u32::from(stmt.range.start()) as usize;
    let end: usize = (u32::from(stmt.range.end()) as usize).min(source.len());
    let body = &source[start..end];
    let upper = body.to_ascii_uppercase();

    if !upper.contains("CREATE") {
      return;
    }
    if !upper.contains("FUNCTION") && !upper.contains("PROCEDURE") {
      return;
    }
    if !upper.contains("RETURNS TRIGGER") {
      return;
    }
    // Find the function body inside `$$ ... $$` and check for RETURN.
    let body_text = match dollar_body(body) {
      Some(b) => b,
      None => return, // No body found (e.g. AS 'string' form)
    };
    let body_upper = body_text.to_ascii_uppercase();
    // Strip line + block comments before scanning so `-- RETURN` doesn't
    // count.
    let stripped = strip_comments(&body_upper);
    if has_return(&stripped) {
      return;
    }

    // Point at the `BEGIN` keyword inside the body so the squiggle
    // lands on the block that should have returned. Fall back to
    // the `$$` opener, then the whole stmt.
    let body_offset = source.find(body_text).unwrap_or(start);
    let range = if let Some(begin_pos) = find_word(&body_upper, "BEGIN") {
      let abs_start = body_offset + begin_pos;
      let abs_end = abs_start + 5;
      text_size::TextRange::new((abs_start as u32).into(), (abs_end as u32).into())
    } else {
      stmt.range
    };
    out.push(Diagnostic {
      code: "sql030",
      severity: Severity::Error,
      message: "trigger function has no RETURN -- add `RETURN NEW;`, `RETURN OLD;`, or `RETURN NULL;` before the END"
        .into(),
      range,
    });
  }
}

fn find_word(haystack: &str, needle: &str) -> Option<usize> {
  let bytes = haystack.as_bytes();
  let nb = needle.as_bytes();
  let mut i = 0;
  while i + nb.len() <= bytes.len() {
    if &bytes[i..i + nb.len()] == nb {
      let prev_ok = i == 0 || !is_word(bytes[i - 1] as char);
      let next_ok = i + nb.len() == bytes.len() || !is_word(bytes[i + nb.len()] as char);
      if prev_ok && next_ok {
        return Some(i);
      }
    }
    i += 1;
  }
  None
}

fn dollar_body(text: &str) -> Option<&str> {
  let start = text.find("$$")?;
  let after = start + 2;
  let end_rel = text[after..].find("$$")?;
  Some(&text[after..after + end_rel])
}

fn strip_comments(s: &str) -> String {
  let mut out = String::with_capacity(s.len());
  let bytes = s.as_bytes();
  let mut i = 0;
  let n = bytes.len();
  while i < n {
    if i + 1 < n && bytes[i] == b'-' && bytes[i + 1] == b'-' {
      while i < n && bytes[i] != b'\n' {
        i += 1;
      }
    } else if i + 1 < n && bytes[i] == b'/' && bytes[i + 1] == b'*' {
      i += 2;
      while i + 1 < n && !(bytes[i] == b'*' && bytes[i + 1] == b'/') {
        i += 1;
      }
      i = (i + 2).min(n);
    } else {
      out.push(bytes[i] as char);
      i += 1;
    }
  }
  out
}

fn has_return(upper: &str) -> bool {
  // Whole-word RETURN match.
  let needle = "RETURN";
  let bytes = upper.as_bytes();
  let n = bytes.len();
  let mut i = 0;
  while i + needle.len() <= n {
    if &upper[i..i + needle.len()] == needle {
      let prev_ok = i == 0 || !is_word(bytes[i - 1] as char);
      let next_ok = i + needle.len() == n || !is_word(bytes[i + needle.len()] as char);
      if prev_ok && next_ok {
        return true;
      }
    }
    i += 1;
  }
  false
}

fn is_word(c: char) -> bool {
  c.is_alphanumeric() || c == '_'
}
