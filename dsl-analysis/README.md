# dsl-analysis

**SQL lint engine** for duck-sqllsp.

701 diagnostics covering schema correctness, transaction safety, query
smells, migration footguns, and vendor mismatches - each with a range
aimed at the offending token rather than the whole statement.

```rust
let file = dsl_parse::parse(sql, dsl_parse::Dialect::Postgres);
let scopes = dsl_resolve::resolve_with_source(&file.statements, sql);
let diagnostics = dsl_analysis::run(sql, &file, &scopes, &catalog);
```

Every rule is one module in `src/rules/`, opening with a
`//! sqlNNN: <summary> -- <detail>` doc comment that is the single source
of truth for the [rule reference](https://github.com/gentleeduck/duck-sqllsp/blob/master/dsl-analysis/docs/rules.md) - regenerate it with
`cargo test -p dsl-analysis --test rule_reference -- --ignored`.

Rules that need a schema stay quiet without one, so an offline buffer
does not light up with false positives for tables you have not created
yet.

Part of [duck-sqllsp](https://github.com/gentleeduck/duck-sqllsp), a
multi-dialect SQL language server. This crate is usable on its own.

Licensed MIT.
