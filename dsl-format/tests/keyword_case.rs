//! `formatter.keywordCase` must apply on the built-in path too.
//!
//! The alignment pass *re-emits* `NOT NULL` and `DEFAULT` rather than
//! copying the user's text, so it decides their case. It used to
//! hardcode uppercase, which meant `keywordCase = "lower"` did nothing
//! whenever the external `sql-formatter` binary was missing -- and that
//! is the documented fallback, not an edge case. The setting appeared
//! to work only for people who had the optional binary installed.

use dsl_format::{CreateTableStyle, FormatterStyle, KeywordCase, format, rewrite_with_case};

const DDL: &str = "create table t(\n\
                   id uuid primary key,\n\
                   email text not null,\n\
                   age int not null default 0\n\
                   );";

fn formatted_with(case: &str) -> String {
  let style = FormatterStyle { keyword_case: case.into(), ..FormatterStyle::default() };
  format(DDL, &style, &CreateTableStyle::default())
}

#[test]
fn lower_keeps_emitted_keywords_lowercase() {
  let out = formatted_with("lower");
  assert!(out.contains("not null"), "expected lowercase `not null`:\n{out}");
  assert!(out.contains("default 0"), "expected lowercase `default`:\n{out}");
  assert!(!out.contains("NOT NULL"), "uppercase leaked through:\n{out}");
}

#[test]
fn upper_is_the_default() {
  let out = formatted_with("upper");
  assert!(out.contains("NOT NULL"), "expected uppercase `NOT NULL`:\n{out}");
  assert!(out.contains("DEFAULT 0"), "expected uppercase `DEFAULT`:\n{out}");
}

#[test]
fn preserve_keeps_what_the_user_wrote() {
  let mixed = "create table t(\n  a int Not Null,\n  b int DeFaUlT 1\n);";
  let style = FormatterStyle { keyword_case: "preserve".into(), ..FormatterStyle::default() };
  let out = format(mixed, &style, &CreateTableStyle::default());
  assert!(out.contains("Not Null"), "preserve should not re-case:\n{out}");
  assert!(out.contains("DeFaUlT"), "preserve should not re-case:\n{out}");
}

/// Trailing constraints are the user's own text and must survive
/// untouched -- case-folding them would mean rewriting expressions, and
/// identifiers inside a CHECK are case-sensitive.
#[test]
fn trailing_constraints_are_never_recased() {
  for case in ["upper", "lower", "preserve"] {
    let src = "create table t(\n  id int primary key,\n  v int check (MyCol > 0)\n);";
    let style = FormatterStyle { keyword_case: case.into(), ..FormatterStyle::default() };
    let out = format(src, &style, &CreateTableStyle::default());
    assert!(out.contains("primary key"), "{case}: constraint was re-cased:\n{out}");
    assert!(out.contains("MyCol"), "{case}: identifier inside CHECK was re-cased:\n{out}");
  }
}

#[test]
fn an_unknown_case_value_falls_back_to_upper() {
  assert_eq!(KeywordCase::from_config("nonsense"), KeywordCase::Upper);
  assert_eq!(KeywordCase::from_config(""), KeywordCase::Upper);
  assert_eq!(KeywordCase::from_config("LOWER"), KeywordCase::Lower);
  assert_eq!(KeywordCase::from_config(" Preserve "), KeywordCase::Preserve);
}

#[test]
fn rewrite_without_a_case_still_defaults_to_upper() {
  // `rewrite` is public API; its behaviour must not change.
  let out = dsl_format::rewrite(DDL, &CreateTableStyle::default());
  assert!(out.contains("NOT NULL"), "{out}");
  let explicit = rewrite_with_case(DDL, &CreateTableStyle::default(), KeywordCase::Upper);
  assert_eq!(out, explicit);
}

// ---------------------------------------------------------------------
// tabWidth, same class of bug: the aligner emits the column body's
// indentation itself, and hardcoded four spaces.
// ---------------------------------------------------------------------

fn indent_of_first_column(out: &str) -> usize {
  out
    .lines()
    .find(|l| l.trim_start().starts_with("id "))
    .map(|l| l.len() - l.trim_start().len())
    .unwrap_or_else(|| panic!("no column row in:\n{out}"))
}

fn formatted_with_width(width: usize) -> String {
  let style = FormatterStyle { tab_width: width, ..FormatterStyle::default() };
  format(DDL, &style, &CreateTableStyle::default())
}

#[test]
fn tab_width_sets_the_column_body_indent() {
  assert_eq!(indent_of_first_column(&formatted_with_width(2)), 2);
  assert_eq!(indent_of_first_column(&formatted_with_width(8)), 8);
}

#[test]
fn the_default_width_is_unchanged() {
  // Four spaces is what the aligner always produced, and the default
  // value of `tabWidth` -- so nobody's output moves unless they asked.
  assert_eq!(indent_of_first_column(&formatted_with_width(4)), 4);
  assert_eq!(indent_of_first_column(&dsl_format::rewrite(DDL, &CreateTableStyle::default())), 4);
}

#[test]
fn a_zero_width_falls_back_rather_than_collapsing() {
  // Zero would put the columns hard against the paren margin.
  assert_eq!(indent_of_first_column(&formatted_with_width(0)), 4);
}

#[test]
fn width_and_case_apply_together() {
  let style = FormatterStyle { tab_width: 2, keyword_case: "lower".into(), ..FormatterStyle::default() };
  let out = format(DDL, &style, &CreateTableStyle::default());
  assert_eq!(indent_of_first_column(&out), 2);
  assert!(out.contains("not null"), "{out}");
}
