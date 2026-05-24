use dsl_conn::ConnectionSpec;

#[test]
fn url_round_trips_unchanged() {
  let s = ConnectionSpec {
    name: "n".into(),
    url: "postgres://u:p@h:5433/d".into(),
  };
  assert_eq!(s.url(), "postgres://u:p@h:5433/d");
}

#[test]
fn driver_inferred_from_postgres_scheme() {
  let s = ConnectionSpec { name: "n".into(), url: "postgres://u@h/d".into() };
  assert_eq!(s.driver(), "postgres");
  let s = ConnectionSpec { name: "n".into(), url: "postgresql://u@h/d".into() };
  assert_eq!(s.driver(), "postgres");
}

#[test]
fn driver_inferred_from_mysql_scheme() {
  let s = ConnectionSpec { name: "n".into(), url: "mysql://u@h/d".into() };
  assert_eq!(s.driver(), "mysql");
  let s = ConnectionSpec { name: "n".into(), url: "mariadb://u@h/d".into() };
  assert_eq!(s.driver(), "mysql");
}

#[test]
fn driver_inferred_from_sqlite_scheme() {
  let s = ConnectionSpec { name: "n".into(), url: "sqlite:///tmp/x.db".into() };
  assert_eq!(s.driver(), "sqlite");
}

#[test]
fn unknown_scheme_returns_unknown() {
  let s = ConnectionSpec { name: "n".into(), url: "mongo://h/d".into() };
  assert_eq!(s.driver(), "unknown");
}
