//! Role hover: cursor on a role identifier in OWNER TO / GRANT TO /
//! REVOKE FROM / SET ROLE / CREATE POLICY contexts surfaces a role
//! card that names the source (catalog / built-in / pseudo).

use dsl_catalog::{CATALOG_VERSION, Catalog, Column, Schema, Table, TableKind};
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

/// A catalog with a real table + column, for the POLICY expression
/// tests below -- `cat_with_roles` above has no table support, and
/// these need one to prove a column inside `USING (...)` resolves as
/// a column, not a role.
fn cat_with_users_table(rs: &[&str]) -> Catalog {
  let users = Table {
    schema: "public".into(),
    name: "users".into(),
    kind: TableKind::Table,
    columns: vec![Column {
      name: "org_id".into(),
      data_type: "uuid".into(),
      nullable: false,
      default: None,
      comment: None,
      generated: None,
      json_keys: None,
    }],
    constraints: vec![],
    indexes: vec![],
    triggers: vec![],
    policies: vec![],
    comment: None,
    row_estimate: None,
    owner: None,
    definition: None,
    strict: false,
    options: None,
  };
  Catalog {
    version: CATALOG_VERSION,
    connection_id: "test".into(),
    schemas: vec![Schema { name: "public".into(), tables: vec![users] }],
    functions: vec![],
    types: vec![],
    roles: rs.iter().map(|s| (*s).to_string()).collect(),
    sequences: vec![],
    extensions: vec![],
  }
}

#[test]
fn policy_using_expr_column_not_misidentified_as_role() {
  // near_role_slot used to trigger on the bare word "POLICY" anywhere
  // within 60 chars before the cursor, not specifically the TO <role>
  // slot -- so a column reference inside USING (...) false-triggered
  // the role card instead of resolving as a column.
  let c = cat_with_users_table(&[]);
  let src = "CREATE POLICY p ON users USING (org_id = 1);";
  let cur = src.find("org_id").unwrap() + 3;
  let md = hover(src, TextSize::from(cur as u32), &c).expect("hover result");
  assert!(!md.contains("_role_"), "org_id should not be identified as a role; got: {md}");
  assert!(md.contains("org_id"), "expected a column card for org_id; got: {md}");
}

#[test]
fn policy_to_role_slot_still_resolves_as_role() {
  // The real role slot (TO <role>) must keep working after removing
  // the over-broad "POLICY" trigger -- caught by the " TO " entry in
  // the same keyword list, independent of "POLICY".
  let c = cat_with_roles(&["admin_role"]);
  let src = "CREATE POLICY p ON users FOR ALL TO admin_role;";
  let cur = src.find("admin_role").unwrap() + 3;
  let md = hover(src, TextSize::from(cur as u32), &c).expect("role card");
  assert!(md.contains("admin_role"), "missing role name: {md}");
  assert!(md.contains("pg_roles"), "missing catalog source mention: {md}");
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
    assert!(!md.contains("bootstrap superuser"), "role card hijacked a plain identifier: {md}");
  }
}
