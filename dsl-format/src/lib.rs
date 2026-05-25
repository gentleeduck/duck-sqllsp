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
  let after_external = external::run_sql_formatter(input, fmt_style).unwrap_or_else(|| input.to_string());
  let after_align = align::rewrite(&after_external, ct_style);
  let after_tighten = tighten_call_parens(&after_align);
  normalize_blank_lines(&after_tighten)
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
        out.push(bytes[i] as char);
        i += 1;
      }
      if i < bytes.len() {
        out.push('\'');
        i += 1;
      }
      continue;
    }
    if i + 1 < bytes.len() && bytes[i] == b'-' && bytes[i + 1] == b'-' {
      while i < bytes.len() && bytes[i] != b'\n' {
        out.push(bytes[i] as char);
        i += 1;
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
    out.push(bytes[i] as char);
    i += 1;
  }
  out
}

/// SQL keywords whose following `(` is a grouping / sub-query / IN-list
/// paren rather than a function call. Keep the space for readability.
const KEEP_SPACE_BEFORE_PAREN: &[&str] = &[
  "SELECT", "IN", "NOT", "EXISTS", "VALUES", "RETURNING", "WHERE",
  "HAVING", "ON", "USING", "FROM", "INTO", "AS", "WITH", "CASE",
  "WHEN", "THEN", "ELSE", "ANY", "ALL", "SOME", "AND", "OR", "BY",
  "IS", "BETWEEN", "LIKE", "ILIKE", "SIMILAR", "OVERLAPS", "FILTER",
  "OVER", "PARTITION", "WITHIN", "PRECEDING", "FOLLOWING",
  "UNBOUNDED", "FOR", "ROW", "ROWS", "GROUPS", "RANGE",
  "DEFAULT", "REFERENCES", "CHECK", "UNIQUE", "PRIMARY", "FOREIGN",
  "KEY", "CONSTRAINT", "DISTINCT", "GROUP", "ORDER", "LIMIT",
  "OFFSET", "FETCH", "INTERSECT", "UNION", "EXCEPT", "DO", "LANGUAGE",
  "MATCH", "TO", "OF", "RESTRICT", "CASCADE",
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
  while out.last().map_or(false, |l| l.chars().all(|c| c.is_whitespace())) {
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
