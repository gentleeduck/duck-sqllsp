# Try it out

Open any `.sql` file (or create one and choose `SQL` as the
language). You should immediately get:

- **Completion** as you type (tables, columns, functions, types,
  privileges, role names...).
- **Hover** on any identifier (keywords, columns, tables, functions,
  sequences, extensions, RLS policies...).
- **Diagnostics** for unresolved tables, unknown columns, missing
  ON in JOIN, NULL in NOT NULL column, sql169 unknown role, and
  ~150 more rules.
- **Inlay hints** -- column-name chips inside `INSERT VALUES`,
  predicate guesses next to JOIN-without-ON, type hints next to
  INSERT literals.
- **Code actions** -- extract subquery to CTE, EXISTS -> LATERAL,
  IN -> ANY, EXPLAIN ANALYZE wrap, expand `SELECT *` to columns.
- **Snippets** -- type `sfw`, `cte`, `ctab`, `cfn`, `upsert`, etc
  for ready-made scaffolds.

## Snippet shortcuts

| Prefix | Expands to |
|---|---|
| `sfw`    | SELECT/FROM/WHERE |
| `sjoin`  | SELECT with JOIN |
| `cte`    | WITH ... AS (...) SELECT |
| `rcte`   | WITH RECURSIVE for graphs |
| `ctab`   | CREATE TABLE with id + timestamps |
| `cfn`    | CREATE FUNCTION (plpgsql) skeleton |
| `ctrig`  | row-level UPDATE trigger + fn pair |
| `cidx`   | CREATE INDEX |
| `ins`    | INSERT INTO ... VALUES |
| `upsert` | INSERT ON CONFLICT DO UPDATE |
| `upd`    | UPDATE ... RETURNING |
| `del`    | DELETE ... RETURNING |
