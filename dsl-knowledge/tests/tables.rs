use dsl_knowledge::{Kind, functions, keywords, types};

#[test]
fn keywords_table_populates() {
  let m = keywords();
  assert!(m.contains_key("SELECT"));
  assert!(m.contains_key("FROM"));
  assert!(m.contains_key("INNER JOIN"));
  let entry = m.get("SELECT").unwrap();
  assert_eq!(entry.kind, Kind::Keyword);
  assert!(entry.doc.contains("Retrieve"));
}

#[test]
fn types_table_populates() {
  let m = types();
  assert!(m.contains_key("UUID"));
  assert!(m.contains_key("TIMESTAMPTZ"));
  assert_eq!(m.get("UUID").unwrap().kind, Kind::Type);
}

#[test]
fn functions_table_populates() {
  let m = functions();
  assert!(m.contains_key("count"));
  assert!(m.contains_key("gen_random_uuid"));
  let entry = m.get("count").unwrap();
  assert_eq!(entry.kind, Kind::Function);
  assert!(entry.signature.is_some());
}

// The three tests below exist to give a clear, attributable failure if
// the panic-on-duplicate-label guard in each table's `build()` macro
// ever fires (see the 2026-08-18 dedup commit) -- without them, a
// regression would only surface as an incidental panic inside whichever
// *other* test happened to touch that table's `Lazy` first, which
// `once_cell::sync::Lazy` poisons for every subsequent access in the
// same test binary, obscuring the actual cause.
#[test]
fn keywords_table_has_no_duplicate_entries() {
  let _ = keywords();
}

#[test]
fn types_table_has_no_duplicate_entries() {
  let _ = types();
}

#[test]
fn functions_table_has_no_duplicate_entries() {
  let _ = functions();
}
