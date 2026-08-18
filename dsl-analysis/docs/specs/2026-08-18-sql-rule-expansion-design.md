# SQL lint rule expansion: design

Status: approved, pending implementation.
Owner: dsl-analysis crate.
Date: 2026-08-18.

## Motivation

`dsl-analysis` has 654 distinct lint rule codes (`sql001`..`sql756`,
non-contiguous -- some early codes were retired). The registry is deep on
common query/DDL smells and dialect portability, but a grep sweep of every
file in `dsl-analysis/src/rules/` turned up zero coverage for several
PostgreSQL surface areas that show up in real migrations and hand-written
queries:

- Declarative table partitioning (`PARTITION BY`, `ATTACH`/`DETACH
  PARTITION`).
- Exclusion constraints (`EXCLUDE USING`).
- Domains and composite types.
- Multirange types, `CREATE STATISTICS`, `UNIQUE NULLS NOT DISTINCT`.
- SQL-standard JSON functions (`JSON_TABLE`, `IS JSON`, `JSON_VALUE`,
  `JSON_QUERY`, `JSON_EXISTS`).
- Logical replication (`CREATE PUBLICATION`/`SUBSCRIPTION`).

Verification command (run from repo root):

```sh
grep -riE '"?partition|exclude using|multirange|json_table|nulls not distinct|publication|subscription|statistics object|domain|composite type' \
  dsl-analysis/src/rules/*.rs -l
```

The handful of hits it returns are false positives on unrelated
substrings (`partition_by_constant.rs` covers window `PARTITION BY`, not
table partitioning; `gist_on_scalar.rs` matches "exclude" only in a
comment; etc.) -- confirmed by reading each file.

Beyond pure gap-filling, several existing rule *families* have an obvious
next-level sibling that the current rule set doesn't have yet (e.g.
`left_join_defeated_by_where` has no `FULL OUTER JOIN` counterpart,
`recursive_cte_no_union` doesn't cover the recursive term's other hard PG
restrictions, `jsonb_build_object_duplicate_key` doesn't cover a null
key). This design folds those in alongside the pure gaps.

## Scope decisions (from brainstorming)

- Focus areas: advanced DDL/schema objects, SQL/JSON + JSONB depth,
  complex query-shape correctness, PL/pgSQL + dynamic SQL control flow --
  all four selected.
- Architecture: **stay within the current pattern.** Every rule here is a
  self-contained per-statement check (text/token scan, optionally backed
  by `ct_model`, `clause_scan`, `typing`, or `dsl-resolve` scope/catalog
  lookups) -- exactly like all 654 existing rules. No cross-statement or
  session-level modeling gets added by this work. A few ideas that
  *would* need that are listed under Deferred instead of silently
  dropped.
- Delivery shape: small themed batches, one commit per batch, mirroring
  the existing `feat(analysis): flag <theme>` commit history.
- This plan touches `dsl-analysis` only. No changes to `dsl-completion`,
  `dsl-hover`, `dsl-format`, `dsl-server`, or the VS Code extension.

## Numbering

Next free code is `sql757`. Codes below are assigned in the batch order
this doc proposes (see Sequencing) so the plan has concrete anchors. If
the implementation order changes, renumber to stay contiguous from
`sql757` -- do not leave gaps or reuse a number assigned to a dropped
rule.

