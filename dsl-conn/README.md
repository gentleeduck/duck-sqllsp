# dsl-conn

**Live catalog introspection** for duck-sqllsp.

Reads schema from a running database into a `dsl_catalog::Catalog`.
PostgreSQL, MySQL / MariaDB, and SQLite, over `sqlx`.

```rust
let driver = dsl_conn::build(&connection_spec)?;
let catalog = driver.introspect().await?;
```

The driver is chosen from the URL scheme, so it must be present —
`postgres://`, `mysql://`, `sqlite:` — a bare path is not enough.

**Introspection is read-only.** It issues nothing but `SELECT` against
`information_schema` / `pg_catalog`, so the role it connects as needs no
write access.

Part of [duck-sqllsp](https://github.com/gentleeduck/duck-sqllsp), a
multi-dialect SQL language server. This crate is usable on its own.

Licensed MIT.
