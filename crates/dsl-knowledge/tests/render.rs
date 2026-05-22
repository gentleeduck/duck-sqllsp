use dsl_knowledge::{lookup, render_markdown};

#[test]
fn render_has_header_and_kind() {
    let entry = lookup("SELECT").expect("SELECT exists");
    let md = render_markdown(entry);
    assert!(md.contains("# SELECT"), "header missing: {md}");
    assert!(md.contains("_Keyword_"), "kind tag missing: {md}");
}

#[test]
fn render_has_docs_link() {
    let entry = lookup("SELECT").expect("SELECT exists");
    let md = render_markdown(entry);
    assert!(md.contains("[Postgres docs]"), "docs link missing");
}

#[test]
fn render_includes_signature_for_functions() {
    let entry = lookup("count").expect("count exists");
    let md = render_markdown(entry);
    assert!(md.contains("count(* | expr) -> bigint"));
    assert!(md.contains("_Function_"));
}

#[test]
fn render_includes_example_block() {
    let entry = lookup("UUID").expect("UUID exists");
    let md = render_markdown(entry);
    assert!(md.contains("```sql"), "missing code fence");
    assert!(md.contains("_Type_"), "missing kind tag");
}
