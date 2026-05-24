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
  normalize_blank_lines(&after_align)
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
