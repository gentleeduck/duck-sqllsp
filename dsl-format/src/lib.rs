//! SQL formatting business logic for duck-sqllsp.
//!
//! Two layers cooperate:
//!
//!   1. [`external::run_sql_formatter`] -- shells out to the `sql-formatter`
//!      npm CLI (when present) for the bulk reflow / keyword casing /
//!      expression-width wrapping work that a dedicated grammar-aware tool
//!      already solves well.
//!   2. [`align::rewrite`] -- DataGrip-style CREATE TABLE / FUNCTION /
//!      TRIGGER / INDEX post-pass that aligns columns into padded sub-
//!      columns and breaks long clause-chained headers onto their own
//!      lines.
//!
//! The composite [`format`] entry runs both in order. Server handlers stay
//! thin shims that read the document, call this, build a TextEdit.

pub mod align;
pub mod external;
pub mod style;

pub use align::rewrite;
pub use external::run_sql_formatter;
pub use style::{CreateTableStyle, FormatterStyle};

/// One-shot format pipeline: external sql-formatter first (when available),
/// DataGrip-style alignment second. Falls back to passing the input through
/// unchanged when the external binary is missing -- callers can then
/// decide whether to skip emitting a no-op TextEdit.
pub fn format(input: &str, fmt_style: &FormatterStyle, ct_style: &CreateTableStyle) -> String {
  // Strip a leading UTF-8 BOM (`U+FEFF`). PG ignores it but tools that
  // preserve it leave invisible bytes in the file, which version control
  // and other formatters then fight over. BOMs inside string literals
  // or comments are preserved -- only the leading one is meaningful
  // metadata, and the convention is to drop it on format.
  let input = input.strip_prefix('\u{feff}').unwrap_or(input);
  let after_external = external::run_sql_formatter(input, fmt_style).unwrap_or_else(|| input.to_string());
  let after_align = align::rewrite(&after_external, ct_style);
  let after_tighten = tighten_call_parens(&after_align);
  let after_normalised = normalize_blank_lines(&after_tighten);
  if fmt_style.single_line { collapse_dml_lines(&after_normalised) } else { after_normalised }
}

/// Collapse each DML statement (SELECT / INSERT / UPDATE / DELETE / WITH)
/// onto a single line. Walks top-level statements, joining internal
/// whitespace runs into single spaces. Leaves CREATE TABLE / FUNCTION /
/// VIEW / TRIGGER / etc untouched so table layouts stay readable.
fn collapse_dml_lines(input: &str) -> String {
  let stmts = split_top_level_statements(input);
  let mut out = String::with_capacity(input.len());
  for stmt in stmts {
    let upper = stmt.trim_start().to_ascii_uppercase();
    let is_dml = upper.starts_with("SELECT")
      || upper.starts_with("INSERT")
      || upper.starts_with("UPDATE")
      || upper.starts_with("DELETE")
      || upper.starts_with("WITH")
      || upper.starts_with("VALUES");
    if is_dml {
      out.push_str(&collapse_whitespace(&stmt));
    } else {
      out.push_str(&stmt);
    }
    if !out.ends_with('\n') {
      out.push('\n');
    }
  }
  out
}

/// Split `src` at every top-level `;`, returning each statement (with
/// its trailing `;`). Honours single-quoted strings, line comments,
/// block comments, and `$$ ... $$` bodies so semicolons inside any of
/// those don't split.
fn split_top_level_statements(src: &str) -> Vec<String> {
  let bytes = src.as_bytes();
  let n = bytes.len();
  let mut out = Vec::new();
  let mut start = 0usize;
  let mut i = 0usize;
  while i < n {
    match bytes[i] {
      b'\'' => {
        i += 1;
        while i < n && bytes[i] != b'\'' {
          i += 1;
        }
        if i < n {
          i += 1;
        }
      },
      b'-' if i + 1 < n && bytes[i + 1] == b'-' => {
        while i < n && bytes[i] != b'\n' {
          i += 1;
        }
      },
      b'/' if i + 1 < n && bytes[i + 1] == b'*' => {
        i += 2;
        while i + 1 < n && !(bytes[i] == b'*' && bytes[i + 1] == b'/') {
          i += 1;
        }
        if i + 1 < n {
          i += 2;
        }
      },
      b'$' if i + 1 < n && bytes[i + 1] == b'$' => {
        i += 2;
        while i + 1 < n && !(bytes[i] == b'$' && bytes[i + 1] == b'$') {
          i += 1;
        }
        if i + 1 < n {
          i += 2;
        }
      },
      b';' => {
        i += 1;
        out.push(src[start..i].to_string());
        start = i;
      },
      _ => {
        i += 1;
      },
    }
  }
  if start < n {
    out.push(src[start..].to_string());
  }
  out
}

