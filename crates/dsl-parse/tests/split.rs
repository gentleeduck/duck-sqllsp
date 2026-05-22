use dsl_parse::split::split_statements;

#[test]
fn splits_two_simple_statements() {
    let parts = split_statements("SELECT 1; SELECT 2;");
    assert_eq!(parts.len(), 2);
    assert_eq!(parts[0].0, "SELECT 1");
    assert_eq!(parts[1].0, "SELECT 2");
}

#[test]
fn trailing_statement_without_semicolon() {
    let parts = split_statements("SELECT 1; SELECT 2");
    assert_eq!(parts.len(), 2);
    assert_eq!(parts[1].0, "SELECT 2");
}

#[test]
fn ignores_blank_chunks() {
    let parts = split_statements(";;\n;\n;");
    assert!(parts.is_empty());
}

#[test]
fn respects_single_quoted_strings() {
    let parts = split_statements("SELECT 'a;b;c'; SELECT 2;");
    assert_eq!(parts.len(), 2);
    assert_eq!(parts[0].0, "SELECT 'a;b;c'");
}

#[test]
fn respects_double_quoted_identifiers() {
    let parts = split_statements(r#"SELECT "tab;name"; SELECT 2;"#);
    assert_eq!(parts.len(), 2);
}

#[test]
fn respects_dollar_quoting_with_tag() {
    let src = "DO $body$ BEGIN RAISE NOTICE 'hi; bye'; END $body$; SELECT 1;";
    let parts = split_statements(src);
    assert_eq!(parts.len(), 2);
    assert!(parts[0].0.contains("RAISE NOTICE"));
}

#[test]
fn respects_empty_dollar_tag() {
    let src = "DO $$ BEGIN x := 1; END $$; SELECT 1;";
    let parts = split_statements(src);
    assert_eq!(parts.len(), 2);
}