Severities are provisional (Error / Warning / Hint per the existing
`Severity` enum) based on the SQL semantics described; each rule's doc
comment must confirm the exact PostgreSQL error/SQLSTATE (or confirm
there isn't one, downgrading to Warning/Hint) during implementation,
matching how existing rules cite e.g. 42883 or 22023.

## Rule inventory

### Batch 1 -- Partitioning DDL (sql757-sql762)

Zero prior coverage. All checkable from a single `CREATE TABLE` /
`ALTER TABLE` statement.

| Code | Rule | Severity | Flags |
| --- | --- | --- | --- |
| sql757 | `partition_by_no_key_in_pk` | Error | `PRIMARY KEY`/`UNIQUE` on a partitioned table that omits a partition-key column -- PG requires every unique constraint to include all partitioning columns. |
| sql758 | `partition_range_bound_reversed` | Error | `FOR VALUES FROM (x) TO (y)` where the lower literal bound is not less than the upper -- PG rejects an empty partition range. |
| sql759 | `partition_by_expression_volatile` | Error | `PARTITION BY RANGE/LIST/HASH (expr)` where `expr` calls a known-volatile builtin (`now()`, `random()`, ...) -- partition key expressions must be immutable. |
| sql760 | `attach_partition_no_for_values` | Error | `ALTER TABLE ... ATTACH PARTITION x` missing `FOR VALUES` (and not `DEFAULT`) -- required clause. |
| sql761 | `detach_partition_concurrently_in_tx` | Error | `DETACH PARTITION ... CONCURRENTLY` inside an explicit `BEGIN`/`COMMIT` block -- PG forbids this combination, same detection shape as the existing `drop_index_concurrently_in_tx`/`reindex_in_tx`. |
| sql762 | `hash_partition_modulus_remainder` | Error | `FOR VALUES WITH (MODULUS m, REMAINDER r)` where `r >= m` -- PG requires remainder strictly less than modulus. |

### Batch 2 -- SQL-standard JSON functions (sql763-sql768)

Zero prior coverage (PG17 surface). The existing `jsonb_*` rules cover
PG's native jsonb operators/functions, not the SQL-standard constructors.

| Code | Rule | Severity | Flags |
| --- | --- | --- | --- |
| sql763 | `json_exists_bad_path` | Error | `JSON_EXISTS(doc, 'literal')` where the literal path string doesn't start with `$` -- invalid jsonpath. |
| sql764 | `json_value_returning_without_on_error` | Hint | `JSON_VALUE(... RETURNING <type>)` narrowing the type with no `ON ERROR` clause -- unhandled runtime error risk on mismatch. |
| sql765 | `json_query_wrapper_conflict` | Error | `JSON_QUERY(... WITH WRAPPER ... OMIT QUOTES)` -- `OMIT QUOTES` is disallowed together with a wrapper. |
| sql766 | `json_table_no_columns` | Warning | `JSON_TABLE(doc, path COLUMNS ())` -- empty output-column list. |
| sql767 | `is_json_redundant_with_jsonb_column` | Hint | `col IS JSON` where the catalog already types `col` as `json`/`jsonb` -- always true. |
| sql768 | `is_json_scalar_object_conflict` | Warning | `col IS JSON OBJECT AND col IS JSON ARRAY` (or similar contradictory pair joined by `AND`) -- always false. |

### Batch 3 -- Recursive CTE hard restrictions (sql769-sql773)

Extends the existing `recursive_cte_no_union` / `cte_missing_recursive`
pair. Each of these encodes a documented, hard PostgreSQL restriction on
the recursive term, not a style preference.

| Code | Rule | Severity | Flags |
| --- | --- | --- | --- |
| sql769 | `recursive_cte_cycle_column_reused` | Error | `SEARCH`/`CYCLE` clause names a working column (`ordercol`, cycle-mark, path) that collides with a column already produced by the CTE's own `SELECT` list. |
| sql770 | `recursive_cte_missing_base_union` | Error | The recursive term references the CTE name more than once -- PG allows exactly one self-reference. |
| sql771 | `recursive_term_has_aggregate` | Error | The recursive branch contains an aggregate function -- disallowed in a recursive term. |
| sql772 | `recursive_term_has_order_or_limit` | Error | The recursive branch contains `ORDER BY`/`LIMIT`/`DISTINCT` -- disallowed in a recursive term. |
| sql773 | `recursive_cte_outer_join_recursive_side` | Error | The self-reference sits on the nullable side of an outer join inside its own recursive term -- disallowed. |

### Batch 4 -- Exclusion constraints (sql774-sql776)

Zero prior coverage.

| Code | Rule | Severity | Flags |
| --- | --- | --- | --- |
| sql774 | `exclude_using_no_operator` | Error | `EXCLUDE USING gist (col WITH)` -- operator missing after `WITH`. |
| sql775 | `exclude_using_btree_index_type` | Error | `EXCLUDE USING btree (...)` -- btree does not support exclusion constraints. |
| sql776 | `exclude_using_single_column_eq` | Hint | `EXCLUDE USING gist (col WITH =)` on one column -- functionally a weaker, slower `UNIQUE`. |

### Batch 5 -- Domains and composite types (sql777-sql779)

Zero prior coverage.

| Code | Rule | Severity | Flags |
| --- | --- | --- | --- |
| sql777 | `domain_check_references_value_missing` | Warning | `CREATE DOMAIN ... CHECK (expr)` where `expr` never references `VALUE` -- evaluates to the same result for every input. |
| sql778 | `domain_default_violates_check` | Warning | `DEFAULT <literal>` that plainly fails an adjacent `CHECK (VALUE ...)` in the same statement. Verify PG's validation timing for `CREATE DOMAIN` during implementation; may upgrade to Error. |
| sql779 | `composite_type_dup_field` | Error | `CREATE TYPE ... AS (a int, a text)` -- duplicate field name, sibling to `create_table_dup_column`. |

### Batch 6 -- jsonpath / jsonb operator depth (sql780-sql783)

Extends the existing jsonb rule family.

| Code | Rule | Severity | Flags |
| --- | --- | --- | --- |
| sql780 | `jsonb_path_exists_static_false` | Warning | `jsonb_path_exists(doc, '$.a ? (1 == 2)')` -- literal-vs-literal predicate inside the jsonpath filter is always false. |
| sql781 | `jsonb_array_length_on_object_literal` | Error | `jsonb_array_length('{"a":1}'::jsonb)` -- literal is an object, guaranteed 22023. |
| sql782 | `jsonb_minus_integer_on_object` | Error | `jsonb_col - 5` where context shows an object, not array -- integer-index delete is array-only. |
| sql783 | `jsonb_build_object_null_key` | Error | `jsonb_build_object(NULL, 1, ...)` -- literal null key, PG rejects at runtime. Sibling to the existing `jsonb_build_object_duplicate_key`. |

### Batch 7 -- GROUPING SETS / CUBE / ROLLUP depth (sql784-sql786)

Extends the existing `rollup_cube_single`.

| Code | Rule | Severity | Flags |
| --- | --- | --- | --- |
| sql784 | `grouping_sets_duplicate_set` | Hint | `GROUPING SETS ((a,b), (a,b))` -- literal duplicate set. |
| sql785 | `grouping_function_arg_not_in_group_by` | Error | `GROUPING(x)` where `x` is absent from the `GROUP BY` list -- PG 42803. |
| sql786 | `cube_rollup_empty_column_list` | Warning | `ROLLUP ()` / `CUBE ()` with nothing inside -- degenerates to a single group. |

### Batch 8 -- Correlated subquery / join depth (sql787-sql789)

| Code | Rule | Severity | Flags |
| --- | --- | --- | --- |
| sql787 | `correlated_subquery_select_no_limit1_no_agg` | Warning | Scalar subquery in the `SELECT` list, correlated to the outer query, with no aggregate and no `LIMIT 1` -- risk of "more than one row returned by a subquery used as an expression". |
| sql788 | `lateral_join_references_later_table` | Error | `LATERAL` subquery references an alias introduced later in the same `FROM`/`JOIN` list -- out of scope at that point. |
| sql789 | `full_outer_join_where_defeats` | Warning | `WHERE` predicate on the join's nullable side silently turns a `FULL OUTER JOIN` into an inner join. Sibling to the existing `left_join_defeated_by_where`. |

### Batch 9 -- Statistics objects and NULLS NOT DISTINCT (sql790-sql792)

| Code | Rule | Severity | Flags |
| --- | --- | --- | --- |
| sql790 | `unique_nulls_distinct_redundant` | Hint | `UNIQUE NULLS NOT DISTINCT` on a column that's also `NOT NULL` -- NULLs can't occur, clause is a no-op. |
| sql791 | `create_statistics_no_columns` | Error | `CREATE STATISTICS name (ndistinct) ON` with fewer than 2 columns/expressions -- meaningless for multi-column statistics kinds. |
| sql792 | `create_statistics_dup_column` | Warning | Same column named twice in `CREATE STATISTICS ... ON (a, a)`. |

### Batch 10 -- MERGE depth (sql793-sql794)

Extends the existing `merge_missing_when`.

| Code | Rule | Severity | Flags |
| --- | --- | --- | --- |
| sql793 | `merge_when_matched_unreachable` | Warning | An unconditional `WHEN MATCHED THEN` clause appears before another, conditioned `WHEN MATCHED AND ...` -- the later branch is dead code. |
| sql794 | `merge_insert_references_target` | Error | `WHEN NOT MATCHED THEN INSERT ... VALUES (target.col, ...)` -- referencing the target alias when no target row exists. |

### Batch 11 -- Logical replication (sql795-sql797)

Zero prior coverage.

| Code | Rule | Severity | Flags |
| --- | --- | --- | --- |
| sql795 | `publication_for_all_tables_and_list` | Error | `CREATE PUBLICATION p FOR ALL TABLES, TABLE x` -- contradictory combination. |
| sql796 | `subscription_no_slot_name_with_create_false` | Error | `CREATE SUBSCRIPTION ... WITH (create_slot = false)` with no explicit `slot_name` -- PG can't infer which slot to use. |
| sql797 | `publication_duplicate_table` | Error | Same table listed twice in `CREATE PUBLICATION ... FOR TABLE a, a`. |

### Batch 12 -- PL/pgSQL loop and exception control flow (sql798-sql800)

Extends the existing `exit_outside_loop` / `unreachable_after_return`.

| Code | Rule | Severity | Flags |
| --- | --- | --- | --- |
| sql798 | `loop_no_exit` | Warning | A bare `LOOP ... END LOOP` (not `FOR`/`WHILE`) whose body contains no `EXIT`/`RETURN`/`RAISE` anywhere -- guaranteed infinite loop. |
| sql799 | `for_loop_variable_shadows_column` | Hint | `FOR i IN ...` loop variable name shadows a column of a table referenced in the same function body. |
| sql800 | `exception_block_swallows_all` | Warning | `EXCEPTION WHEN OTHERS THEN` with an empty (or comment-only) body -- silently discards every error. |

### Batch 13 -- Dynamic SQL arity (sql801-sql802)

Extends the existing `execute_string_concat` / `format_no_placeholders` /
`raise_arg_count` family (placeholder-vs-argument counting is an
established pattern in this codebase).

| Code | Rule | Severity | Flags |
| --- | --- | --- | --- |
| sql801 | `execute_using_arg_count_mismatch` | Error | `EXECUTE format('...%s...%I...') USING a, b` -- placeholder count in the `format()` call doesn't match the `USING` argument count. |
| sql802 | `execute_into_arity_mismatch` | Warning | `EXECUTE '<literal SELECT>' INTO a, b` -- statically-known output column count doesn't match the `INTO` target list. |

### Batch 14 -- Perf and dead-variable smells (sql803-sql804)

| Code | Rule | Severity | Flags |
| --- | --- | --- | --- |
| sql803 | `raise_notice_in_hot_loop` | Hint | Unconditional `RAISE NOTICE` inside a `FOR`/`WHILE`/`LOOP` body -- per-row notice in bulk operations. |
| sql804 | `variable_declared_unused` | Hint | A `DECLARE x type;` variable never referenced anywhere in the function body. |

## Sequencing

Risk-ordered: mechanical/literal-evaluable checks and well-documented
hard PG restrictions first (batches 1-3), then the remaining gap-filling
batches, ending with the ones needing the most false-positive care
(control-flow analysis in batch 12, arity heuristics in batch 13).
Batch order as numbered above is the intended implementation order.

## Per-batch implementation process

Matches the existing convention exactly (observed directly in
`agg_missing_delimiter.rs` and `dsl-analysis/src/rules/mod.rs`):

1. New file per rule: `dsl-analysis/src/rules/<name>.rs` with a
   `//! sqlNNN: <what/why/runtime consequence>` header, `pub struct
   Rule;` implementing `LintRule` (`code`, `default_severity`, `check`).
2. Register in `dsl-analysis/src/rules/mod.rs`: append `pub mod
   <name>;` near the top and `Box::new(<name>::Rule),` inside `all()`.
   Existing order is insertion-order, not alphabetical -- append, don't
   reorder.
3. If a rule should not fire on non-Postgres buffers (e.g. it encodes a
   PG-only restriction that's meaningless/wrong on MySQL), add its code
   to the relevant `*_PORT_CODES` const in `dsl-analysis/src/lib.rs`.
   Default assumption: everything in this plan is Postgres-specific
   syntax (partitioning, JSON/SQL standard functions, PL/pgSQL, logical
   replication) and PG is already the default dialect rules run under,
   so most of these need no dialect-skip entry. Confirm per rule.
4. Tests in `dsl-analysis/tests/rules_<category>.rs` (new files -- see
   Testing strategy below), minimum one "flags the bad case" + one
   "quiet on the legitimate case" per rule, following the existing
   `sql001_unresolved_table` / `sql001_quiet_when_table_exists` pattern
   (reuse the `diags()`/`cat()` helpers already defined in
   `dsl-analysis/tests/rules.rs`, or a local equivalent in the new file).
5. `cargo test --workspace --release` and `cargo clippy --workspace
   --all-features --release -- -D warnings` both clean before commit.
6. Commit as `feat(analysis): flag <theme>`, one batch per commit --
   mirrors the last five commits in this repo's history exactly.

## Testing strategy

`dsl-analysis/tests/rules.rs` is already ~29k lines. Fourteen more
batches appending to that one file is a needless merge-conflict magnet.
New batches get their own `dsl-analysis/tests/rules_<category>.rs` file
(e.g. `rules_partitioning.rs`, `rules_json_standard.rs`,
`rules_plpgsql_control_flow.rs`) instead. This is purely additive --
`rules.rs` and its existing tests are not touched or migrated.

## Out of scope / deferred

Not part of this plan; each of these needs cross-statement or session-
level modeling, which the architecture decision above explicitly
excludes. Listed so they aren't silently lost:

- Partition-bound overlap detection across multiple `ATTACH PARTITION`
  statements in the same file (needs a cross-statement bound-interval
  model).
- Full CTE dependency-graph cycle detection beyond the single-statement
  recursive-CTE checks in batch 3.
- Self-join alias ambiguity resolution across 3+ table chains.

Also explicitly not part of this plan:

- No changes to `dsl-completion`, `dsl-hover`, `dsl-format`,
  `dsl-server`, or the VS Code extension.
- No refactor of the existing 654 rules.
- Two unrelated doc-drift issues noticed during research, left alone
  unless separately requested: `CONTRIBUTING.md` in this repo currently
  contains content for the sibling `duck-mc` project, not this one; and
  the README/CHANGELOG "300+ lint rules" figure undercounts the actual
  654.

## Success criteria

- All 48 rules (sql757-sql804) implemented, each with passing tests and
  clean clippy.
- Each batch lands as its own commit, independently reviewable.
- Zero behavior change to the existing 654 rules.
- Every new rule has at least one "quiet on the legitimate case" test,
  not just a "flags the bad case" test.
