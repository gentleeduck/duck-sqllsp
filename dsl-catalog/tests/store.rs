use dsl_catalog::{CATALOG_VERSION, Catalog, CatalogStore};

#[test]
fn store_replace_visible_to_readers() {
  let store = CatalogStore::new();
  assert_eq!(store.read().version, 0);

  let mut cat = Catalog::default();
  cat.version = CATALOG_VERSION;
  cat.connection_id = "demo".into();
  store.replace(cat);

  let r = store.read();
  assert_eq!(r.version, CATALOG_VERSION);
  assert_eq!(r.connection_id, "demo");
}
