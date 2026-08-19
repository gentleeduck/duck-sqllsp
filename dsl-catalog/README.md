# dsl-catalog

**Schema model** for duck-sqllsp.

The schema representation everything else reads: tables, columns,
constraints (inline and table-level), indexes, triggers, policies,
sequences, types, and functions.

Populated either from a live database (`dsl-conn`) or derived from
`CREATE TABLE` statements in the files you have open, so the same shape
serves online and offline use.

`display_type` strips the schema qualification PostgreSQL reports, so
hover and completion show `int4` rather than `pg_catalog.int4`:

```rust
assert_eq!(dsl_catalog::display_type("pg_catalog.int4"), "int4");
```

Part of [duck-sqllsp](https://github.com/gentleeduck/duck-sqllsp), a
multi-dialect SQL language server. This crate is usable on its own.

Licensed MIT.
