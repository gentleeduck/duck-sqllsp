use dsl_completion::phase::{detect, Phase};
use text_size::TextSize;

fn ph(src: &str) -> Phase {
    detect(src, TextSize::from(src.len() as u32))
}

#[test]
fn empty_buffer_is_start() {
    assert_eq!(ph(""), Phase::Start);
    assert_eq!(ph("  \n  "), Phase::Start);
}

#[test]
fn after_select_keyword_is_projection() {
    assert_eq!(ph("SELECT "), Phase::SelectProjection);
}

#[test]
fn in_projection_after_column() {
    assert_eq!(ph("SELECT id"), Phase::SelectProjection); // partial id stripped
    assert_eq!(ph("SELECT id "), Phase::InProjection);
}

#[test]
fn after_star_expects_from() {
    assert_eq!(ph("SELECT * "), Phase::AfterStar);
}

#[test]
fn after_comma_expects_next_projection() {
    assert_eq!(ph("SELECT id, "), Phase::NextProjection);
}

#[test]
fn after_from_keyword_expects_table() {
    assert_eq!(ph("SELECT * FROM "), Phase::ExpectTable);
}

#[test]
fn after_table_name_expects_join_or_clauses() {
    assert_eq!(ph("SELECT * FROM users "), Phase::AfterTable);
}

#[test]
fn after_join_keyword_expects_table() {
    assert_eq!(ph("SELECT * FROM users JOIN "), Phase::ExpectTable);
    assert_eq!(ph("SELECT * FROM users INNER JOIN "), Phase::ExpectTable);
}

#[test]
fn after_on_expects_predicate() {
    assert_eq!(ph("SELECT * FROM a JOIN b ON "), Phase::OnClause);
}

#[test]
fn after_where_expects_predicate() {
    assert_eq!(ph("SELECT * FROM users WHERE "), Phase::WhereClause);
}

#[test]
fn after_group_keyword_expects_by() {
    assert_eq!(ph("SELECT * FROM users GROUP "), Phase::AfterGroup);
}

#[test]
fn after_group_by_expects_columns() {
    assert_eq!(ph("SELECT * FROM users GROUP BY "), Phase::GroupByList);
}

#[test]
fn after_order_by_expects_columns() {
    assert_eq!(ph("SELECT * FROM users ORDER BY "), Phase::OrderByList);
}

#[test]
fn semicolon_resets_phase() {
    assert_eq!(ph("SELECT 1; "), Phase::Start);
    assert_eq!(ph("DELETE FROM users; "), Phase::Start);
}

#[test]
fn comment_lines_skip_correctly() {
    assert_eq!(ph("-- a comment\nSELECT "), Phase::SelectProjection);
}

#[test]
fn quoted_strings_dont_count() {
    // The semicolon inside the literal must not reset us.
    assert_eq!(
        ph("SELECT 'a;b' "),
        Phase::InProjection
    );
}

#[test]
fn dollar_quoted_blocks_skipped() {
    assert_eq!(
        ph("DO $$ BEGIN END $$; SELECT "),
        Phase::SelectProjection,
    );
}

#[test]
fn insert_into_then_table() {
    assert_eq!(ph("INSERT INTO "), Phase::AfterInsertTable);
    assert_eq!(ph("INSERT INTO users "), Phase::InsertColumnList);
}

#[test]
fn update_then_table_then_set() {
    assert_eq!(ph("UPDATE "), Phase::AfterUpdate);
    assert_eq!(ph("UPDATE users "), Phase::AfterUpdateTable);
    assert_eq!(ph("UPDATE users SET "), Phase::UpdateAssignment);
}

#[test]
fn delete_then_from() {
    assert_eq!(ph("DELETE "), Phase::AfterDelete);
    assert_eq!(ph("DELETE FROM "), Phase::ExpectTable);
}

#[test]
fn predicate_with_and_continues() {
    let p = ph("SELECT * FROM users WHERE id = 1 AND ");
    assert!(matches!(p, Phase::WhereClause | Phase::InPredicate));
}

#[test]
fn join_then_on_then_clauses() {
    let p = ph("SELECT * FROM a JOIN b ON a.id = b.aid ");
    assert!(matches!(p, Phase::OnClause | Phase::InPredicate));
}

#[test]
fn dotted_table_keeps_after_table() {
    assert_eq!(ph("SELECT * FROM public.users "), Phase::AfterTable);
}
