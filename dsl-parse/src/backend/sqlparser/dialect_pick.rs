//! Map our [`Dialect`] enum onto sqlparser's `Dialect` trait objects.

use crate::dialect::Dialect;
use sqlparser::dialect::{
  Dialect as SqlpDialect, GenericDialect, MsSqlDialect, MySqlDialect, PostgreSqlDialect, SQLiteDialect,
};

pub fn pick(d: Dialect) -> Box<dyn SqlpDialect> {
  match d {
    Dialect::Postgres => Box::new(PostgreSqlDialect {}),
    Dialect::MySql => Box::new(MySqlDialect {}),
    Dialect::SQLite => Box::new(SQLiteDialect {}),
    Dialect::MsSql => Box::new(MsSqlDialect {}),
    Dialect::Generic => Box::new(GenericDialect {}),
  }
}
