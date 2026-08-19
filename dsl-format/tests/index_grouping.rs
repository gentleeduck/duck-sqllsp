//! `groupIndexes` and the keyword case of broken-out index headers.
//!
//! `groupIndexes` did nothing in either position. `collapse_index_runs`
//! looks for a blank line immediately after a `CREATE INDEX` line, but
//! it ran *after* `break_index_headers` had already split the statement
//! in two -- so by then the blank line followed the `ON ...;` line and
//! the check never matched. An ordering bug, invisible from either
//! function on its own.

use dsl_format::{CreateTableStyle, FormatterStyle, format};

const INDEXES: &str = "create index ix_a on t(email);\n\n\
                       create index ix_b on t(id);\n\n\
                       create index ix_c on t(id, email);\n";

fn formatted(group: bool) -> String {
  let ct = CreateTableStyle { group_indexes: group, ..CreateTableStyle::default() };
  format(INDEXES, &FormatterStyle::default(), &ct)
}

/// Blank lines *between* consecutive index statements, ignoring any
/// leading or trailing ones.
fn interior_blank_lines(s: &str) -> usize {
  let lines: Vec<&str> = s.lines().collect();
  let first = lines.iter().position(|l| !l.trim().is_empty()).unwrap_or(0);
  let last = lines.iter().rposition(|l| !l.trim().is_empty()).unwrap_or(0);
  lines[first..=last].iter().filter(|l| l.trim().is_empty()).count()
}

#[test]
fn grouping_removes_the_blank_lines_between_indexes() {
  let out = formatted(true);
  assert_eq!(interior_blank_lines(&out), 0, "expected a packed run:\n{out}");
  // All three must survive -- collapsing runs must not eat statements.
  for ix in ["ix_a", "ix_b", "ix_c"] {
    assert!(out.contains(ix), "{ix} was dropped:\n{out}");
  }
}

#[test]
fn disabling_grouping_keeps_them_apart() {
  let out = formatted(false);
  assert!(interior_blank_lines(&out) >= 2, "expected the blank lines kept:\n{out}");
}

#[test]
fn the_two_settings_actually_differ() {
  // The regression this file exists for: both produced identical output.
  assert_ne!(formatted(true), formatted(false));
}

// ---------------------------------------------------------------------
// Broken-out header keywords follow `keywordCase`.
// ---------------------------------------------------------------------

fn header_keyword(case: &str) -> String {
  let style = FormatterStyle { keyword_case: case.into(), ..FormatterStyle::default() };
  let out = format("create index ix_a on t(email);\n", &style, &CreateTableStyle::default());
  out.lines().find(|l| l.trim_start().starts_with(['O', 'o'])).unwrap_or("").trim().to_string()
}

#[test]
fn broken_out_on_follows_the_configured_case() {
  assert!(header_keyword("upper").starts_with("ON "), "{}", header_keyword("upper"));
  assert!(header_keyword("lower").starts_with("on "), "{}", header_keyword("lower"));
}

#[test]
fn preserve_keeps_the_users_spelling() {
  // Previously the break always emitted the canonical uppercase needle,
  // so `create index ... on ...` came back with an uppercase `ON` next
  // to a lowercase `create index`.
  assert!(header_keyword("preserve").starts_with("on "), "{}", header_keyword("preserve"));
  let style = FormatterStyle { keyword_case: "preserve".into(), ..FormatterStyle::default() };
  let out = format("CREATE INDEX ix_a ON t(email);\n", &style, &CreateTableStyle::default());
  assert!(out.contains("ON t(email)"), "uppercase input must stay uppercase:\n{out}");
}