/// Replace every whitespace run inside `s` with a single space, but
/// preserve single-quoted strings + dollar-quoted bodies + line comments
/// (those would change semantics if collapsed). Also keep a single
/// leading newline so adjacent statements visually separate.
fn collapse_whitespace(s: &str) -> String {
  let bytes = s.as_bytes();
  let n = bytes.len();
  let leading_nl = bytes.iter().take_while(|b| **b == b'\n' || **b == b'\r').count();
  let leading = "\n".repeat(leading_nl.min(1));
  let mut out = String::with_capacity(s.len());
  out.push_str(&leading);
  let mut i = leading_nl;
  let mut prev_space = true; // suppress leading run
  while i < n {
    match bytes[i] {
      b'\'' => {
        out.push('\'');
        i += 1;
        while i < n && bytes[i] != b'\'' {
          out.push(bytes[i] as char);
          i += 1;
        }
        if i < n {
          out.push('\'');
          i += 1;
        }
        prev_space = false;
      },
      b'-' if i + 1 < n && bytes[i + 1] == b'-' => {
        // Line comment up to end of line. To keep the rest on one line,
        // convert to a block comment.
        let mut end = i + 2;
        while end < n && bytes[end] != b'\n' {
          end += 1;
        }
        let comment = &s[i + 2..end];
        out.push_str("/* ");
        out.push_str(comment.trim());
        out.push_str(" */");
        i = end;
        prev_space = false;
      },
      b'/' if i + 1 < n && bytes[i + 1] == b'*' => {
        let start = i;
        i += 2;
        while i + 1 < n && !(bytes[i] == b'*' && bytes[i + 1] == b'/') {
          i += 1;
        }
        if i + 1 < n {
          i += 2;
        }
        out.push_str(&s[start..i]);
        prev_space = false;
      },
      b'$' if i + 1 < n && bytes[i + 1] == b'$' => {
        let start = i;
        i += 2;
        while i + 1 < n && !(bytes[i] == b'$' && bytes[i + 1] == b'$') {
          i += 1;
        }
        if i + 1 < n {
          i += 2;
        }
        out.push_str(&s[start..i]);
        prev_space = false;
      },
      c if (c as char).is_whitespace() => {
        if !prev_space {
          out.push(' ');
          prev_space = true;
        }
        i += 1;
      },
      c => {
        out.push(c as char);
        i += 1;
        prev_space = false;
      },
    }
  }
  // Strip a trailing space before `;` to keep `SELECT 1;` not `SELECT 1 ;`.
  if out.ends_with(' ') {
    out.pop();
  }
  out
}

/// Drop the space between a function name and its opening `(`. PG's
/// canonical style is `length(x)`, not `length (x)`. Applies to both
/// declarations (`CREATE FUNCTION foo()`) and calls (`EXECUTE FUNCTION
/// set_updated_at()`, `SELECT length(x)`).
///
/// Conservative: only collapse when the identifier sits in a function
/// position. SQL keywords that introduce a grouping paren (`IN (...)`,
/// `EXISTS (...)`, `VALUES (...)`, `SELECT ...(...)`, etc.) are left
/// alone because their paren is not a call.
fn tighten_call_parens(input: &str) -> String {
  let bytes = input.as_bytes();
  let mut out = String::with_capacity(input.len());
  let mut i = 0usize;
  while i < bytes.len() {
    // Skip string literals + line comments + dollar-quoted bodies untouched.
    if bytes[i] == b'\'' {
      out.push('\'');
      i += 1;
      while i < bytes.len() && bytes[i] != b'\'' {
        i = push_one_char(&mut out, input, i);
      }
      if i < bytes.len() {
        out.push('\'');
        i += 1;
      }
      continue;
    }
    if i + 1 < bytes.len() && bytes[i] == b'-' && bytes[i + 1] == b'-' {
      while i < bytes.len() && bytes[i] != b'\n' {
        i = push_one_char(&mut out, input, i);
      }
      continue;
    }
    // Identifier?
    if bytes[i].is_ascii_alphabetic() || bytes[i] == b'_' {
      let id_start = i;
      while i < bytes.len() && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_') {
        i += 1;
      }
      let id = &input[id_start..i];
      // Look at whitespace + `(`.
      let mut j = i;
      let ws_start = j;
      while j < bytes.len() && (bytes[j] == b' ' || bytes[j] == b'\t') {
        j += 1;
      }
      if j > ws_start && j < bytes.len() && bytes[j] == b'(' {
        let upper = id.to_ascii_uppercase();
        if !KEEP_SPACE_BEFORE_PAREN.contains(&upper.as_str()) {
          out.push_str(id);
          // Skip the whitespace -- write nothing then continue from `(`.
          i = j;
          continue;
        }
      }
      out.push_str(id);
      continue;
    }
    i = push_one_char(&mut out, input, i);
  }
  out
}

/// Push the UTF-8 char starting at byte index `i` of `src` onto `out`
/// and return the new index past that char. Multi-byte aware -- using
/// `bytes[i] as char` here would reinterpret each UTF-8 continuation
/// byte as a Latin-1 codepoint, mangling every non-ASCII character.
pub(crate) fn push_one_char(out: &mut String, src: &str, i: usize) -> usize {
  let c = src[i..].chars().next().expect("caller guarantees i < src.len()");
  out.push(c);
  i + c.len_utf8()
}

