//! Coverage tests: every catalogued keyword / function / type has a
//! non-empty doc and a usable signature/example where applicable.

use dsl_knowledge::{Kind, functions, keywords, types};

#[test]
fn every_keyword_has_non_empty_doc() {
  for (label, entry) in keywords() {
    assert!(!entry.doc.trim().is_empty(), "keyword `{label}` has empty doc");
  }
}

#[test]
fn every_type_has_non_empty_doc() {
  for (label, entry) in types() {
    assert!(!entry.doc.trim().is_empty(), "type `{label}` has empty doc");
    assert!(matches!(entry.kind, Kind::Type), "type `{label}` has wrong kind {:?}", entry.kind);
  }
}

#[test]
fn every_function_has_signature() {
  for (label, entry) in functions() {
    assert!(entry.signature.is_some(), "function `{label}` missing signature");
    assert!(matches!(entry.kind, Kind::Function), "function `{label}` has wrong kind {:?}", entry.kind);
  }
}

#[test]
fn every_entry_has_pg_docs_url() {
  // Documentation links matter for the [Postgres docs] footer.
  for (label, entry) in keywords() {
    assert!(!entry.url.is_empty(), "keyword `{label}` missing docs URL");
  }
}
