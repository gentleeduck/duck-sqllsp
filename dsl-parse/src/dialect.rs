//! SQL dialect selector.

/// SQL dialect to parse against. Defaults to `Postgres` because that is
/// duck-sqllsp's primary target.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Dialect {
  #[default]
  Postgres,
  MySql,
  SQLite,
  /// Microsoft SQL Server / Sybase / T-SQL.
  MsSql,
  Generic,
}
