//! Role hover: cursor on a role identifier in OWNER TO / GRANT TO /
//! REVOKE FROM / SET ROLE / CREATE POLICY contexts surfaces a role
//! card that names the source (catalog / built-in / pseudo).

use dsl_catalog::{CATALOG_VERSION, Catalog, Schema};
use dsl_hover::hover;
use text_size::TextSize;

fn cat_with_roles(rs: &[&str]) -> Catalog {
  Catalog {
    version: CATALOG_VERSION,
    connection_id: "test".into(),
    schemas: vec![Schema { name: "public".into(), tables: vec![] }],
    functions: vec![],
    types: vec![],
    roles: rs.iter().map(|s| (*s).to_string()).collect(),
    sequences: vec![],
    extensions: vec![],
  }
}

#[test]
fn hover_on_role_in_owner_to_shows_card() {
  let c = cat_with_roles(&["app_owner"]);
  let src = "ALTER TABLE users OWNER TO app_owner;";
  let cur = src.find("app_owner").unwrap() + 3;
  let md = hover(src, TextSize::from(cur as u32), &c).expect("role card");
  assert!(md.contains("app_owner"), "missing role name: {md}");
  assert!(md.contains("pg_roles"), "missing catalog source mention: {md}");
}

#[test]
fn hover_on_postgres_role_marks_built_in() {
  let c = cat_with_roles(&["app_owner"]);
  let src = "ALTER TABLE users OWNER TO postgres;";
  let cur = src.find("postgres").unwrap() + 3;
  let md = hover(src, TextSize::from(cur as u32), &c).expect("postgres card");
  assert!(md.contains("postgres"));
  assert!(md.contains("bootstrap superuser"), "missing built-in label: {md}");
}

#[test]
fn hover_on_unknown_role_flags_missing() {
  let c = cat_with_roles(&["app_owner"]);
  let src = "ALTER TABLE users OWNER TO mystery_role;";
  let cur = src.find("mystery_role").unwrap() + 3;
  let md = hover(src, TextSize::from(cur as u32), &c).expect("unknown card");
  assert!(md.contains("not found"), "missing 'not found' label: {md}");
}

#[test]
fn hover_on_public_pseudo_role() {
  let c = cat_with_roles(&[]);
  let src = "GRANT SELECT ON users TO PUBLIC;";
  let cur = src.find("PUBLIC").unwrap() + 2;
  let md = hover(src, TextSize::from(cur as u32), &c).expect("public card");
  assert!(md.contains("pseudo-role"), "missing pseudo-role label: {md}");
}

#[test]
fn hover_role_outside_role_slot_does_not_hijack_identifier() {
  // No role context around it -- `postgres` here is just a plain
  // identifier; the role card MUST NOT fire so other lookups can win.
  let c = cat_with_roles(&["postgres"]);
  let src = "SELECT postgres FROM whatever;";
  let cur = src.find("postgres").unwrap() + 2;
  // Plain SELECT identifier has no other resolution either -> None.
  // We just assert the role card didn't claim it: if hover IS Some,
  // it must not be the role card.
  if let Some(md) = hover(src, TextSize::from(cur as u32), &c) {
    assert!(!md.contains("bootstrap superuser"),
            "role card hijacked a plain identifier: {md}");
  }
}
