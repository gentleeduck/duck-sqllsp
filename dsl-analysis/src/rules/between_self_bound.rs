//! sql409: `WHERE col BETWEEN col AND ...` or `WHERE col BETWEEN ...
//! AND col` -- one of the bounds is the same column being tested, so
//! the predicate collapses. `col BETWEEN col AND high` is equivalent
//! to `col <= high`; `col BETWEEN low AND col` is equivalent to
//! `col >= low`. Almost always a typo for two real bounds.

use crate::clause_scan::{find_clause, find_clause_end, is_word};
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;
use text_size::TextRange;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql409"
  }
  fn default_severity(&self) -> Severity {
    Severity::Warning
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let start: usize = u32::from(stmt.range.start()) as usize;
    let end: usize = (u32::from(stmt.range.end()) as usize).min(source.len());
    let raw = &source[start..end];
    let cleaned = crate::textutil::strip_noise_full(raw);
    let upper = cleaned.to_ascii_uppercase();
    let bytes_u = upper.as_bytes();
    let bytes = cleaned.as_bytes();
    let stopwords = ["GROUP BY", "ORDER BY", "LIMIT", "OFFSET", "HAVING", "FOR", "FETCH", "WINDOW", "RETURNING", "UNION", "INTERSECT", "EXCEPT"];

    // For each WHERE / ON clause: scan its body for BETWEEN occurrences.
    for needle in [&b"WHERE"[..], &b"ON"[..]] {
      let mut from = 0usize;
      while let Some(rel) = find_clause(&bytes_u[from..], needle).map(|p| p + from) {
        let pred_start = rel + needle.len();
        let pred_end = find_clause_end(bytes_u, pred_start, &stopwords);
        scan_between(bytes, bytes_u, pred_start, pred_end, start, out);
        from = pred_end.max(rel + needle.len());
      }
    }
  }
}

fn scan_between(bytes: &[u8], upper_bytes: &[u8], from: usize, to: usize, abs_off: usize, out: &mut Vec<Diagnostic>) {
  let needle = b"BETWEEN";
  let mut i = from;
  while i + needle.len() <= to {
    // Skip strings.
    if bytes[i] == b'\'' {
      i += 1;
      while i < to && bytes[i] != b'\'' {
        i += 1;
      }
      i = (i + 1).min(to);
      continue;
    }
    if upper_bytes[i..i + needle.len()] != *needle
      || (i > 0 && is_word(upper_bytes[i - 1] as char))
      || (i + needle.len() < upper_bytes.len() && is_word(upper_bytes[i + needle.len()] as char))
    {
      i += 1;
      continue;
    }
    // X is the ident immediately before BETWEEN. NOT BETWEEN: read
    // backwards skipping past the bare `NOT` keyword to land on the
    // real column ident.
    let mut x_end = i;
    while x_end > from && bytes[x_end - 1].is_ascii_whitespace() {
      x_end -= 1;
    }
    let mut x_read = read_ident_backward(bytes, from, x_end);
    let mut is_not = false;
    if let Some((xs, _xe, ref t)) = x_read
      && t.eq_ignore_ascii_case("NOT")
    {
      is_not = true;
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
    // Skip optional NOT (`X NOT BETWEEN ...` -- still want to flag).
    let mut after = i + needle.len();
    while after < to && bytes[after].is_ascii_whitespace() {
      after += 1;
    }
    // Y is the ident after BETWEEN.
    let Some((_ys, ye, y_text)) = read_ident_forward(bytes, after, to) else {
      i = after.max(i + needle.len());
      continue;
    };
    // Find AND at depth 0 between Y and the rest. Skip first scan past
    // arithmetic / function args; simplest: find word-bounded AND in
    // the remaining body.
    let mut and_search = ye;
    let mut depth = 0i32;
    let mut and_at: Option<usize> = None;
    while and_search + 3 <= to {
      let c = bytes[and_search];
      if c == b'(' {
        depth += 1;
      } else if c == b')' {
        depth -= 1;
      } else if depth == 0
        && (and_search == ye || !is_word(upper_bytes[and_search - 1] as char))
        && and_search + 3 <= upper_bytes.len()
        && &upper_bytes[and_search..and_search + 3] == b"AND"
        && (and_search + 3 == upper_bytes.len() || !is_word(upper_bytes[and_search + 3] as char))
      {
        and_at = Some(and_search);
        break;
      }
      and_search += 1;
    }
    let Some(and_at) = and_at else {
      i = ye;
      continue;
    };
    // Z is the ident after AND.
    let mut z_start = and_at + 3;
    while z_start < to && bytes[z_start].is_ascii_whitespace() {
      z_start += 1;
    }
    let z_opt = read_ident_forward(bytes, z_start, to);
    let mut hit_side: Option<&'static str> = None;
    if x_text.eq_ignore_ascii_case(&y_text) {
      hit_side = Some("low");
    } else if let Some((_zs, _ze, z_text)) = &z_opt
      && x_text.eq_ignore_ascii_case(z_text)
    {
      hit_side = Some("high");
    }
    if let Some(side) = hit_side {
      let abs_s = abs_off + i;
      let z_end_pos = z_opt.as_ref().map(|(_, e, _)| *e).unwrap_or(and_at + 3);
      let abs_e = abs_off + z_end_pos;
      // Flip the implication for NOT BETWEEN:
      //   col NOT BETWEEN col AND high == col > high
      //   col NOT BETWEEN low AND col == col < low
      let detail = match (side, is_not) {
        ("low", false) => format!("low bound is `{x_text}` -- equivalent to `{x_text} <= <high>`"),
        ("high", false) => format!("high bound is `{x_text}` -- equivalent to `{x_text} >= <low>`"),
        ("low", true) => format!("low bound is `{x_text}` -- with NOT BETWEEN this is equivalent to `{x_text} > <high>`"),
        ("high", true) => format!("high bound is `{x_text}` -- with NOT BETWEEN this is equivalent to `{x_text} < <low>`"),
        _ => unreachable!(),
      };
      let kw = if is_not { "NOT BETWEEN" } else { "BETWEEN" };
      out.push(Diagnostic {
        code: "sql409",
        severity: Severity::Warning,
        message: format!("`{x_text} {kw} ...` uses the same column as a bound; {detail}"),
        range: TextRange::new((abs_s as u32).into(), (abs_e as u32).into()),
      });
    }
    i = z_opt.map(|(_, e, _)| e).unwrap_or(and_at + 3);
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

fn read_ident_forward(bytes: &[u8], start: usize, upper: usize) -> Option<(usize, usize, String)> {
  if start >= upper || !is_ident_byte(bytes[start]) {
    return None;
  }
  let mut end = start;
  while end < upper {
    let b = bytes[end];
    if is_ident_byte(b) || b == b'.' {
      end += 1;
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
