use dsl_completion::context::{detect, Context};
use text_size::TextSize;

fn at_end(s: &str) -> TextSize { TextSize::from(s.len() as u32) }

#[test]
fn dot_after_alias() {
    assert_eq!(detect("SELECT u.", at_end("SELECT u.")), Context::DotOf { alias: "u".into() });
}

#[test]
fn dot_with_partial() {
    assert_eq!(detect("SELECT u.id", at_end("SELECT u.id")), Context::DotOf { alias: "u".into() });
}

#[test]
fn from_implies_table() {
    assert_eq!(detect("SELECT * FROM ", at_end("SELECT * FROM ")), Context::Table);
}

#[test]
fn from_with_partial() {
    assert_eq!(detect("SELECT * FROM us", at_end("SELECT * FROM us")), Context::Table);
}

#[test]
fn join_implies_table() {
    assert_eq!(detect("FROM users JOIN ", at_end("FROM users JOIN ")), Context::Table);
}

#[test]
fn alter_table_implies_table() {
    assert_eq!(detect("ALTER TABLE ", at_end("ALTER TABLE ")), Context::Table);
}

#[test]
fn statement_start_for_bare_word() {
    // A bare prefix at the start of the buffer is start of a statement.
    // We emit keywords first there, which is what `Statement` triggers.
    assert_eq!(detect("SEL", at_end("SEL")), Context::Statement);
}

#[test]
fn predicate_after_where() {
    assert_eq!(detect("SELECT * FROM users WHERE ", at_end("SELECT * FROM users WHERE ")),
               Context::Predicate);
}

#[test]
fn predicate_after_on() {
    assert_eq!(detect("FROM a JOIN b ON ", at_end("FROM a JOIN b ON ")),
               Context::Predicate);
}
