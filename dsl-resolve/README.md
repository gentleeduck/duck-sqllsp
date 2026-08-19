# dsl-resolve

**Name resolution and scope** for duck-sqllsp.

Works out what an identifier refers to: which table a column belongs to,
what an alias points at, what a CTE exposes.

```rust
use dsl_resolve::resolve_with_source;

let file = dsl_parse::parse("SELECT u.id FROM users u", dsl_parse::Dialect::Postgres);
let scopes = resolve_with_source(&file.statements, "SELECT u.id FROM users u");
```

Handles `FROM` / `JOIN` / `LATERAL` scope, alias chains, and CTE column
lists. Completion, hover, and the analysis rules all read the scopes it
produces rather than re-deriving them.

Part of [duck-sqllsp](https://github.com/gentleeduck/duck-sqllsp), a
multi-dialect SQL language server. This crate is usable on its own.

Licensed MIT.
