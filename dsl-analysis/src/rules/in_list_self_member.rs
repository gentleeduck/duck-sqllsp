//! sql414: `WHERE col IN (col, ...)` or `... col NOT IN (col, ...)` --
//! the column appears in its own IN list. For non-NULL rows the
//! membership is unconditionally true (or unconditionally false in
//! the NOT IN form), so the predicate collapses. Almost always a
//! typo for a different column or a literal.

use crate::clause_scan::{find_clause, find_clause_end, is_word};
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;
use text_size::TextRange;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql414"
  }
  fn default_severity(&self) -> Severity {
    Severity::Warning
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, raw) = crate::stmt_body(stmt, source);
    let cleaned = crate::textutil::strip_noise_full(raw);
    let upper = cleaned.to_ascii_uppercase();
    let bytes_u = upper.as_bytes();
    let bytes = cleaned.as_bytes();
    let stopwords = ["GROUP BY", "ORDER BY", "LIMIT", "OFFSET", "HAVING", "FOR", "FETCH", "WINDOW", "RETURNING", "UNION", "INTERSECT", "EXCEPT"];

    for needle in [&b"WHERE"[..], &b"ON"[..]] {
      let mut from = 0usize;
      while let Some(rel) = find_clause(&bytes_u[from..], needle).map(|p| p + from) {
        let pred_start = rel + needle.len();
        let pred_end = find_clause_end(bytes_u, pred_start, &stopwords);
        scan_in(bytes, bytes_u, pred_start, pred_end, start, out);
        from = pred_end.max(rel + needle.len());
      }
    }
  }
}

fn scan_in(bytes: &[u8], upper: &[u8], from: usize, to: usize, abs_off: usize, out: &mut Vec<Diagnostic>) {
  let needle = b"IN";
  let mut i = from;
  while i + needle.len() <= to {
    if bytes[i] == b'\'' {
      i += 1;
      while i < to && bytes[i] != b'\'' {
        i += 1;
      }
      i = (i + 1).min(to);
      continue;
    }
    let word_match = upper[i..i + needle.len()] == *needle
      && (i == 0 || !is_word(upper[i - 1] as char))
      && (i + needle.len() == upper.len() || !is_word(upper[i + needle.len()] as char));
    if !word_match {
      i += 1;
      continue;
    }
    // Read LHS identifier (skip whitespace going back; if NOT, skip
    // one more word so `col NOT IN` lands on `col`).
    let mut x_end = i;
    while x_end > from && bytes[x_end - 1].is_ascii_whitespace() {
      x_end -= 1;
    }
    let mut x_read = read_ident_backward(bytes, from, x_end);
    if let Some((xs, _xe, ref t)) = x_read
      && t.eq_ignore_ascii_case("NOT")
    {
      let mut probe_end = xs;
      while probe_end > from && bytes[probe_end - 1].is_ascii_whitespace() {
        probe_end -= 1;
      }
      x_read = read_ident_backward(bytes, from, probe_end);
    }
    let Some((_xs, _xe, x_text)) = x_read else {
      i += needle.len();
      continue;
    };
    // Numeric LHS isn't a column; skip.
    if x_text.chars().all(|c| c.is_ascii_digit() || c == '.') {
      i += needle.len();
      continue;
    }
    // Find the `(` following IN.
    let mut after = i + needle.len();
    while after < to && bytes[after].is_ascii_whitespace() {
      after += 1;
    }
    if after >= to || bytes[after] != b'(' {
      i = after.max(i + needle.len());
      continue;
    }
    // Walk the paren body collecting comma-separated items at depth 0.
    let mut depth: i32 = 1;
    let mut item_start = after + 1;
    let mut j = after + 1;
    let mut found = false;
    let mut hit_end = j;
    while j < to && depth > 0 {
      match bytes[j] {
        b'\'' => {
          j += 1;
          while j < to && bytes[j] != b'\'' {
            j += 1;
          }
          j = (j + 1).min(to);
          continue;
        },
        b'(' => depth += 1,
        b')' => {
          depth -= 1;
          if depth == 0 {
            // last item
            let item = std::str::from_utf8(&bytes[item_start..j]).unwrap_or("").trim();
            if item.eq_ignore_ascii_case(&x_text) {
              found = true;
            }
            hit_end = j + 1;
            break;
          }
        },
        b',' if depth == 1 => {
          let item = std::str::from_utf8(&bytes[item_start..j]).unwrap_or("").trim();
          if item.eq_ignore_ascii_case(&x_text) {
            found = true;
          }
          item_start = j + 1;
        },
        _ => {},
      }
      j += 1;
    }
    if found {
      let abs_s = abs_off + i;
      let abs_e = abs_off + hit_end;
      out.push(Diagnostic {
        code: "sql414",
        severity: Severity::Warning,
        message: format!(
          "`{x_text}` appears in its own IN-list -- the membership is unconditionally true for non-NULL rows; likely a typo"
        ),
        range: TextRange::new((abs_s as u32).into(), (abs_e as u32).into()),
      });
    }
    i = hit_end.max(i + needle.len());
  }
}

fn read_ident_backward(bytes: &[u8], lower: usize, end: usize) -> Option<(usize, usize, String)> {
  if end <= lower || !is_ident_byte(bytes[end - 1]) {
    return None;
  }
  let mut start = end;
  while start > lower {
    let b = bytes[start - 1];
    if is_ident_byte(b) || b == b'.' {
      start -= 1;
    } else {
      break;
    }
  }
  let text = std::str::from_utf8(&bytes[start..end]).ok()?.to_string();
  if text.is_empty() || text.starts_with('.') || text.ends_with('.') {
    return None;
  }
  Some((start, end, text))
}

fn is_ident_byte(b: u8) -> bool {
  is_word(b as char)
}
