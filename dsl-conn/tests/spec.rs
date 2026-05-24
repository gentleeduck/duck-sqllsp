use dsl_conn::ConnectionSpec;

#[test]
fn url_for_postgres_with_password() {
  let s = ConnectionSpec {
    name: "n".into(),
    driver: "postgres".into(),
    host: Some("h".into()),
    port: Some(5433),
    user: Some("u".into()),
    password: Some("p".into()),
    database: Some("d".into()),
    schema: None,
  };
  assert_eq!(s.url(), "postgres://u:p@h:5433/d");
}

#[test]
fn url_for_postgres_without_password() {
  let s = ConnectionSpec {
    name: "n".into(),
    driver: "postgresql".into(),
    host: None,
    port: None,
    user: Some("u".into()),
    password: None,
    database: Some("d".into()),
    schema: None,
  };
  assert_eq!(s.url(), "postgres://u@localhost:5432/d");
}

#[test]
fn unknown_driver_returns_empty_url() {
  let s = ConnectionSpec {
    name: "n".into(),
    driver: "mongo".into(),
    host: None,
    port: None,
    user: None,
    password: None,
    database: None,
    schema: None,
  };
  assert_eq!(s.url(), "");
}
