use dsl_knowledge::{lookup, Kind};

#[test]
fn finds_uppercase_keyword() {
    let e = lookup("SELECT").unwrap();
    assert_eq!(e.kind, Kind::Keyword);
}

#[test]
fn finds_lowercase_keyword() {
    let e = lookup("select").unwrap();
    assert_eq!(e.kind, Kind::Keyword);
}

#[test]
fn finds_function_case_insensitive() {
    let e = lookup("Count").unwrap();
    assert_eq!(e.kind, Kind::Function);
}

#[test]
fn finds_type() {
    let e = lookup("uuid").unwrap();
    assert_eq!(e.kind, Kind::Type);
}

#[test]
fn returns_none_for_unknown() {
    assert!(lookup("frobnicate_xyz").is_none());
}
