# dsl-server

**LSP wire layer** for duck-sqllsp.

The `tower-lsp` server: 25 request handlers, the document store, the
config loader, and the catalog refresh loop. Everything it answers with
comes from the sibling crates; this one owns protocol, state, and
lifecycle.

Capabilities include completion (with `completionItem/resolve`), hover,
pull *and* push diagnostics, document and range formatting, semantic
tokens with modifiers and range support, document links, inlay hints,
code lenses, code actions, call hierarchy, and incremental document
sync.

Documentation:

- [Configuration reference](https://github.com/gentleeduck/duck-sqllsp/blob/master/dsl-server/docs/configuration.md) - every setting, its
  default, and what it changes.
- [Editor setup](https://github.com/gentleeduck/duck-sqllsp/blob/master/dsl-server/docs/editors.md) - VS Code, neovim, Helix, Emacs,
  Sublime, and the generic stdio contract.
- [Troubleshooting](https://github.com/gentleeduck/duck-sqllsp/blob/master/dsl-server/docs/troubleshooting.md) - start with
  `duck-sqllsp doctor`.

Part of [duck-sqllsp](https://github.com/gentleeduck/duck-sqllsp), a
multi-dialect SQL language server. This crate is usable on its own.

Licensed MIT.
