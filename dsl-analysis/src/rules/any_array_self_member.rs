//! sql420: `WHERE col = ANY(ARRAY[col, ...])` -- the column appears
//! in its own ANY-array, which (like sql414's IN-list) makes the
//! membership unconditionally true for non-NULL rows. Same for the
//! `ALL` variant which becomes tautologically true only when every
//! other entry equals the column too. Likely a typo.

use crate::clause_scan::{find_clause, find_clause_end, is_word};
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;
use text_size::TextRange;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql420"
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
        scan_any_all(bytes, bytes_u, pred_start, pred_end, start, out);
        from = pred_end.max(rel + needle.len());
      }
    }
  }
}

fn scan_any_all(bytes: &[u8], upper: &[u8], from: usize, to: usize, abs_off: usize, out: &mut Vec<Diagnostic>) {
  let mut i = from;
  while i + 4 <= to {
    // Look for word-bounded ANY or ALL followed by `(ARRAY[`.
    let is_any = upper[i..i + 3] == *b"ANY";
    let is_all = upper[i..i + 3] == *b"ALL";
    if !(is_any || is_all)
      || (i > 0 && is_word(upper[i - 1] as char))
      || (i + 3 < upper.len() && is_word(upper[i + 3] as char))
    {
      i += 1;
      continue;
    }
    let kw_name = if is_any { "ANY" } else { "ALL" };
    let kw_end = i + 3;
    // Skip whitespace; expect `(`.
    let mut j = kw_end;
    while j < to && bytes[j].is_ascii_whitespace() {
      j += 1;
    }
    if j >= to || bytes[j] != b'(' {
      i = kw_end;
      continue;
    }
    // Inside: optional whitespace + ARRAY[...] (or other expr -- skip).
    let mut k = j + 1;
    while k < to && bytes[k].is_ascii_whitespace() {
      k += 1;
    }
    if k + 5 > to || &upper[k..k + 5] != b"ARRAY" {
      i = kw_end;
      continue;
    }
    let mut m = k + 5;
    while m < to && bytes[m].is_ascii_whitespace() {
      m += 1;
    }
    if m >= to || bytes[m] != b'[' {
      i = kw_end;
      continue;
    }
    // Walk the bracket body.
    let mut depth: i32 = 1;
    let mut item_start = m + 1;
    let mut p = m + 1;
    let mut bracket_end = p;
    let mut items: Vec<(usize, usize)> = Vec::new();
    while p < to && depth > 0 {
      match bytes[p] {
        b'\'' => {
          p += 1;
          while p < to && bytes[p] != b'\'' {
            p += 1;
          }
          p = (p + 1).min(to);
          continue;
        },
        b'[' => depth += 1,
        b']' => {
          depth -= 1;
          if depth == 0 {
            items.push((item_start, p));
            bracket_end = p + 1;
            break;
          }
        },
        b',' if depth == 1 => {
          items.push((item_start, p));
          item_start = p + 1;
        },
        _ => {},
      }
      p += 1;
    }
    // Read LHS identifier preceding the operator preceding ANY/ALL.
    // Pattern: `<X> <OP> ANY|ALL ( ARRAY[...] )` where OP is = / <> /
    // != / < / > / <= / >=.
    let mut lhs_end = i;
    while lhs_end > from && bytes[lhs_end - 1].is_ascii_whitespace() {
      lhs_end -= 1;
    }
    // Skip the operator (1 or 2 bytes).
    let mut op_end = lhs_end;
    if op_end >= 2 {
      let pair = &bytes[op_end - 2..op_end];
      if matches!(pair, b"<>" | b"!=" | b"<=" | b">=") {
        op_end -= 2;
      } else if matches!(bytes[op_end - 1], b'=' | b'<' | b'>') {
        op_end -= 1;
      } else {
        i = kw_end;
        continue;
      }
    } else if op_end >= 1 && matches!(bytes[op_end - 1], b'=' | b'<' | b'>') {
      op_end -= 1;
    } else {
      i = kw_end;
      continue;
    }
    while op_end > from && bytes[op_end - 1].is_ascii_whitespace() {
      op_end -= 1;
    }
    let Some((_xs, _xe, x_text)) = read_ident_backward(bytes, from, op_end) else {
      i = kw_end;
      continue;
    };
    if x_text.chars().all(|c| c.is_ascii_digit() || c == '.') {
      i = kw_end;
      continue;
    }
    // Check items for an identical entry.
    let mut found = false;
    for (s, e) in &items {
      let item = std::str::from_utf8(&bytes[*s..*e]).unwrap_or("").trim();
      if item.eq_ignore_ascii_case(&x_text) {
        found = true;
        break;
      }
    }
    if found {
      let abs_s = abs_off + i;
      let abs_e = abs_off + bracket_end;
      out.push(Diagnostic {
        code: "sql420",
        severity: Severity::Warning,
        message: format!(
          "`{x_text}` appears in its own {kw_name} ARRAY -- the membership is unconditionally true (or trivially true under ALL) for non-NULL rows; likely a typo"
        ),
        range: TextRange::new((abs_s as u32).into(), (abs_e as u32).into()),
      });
    }
    i = bracket_end.max(kw_end);
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
