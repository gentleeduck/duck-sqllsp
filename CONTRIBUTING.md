# Contributing to duck-sqllsp

Thanks for the interest. This covers the workflow, the layout, and the
one checklist that matters most — adding a lint rule.

## Repo layout

Eleven crates, roughly in dependency order:

```
duck-sqllsp/
|- dsl-parse/       SQL parser: libpg_query primary, sqlparser fallback
|- dsl-catalog/     schema model: tables, columns, constraints, indexes, ...
|- dsl-knowledge/   static keyword / type / function reference
|- dsl-resolve/     name resolution, FROM / JOIN / LATERAL scope, CTEs
|- dsl-format/      formatter: sql-formatter reflow + DataGrip alignment
|- dsl-analysis/    lint rule engine (701 rules)
|- dsl-completion/  context-aware completion engine
|- dsl-hover/       hover cards
|- dsl-conn/        live PG / MySQL / SQLite introspection (sqlx)
|- dsl-server/      tower-lsp server, all LSP request handlers
`- dsl-cli/         `duck-sqllsp` binary
```

## Build and test

```sh
cargo build --workspace
cargo test  --workspace --all-features
cargo clippy --workspace --all-features -- -D warnings
cargo fmt --all
```

Those four are exactly what CI runs, on Linux, macOS, and Windows. There
is no npm/pnpm step; the optional `sql-formatter` binary is a runtime
dependency of the formatter, not a build dependency.

`duck-sqllsp doctor` reports what the server can see in a given
directory — handy when a test behaves differently from the editor.

## Adding a lint rule

One file per rule, in `dsl-analysis/src/rules/`.

**1. Write the rule.** The module doc comment is not optional — it is
the source of truth for the rule reference:

```rust
//! sql805: `<the construct>` -- what goes wrong, and what to do instead.
//! Longer explanation, PG error codes, related rules.

use crate::{Diagnostic, LintRule, Severity};

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str { "sql805" }
  fn default_severity(&self) -> Severity { Severity::Warning }
  fn check(&self, source: &str, stmt: &Statement, scope: &Scope,
           catalog: &Catalog, out: &mut Vec<Diagnostic>) { /* ... */ }
}
```

The text before ` -- ` becomes the one-line summary in
`duck-sqllsp rules`; the whole paragraph becomes the entry in
[`dsl-analysis/docs/rules.md`](dsl-analysis/docs/rules.md). Write it for
someone who just saw the code in their editor and has no idea what it
means.

**2. Take the next free code.** `duck-sqllsp rules` lists what is taken.
Codes are never reused.

**3. Register it** in `dsl-analysis/src/rules/mod.rs` — both the
`pub mod` line and a `Box::new(your_rule::Rule),` in `all()`. A rule
that is implemented but not registered never fires; `sql129` is
deliberately in that state, with a comment explaining why.

**4. Test it.** Rule tests live in `dsl-analysis/tests/`, grouped by
theme. Cover the negative case — the shape that looks similar but must
*not* fire — as well as the positive one. False positives are worse than
gaps here: a noisy rule gets the whole server turned off.

**5. Regenerate the rule reference.**

```sh
cargo test -p dsl-analysis --test rule_reference -- --ignored
```

This rewrites `src/rules/titles.rs` and `docs/rules.md` from the doc
comments. CI fails if you skip it.

## Aim diagnostics narrowly

Point the range at the offending token, not the whole statement. A
warning that highlights forty lines is a warning people stop reading.

## Style

- rustfmt config is in `rustfmt.toml` — 2-space indent, 120 columns.
- Comments explain *why*, not *what*. The code already says what.
- ASCII in prose: `--` not an em-dash, straight quotes.
- Commit messages: `type(scope): summary`, then the reasoning. Explain
  what was wrong and why this fixes it, not just what changed.

## PR checklist

- [ ] `cargo test --workspace --all-features`
- [ ] `cargo clippy --workspace --all-features -- -D warnings`
- [ ] `cargo fmt --all`
- [ ] Rule reference regenerated, if you touched a rule doc comment
- [ ] `CHANGELOG.md` updated under `[Unreleased]` for user-visible changes
- [ ] Docs updated if you changed configuration or behaviour

## Reporting bugs

Include the output of `duck-sqllsp doctor` and `duck-sqllsp version`,
your editor, and the smallest SQL that reproduces it. For a wrong or
missing diagnostic, `duck-sqllsp lint file.sql` runs the same analysis
without the editor in the loop, which separates a rule bug from an
integration problem.
