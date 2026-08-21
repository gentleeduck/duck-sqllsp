//! Port-detection rules must not fire on the dialect they detect.
//!
//! Each of these rules exists to say "this is MySQL syntax, PostgreSQL
//! rejects it". On a buffer that *is* MySQL the statement is correct,
//! so the rule is a false positive -- and MySQL is a first-class
//! dialect here.
//!
//! Only five of nineteen such rules were being skipped, so linting an
//! ordinary MySQL file reported errors on backtick identifiers,
//! `LIMIT 10, 20`, `ON DUPLICATE KEY UPDATE`, `REPLACE INTO`,
//! `GROUP_CONCAT`, `REGEXP`, `UNSIGNED`, and more.

use dsl_parse::Dialect;

/// Codes reported for `sql` under `dialect`.
fn codes_for(sql: &str, dialect: Dialect) -> Vec<String> {
  let file = dsl_parse::parse(sql, dialect);
  let scopes = dsl_resolve::resolve_with_source(&file.statements, sql);
  let catalog = dsl_catalog::Catalog::default();
  dsl_analysis::run_with_dialect(sql, &file, &scopes, &catalog, dialect)
    .into_iter()
    .map(|d| d.code.to_string())
    .collect()
}

/// `(code, idiomatic MySQL that the rule exists to flag on Postgres)`
const MYSQL_IDIOMS: &[(&str, &str)] = &[
  ("sql593", "SELECT a FROM t LIMIT 10, 20;"),
  ("sql594", "INSERT INTO t (a) VALUES (1) ON DUPLICATE KEY UPDATE a = 1;"),
  ("sql595", "REPLACE INTO t (a) VALUES (1);"),
  ("sql596", "SELECT GROUP_CONCAT(x) FROM t;"),
  ("sql597", "SELECT a FROM t WHERE name REGEXP 'x';"),
  ("sql599", "CREATE TABLE t (a int unsigned);"),
  ("sql600", "SELECT `col` FROM `tbl`;"),
  ("sql622", "SELECT LCASE(name) FROM t;"),
  ("sql666", "INSERT IGNORE INTO t (a) VALUES (1);"),
  ("sql667", "INSERT INTO t SET a = 1, b = 2;"),
  ("sql669", "SELECT a FROM t LOCK IN SHARE MODE;"),
  ("sql672", "ALTER TABLE t CHANGE COLUMN a b int;"),
];

#[test]
fn mysql_idioms_are_not_flagged_on_a_mysql_buffer() {
  let mut wrong = Vec::new();
  for (code, sql) in MYSQL_IDIOMS {
    if codes_for(sql, Dialect::MySql).iter().any(|c| c == code) {
      wrong.push(format!("{code} still fires on MySQL for: {sql}"));
    }
  }
  assert!(wrong.is_empty(), "port-detection false positives on MySQL:\n{}", wrong.join("\n"));
}

/// The other half of the contract: skipping them on MySQL must not
/// disable them on PostgreSQL, which is the whole reason they exist.
#[test]
fn the_same_idioms_are_still_flagged_on_postgres() {
  let mut missing = Vec::new();
  for (code, sql) in MYSQL_IDIOMS {
    if !codes_for(sql, Dialect::Postgres).iter().any(|c| c == code) {
      missing.push(format!("{code} no longer fires on Postgres for: {sql}"));
    }
  }
  assert!(missing.is_empty(), "port detection lost on Postgres:\n{}", missing.join("\n"));
}

/// `sql429` is deliberately *not* skipped: it covers `<=>` (valid
/// MySQL) and `==` (invalid everywhere, MySQL included). Skipping it
/// wholesale would lose the `==` check on MySQL.
#[test]
fn mixed_validity_rules_stay_active_on_mysql() {
  let codes = codes_for("SELECT a FROM t WHERE b == 1;", Dialect::MySql);
  assert!(codes.iter().any(|c| c == "sql429"), "`==` is invalid in MySQL too and must still be caught: {codes:?}");
}

/// Dialect-independent rules must be unaffected by any of this.
#[test]
fn ordinary_rules_still_fire_on_mysql() {
  let codes = codes_for("SELECT a FROM t WHERE x = NULL;", Dialect::MySql);
  assert!(codes.iter().any(|c| c == "sql015"), "`= NULL` is wrong in every dialect: {codes:?}");
}

/// `(code, idiomatic SQLite that the rule exists to flag on Postgres)`
///
/// SQLite is a first-class dialect too, and the same argument applies: a
/// schema that is SQLite does not want to be told its `AUTOINCREMENT` should
/// have been `GENERATED ALWAYS AS IDENTITY`, and it has type affinity rather
/// than types, so `NVARCHAR(160)` is an ordinary column and not a T-SQL type
/// that leaked in.
const SQLITE_IDIOMS: &[(&str, &str)] = &[
  ("sql636", "CREATE TABLE t (id INTEGER PRIMARY KEY AUTOINCREMENT);"),
  ("sql629", "CREATE TABLE t (title NVARCHAR(160) NOT NULL);"),
];

#[test]
fn sqlite_idioms_are_not_flagged_on_a_sqlite_buffer() {
  let mut wrong = Vec::new();
  for (code, sql) in SQLITE_IDIOMS {
    if codes_for(sql, Dialect::SQLite).iter().any(|c| c == code) {
      wrong.push(format!("{code} still fires on SQLite for: {sql}"));
    }
  }
  assert!(wrong.is_empty(), "port-detection false positives on SQLite:\n{}", wrong.join("\n"));
}

#[test]
fn the_same_sqlite_idioms_are_still_flagged_on_postgres() {
  let mut missing = Vec::new();
  for (code, sql) in SQLITE_IDIOMS {
    if !codes_for(sql, Dialect::Postgres).iter().any(|c| c == code) {
      missing.push(format!("{code} no longer fires on Postgres for: {sql}"));
    }
  }
  assert!(missing.is_empty(), "port detection lost on Postgres:\n{}", missing.join("\n"));
}

/// Bracket-quoted identifiers are how sqlite stores the DDL for a table most
/// tools created, so a SQLite buffer full of them has to lint clean.
#[test]
fn real_sqlite_ddl_lints_clean() {
  let sql = "CREATE TABLE \"albums\"\n(\n    [AlbumId] INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,\n    [Title] NVARCHAR(160)  NOT NULL,\n    [ArtistId] INTEGER  NOT NULL,\n    FOREIGN KEY ([ArtistId]) REFERENCES \"artists\" ([ArtistId])\n);\n";
  let codes = codes_for(sql, Dialect::SQLite);
  assert!(codes.is_empty(), "the DDL sqlite itself stores should lint clean, got: {codes:?}");
}

#[test]
fn ordinary_rules_still_fire_on_sqlite() {
  let codes = codes_for("SELECT a FROM t WHERE x = NULL;", Dialect::SQLite);
  assert!(codes.iter().any(|c| c == "sql015"), "`= NULL` is wrong in every dialect: {codes:?}");
}
