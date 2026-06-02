//! sql427: `WHERE date(ts) = '2024-01-01'` / `WHERE ts::date = ...`
//! / `WHERE CAST(ts AS date) = ...` -- wrapping a column in a
//! function call or cast prevents the btree index on that column
//! from being used. Use a range predicate or build a functional
//! index instead.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;
use text_size::TextRange;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql427"
  }
  fn default_severity(&self) -> Severity {
    Severity::Hint
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, body, upper) = crate::stmt_body_upper(stmt, source);
    if !upper.contains("WHERE") {
      return;
    }
    let bytes = body.as_bytes();

    // `date(col) = '...'` -- single-arg wrapper, column inside.
    let mut from = 0usize;
    while let Some(rel) = upper[from..].find("DATE(") {
      let at = from + rel;
      if at > 0 {
        let prev = bytes[at - 1] as char;
        if prev.is_ascii_alphanumeric() || prev == '_' {
          from = at + 5;
          continue;
        }
      }
      let open = at + 4;
      let Some(close) = match_paren(bytes, open) else {
        break;
      };
      let inner = body[open + 1..close].trim();
      let post = body[close + 1..].trim_start();
      if !post.starts_with('=') || !is_column_shape(inner) {
        from = close + 1;
        continue;
      }
      out.push(Diagnostic {
        code: "sql427",
        severity: Severity::Hint,
        message: "`date(col) = ...` wraps the column and blocks the btree index -- prefer a range `col >= 'YYYY-MM-DD' AND col < ...` or build an expression index on `date(col)`".into(),
        range: TextRange::new(((start + at) as u32).into(), ((start + close + 1) as u32).into()),
      });
      from = close + 1;
    }

    // Other common wrapping functions (lower/upper/trim/substring/...)
    // -- first arg is the column, fires when arg is column-shape and
    // `=` follows the close paren.
    const GENERIC_WRAPPERS: &[&str] = &["LOWER(", "UPPER(", "TRIM(", "BTRIM(", "LTRIM(", "RTRIM(", "SUBSTRING(", "SUBSTR(", "LEFT(", "RIGHT(", "ABS(", "ROUND(", "FLOOR(", "CEIL(", "CEILING("];
    for needle in GENERIC_WRAPPERS {
      let lower_kw = needle.trim_end_matches('(').to_ascii_lowercase();
      let mut from = 0usize;
      while let Some(rel) = upper[from..].find(needle) {
        let at = from + rel;
        if at > 0 {
          let prev = bytes[at - 1] as char;
          if prev.is_ascii_alphanumeric() || prev == '_' {
            from = at + needle.len();
            continue;
          }
        }
        let open = at + needle.len() - 1;
        let Some(close) = match_paren(bytes, open) else {
          break;
        };
        let inner = &body[open + 1..close];
        // First top-level arg.
        let first = top_level_first_arg(inner).trim();
        let post = body[close + 1..].trim_start();
        if !post.starts_with('=') || !is_column_shape(first) {
          from = close + 1;
          continue;
        }
        out.push(Diagnostic {
          code: "sql427",
          severity: Severity::Hint,
          message: format!(
            "`{lower_kw}({first}...) = ...` wraps the column and blocks the btree index on `{first}` -- consider an expression index on `{lower_kw}({first})` or compare the unwrapped column"
          ),
          range: TextRange::new(((start + at) as u32).into(), ((start + close + 1) as u32).into()),
        });
        from = close + 1;
      }
    }

    // `CAST(col AS TYPE) = ...` -- function-form cast.
    let mut from = 0usize;
    while let Some(rel) = upper[from..].find("CAST(") {
      let at = from + rel;
      if at > 0 {
        let prev = bytes[at - 1] as char;
        if prev.is_ascii_alphanumeric() || prev == '_' {
          from = at + 5;
          continue;
        }
      }
      let open = at + 4;
      let Some(close) = match_paren(bytes, open) else {
        break;
      };
      // Body is `<expr> AS <type>` (case-insensitive). Pull the
      // pre-AS column ident.
      let inner = &body[open + 1..close];
      let inner_upper = inner.to_ascii_uppercase();
      let Some(as_at) = inner_upper.find(" AS ") else {
        from = close + 1;
        continue;
      };
      let col_text = inner[..as_at].trim();
      let post = body[close + 1..].trim_start();
      if !post.starts_with('=') || !is_column_shape(col_text) {
        from = close + 1;
        continue;
      }
      out.push(Diagnostic {
        code: "sql427",
        severity: Severity::Hint,
        message: format!(
          "`CAST({col_text} AS ...) = ...` wraps the column in a cast and blocks any btree index on `{col_text}` -- compare without the cast (literal on right side gets cast) or build an expression index"
        ),
        range: TextRange::new(((start + at) as u32).into(), ((start + close + 1) as u32).into()),
      });
      from = close + 1;
    }

    // `col::TYPE = ...` -- cast operator. Walk for `::` and check
    // surrounding shape.
    let mut i = 0usize;
    while i + 1 < bytes.len() {
      if bytes[i] != b':' || bytes[i + 1] != b':' {
        i += 1;
        continue;
      }
      // Read column ident to the left.
      let mut left_end = i;
      while left_end > 0 && bytes[left_end - 1].is_ascii_whitespace() {
        left_end -= 1;
      }
      let mut left_start = left_end;
      while left_start > 0 {
        let b = bytes[left_start - 1];
        if b.is_ascii_alphanumeric() || b == b'_' || b == b'.' {
          left_start -= 1;
        } else {
          break;
        }
      }
      let col = &body[left_start..left_end];
      if !is_column_shape(col) {
        i += 2;
        continue;
      }
      // Read type to the right (single word).
      let mut j = i + 2;
      while j < bytes.len() && bytes[j].is_ascii_whitespace() {
        j += 1;
      }
      let type_start = j;
      while j < bytes.len() && (bytes[j].is_ascii_alphanumeric() || bytes[j] == b'_') {
        j += 1;
      }
      if type_start == j {
        i += 2;
        continue;
      }
      // Look ahead for `=` (or comparison op).
      let mut k = j;
      while k < bytes.len() && bytes[k].is_ascii_whitespace() {
        k += 1;
      }
      if k >= bytes.len() || bytes[k] != b'=' {
        i = j;
        continue;
      }
      // Skip when this cast is on the right side of `=` (col = literal::type
      // is legitimate). The cast applies to the LHS only if the previous
      // non-whitespace before col_start is not `=`.
      let mut prev_end = left_start;
      while prev_end > 0 && bytes[prev_end - 1].is_ascii_whitespace() {
        prev_end -= 1;
      }
      if prev_end > 0 && bytes[prev_end - 1] == b'=' {
        i = j;
        continue;
      }
      out.push(Diagnostic {
        code: "sql427",
        severity: Severity::Hint,
        message: format!(
          "`{col}::{} = ...` wraps the column in a cast and blocks any btree index on `{col}` -- compare without the cast (literal on right side gets cast) or build an expression index",
          std::str::from_utf8(&bytes[type_start..j]).unwrap_or("?")
        ),
        range: TextRange::new(((start + left_start) as u32).into(), ((start + j) as u32).into()),
      });
      i = j;
    }
  }
}

