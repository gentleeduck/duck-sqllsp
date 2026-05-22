//! Edge-case tests for `wrap_paragraphs` -- it must preserve code
//! blocks, indented lines, list items, tables, blank lines, and never
//! exceed the requested width on a wrappable line.

use dsl_knowledge::wrap_paragraphs;

#[test]
fn short_text_passes_through_unchanged() {
    let s = "hello world";
    assert_eq!(wrap_paragraphs(s, 80), s);
}

#[test]
fn long_paragraph_breaks_at_word_boundary() {
    let s = "alpha beta gamma delta epsilon zeta eta theta iota kappa lambda mu nu";
    let out = wrap_paragraphs(s, 20);
    for line in out.lines() {
        // Single very-long words are allowed to exceed.
        if line.split_whitespace().count() > 1 {
            assert!(
                line.chars().count() <= 20,
                "line exceeds 20: `{line}` ({})",
                line.chars().count()
            );
        }
    }
}

#[test]
fn fenced_code_block_is_preserved_verbatim() {
    let s = "intro\n\n```sql\nSELECT a very very very very very very very long column FROM users\n```\nafter";
    let out = wrap_paragraphs(s, 30);
    // The SELECT line inside the fence stays put.
    assert!(out.contains("SELECT a very very very very very very very long column FROM users"));
}

#[test]
fn blank_lines_are_kept() {
    let s = "para one\n\npara two";
    let out = wrap_paragraphs(s, 80);
    assert!(out.contains("\n\n"), "blank line collapsed: {out:?}");
}

#[test]
fn list_items_pass_through() {
    let s = "- item one\n- item two\n* item three";
    let out = wrap_paragraphs(s, 5);
    // Even with width 5, list markers are preserved verbatim.
    assert!(out.contains("- item one"));
    assert!(out.contains("- item two"));
    assert!(out.contains("* item three"));
}

#[test]
fn indented_code_is_preserved() {
    let s = "intro\n    SELECT col_super_long FROM users\nafter";
    let out = wrap_paragraphs(s, 20);
    assert!(out.contains("    SELECT col_super_long FROM users"));
}

#[test]
fn empty_input_is_empty_output() {
    assert_eq!(wrap_paragraphs("", 80), "");
}

#[test]
fn single_word_longer_than_width_is_not_split() {
    let s = "supercalifragilisticexpialidocious";
    let out = wrap_paragraphs(s, 5);
    assert_eq!(out, s);
}

#[test]
fn unicode_paragraph_wraps() {
    let s = "αβγ δεζ ηθι κλμ νξο πρς";
    let out = wrap_paragraphs(s, 10);
    for line in out.lines() {
        if line.split_whitespace().count() > 1 {
            assert!(line.chars().count() <= 10);
        }
    }
}

#[test]
fn table_rows_are_kept() {
    let s = "| col | type |\n|-----|------|\n| id  | uuid |";
    let out = wrap_paragraphs(s, 5);
    assert!(out.contains("| col | type |"));
    assert!(out.contains("|-----|------|"));
}
