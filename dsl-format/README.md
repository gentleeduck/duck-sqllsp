# dsl-format

**SQL formatter** for duck-sqllsp.

Two passes that cooperate:

1. **External reflow** — shells out to the
   [`sql-formatter`](https://github.com/sql-formatter-org/sql-formatter)
   CLI (v15+) for keyword casing, wrapping, and general layout.
2. **Alignment** — a DataGrip-style `CREATE TABLE` pass that pads column
   name / type / `NOT NULL` / `DEFAULT` into aligned columns, plus a
   PL/pgSQL block indenter.

```rust
use dsl_format::{format, FormatterStyle, CreateTableStyle};

let out = format("select 1", &FormatterStyle::default(), &CreateTableStyle::default());
```

**If the `sql-formatter` binary is not on `PATH`, only the second pass
runs.** Output still changes, just far less — which is invisible unless
you know to look for it.

Part of [duck-sqllsp](https://github.com/gentleeduck/duck-sqllsp), a
multi-dialect SQL language server. This crate is usable on its own.

Licensed MIT.
