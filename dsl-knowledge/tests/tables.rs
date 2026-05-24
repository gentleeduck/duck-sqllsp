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
