//! sql495: `WHERE col = ALL(<array-literal>)` -- `= ALL` requires
//! col to equal *every* element. With 2+ literal elements that
//! aren't all identical, the predicate is always FALSE. With all
//! identical elements, it's equivalent to a single `col = <elem>`.
//! Almost always the author meant `= ANY` (i.e. IN).

use crate::clause_scan::{find_clause, find_clause_end, is_word};
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;
use text_size::TextRange;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql495"
  }
  fn default_severity(&self) -> Severity {
    Severity::Warning
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, raw) = crate::stmt_body(stmt, source);
    let raw_bytes = raw.as_bytes();
    let cleaned = crate::textutil::strip_noise_full(raw);
    let upper = cleaned.to_ascii_uppercase();
    let ub = upper.as_bytes();
    let bytes = cleaned.as_bytes();
    let stopwords = ["GROUP BY", "ORDER BY", "HAVING", "LIMIT", "OFFSET", "FOR", "FETCH", "WINDOW", "RETURNING", "UNION", "INTERSECT", "EXCEPT"];
    let Some(rel_where) = find_clause(ub, b"WHERE") else {
      return;
    };
    let pred_start = rel_where + 5;
    let pred_end = find_clause_end(ub, pred_start, &stopwords).min(ub.len());
    let mut emitted: std::collections::HashSet<usize> = std::collections::HashSet::new();
    let mut i = pred_start;
    while i + 3 <= pred_end {
      // Look for `= ALL` (with whitespace between).
      if bytes[i] != b'=' {
        i += 1;
        continue;
      }
      let eq_at = i;
      let mut k = i + 1;
      while k < pred_end && bytes[k].is_ascii_whitespace() {
        k += 1;
      }
      if k + 3 > pred_end || &ub[k..k + 3] != b"ALL" || (k + 3 < pred_end && is_word(ub[k + 3] as char)) {
        i += 1;
        continue;
      }
      let all_end = k + 3;
      let mut p = all_end;
      while p < pred_end && bytes[p].is_ascii_whitespace() {
        p += 1;
      }
      if p >= pred_end || bytes[p] != b'(' {
        i += 1;
        continue;
      }
      let Some(close) = match_paren(bytes, p, pred_end) else {
        i += 1;
        continue;
      };
      let inner_start = p + 1;
      let inner_end = close;
      // Skip subqueries (SELECT/VALUES/WITH).
      let inner_trim = cleaned[inner_start..inner_end].trim_start().to_ascii_uppercase();
      if inner_trim.starts_with("SELECT") || inner_trim.starts_with("VALUES") || inner_trim.starts_with("WITH") {
        i = close + 1;
        continue;
      }
      // Extract array literal elements: support ARRAY[a, b, ...] and
      // '{a,b,...}'::type[].
      let Some(elems) = extract_array_elems(raw, inner_start, inner_end, raw_bytes) else {
        i = close + 1;
        continue;
      };
      if elems.len() < 2 {
        i = close + 1;
        continue;
      }
      let all_same = elems.iter().all(|e| e == &elems[0]);
      if emitted.insert(eq_at) {
        let abs_s = start + eq_at;
        let abs_e = start + close + 1;
        let (severity, message) = if all_same {
          (
            Severity::Hint,
            format!(
              "`= ALL(<array>)` with {} identical elements is equivalent to `col = {}` -- drop the wrapper.",
              elems.len(),
              elems[0]
            ),
          )
        } else {
          (
            Severity::Warning,
            "`= ALL(<array>)` with distinct elements is always FALSE -- a column cannot equal multiple distinct values at once. Did you mean `= ANY(...)` (IN)?".into(),
          )
        };
        out.push(Diagnostic {
          code: "sql495",
          severity,
          message,
          range: TextRange::new((abs_s as u32).into(), (abs_e as u32).into()),
        });
      }
      i = close + 1;
    }
  }
}

/// Parse `ARRAY[a, b, c]` or `'{a,b,c}'::type[]` from the inner of
/// the ALL() call. Returns the trimmed string of each element, or
/// None if the form isn't a recognized literal array.
fn extract_array_elems(raw: &str, inner_start: usize, inner_end: usize, raw_bytes: &[u8]) -> Option<Vec<String>> {
  let inner = raw[inner_start..inner_end].trim();
  // Strip trailing cast like `::int[]` for both forms.
  let inner_nocast = strip_trailing_cast(inner);
  // ARRAY[...] form
  let upper = inner_nocast.to_ascii_uppercase();
  if let Some(after) = upper.strip_prefix("ARRAY[")
    && let Some(close_off) = after.rfind(']')
  {
    // Compute the original (un-uppercased) slice for the same range.
    let start_off = "ARRAY[".len();
    let body = &inner_nocast[start_off..(start_off + close_off)];
    return Some(split_top_commas(body).iter().map(|s| s.trim().to_string()).collect());
  }
  // '{a,b,c}' form (PG array literal as string)
  if let Some(stripped) = inner_nocast.strip_prefix('\'')
    && let Some(s) = stripped.strip_suffix('\'')
    && s.starts_with('{')
    && s.ends_with('}')
    && s.len() >= 2
  {
    let body = &s[1..s.len() - 1];
    return Some(body.split(',').map(|x| x.trim().to_string()).collect());
  }
  let _ = (raw_bytes, raw);
  None
}

fn strip_trailing_cast(s: &str) -> &str {
  // Strip `::<word/bracket chars>+` at the end.
  let bytes = s.as_bytes();
  let n = bytes.len();
  if n < 3 {
    return s;
  }
  let mut k = n;
  while k > 0 {
    let c = bytes[k - 1];
    if c.is_ascii_alphanumeric() || c == b'_' || c == b'[' || c == b']' {
      k -= 1;
    } else {
      break;
    }
  }
  if k >= 2 && &bytes[k - 2..k] == b"::" {
    return s[..k - 2].trim_end();
  }
  s
}

fn split_top_commas(s: &str) -> Vec<&str> {
  let mut out = Vec::new();
  let bytes = s.as_bytes();
  let n = bytes.len();
  let mut depth: i32 = 0;
  let mut start = 0usize;
  let mut i = 0usize;
  while i < n {
    let c = bytes[i];
    if c == b'\'' {
      i += 1;
      while i < n && bytes[i] != b'\'' {
        i += 1;
      }
      i = (i + 1).min(n);
      continue;
    }
    if c == b'(' || c == b'[' {
      depth += 1;
    } else if c == b')' || c == b']' {
      depth -= 1;
    } else if c == b',' && depth == 0 {
      out.push(&s[start..i]);
      start = i + 1;
    }
    i += 1;
  }
  out.push(&s[start..n]);
  out
}

fn match_paren(bytes: &[u8], open: usize, end: usize) -> Option<usize> {
  let mut depth: i32 = 0;
  let mut i = open;
  while i < end {
    let c = bytes[i];
    if c == b'\'' {
      i += 1;
      while i < end && bytes[i] != b'\'' {
        i += 1;
      }
      i = (i + 1).min(end);
      continue;
    }
    if c == b'(' {
      depth += 1;
    } else if c == b')' {
      depth -= 1;
      if depth == 0 {
        return Some(i);
      }
    }
    i += 1;
  }
  None
}