/// SQL keywords whose following `(` is a grouping / sub-query / IN-list
/// paren rather than a function call. Keep the space for readability.
const KEEP_SPACE_BEFORE_PAREN: &[&str] = &[
  "SELECT",
  "IN",
  "NOT",
  "EXISTS",
  "VALUES",
  "RETURNING",
  "WHERE",
  "HAVING",
  "ON",
  "USING",
  "FROM",
  "INTO",
  "AS",
  "WITH",
  "CASE",
  "WHEN",
  "THEN",
  "ELSE",
  "ANY",
  "ALL",
  "SOME",
  "AND",
  "OR",
  "BY",
  "IS",
  "BETWEEN",
  "LIKE",
  "ILIKE",
  "SIMILAR",
  "OVERLAPS",
  "FILTER",
  "OVER",
  "PARTITION",
  "WITHIN",
  "PRECEDING",
  "FOLLOWING",
  "UNBOUNDED",
  "FOR",
  "ROW",
  "ROWS",
  "GROUPS",
  "RANGE",
  "DEFAULT",
  "REFERENCES",
  "CHECK",
  "UNIQUE",
  "PRIMARY",
  "FOREIGN",
  "KEY",
  "CONSTRAINT",
  "DISTINCT",
  "GROUP",
  "ORDER",
  "LIMIT",
  "OFFSET",
  "FETCH",
  "INTERSECT",
  "UNION",
  "EXCEPT",
  "DO",
  "LANGUAGE",
  "MATCH",
  "TO",
  "OF",
  "RESTRICT",
  "CASCADE",
];

#[cfg(test)]
mod tighten_tests {
  use super::*;

  #[test]
  fn collapses_function_call_space() {
    assert_eq!(tighten_call_parens("SELECT length (x);"), "SELECT length(x);");
  }

  #[test]
  fn collapses_execute_function() {
    let input = "CREATE TRIGGER t BEFORE UPDATE ON users EXECUTE FUNCTION set_updated_at ();";
    let output = tighten_call_parens(input);
    assert!(output.contains("set_updated_at()"), "got: {output}");
  }

  #[test]
  fn collapses_create_function_decl() {
    let input = "CREATE FUNCTION foo () RETURNS int AS $$ SELECT 1 $$ LANGUAGE sql;";
    let output = tighten_call_parens(input);
    assert!(output.contains("foo()"), "got: {output}");
  }

  #[test]
  fn keeps_space_after_select() {
    assert_eq!(tighten_call_parens("SELECT (1 + 2);"), "SELECT (1 + 2);");
  }

  #[test]
  fn keeps_space_after_in() {
    assert_eq!(tighten_call_parens("WHERE id IN (1, 2);"), "WHERE id IN (1, 2);");
  }

  #[test]
  fn leaves_string_literal_alone() {
    assert_eq!(tighten_call_parens("SELECT 'foo (bar)';"), "SELECT 'foo (bar)';");
  }
}

/// Collapse runs of >=2 blank lines down to a single blank line and
/// strip trailing blank lines entirely, ensuring the output ends with
/// exactly one `\n`. Common editor hygiene that every formatter is
/// expected to do.
fn normalize_blank_lines(input: &str) -> String {
  let mut out: Vec<&str> = Vec::with_capacity(input.lines().count());
  let mut prev_blank = false;
  for line in input.lines() {
    let blank = line.chars().all(|c| c.is_whitespace());
    if blank && prev_blank {
      continue;
    }
    out.push(line);
    prev_blank = blank;
  }
  while out.last().is_some_and(|l| l.chars().all(|c| c.is_whitespace())) {
    out.pop();
  }
  let mut s = out.join("\n");
  if !s.is_empty() {
    s.push('\n');
  }
  s
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn collapses_consecutive_blank_lines() {
    let input = "SELECT 1;\n\n\n\nSELECT 2;\n";
    assert_eq!(normalize_blank_lines(input), "SELECT 1;\n\nSELECT 2;\n");
  }

  #[test]
  fn strips_trailing_blank_lines() {
    let input = "SELECT 1;\n\n\n\n";
    assert_eq!(normalize_blank_lines(input), "SELECT 1;\n");
  }

  #[test]
  fn preserves_single_blank_separator() {
    let input = "SELECT 1;\n\nSELECT 2;\n";
    assert_eq!(normalize_blank_lines(input), "SELECT 1;\n\nSELECT 2;\n");
  }

  #[test]
  fn ensures_trailing_newline() {
    let input = "SELECT 1;";
    assert_eq!(normalize_blank_lines(input), "SELECT 1;\n");
  }

  #[test]
  fn empty_input_stays_empty() {
    assert_eq!(normalize_blank_lines(""), "");
    assert_eq!(normalize_blank_lines("\n\n\n"), "");
  }
}
