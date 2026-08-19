# dsl-knowledge

**SQL knowledge base** for duck-sqllsp.

Curated reference data for SQL keywords, types, and built-in functions —
signature, description, example, and a link to the PostgreSQL docs.

```rust
let entry = dsl_knowledge::lookup("coalesce").expect("known function");
let markdown = dsl_knowledge::render_markdown(entry);
```

Keywords and types are keyed uppercase, functions lowercase; `lookup`
tries all three. Rendering is deliberately separate from the tables:
duck-sqllsp defers it to `completionItem/resolve`, because rendering
every entry per keystroke costs about a millisecond and is almost
entirely wasted.

Part of [duck-sqllsp](https://github.com/gentleeduck/duck-sqllsp), a
multi-dialect SQL language server. This crate is usable on its own.

Licensed MIT.
