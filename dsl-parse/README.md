# dsl-parse

**SQL parser facade** for duck-sqllsp.

Parses SQL into a unified `Statement` representation, so the rest of
duck-sqllsp never has to care which backend produced it.

Two backends sit behind one API:

- **libpg_query** (default) — the real PostgreSQL grammar, via FFI. This
  is what gives PG-complete syntax coverage: `MERGE`, `FETCH FIRST`,
  `ON DELETE SET NULL (col)`, and the rest.
- **sqlparser** — pure Rust, no C toolchain, and the fallback for
  MySQL / SQLite / MSSQL.

```rust
use dsl_parse::{parse, Dialect};

let file = parse("SELECT id FROM users;", Dialect::Postgres);
assert_eq!(file.statements.len(), 1);
```

`split::split_statements` slices a document on top-level semicolons
before parsing, so one syntax error does not poison the whole file — it
handles dollar-quoted bodies, nested block comments, and escapes.

Part of [duck-sqllsp](https://github.com/gentleeduck/duck-sqllsp), a
multi-dialect SQL language server. This crate is usable on its own.

Licensed MIT.
