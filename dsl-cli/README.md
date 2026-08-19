# dsl-cli

The **`duck-sqllsp` binary**.

```sh
duck-sqllsp server                     # LSP over stdio (the default)
duck-sqllsp doctor                     # check formatter, config, workspace, connection
duck-sqllsp rules                      # every diagnostic code with a summary
duck-sqllsp lint file.sql              # analysis without an editor in the loop
duck-sqllsp format file.sql --stdout
duck-sqllsp introspect --url postgres://user:pass@host/db
```

`doctor` is the one to reach for first when something is not working: it
reports whether the external formatter is on `PATH`, which config file
was found and whether it set anything, where the workspace root landed,
how many `.sql` files the offline scan reached, and whether the
connection answers. It exits non-zero only on real problems, so it also
works as a CI check.

`lint` runs the same analysis the server does, which separates a rule
bug from an editor integration problem.

Part of [duck-sqllsp](https://github.com/gentleeduck/duck-sqllsp), a
multi-dialect SQL language server.

Licensed MIT.
