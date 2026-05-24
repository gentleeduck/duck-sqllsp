use dsl_parse::{parse, Dialect, StatementKind};

#[test]
fn parses_basic_select() {
    let p = parse("SELECT id FROM users", Dialect::Postgres);
    assert!(p.errors.is_empty());
    assert_eq!(p.statements.len(), 1);
    let StatementKind::Select(s) = &p.statements[0].kind else {
        panic!("expected Select");
    };
    assert_eq!(s.from.len(), 1);
    assert_eq!(s.from[0].name, "users");
}

#[test]
fn captures_alias() {
    let p = parse("SELECT u.id FROM users u", Dialect::Postgres);
    let StatementKind::Select(s) = &p.statements[0].kind else { panic!() };
    assert_eq!(s.from[0].alias.as_deref(), Some("u"));
}

#[test]
fn captures_join() {
    let p = parse("SELECT * FROM users u JOIN orders o ON o.user_id = u.id", Dialect::Postgres);
    let StatementKind::Select(s) = &p.statements[0].kind else { panic!() };
    assert_eq!(s.joins.len(), 1);
    assert_eq!(s.joins[0].table.name, "orders");
}

#[test]
fn surfaces_per_statement_errors() {
    let p = parse("SELEKT 1; SELECT 2;", Dialect::Postgres);
    assert_eq!(p.errors.len(), 1);
    assert_eq!(p.statements.len(), 2);
}

#[test]
fn captures_schema_qualified_table() {
    let p = parse("SELECT * FROM public.users", Dialect::Postgres);
    let StatementKind::Select(s) = &p.statements[0].kind else { panic!() };
    assert_eq!(s.from[0].schema.as_deref(), Some("public"));
    assert_eq!(s.from[0].name, "users");
}