fn top_level_first_arg(s: &str) -> &str {
  let bytes = s.as_bytes();
  let mut depth = 0i32;
  let mut i = 0usize;
  while i < bytes.len() {
    match bytes[i] {
      b'(' => depth += 1,
      b')' => depth -= 1,
      b'\'' => {
        i += 1;
        while i < bytes.len() && bytes[i] != b'\'' {
          i += 1;
        }
      },
      b',' if depth == 0 => return &s[..i],
      _ => {},
    }
    i += 1;
  }
  s
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
      },
      b'\'' => {
        i += 1;
        while i < n && bytes[i] != b'\'' {
          i += 1;
        }
      },
      _ => {},
    }
    i += 1;
  }
  None
}

fn is_column_shape(s: &str) -> bool {
  let s = s.trim();
  if s.is_empty() {
    return false;
  }
  for c in s.chars() {
    if !(c.is_alphanumeric() || c == '_' || c == '.') {
      return false;
    }
  }
  let up = s.to_ascii_uppercase();
  let bare = up.rsplit('.').next().unwrap_or(&up);
  !matches!(bare, "CURRENT_DATE" | "CURRENT_TIME" | "CURRENT_TIMESTAMP" | "LOCALTIME" | "LOCALTIMESTAMP" | "NOW" | "NULL" | "TRUE" | "FALSE")
    && !bare.chars().all(|c| c.is_ascii_digit() || c == '.')
}
