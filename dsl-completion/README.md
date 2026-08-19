# dsl-completion

**Context-aware completion** for duck-sqllsp.

Decides what can legally follow the cursor across roughly 50 contexts —
`CREATE INDEX ... USING` and its opclass slot, `CREATE TRIGGER ...
EXECUTE FUNCTION`, `CREATE POLICY ... FOR / TO`, `ALTER COLUMN TYPE`,
`CALL <proc>`, PL/pgSQL local scope, JOIN target resolution, JSON path
keys, and dynamic SQL inside `EXECUTE`.

```rust
let items = dsl_completion::complete(source, &file, &scopes, &catalog, offset);
```

Read documentation through `Item::documentation()`, never
`documentation_md` directly: knowledge-base items leave that field empty
on purpose so the server can defer rendering to
`completionItem/resolve`.

Part of [duck-sqllsp](https://github.com/gentleeduck/duck-sqllsp), a
multi-dialect SQL language server. This crate is usable on its own.

Licensed MIT.
