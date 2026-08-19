# dsl-hover

**Hover cards** for duck-sqllsp.

Builds the card shown when you hover: a compact `CREATE TABLE` with
indexes, triggers, policies and comments for a table; type and
nullability for a column; signature and docs for a function; and
three-valued-logic notes for `NULL`.

```rust
let card = dsl_hover::hover(source, offset, &catalog);
```

Resolution narrows from the cursor side, so hovering `schema.table`
gives the table rather than the schema.

Part of [duck-sqllsp](https://github.com/gentleeduck/duck-sqllsp), a
multi-dialect SQL language server. This crate is usable on its own.

Licensed MIT.
