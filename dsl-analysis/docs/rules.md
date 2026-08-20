# Lint rule reference

Every diagnostic duck-sqllsp can emit. Generated from each rule's own doc
comment — see `dsl-analysis/src/rules/`.

Silence or re-level any of these by code:

```toml
[duck_sqllsp.rules]
sql015 = "off"      # off / ignore / none
sql001 = "hint"     # or error / warning / info
```

`duck-sqllsp rules` prints the same list from the command line.
`duck-sqllsp rules --search partition` narrows it, and `--json`
emits it machine-readably.

701 rules.

## Index

| Code | Summary |
| --- | --- |
| [`sql001`](#sql001--table-referenced-by-from--join--update--delete--insert-into-does-not-exist-in-the-catalog) | table referenced by FROM / JOIN / UPDATE / DELETE / INSERT INTO does not exist in the catalog |
| [`sql002`](#sql002--column-reference-does-not-exist-in-any-in-scope-table) | column reference does not exist in any in-scope table |
| [`sql003`](#sql003--unqualified-column-reference-exists-in-more-than-one-in-scope-table-the-user-must-qualify-it) | unqualified column reference exists in more than one in-scope table; the user must qualify it |
| [`sql010`](#sql010--union--intersect--except-column-count-mismatch) | UNION / INTERSECT / EXCEPT column-count mismatch |
| [`sql013`](#sql013--update-or-delete-without-a-where-clause) | UPDATE or DELETE without a WHERE clause |
| [`sql014`](#sql014--implicit-cross-join) | implicit cross join |
| [`sql015`](#sql015--comparison-with-null-using--or--or--always-yields-null-the-user-almost-always-meant-is-null) | comparison with NULL using `=` or `<>` (or `!=`). Always yields NULL; the user almost always meant `IS NULL`... |
| [`sql016`](#sql016--insert-into-t-select--is-arity-fragile) | `INSERT INTO t SELECT *` is arity-fragile |
| [`sql017`](#sql017--select-mixes-aggregates-with-bare-column-references-but-the-bare-columns-are-not-all-listed-in-group-by) | SELECT mixes aggregates with bare column references but the bare columns are not all listed in GROUP BY |
| [`sql018`](#sql018--not-in-subquery-is-dangerous-when-the-subquery-can-return-null-postgres-treats-x-not-in-null-as) | `NOT IN (subquery)` is dangerous when the subquery can return NULL. Postgres treats `x NOT IN (NULL)` as... |
| [`sql020`](#sql020--deprecated--non-recommended-function-call) | deprecated / non-recommended function call |
| [`sql021`](#sql021--prefer-the-declared-alias-over-the-bare-table-name) | prefer the declared alias over the bare table name |
| [`sql030`](#sql030--trigger-function-body-has-no-return) | trigger function body has no RETURN |
| [`sql031`](#sql031--return-literal-type-doesnt-match-declared-returns-type-catches-the-easy-literal-return-mismatches) | `RETURN <literal>` type doesn't match declared `RETURNS <type>`. Catches the easy literal-return mismatches... |
| [`sql032`](#sql032--bare-return-inside-a-function-that-declares-a-non-void-return-type) | bare `RETURN;` inside a function that declares a non-void return type |
| [`sql036`](#sql036--raise-exception-or-noticewarningetc-format-string--placeholder-count-doesnt-match-the-supplied) | `RAISE EXCEPTION` (or NOTICE/WARNING/etc.) format string `%` placeholder count doesn't match the supplied... |
| [`sql037`](#sql037--select) | `SELECT |
| [`sql038`](#sql038--insert-into-t-a-b-values-1) | `INSERT INTO t (a, b) VALUES (1)` |
| [`sql039`](#sql039--insert-into-t-col1-col2-values-lit1-lit2-literal-types-must-match-the-target-column-types) | `INSERT INTO t (col1, col2) VALUES (lit1, lit2)` literal types must match the target column types |
| [`sql040`](#sql040--an-immutable-function-body-calls-a-known-volatile-built-in) | an `IMMUTABLE` function body calls a known `VOLATILE` built-in |
| [`sql041`](#sql041--language-sql-function-body-references-new-or-old) | `LANGUAGE sql` function body references `NEW` or `OLD` |
| [`sql042`](#sql042--update-table-set-col---where-col-is-not-in-the-target-tables-catalog-definition) | `UPDATE <table> SET <col> = ...` where `<col>` is not in the target table's catalog definition |
| [`sql043`](#sql043--delete-from-tbl-without-where-inside-a-function-body) | `DELETE FROM <tbl>` without WHERE inside a function body |
| [`sql044`](#sql044--exit--continue-used-outside-a-loop--while--for-block) | `EXIT` / `CONTINUE` used outside a LOOP / WHILE / FOR block |
| [`sql045`](#sql045--unreachable-code-after-an-unconditional-return-or-raise-exception-postgres-wont-error-on-dead-code-but) | unreachable code after an unconditional `RETURN` or `RAISE EXCEPTION`. Postgres won't error on dead code but... |
| [`sql046`](#sql046--create-table-without-a-primary-key-heap-tables-without-a-primary-key-cause-replication-orm-and-audit) | `CREATE TABLE` without a PRIMARY KEY. Heap tables without a primary key cause replication, ORM, and audit... |
| [`sql048`](#sql048--insert-into-t-values--without-a-column-list-positional-insert-works-but-is-fragile) | `INSERT INTO t VALUES (...)` without a column list. Positional INSERT works but is fragile |
| [`sql050`](#sql050--a-column-or-table-identifier-in-create-table-matches-a-pg-reserved-keyword-postgres-still-accepts-it-but) | a column or table identifier in CREATE TABLE matches a PG reserved keyword. Postgres still accepts it but... |
| [`sql051`](#sql051--limit-without-order-by-produces-non-deterministic-rows) | `LIMIT` without `ORDER BY` produces non-deterministic rows |
| [`sql052`](#sql052--like-plain-string) | `LIKE 'plain string'` |
| [`sql054`](#sql054--where-x--true--where-x--false) | `WHERE x = true` / `WHERE x = false` |
| [`sql055`](#sql055--where-single-condition) | `WHERE (single condition)` |
| [`sql056`](#sql056--union-deduplicates-is-often-slower-than-union-all-and-used-by-mistake) | `UNION` (deduplicates) is often slower than `UNION ALL` and used by mistake |
| [`sql058`](#sql058--case-when) | `CASE WHEN |
| [`sql061`](#sql061--bare-null-inside-values--without-an-explicit-cast) | bare `NULL` inside `VALUES (...)` without an explicit cast |
| [`sql062`](#sql062--savepoint-x-declared-but-never-released-or-rolled-back-to) | `SAVEPOINT x` declared but never `RELEASE`d (or rolled back to) |
| [`sql064`](#sql064--) | ` |
| [`sql065`](#sql065--group-by-1-2) | `GROUP BY 1, 2` |
| [`sql068`](#sql068--begin--commit-pair-wrapping-a-single-statement) | BEGIN / COMMIT pair wrapping a single statement |
| [`sql069`](#sql069--column-declared-not-null-but-default-null) | column declared `NOT NULL` but `DEFAULT NULL` |
| [`sql072`](#sql072--select--for-update-without-a-where-clause-locks-every-row-of-the-target-table) | `SELECT ... FOR UPDATE` without a WHERE clause locks every row of the target table |
| [`sql074`](#sql074--where-x-in-a-b-c--with--50-items) | `WHERE x IN (a, b, c, ...)` with > 50 items |
| [`sql075`](#sql075--column-declared-as-time-with-time-zone-alias-timetz-pg-docs-recommend-against-timetz) | column declared as `TIME WITH TIME ZONE` (alias `TIMETZ`). PG docs recommend against TIMETZ |
| [`sql076`](#sql076--limit--1--offset--1) | `LIMIT -1` / `OFFSET -1` |
| [`sql081`](#sql081--order-by-random) | `ORDER BY random()` |
| [`sql083`](#sql083--insert-into-t-id--referencing-the-primary-key-without-on-conflict) | `INSERT INTO t (id, ...)` referencing the primary key without `ON CONFLICT` |
| [`sql084`](#sql084--count1-is-equivalent-to-count) | `COUNT(1)` is equivalent to `COUNT(*)` |
| [`sql085`](#sql085--nullifx-x-always-returns-null) | `NULLIF(x, x)` always returns NULL |
| [`sql087`](#sql087--x-between-high-and-low) | `x BETWEEN <high> AND <low>` |
| [`sql088`](#sql088--like-foo) | `LIKE '%foo'` |
| [`sql089`](#sql089--two-raise-exception-calls-back-to-back) | two `RAISE EXCEPTION` calls back-to-back |
| [`sql090`](#sql090--pg-17-added-group-by-all-shorthand) | PG 17 added `GROUP BY ALL` shorthand |
| [`sql091`](#sql091--comment-on--is-) | `COMMENT ON ... IS ''` |
| [`sql093`](#sql093--select-distinct-count-from-t) | `SELECT DISTINCT count(...) FROM t` |
| [`sql094`](#sql094--case-expressions-nested-more-than-3-deep) | `CASE` expressions nested more than 3 deep |
| [`sql095`](#sql095--x-is-not-distinct-from-null-is-just-x-is-null-the-other-form-is-x-is-distinct-from-null--x-is-not) | `x IS NOT DISTINCT FROM NULL` is just `x IS NULL`; the other form is `x IS DISTINCT FROM NULL` ≡ `x IS NOT... |
| [`sql096`](#sql096--insert-into-t-values-1-2-) | `INSERT INTO t VALUES (1, 2, );` |
| [`sql097`](#sql097--select-col-from-nothing) | `SELECT col FROM nothing` |
| [`sql098`](#sql098--more-than-one-where-clause-in-the-same-statement-outside-parenthesessubqueries-usually-a-copypaste) | more than one `WHERE` clause in the same statement (outside parentheses/subqueries). Usually a copy/paste... |
| [`sql099`](#sql099--order-by-1-2) | `ORDER BY 1, 2` |
| [`sql101`](#sql101--select-distinct-on-x--from-t-without-an-order-by-that-starts-with-x) | `SELECT DISTINCT ON (x) ... FROM t` without an `ORDER BY` that starts with `x` |
| [`sql104`](#sql104--charn--charactern) | `CHAR(n)` / `CHARACTER(n)` |
| [`sql105`](#sql105--truncate-t-without-cascade) | `TRUNCATE t` without `CASCADE` |
| [`sql107`](#sql107--comparing-a-jsonb-column-to-a-text-literal-without-text--jsonb) | comparing a `jsonb` column to a text literal without `::text` / `::jsonb` |
| [`sql109`](#sql109--lengthtext-col-returns-bytes-use-char-length-for-characters) | `length(text_col)` returns *bytes*. Use `char_length` for characters |
| [`sql111`](#sql111--lock-table-outside-an-explicit-transaction-has-no-effect-beyond-the-single-statement) | `LOCK TABLE` outside an explicit transaction has no effect beyond the single statement |
| [`sql112`](#sql112--generate-series-in-a-from-clause-without-an-alias-ends-up-named-generate-series-which-makes-queries) | `generate_series(...)` in a FROM clause without an alias ends up named `generate_series` which makes queries... |
| [`sql113`](#sql113--timestamp-without-time-zone) | `TIMESTAMP` without time zone |
| [`sql115`](#sql115--jsonb-setcol-path-val) | `jsonb_set(col, path, val)` |
| [`sql116`](#sql116--bare-numeric--decimal) | bare `NUMERIC` / `DECIMAL` |
| [`sql117`](#sql117--insert-into-t-col-values-true-where-col-is-boolean) | `INSERT INTO t (col) VALUES ('true')` where `col` is boolean |
| [`sql118`](#sql118--select--into-foo-from-t-at-the-top-level-is-ddl) | `SELECT ... INTO foo FROM t` at the top level is **DDL** |
| [`sql119`](#sql119--set-transaction-isolation-level--must-be-the-first-statement-after-begin) | `SET TRANSACTION ISOLATION LEVEL ...` must be the **first** statement after `BEGIN` |
| [`sql120`](#sql120--select-distinct--group-by-) | `SELECT DISTINCT ... GROUP BY ...` |
| [`sql121`](#sql121--comparing-a-text-expression-to-an-int-literal-in-where-common-bug) | comparing a text expression to an int literal in WHERE. Common bug |
| [`sql122`](#sql122--like-inside-a-query-without-explicit-collate) | `LIKE` inside a query without explicit `COLLATE` |
| [`sql123`](#sql123--n-t--inside-a-plain--string-pg-91-defaults-to-standard-conforming-strings--on) | `\n`, `\t`, `\\` inside a plain `'...'` string. PG 9.1+ defaults to `standard_conforming_strings = on` |
| [`sql124`](#sql124--with-t-as-select) | `WITH t AS (SELECT |
| [`sql125`](#sql125--explain-analyze-insertupdatedelete) | `EXPLAIN ANALYZE INSERT/UPDATE/DELETE` |
| [`sql126`](#sql126--dml-inside-a-plpgsql-function-without-a-subsequent-get-diagnostics-rows--row-count) | DML inside a PL/pgSQL function without a subsequent `GET DIAGNOSTICS rows = ROW_COUNT` |
| [`sql127`](#sql127--update-t-set--from-other-without-a-where-that-joins-t-and-other) | `UPDATE t SET ... FROM other` without a WHERE that joins `t` and `other` |
| [`sql128`](#sql128--grant--to-public) | `GRANT ... TO PUBLIC` |
| [`sql130`](#sql130--multiple-truncate-statements-in-one-transaction-pg-supports-truncate-a-b-c-directly) | multiple `TRUNCATE` statements in one transaction. PG supports `TRUNCATE a, b, c` directly |
| [`sql131`](#sql131--raise-notice-value-is-s) | `RAISE NOTICE 'value is %s'` |
| [`sql132`](#sql132--select--for-update-inside-the-recursive-arm-of-a-cte-is-forbidden-by-pg) | `SELECT ... FOR UPDATE` inside the recursive arm of a CTE is forbidden by PG |
| [`sql133`](#sql133--grant--with-grant-option-lets-the-grantee-re-grant-the-privilege-chain-to-anyone-else) | `GRANT ... WITH GRANT OPTION` lets the grantee re-grant the privilege chain to anyone else |
| [`sql134`](#sql134--vacuum-cannot-run-inside-an-explicit-transaction-block) | `VACUUM` cannot run inside an explicit transaction block |
| [`sql135`](#sql135--set-role-x-inside-a-transaction-without-a-matching-reset-role) | `SET ROLE x` inside a transaction without a matching `RESET ROLE` |
| [`sql136`](#sql136--copy-t-from-file-without-a-format-clause) | `COPY t FROM 'file'` without a `FORMAT` clause |
| [`sql137`](#sql137--bare-listen-channel-in-a-session-that-never-unlistens) | bare `LISTEN <channel>` in a session that never `UNLISTEN`s |
| [`sql138`](#sql138--select-distinct-coltext-from-t) | `SELECT DISTINCT (col)::text FROM t` |
| [`sql139`](#sql139--unique-on-a-nullable-column-with-nulls-distinct-the-pg-default) | `UNIQUE` on a nullable column with `NULLS DISTINCT` (the PG default) |
| [`sql140`](#sql140--create-trigger--after-insert--when-oldx-) | `CREATE TRIGGER ... AFTER INSERT ... WHEN (OLD.x ...)` |
| [`sql141`](#sql141--alter-type-x-add-value-y-cannot-run-inside-an-explicit-transaction-block) | `ALTER TYPE x ADD VALUE 'y'` cannot run inside an explicit transaction block |
| [`sql142`](#sql142--create-or-replace-function--immutable-whose-body-issues-ddl-create-alter-drop-truncate) | `CREATE [OR REPLACE] FUNCTION ... IMMUTABLE` whose body issues DDL (CREATE, ALTER, DROP, TRUNCATE) |
| [`sql143`](#sql143--insertupdatedelete--returning--inside-a-plpgsql-block-without-into-vars-or-strict) | `INSERT/UPDATE/DELETE ... RETURNING ...` inside a PL/pgSQL block without `INTO <vars>` or `STRICT` |
| [`sql144`](#sql144--create-trigger--after-delete--when-newx-) | `CREATE TRIGGER ... AFTER DELETE ... WHEN (NEW.x ...)` |
| [`sql145`](#sql145--column-default-now-or-any-volatile-expression-freezes-the-value-at-insert-time-which-is-usually-fine) | column `DEFAULT now()` (or any volatile expression) freezes the value at insert time, which is usually fine |
| [`sql146`](#sql146--varchar--character-varying-without-an-explicit-length-unbounded-varchar-is-effectively-text-but-with) | `VARCHAR` / `CHARACTER VARYING` without an explicit length. Unbounded VARCHAR is effectively TEXT but with... |
| [`sql148`](#sql148--array-subscript-arr0-or-arr-1) | array subscript `arr[0]` or `arr[-1]` |
| [`sql149`](#sql149--update-t-set-x--x) | `UPDATE t SET x = x` |
| [`sql150`](#sql150--case-when) | `CASE WHEN |
| [`sql151`](#sql151--select--from-t-generate-seriestcol-10) | `SELECT ... FROM t, generate_series(t.col, 10)` |
| [`sql152`](#sql152--begin-for-a-transaction-that-needs-to-updatedelete-many-rows-without-an-explicit-lock-table-or-for) | `BEGIN` for a transaction that needs to UPDATE/DELETE many rows without an explicit `LOCK TABLE` or `FOR... |
| [`sql153`](#sql153--now--1-created-at--30) | `now() + 1`, `created_at + 30` |
| [`sql154`](#sql154--select-count-from-t-where--no-group-by-returns-one-row-even-when-the-where-matches-nothing) | `SELECT count(*) FROM t WHERE ...` (no GROUP BY) returns **one row** even when the WHERE matches nothing |
| [`sql155`](#sql155--truncate-t-returning-) | `TRUNCATE t RETURNING ...` |
| [`sql156`](#sql156--select--into-strict-var-inside-plpgsql-without-a-surrounding-exception-block-strict-raises) | `SELECT ... INTO STRICT var` inside PL/pgSQL without a surrounding EXCEPTION block. STRICT raises... |
| [`sql157`](#sql157--raise-exception--using-errcode--my-var) | `RAISE EXCEPTION ... USING ERRCODE = my_var` |
| [`sql158`](#sql158--perform-select-inside-plpgsql-where-the-select-calls-no-function-with-side-effects) | `PERFORM <select>` inside PL/pgSQL where the SELECT calls no function with side effects |
| [`sql159`](#sql159--create-trigger--for-each-statement--new) | `CREATE TRIGGER ... FOR EACH STATEMENT ... NEW` |
| [`sql160`](#sql160--pg-advisory-lock-session-level-without-a-matching-pg-advisory-unlock-in-the-same-source) | `pg_advisory_lock(...)` (session-level) without a matching `pg_advisory_unlock(...)` in the same source |
| [`sql164`](#sql164--foo--1-or-a--1) | `'foo' \|\| 1` or `'a' + 1` |
| [`sql166`](#sql166--rowx-with-a-single-element) | `ROW(x)` with a single element |
| [`sql167`](#sql167--create-index) | `CREATE INDEX |
| [`sql168`](#sql168--create-unique-index) | `CREATE UNIQUE INDEX |
| [`sql169`](#sql169--alter-table-x-owner-to-some-role) | `ALTER TABLE x OWNER TO some_role` |
| [`sql170`](#sql170--x--lit-inside-a-plpgsql-body-where-the-literal-kind-disagrees-with-xs-declared-type-catches-declare) | `x := <lit>` inside a PL/pgSQL body where the literal kind disagrees with x's declared type. Catches `DECLARE... |
| [`sql171`](#sql171--update-t-set-col--literal-where-the-literal-kind-disagrees-with-the-columns-catalog-type) | `UPDATE t SET <col> = <literal>` where the literal kind disagrees with the column's catalog type |
| [`sql172`](#sql172--col--literal-or------where-the-literal-kind-disagrees-with-the-columns) | `<col> = <literal>` (or `<>`, `>`, `<`, `>=`, `<=`) where the literal kind disagrees with the column's... |
| [`sql173`](#sql173--workspace-create-table-diverges-from-the-live-catalog) | workspace CREATE TABLE diverges from the live catalog |
| [`sql174`](#sql174--countcol-where-col-is-nullable) | `COUNT(col)` where `col` is nullable |
| [`sql175`](#sql175--select--from-view-for-update) | `SELECT ... FROM <view> FOR UPDATE` |
| [`sql176`](#sql176--where-col-is-null-where-the-catalog-says-col-is-not-null) | `WHERE col IS NULL` where the catalog says `col` is NOT NULL |
| [`sql177`](#sql177--insert-into-t-a--values-null--where-a-is-not-null-and-has-no-default) | `INSERT INTO t (a, ...) VALUES (NULL, ...)` where `a` is NOT NULL and has no default |
| [`sql178`](#sql178--writing-to-a-generated-always-column-pg-rejects-writes-to-identitystored-generated-columns--insert) | Writing to a `GENERATED ALWAYS` column. PG rejects writes to identity/stored generated columns: * `INSERT... |
| [`sql179`](#sql179--savepoint-s-outside-a-transaction-errors-with-25p01-savepoint-can-only-be-used-in-transaction-blocks) | `SAVEPOINT s;` outside a transaction errors with 25P01 ("SAVEPOINT can only be used in transaction blocks") |
| [`sql180`](#sql180--truncate-inside-a-trigger-function-body) | `TRUNCATE` inside a trigger function body |
| [`sql181`](#sql181--insert-into-t-name-values-long-string-where-name-is-declared-varcharn-and-the-literal-exceeds-n) | `INSERT INTO t (name) VALUES ('long-string')` where `name` is declared `VARCHAR(n)` and the literal exceeds n |
| [`sql182`](#sql182--insert-into-t-d-values-garbage-where-d-is-date--timestamp--timestamptz--time-and-the-string) | `INSERT INTO t (d) VALUES ('garbage')` where `d` is DATE / TIMESTAMP / TIMESTAMPTZ / TIME and the string... |
| [`sql183`](#sql183--insert-into-t-id-values-not-a-uuid-where-id-is-uuid-pg-raises-22p02-at-runtime-accept-only-) | `INSERT INTO t (id) VALUES ('not-a-uuid')` where `id` is UUID. PG raises 22P02 at runtime. Accept only: *... |
| [`sql184`](#sql184--integer-literal-larger-than-the-columns-declared-type-can-hold-smallint-max-32767-int-max-2147483647) | integer literal larger than the column's declared type can hold (`SMALLINT` max 32767, `INT` max 2147483647) |
| [`sql185`](#sql185--references-othermissing-where-missing-isnt-a-column-on-other) | `REFERENCES other(missing)` where `missing` isn't a column on `other` |
| [`sql186`](#sql186--alter-table-t-drop-column-id-where-another-catalog-table-has-a-fk-that-references-tid) | `ALTER TABLE t DROP COLUMN id` where another catalog table has a FK that references `t(id)` |
| [`sql187`](#sql187--join-other-using-col) | `JOIN other USING (col)` |
| [`sql188`](#sql188--comment-on-table-bogus-is--where-bogus-isnt-a-known-catalog-table) | `COMMENT ON TABLE bogus IS '...'` where bogus isn't a known catalog table |
| [`sql189`](#sql189--alter-table-t-alter-column-c-type-new-type-where-cs-catalog-type-doesnt-auto-cast-to-new-type-and) | `ALTER TABLE t ALTER COLUMN c TYPE <new_type>` where `c`'s catalog type doesn't auto-cast to `<new_type>` and... |
| [`sql190`](#sql190--insert-into-t---on-conflict-col--do--where-col--is-not-the-target-of-any-primary) | `INSERT INTO t (...) ... ON CONFLICT (col, ...) DO ...` where `(col, ...)` is not the target of any PRIMARY... |
| [`sql191`](#sql191--rows-between-n-following-and-m-preceding-or-any-frame-where-the-start-bound-is-strictly-later-than-the) | `ROWS BETWEEN <n> FOLLOWING AND <m> PRECEDING` or any frame where the start bound is strictly later than the... |
| [`sql192`](#sql192--select) | `SELECT |
| [`sql193`](#sql193--generated-always-as-expr-stored-where-expr-calls-a-known-volatile-function-random--now-) | `GENERATED ALWAYS AS (expr) STORED` where `expr` calls a known-volatile function (random / now /... |
| [`sql194`](#sql194--truncate-foo-no-cascade-when-another-table-has-an-fk-referencing-foo) | `TRUNCATE foo` (no CASCADE) when another table has an FK referencing `foo` |
| [`sql195`](#sql195--castlit-as-type-or-littype-where-lit-cant-be-parsed-as-type) | `CAST('lit' AS <type>)` or `'lit'::<type>` where `lit` can't be parsed as `<type>` |
| [`sql196`](#sql196--references-othercol-where-othercol-is-not-the-target-of-a-primary-key-or-unique-constraint--unique) | `REFERENCES other(col)` where `other.col` is not the target of a PRIMARY KEY or UNIQUE constraint / unique... |
| [`sql197`](#sql197--array-lengthcol--unnestcol-cardinalitycol-array-to-stringcol-) | `array_length(col, ...)`, `unnest(col)`, `cardinality(col)`, `array_to_string(col, ...)`... |
| [`sql198`](#sql198--inline-column-check-references-a-different-column) | inline column CHECK references a different column |
| [`sql199`](#sql199--col-type-default-expr-where-expr-references-another-column-on-the-same-table) | `<col> <type> DEFAULT <expr>` where `<expr>` references another column on the same table |
| [`sql200`](#sql200--join-lateral-select) | `JOIN LATERAL (SELECT |
| [`sql201`](#sql201--create-function) | `CREATE FUNCTION |
| [`sql202`](#sql202--plpgsql-trigger-function-body-references-old-inside-an-insert-trigger-or-new-inside-a-delete) | PL/pgSQL trigger function body references `OLD.*` inside an INSERT trigger or `NEW.*` inside a DELETE... |
| [`sql203`](#sql203--raise-msg-inside-a-plpgsql-body-without-a-level-keyword-noticeinfologwarningexceptiondebug-pg) | `RAISE 'msg'` inside a PL/pgSQL body without a level keyword (NOTICE/INFO/LOG/WARNING/EXCEPTION/DEBUG). PG... |
| [`sql204`](#sql204--update-users-u-set-othercol--) | `UPDATE users u SET other.col = ...` |
| [`sql205`](#sql205--notify-channel-where-no-listen-channel-appears-in-the-same-buffer-dead-channel) | `NOTIFY <channel>` where no `LISTEN <channel>` appears in the same buffer. Dead channel |
| [`sql206`](#sql206--insert-into-t-a-b-values-select-1-2) | `INSERT INTO t (a, b) VALUES ((SELECT 1, 2))` |
| [`sql207`](#sql207--coalescex-with-a-single-argument-is-a-no-op) | `COALESCE(x)` with a single argument is a no-op |
| [`sql208`](#sql208--extractfield-from-expr-where-field-is-not-in-the-pg-supported-list) | `EXTRACT(<field> FROM <expr>)` where `<field>` is not in the PG-supported list |
| [`sql209`](#sql209--copy-t-to-filecsv-or-copy-t-from-filecsv) | `COPY t TO 'file.csv'` or `COPY t FROM 'file.csv'` |
| [`sql210`](#sql210--reindex-concurrently-tableindex-pg-x) | `REINDEX [CONCURRENTLY] (TABLE\|INDEX) pg_<x>` |
| [`sql211`](#sql211--bare-rollback--commit-with-no-preceding-begin--start-transaction-in-the-source) | bare `ROLLBACK;` / `COMMIT;` with no preceding BEGIN / START TRANSACTION in the source |
| [`sql212`](#sql212--top-level-select--into-foo-from-bar) | top-level `SELECT * INTO foo FROM bar` |
| [`sql213`](#sql213--create-index) | `CREATE INDEX |
| [`sql214`](#sql214--create-index-concurrently-or-drop-index-concurrently-inside-an-explicit-transaction-block) | `CREATE INDEX CONCURRENTLY` (or `DROP INDEX CONCURRENTLY`) inside an explicit transaction block |
| [`sql215`](#sql215--group-by-rollupa--cubea-with-a-single-grouping-column) | `GROUP BY ROLLUP(a)` / `CUBE(a)` with a single grouping column |
| [`sql216`](#sql216--insert-into-t-values-12-123) | `INSERT INTO t VALUES (1,2), (1,2,3)` |
| [`sql217`](#sql217--select--left-join--for-update) | `SELECT ... LEFT JOIN ... FOR UPDATE` |
| [`sql218`](#sql218--case-when--then-1--when--then-foo--end) | `CASE WHEN ... THEN 1 ... WHEN ... THEN 'foo' ... END` |
| [`sql219`](#sql219--commit--rollback-inside-a-plpgsql-function-body) | `COMMIT` / `ROLLBACK` inside a PL/pgSQL FUNCTION body |
| [`sql220`](#sql220--with-recursive-t-as-single-select-) | `WITH RECURSIVE t(...) AS (<single SELECT>) ...` |
| [`sql221`](#sql221--array1-foo) | `ARRAY[1, 'foo']` |
| [`sql222`](#sql222--select--from-select--limit-n-for-update) | `SELECT * FROM (SELECT ... LIMIT N) FOR UPDATE` |
| [`sql223`](#sql223--jsonb-setcol-key-val) | `jsonb_set(col, 'key', '"val"')` |
| [`sql224`](#sql224--set-constraints-all-deferred-or-any-set-constraints-form-outside-an-explicit-transaction-block-the) | `SET CONSTRAINTS ALL DEFERRED` (or any SET CONSTRAINTS form) outside an explicit transaction block. The... |
| [`sql225`](#sql225--comment-on--is-null-or-is--when-the-target-already-has-a-non-empty-catalog-comment-pg-accepts-this) | `COMMENT ON ... IS NULL` (or `IS ''`) when the target already has a non-empty catalog comment. PG accepts this |
| [`sql226`](#sql226--drop-table-foo-cascade-or-drop-typeetc-cascade-when-the-catalog-shows-3-direct-dependents-fk) | `DROP TABLE foo CASCADE` (or DROP TYPE/etc CASCADE) when the catalog shows 3+ direct dependents (FK... |
| [`sql227`](#sql227--exists-select--from-) | `EXISTS (SELECT * FROM ...)` |
| [`sql228`](#sql228--x--any-select-1-2-from-) | `x = ANY (SELECT 1, 2 FROM ...)` |
| [`sql229`](#sql229--with-foo-as-updateinsertdelete--select--from-foo-where-the-data-modifying-cte-has-no-returning) | `WITH foo AS (UPDATE/INSERT/DELETE ...) SELECT * FROM foo` where the data-modifying CTE has no RETURNING... |
| [`sql230`](#sql230--create-index--using-gin-col-where-col-is-a-plain-scalar-textintetc) | `CREATE INDEX ... USING GIN (col)` where `col` is a plain scalar (text/int/etc) |
| [`sql231`](#sql231--nulls-first--nulls-last-outside-an-order-by-clause) | `NULLS FIRST` / `NULLS LAST` outside an ORDER BY clause |
| [`sql232`](#sql232--jsonb-col--foo-or--where-the-rhs-is-a-plain-text-literal-without-jsonb) | `<jsonb col> @> 'foo'` (or `<@`) where the RHS is a plain text literal without `::jsonb` |
| [`sql233`](#sql233--create-materialized-view-mv) | `CREATE MATERIALIZED VIEW mv |
| [`sql234`](#sql234--where-col-in-) | `WHERE col IN ()` |
| [`sql235`](#sql235--pg-sleepn-inside-an-explicit-transaction-block) | `pg_sleep(n)` inside an explicit transaction block |
| [`sql236`](#sql236--after-trigger-function-returns-newold-row) | `AFTER` trigger function returns NEW/OLD row |
| [`sql237`](#sql237--a-shell-command-pg-dump-psql-pg-restore-createdb-dropdb-appears-as-the-first-token-of-a-statement-pg) | A shell command (pg_dump, psql, pg_restore, createdb, dropdb) appears as the first token of a statement. PG... |
| [`sql238`](#sql238--arr--array-null-) | `<arr> = ARRAY[..., NULL, ...]` |
| [`sql239`](#sql239--alter-table-t-drop-column-c-where-c-was-declared-in-a-create-table-t-) | `ALTER TABLE t DROP COLUMN c` where `c` was declared in a `CREATE TABLE t ( |
| [`sql240`](#sql240--savepoint-s--savepoint-s) | `SAVEPOINT s; ... SAVEPOINT s;` |
| [`sql241`](#sql241--create-or-replace-view-v-as-select--from-t) | `CREATE [OR REPLACE] VIEW v AS SELECT * FROM t` |
| [`sql242`](#sql242--drop-schema-foo-no-cascade--restrict) | `DROP SCHEMA foo` (no CASCADE / RESTRICT) |
| [`sql243`](#sql243--from-values-1-2-where-) | `FROM (VALUES (1, 2)) WHERE ...` |
| [`sql244`](#sql244--check-true--check-11--check-1-constraint-is-trivially-satisfied) | `CHECK (TRUE)` / `CHECK (1=1)` / `CHECK (1)` constraint is trivially satisfied |
| [`sql245`](#sql245--from-pg-class-bare-instead-of-from-pg-catalogpg-class) | `FROM pg_class` (bare) instead of `FROM pg_catalog.pg_class` |
| [`sql246`](#sql246--insert) | `INSERT |
| [`sql247`](#sql247--pg-advisory-lock1-or-pg-advisory-xact-lock1-with-a-hard-coded-literal-key-pg-advisory-locks-are) | `pg_advisory_lock(1)` (or `pg_advisory_xact_lock(1)`) with a hard-coded literal key. PG advisory locks are... |
| [`sql248`](#sql248--alter-table-t-add-column-c-type-not-null-no-default-on-pg11-pg-rewrites-the-whole-table-to-fill-the) | `ALTER TABLE t ADD COLUMN c <type> NOT NULL` (no DEFAULT). On PG<11 PG rewrites the whole table to fill the... |
| [`sql249`](#sql249--insert-into-t-default-values) | `INSERT INTO t DEFAULT VALUES` |
| [`sql250`](#sql250--select-count-from-t-for-update) | `SELECT count(*) FROM t FOR UPDATE` |
| [`sql251`](#sql251--select--from-t-order-by-1) | `SELECT * FROM t ORDER BY 1` |
| [`sql252`](#sql252--select--from-select--order-by-x-sub) | `SELECT * FROM (SELECT ... ORDER BY x) sub` |
| [`sql253`](#sql253--x-not-in-select-col-from-t-where-col-is-nullable) | `x NOT IN (SELECT col FROM t)` where `col` is nullable |
| [`sql254`](#sql254--alter-table-t-set-tablespace-ts-rewrites-the-entire-table-on-disk-and-holds-accessexclusivelock-for-the) | `ALTER TABLE t SET TABLESPACE ts` rewrites the entire table on disk and holds AccessExclusiveLock for the... |
| [`sql255`](#sql255--row-number-over---rank-over---lag-over--without-an-order-by-in-the-window-definition) | `ROW_NUMBER() OVER ()` / `RANK() OVER ()` / `LAG() OVER ()` without an ORDER BY in the window definition |
| [`sql256`](#sql256--current-settingfoo) | `current_setting('foo')` |
| [`sql257`](#sql257--do--begin-select-now-end-) | `DO $$ BEGIN SELECT now(); END $$;` |
| [`sql258`](#sql258--set-local-foo--val-outside-an-explicit-transaction-block) | `SET LOCAL <foo> = <val>` outside an explicit transaction block |
| [`sql259`](#sql259--set-role-foo-inside-a-create-function-body-almost-never-intentional) | `SET ROLE <foo>` inside a CREATE FUNCTION body. Almost never intentional |
| [`sql260`](#sql260--drop-function-foo-without-an-argument-signature-on-pg14-this-works-when-theres-only-one-overload-but-it) | `DROP FUNCTION foo` without an argument signature. On PG14+ this works when there's only one overload, but it... |
| [`sql261`](#sql261--merge-into-t-using-src-on--) | `MERGE INTO t USING src ON ... ;` |
| [`sql262`](#sql262--create-extension-pg-stat-statements-without-if-not-exists) | `CREATE EXTENSION pg_stat_statements` (without IF NOT EXISTS) |
| [`sql263`](#sql263--select--from-select-distinct-on-k--from-t-sub-without-an-order-by-inside-the-subquery-distinct-on) | `SELECT * FROM (SELECT DISTINCT ON (k) ... FROM t) sub` without an ORDER BY inside the subquery. DISTINCT ON... |
| [`sql264`](#sql264--update-pg-class-set---delete-from-pg-class-and-other-direct-dml-against-pg-catalog-system-tables) | `UPDATE pg_class SET ...` / `DELETE FROM pg_class` and other direct DML against `pg_catalog` system tables |
| [`sql265`](#sql265--create-table-t--c-timestamp-default-now-) | `CREATE TABLE t (..., c TIMESTAMP DEFAULT now(), ...)` |
| [`sql266`](#sql266--jsonb-build-objectk1-v1-k2) | `jsonb_build_object(k1, v1, k2)` |
| [`sql267`](#sql267--a--b--c-chained-comparison) | `a = b = c` chained comparison |
| [`sql268`](#sql268--select--order-by-a-union-select-) | `(SELECT ... ORDER BY a) UNION (SELECT ...)` |
| [`sql269`](#sql269--where-extractyear-from-ts--2024-or-where-date-partyear-ts--2024) | `WHERE EXTRACT(YEAR FROM ts) = 2024` or `WHERE date_part('year', ts) = 2024` |
| [`sql270`](#sql270--formathello-world) | `format('hello world')` |
| [`sql271`](#sql271--declare-c-cursor-with-hold-for--outside-an-explicit-transaction) | `DECLARE c CURSOR WITH HOLD FOR ...` outside an explicit transaction |
| [`sql272`](#sql272--create-index) | `CREATE INDEX |
| [`sql273`](#sql273--check-false--check-0-constraint-rejects-every-row) | `CHECK (FALSE)` / `CHECK (0)` constraint rejects every row |
| [`sql274`](#sql274--select--into-temp-foo-from-bar-or-into-temporary-where-foo-is-also-a-real-catalog-table-pg-allows-it) | `SELECT ... INTO TEMP foo FROM bar` (or INTO TEMPORARY) where `foo` is also a real catalog table. PG allows it |
| [`sql275`](#sql275--set-transaction--read-only--read-write--isolation-level-inside-a-create-function-body) | `SET TRANSACTION ...` (READ ONLY / READ WRITE / ISOLATION LEVEL) inside a CREATE FUNCTION body |
| [`sql276`](#sql276--interval-1-day-style-no-quotes) | `INTERVAL 1 DAY` style (no quotes) |
| [`sql277`](#sql277--comment-on-function-foo-is--without-argument-signature-same-hazard-as-drop-function) | `COMMENT ON FUNCTION foo IS '...'` without argument signature. Same hazard as DROP FUNCTION |
| [`sql278`](#sql278--expr--0-literal-division-by-zero) | `<expr> / 0` literal division by zero |
| [`sql279`](#sql279--comment-on-constraint-pk-users-is-) | `COMMENT ON CONSTRAINT pk_users IS '...'` |
| [`sql280`](#sql280--alter-table-t-add-constraint-c-check--without-not-valid) | `ALTER TABLE t ADD CONSTRAINT c CHECK (...)` without `NOT VALID` |
| [`sql281`](#sql281--alter-table-t-alter-column-c-set-not-null) | `ALTER TABLE t ALTER COLUMN c SET NOT NULL` |
| [`sql282`](#sql282--where-11-and---where-true-and-) | `WHERE 1=1 AND ...` / `WHERE TRUE AND ...` |
| [`sql283`](#sql283--analyze-or-analyze-t-inside-an-explicit-transaction) | `ANALYZE` (or `ANALYZE t`) inside an explicit transaction |
| [`sql284`](#sql284--tg-op-tg-table-name-tg-relid-tg-name-tg-when-tg-level-tg-nargs-tg-argv-referenced) | `TG_OP`, `TG_TABLE_NAME`, `TG_RELID`, `TG_NAME`, `TG_WHEN`, `TG_LEVEL`, `TG_NARGS`, `TG_ARGV` referenced... |
| [`sql285`](#sql285--drop-role-foo--drop-user-foo-without-a-preceding-reassign-owned-by-foo--drop-owned-by-foo) | `DROP ROLE foo` / `DROP USER foo` without a preceding `REASSIGN OWNED BY foo` + `DROP OWNED BY foo` |
| [`sql286`](#sql286--alter-type-x-add-value-new-before-bogus-where-bogus-is-not-one-of-xs-enum-labels) | `ALTER TYPE x ADD VALUE 'new' BEFORE 'bogus'` where `bogus` is not one of `x`'s enum labels |
| [`sql287`](#sql287--revoke--cascade-on-a-privilege-the-grantee-may-have-re-granted-cascade-recursively-revokes-from-every) | `REVOKE ... CASCADE` on a privilege the grantee may have re-granted. CASCADE recursively revokes from every... |
| [`sql288`](#sql288--create-index-on-t-col) | `CREATE INDEX ON t (col)` |
| [`sql289`](#sql289--create-table--inherits-parent) | `CREATE TABLE ... INHERITS (parent)` |
| [`sql290`](#sql290--percentile-cont05--percentile-disc05--mode-without-the-required-within-group-order-by-) | `percentile_cont(0.5)` / `percentile_disc(0.5)` / `mode()` without the required `WITHIN GROUP (ORDER BY ...)`... |
| [`sql291`](#sql291--grant-all-privileges-on--or-bare-grant-all) | `GRANT ALL PRIVILEGES ON ...` (or bare `GRANT ALL`) |
| [`sql292`](#sql292--limit-0-returns-zero-rows) | `LIMIT 0` returns zero rows |
| [`sql293`](#sql293--nullif1-foo) | `NULLIF(1, 'foo')` |
| [`sql294`](#sql294--begin-or-start-transaction-when-an-earlier-begin-in-the-source-hasnt-been-commited--rollbacked) | `BEGIN;` (or `START TRANSACTION;`) when an earlier BEGIN in the source hasn't been COMMITed / ROLLBACKed |
| [`sql295`](#sql295--copy--with-header-format-text) | `COPY ... WITH (HEADER, FORMAT TEXT)` |
| [`sql296`](#sql296--reindex-table--index--schema--database-inside-an-open-transaction-pg-holds-accessexclusivelock-for) | `REINDEX` (TABLE / INDEX / SCHEMA / DATABASE) inside an open transaction. PG holds AccessExclusiveLock for... |
| [`sql297`](#sql297--notify-chan-huge-literal) | `NOTIFY chan, '<huge literal>'` |
| [`sql298`](#sql298--create-table--function--type--index--trigger--constraint-name-longer-than-63-bytes) | CREATE TABLE / FUNCTION / TYPE / INDEX / TRIGGER / CONSTRAINT name longer than 63 bytes |
| [`sql299`](#sql299--primary-key-a-a--unique-a-a) | `PRIMARY KEY (a, a)` / `UNIQUE (a, a)` |
| [`sql300`](#sql300--select-a-b-from-t) | `SELECT a, b, FROM t` |
| [`sql301`](#sql301--copy--from-program-cmd--copy--to-program-cmd) | `COPY ... FROM PROGRAM 'cmd'` / `COPY ... TO PROGRAM 'cmd'` |
| [`sql302`](#sql302--drop-table-foo-or-drop-indexviewtriggeretc-without-if-exists) | `DROP TABLE foo` (or DROP INDEX/VIEW/TRIGGER/etc) without `IF EXISTS` |
| [`sql303`](#sql303--array-empty-constructor-without-a-type-cast) | `ARRAY[]` (empty constructor) without a `::type[]` cast |
| [`sql304`](#sql304--create-table-foo--parent-id-references-fooid) | CREATE TABLE foo (..., parent_id REFERENCES foo(id)) |
| [`sql305`](#sql305--from-information-schemaview) | `FROM information_schema.<view>` |
| [`sql306`](#sql306--where-id-in-1-1-2) | `WHERE id IN (1, 1, 2)` |
| [`sql307`](#sql307--update--limit-n--delete--limit-n) | `UPDATE ... LIMIT N` / `DELETE ... LIMIT N` |
| [`sql308`](#sql308--timestamp7--time7--timestamptz7-etc) | `TIMESTAMP(7)` / `TIME(7)` / `TIMESTAMPTZ(7)` etc |
| [`sql309`](#sql309--revoke-select-on-foo) | `REVOKE SELECT ON foo;` |
| [`sql310`](#sql310--line-starts-with-letter) | line starts with `\<letter>` |
| [`sql311`](#sql311--string-aggcol---array-aggcol--json-aggcol--jsonb-aggcol-without-an-order-by-clause) | `string_agg(col, ',')` / `array_agg(col)` / `json_agg(col)` / `jsonb_agg(col)` without an `ORDER BY` clause... |
| [`sql312`](#sql312--column-declared-serial--bigserial--smallserial) | column declared `SERIAL` / `BIGSERIAL` / `SMALLSERIAL` |
| [`sql313`](#sql313--create-table-t--comment-msg) | `CREATE TABLE t (...) COMMENT 'msg'` |
| [`sql314`](#sql314--auto-increment) | `AUTO_INCREMENT` |
| [`sql315`](#sql315--engineinnodb--enginemyisam--similar) | `ENGINE=InnoDB` / `ENGINE=MyISAM` / similar |
| [`sql316`](#sql316--mysql-only-types-tinyint-mediumint-longtext-etc) | MySQL-only types (TINYINT, MEDIUMINT, LONGTEXT, etc) |
| [`sql317`](#sql317--identifier-square-bracket-quoting) | `[identifier]` (square-bracket quoting) |
| [`sql318`](#sql318--select-top-10-) | `SELECT TOP 10 ...` |
| [`sql319`](#sql319--isnullx-y-mssqlmysql--nvlx-y-oracle--ifnullx-y-mysql) | `ISNULL(x, y)` (MSSQL/MySQL) / `NVL(x, y)` (Oracle) / `IFNULL(x, y)` (MySQL) |
| [`sql320`](#sql320--getdate--sysdate--getutcdate) | `GETDATE()` / `SYSDATE` / `GETUTCDATE()` |
| [`sql321`](#sql321--standalone-go) | standalone `GO` |
| [`sql322`](#sql322--begin-tran) | `BEGIN TRAN` |
| [`sql323`](#sql323--select--from-dual) | `SELECT ... FROM DUAL` |
| [`sql324`](#sql324--rownum) | `ROWNUM` |
| [`sql325`](#sql325--connect-by-prior-) | `CONNECT BY PRIOR ...` |
| [`sql326`](#sql326--aid--bid) | `a.id = b.id(+)` |
| [`sql327`](#sql327--create-table-foo--without-an-explicit-schema-qualifier-style-hint-every-create-table-in-a) | `CREATE TABLE foo (...)` without an explicit schema qualifier. Style hint: every CREATE TABLE in a... |
| [`sql328`](#sql328--revoke-in-a-buffer-that-has-no-matching-grant) | REVOKE in a buffer that has no matching GRANT |
| [`sql329`](#sql329--substringtext-from-number-without-a-matching-for-pg-returns-the-rest-of-the-string-from-the-start) | `substring(text FROM <number>)` without a matching `FOR`. PG returns the rest of the string from the start... |
| [`sql331`](#sql331--drop-index-concurrently-inside-an-explicit-transaction) | `DROP INDEX CONCURRENTLY` inside an explicit transaction |
| [`sql332`](#sql332--pg-terminate-backend--pg-cancel-backend-invoked-from-an-unprivileged-buffer) | `pg_terminate_backend(...)` / `pg_cancel_backend(...)` invoked from an unprivileged buffer |
| [`sql333`](#sql333--on-update-cascade-on-a-column-referenced-as-a-primary-key-on-update-cascade-is-rarely-the-right-choice-on) | `ON UPDATE CASCADE` on a column referenced as a primary key. ON UPDATE CASCADE is rarely the right choice on... |
| [`sql334`](#sql334--select-setseed-without-a-nearby-deterministic-guard) | `SELECT setseed(...)` without a nearby deterministic guard |
| [`sql335`](#sql335--explicit-tablespace-name-clause-in-a-buffer-that-likely-runs-as-a-non-superuser-migration) | explicit `TABLESPACE <name>` clause in a buffer that likely runs as a non-superuser migration |
| [`sql336`](#sql336--bytea-literal-xff-without-the-e-escape-string-prefix) | `bytea` literal `'\\xFF'` without the `E''` escape-string prefix |
| [`sql337`](#sql337--group-by-references-a-select-list-alias-instead-of-the-original-column) | `GROUP BY` references a SELECT-list alias instead of the original column |
| [`sql338`](#sql338--create-table-x-partition-of-parent-like-base-including-indexes--including-indexes-inside-a-partition) | `CREATE TABLE x PARTITION OF parent (LIKE base INCLUDING INDEXES ...)` INCLUDING INDEXES inside a PARTITION... |
| [`sql339`](#sql339--truncate-inside-a-plpgsql-function-body-that-also-has-an-exception-block) | `TRUNCATE` inside a PL/pgSQL function body that also has an `EXCEPTION` block |
| [`sql340`](#sql340--newid--expr-inside-a-before-insert-trigger-body) | `NEW.id := <expr>` inside a `BEFORE INSERT` trigger body |
| [`sql341`](#sql341--insert-into-t-col-values-array-where-the-array-element-family-doesnt-match-the-target-columns) | `INSERT INTO t (col) VALUES (ARRAY[...])` where the array element family doesn't match the target column's... |
| [`sql342`](#sql342--bool-andcol--bool-orcol--everycol-on-a-nullable-boolean-column) | `BOOL_AND(col)` / `BOOL_OR(col)` / `EVERY(col)` on a nullable boolean column |
| [`sql343`](#sql343--percent-rank-over-order-by-col--cume-dist-over-order-by-col-where-col-is-a-non-numeric) | `percent_rank() OVER (ORDER BY <col>)` / `cume_dist() OVER (ORDER BY <col>)` where `<col>` is a non-numeric... |
| [`sql344`](#sql344--order-by-col-using-op-where-the-columns-type-family-is-one-of-the-families-that-lacks-a-meaningful) | `ORDER BY <col> USING <op>` where the column's type family is one of the families that lacks a meaningful... |
| [`sql345`](#sql345--alter-table-t-rename-column-old-to-new-while-some-create-view-v-as-select--in-the-same-buffer) | `ALTER TABLE t RENAME COLUMN old TO new` while some `CREATE VIEW v AS SELECT ...` in the same buffer... |
| [`sql346`](#sql346--create-index) | `CREATE INDEX |
| [`sql347`](#sql347--alter-table-t-enabledisable-trigger-) | `ALTER TABLE t ENABLE\|DISABLE TRIGGER ...` |
| [`sql348`](#sql348--function-call-whose-name-isnt-in-the-live-catalog-the-built-in-dsl-knowledge-function-table-or-a) | function call whose name isn't in the live catalog, the built-in dsl-knowledge function table, or a... |
| [`sql349`](#sql349--insert-into-t-col-list-lists-a-column-not-in-the-target-tables-catalog) | `INSERT INTO t (col_list)` lists a column not in the target table's catalog |
| [`sql350`](#sql350--insertupdatedelete) | `INSERT/UPDATE/DELETE |
| [`sql351`](#sql351--deleteupdate-from-t-where-bogus) | `DELETE/UPDATE FROM t WHERE bogus` |
| [`sql402`](#sql402--duplicate-fromjoin-alias-in-a-single-select-example-select--from-users-a-orders-a) | duplicate FROM/JOIN alias in a single SELECT. Example: `SELECT * FROM users a, orders a` |
| [`sql403`](#sql403--order-by-references-a-column-that-doesnt-exist-in-any-in-scope-table-or-projection-alias-pg-models-order-by) | ORDER BY references a column that doesn't exist in any in-scope table or projection alias. PG models ORDER BY... |
| [`sql404`](#sql404--group-by-references-a-column-that-doesnt-exist) | GROUP BY references a column that doesn't exist |
| [`sql405`](#sql405--having-references-a-column-that-doesnt-exist) | HAVING references a column that doesn't exist |
| [`sql406`](#sql406--duplicate-column-in-an-insert-column-list-or-update-set-assignment-list---insert-into-t-a-b-a-values) | duplicate column in an INSERT column list or UPDATE SET assignment list. - `INSERT INTO t (a, b, a) VALUES... |
| [`sql407`](#sql407--where-12--where-false--where-11) | `WHERE 1=2` / `WHERE FALSE` / `WHERE 1<>1` |
| [`sql408`](#sql408--where-col--col-or-col-op-col-for-the-same-column-on-both-sides) | `WHERE col = col` (or `<col> OP <col>` for the same column on both sides) |
| [`sql409`](#sql409--where-col-between-col-and--or-where-col-between--and-col) | `WHERE col BETWEEN col AND ...` or `WHERE col BETWEEN ... AND col` |
| [`sql410`](#sql410--select-id-id-from-) | `SELECT id, id FROM ...` |
| [`sql411`](#sql411--limit-1-offset-n-with-n--0-without-order-by-picks-a-deliberately-non-first-row-but-without-order-by) | `LIMIT 1 OFFSET N` (with N > 0) without ORDER BY picks a deliberately non-first row, but without ORDER BY... |
| [`sql412`](#sql412--order-by-id-id--group-by-id-id) | `ORDER BY id, id` / `GROUP BY id, id` |
| [`sql413`](#sql413--expr--null--null--expr) | `expr \|\| NULL` / `NULL \|\| expr` |
| [`sql414`](#sql414--where-col-in-col--or--col-not-in-col-) | `WHERE col IN (col, ...)` or `... col NOT IN (col, ...)` |
| [`sql415`](#sql415--colt-or-castcol-as-t-where-t-is-the-columns-catalog-data-type) | `col::T` or `CAST(col AS T)` where T is the column's catalog data type |
| [`sql416`](#sql416--case-when--then-x--when--then-x-else-x-end) | `CASE WHEN ... THEN x ... WHEN ... THEN x ELSE x END` |
| [`sql417`](#sql417--coalescea-a--or-coalescea-null-) | `COALESCE(a, a, ...)` or `COALESCE(a, NULL, ...)` |
| [`sql418`](#sql418--select-distinct-pk-col-from-t) | `SELECT DISTINCT pk_col FROM t` |
| [`sql419`](#sql419--nullifx-null-and-nullifnull-x-are-pointless) | `NULLIF(x, NULL)` and `NULLIF(NULL, x)` are pointless |
| [`sql420`](#sql420--where-col--anyarraycol-) | `WHERE col = ANY(ARRAY[col, ...])` |
| [`sql421`](#sql421--where-age--0-and-age--0) | `WHERE age > 0 AND age > 0` |
| [`sql422`](#sql422--where-x-and-not-x) | `WHERE X AND NOT X` |
| [`sql423`](#sql423--col--prefix-or--prefix-where-the-regex-is-just-an-anchored-literal-prefix-could-be-rewritten) | `col ~ '^prefix'` (or `~* '^prefix'`) where the regex is just an anchored literal prefix could be rewritten... |
| [`sql424`](#sql424--where-count--1) | `WHERE count(*) > 1` |
| [`sql425`](#sql425--window-function-in-where--having--join-on) | window function in WHERE / HAVING / JOIN ON |
| [`sql426`](#sql426--select-distinct-id-from-users-order-by-age) | `SELECT DISTINCT id FROM users ORDER BY age` |
| [`sql427`](#sql427--where-datets--2024-01-01--where-tsdate----where-castts-as-date--) | `WHERE date(ts) = '2024-01-01'` / `WHERE ts::date = ...` / `WHERE CAST(ts AS date) = ...` |
| [`sql428`](#sql428--max--sum--avg-etc) | `MAX(*) / SUM(*) / AVG(*)` etc |
| [`sql429`](#sql429--where-col--1-c-style-and-where-col--1-mysql-null-safe-equal) | `WHERE col == 1` (C-style) and `WHERE col <=> 1` (MySQL null-safe equal) |
| [`sql430`](#sql430--select--col-from-t) | `SELECT *, col FROM t` |
| [`sql431`](#sql431--select) | `SELECT |
| [`sql432`](#sql432--case-when-p-then-a-when-p-then-b-end) | `CASE WHEN p THEN a WHEN p THEN b END` |
| [`sql433`](#sql433--order-by-null--order-by-true--order-by-foo) | `ORDER BY NULL` / `ORDER BY TRUE` / `ORDER BY 'foo'` |
| [`sql434`](#sql434--where-col-is-not-null-and-col--5) | `WHERE col IS NOT NULL AND col = 5` |
| [`sql435`](#sql435--where-col-is-null-and-col--5-or-any-strict-op-or-col-is-not-null) | `WHERE col IS NULL AND col = 5` (or any strict op, or `col IS NOT NULL`) |
| [`sql436`](#sql436--sumrow-number-over-) | `sum(row_number() OVER (...))` |
| [`sql437`](#sql437--where-null-in-1-2-3) | `WHERE NULL IN (1, 2, 3)` |
| [`sql438`](#sql438--id-int-generated-always-as-identity-default-0) | `id int GENERATED ALWAYS AS IDENTITY DEFAULT 0` |
| [`sql439`](#sql439--date-2024-13-01--timestamp-2024-02-30) | `DATE '2024-13-01'` / `TIMESTAMP '2024-02-30'` |
| [`sql440`](#sql440--interval-2-mans) | `INTERVAL '2 mans'` |
| [`sql441`](#sql441--where-exists-select-1-from-other-table) | `WHERE EXISTS (SELECT 1 FROM other_table)` |
| [`sql442`](#sql442--regexp-replaces-pattern-replacement) | `regexp_replace(s, pattern, replacement)` |
| [`sql443`](#sql443--substrings-start--3) | `substring(s, start, -3)` |
| [`sql444`](#sql444--generate-series1-10-0) | `generate_series(1, 10, 0)` |
| [`sql445`](#sql445--array-positionarr-null) | `array_position(arr, NULL)` |
| [`sql446`](#sql446--position-in-s--strposs-) | `position('' in s)` / `strpos(s, '')` |
| [`sql447`](#sql447--powerx-0-always-returns-1-and-powerx-1-always-returns-x) | `power(x, 0)` always returns 1 and `power(x, 1)` always returns x |
| [`sql448`](#sql448--lpadhi--3-0) | `lpad('hi', -3, '0')` |
| [`sql449`](#sql449--jsonb-build-objectk-1-k-2) | `jsonb_build_object('k', 1, 'k', 2)` |
| [`sql450`](#sql450--numericp-s-or-decimalp-s-with-s--p) | `NUMERIC(p, s)` (or `DECIMAL(p, s)`) with `s > p` |
| [`sql451`](#sql451--varchar0--char0--character0--character-varying0) | `VARCHAR(0)` / `CHAR(0)` / `CHARACTER(0)` / `CHARACTER VARYING(0)` |
| [`sql452`](#sql452--repeats-0-or-repeats--3) | `repeat(s, 0)` or `repeat(s, -3)` |
| [`sql453`](#sql453--array-lengtharr) | `array_length(arr)` |
| [`sql454`](#sql454--to-timestamps-hhmm) | `to_timestamp(s, 'HH:MM')` |
| [`sql455`](#sql455--where-x-or-not-x) | `WHERE X OR NOT X` |
| [`sql456`](#sql456--where-smallint-col--100000) | `WHERE smallint_col = 100000` |
| [`sql457`](#sql457--select-a-b-from-t-group-by-3) | `SELECT a, b FROM t GROUP BY 3` |
| [`sql458`](#sql458--sumbool-col--avgbool-col) | `SUM(bool_col)` / `AVG(bool_col)` |
| [`sql459`](#sql459--countcol-where-col-is-declared-not-null) | `COUNT(col)` where `col` is declared NOT NULL |
| [`sql460`](#sql460--select-id-from-t-having-id--5) | `SELECT id FROM t HAVING id > 5` |
| [`sql461`](#sql461--array-removenull-1--array-positionnull-1--cardinalitynull) | `array_remove(NULL, 1)` / `array_position(NULL, 1)` / `cardinality(NULL)` |
| [`sql462`](#sql462--x--null-or-----) | `x + NULL` (or `-`, `*`, `/`, `%`) |
| [`sql463`](#sql463--if-tg-op--inserted-then-) | `IF TG_OP = 'inserted' THEN ...` |
| [`sql464`](#sql464--x-is-distinct-from-x) | `x IS DISTINCT FROM x` |
| [`sql465`](#sql465--concat-ws-a-b-c) | `concat_ws('', a, b, c)` |
| [`sql466`](#sql466---offset-0) | `... OFFSET 0` |
| [`sql467`](#sql467--replaces--x--split-parts--n) | `replace(s, '', x)` / `split_part(s, '', n)` |
| [`sql468`](#sql468--greatestnull-null--leastnull-null-null) | `GREATEST(NULL, NULL)` / `LEAST(NULL, NULL, NULL)` |
| [`sql469`](#sql469--not-col-is-null-and-not-col-is-null) | `NOT (col IS NULL)` and `NOT col IS NULL` |
| [`sql470`](#sql470--not-col-in---not-col-like---not-col-between-) | `NOT (col IN (...))` / `NOT (col LIKE ...)` / `NOT (col BETWEEN ...)` |
| [`sql471`](#sql471--where-x-in-select-distinct-y-from-t) | `WHERE x IN (SELECT DISTINCT y FROM t)` |
| [`sql472`](#sql472--extractdow-from-1-dayinterval) | `EXTRACT(dow FROM '1 day'::interval)` |
| [`sql473`](#sql473--col--anyarrayint) | `col = ANY(ARRAY[]::int[])` |
| [`sql474`](#sql474--where-a--a-tautology-where-2--2-tautology-where-a--b-contradiction) | `WHERE 'a' = 'a'` (tautology), `WHERE 2 = 2` (tautology), `WHERE 'a' = 'b'` (contradiction) |
| [`sql475`](#sql475--insert-into-t-select--from-t) | `INSERT INTO t SELECT ... FROM t` |
| [`sql476`](#sql476--case-col-when-null-then-) | `CASE col WHEN NULL THEN ...` |
| [`sql477`](#sql477--col--jsonb--col--jsonb--col--arrayint) | `col @> '{}'::jsonb` / `col @> '[]'::jsonb` / `col @> ARRAY[]::int[]` |
| [`sql478`](#sql478--col--jsonb--col--jsonb--col--arrayint) | `col <@ '{}'::jsonb` / `col <@ '[]'::jsonb` / `col <@ ARRAY[]::int[]` |
| [`sql479`](#sql479--substrings-0-n--substrings-from-0-for-n--substrs-0-n) | `substring(s, 0, n)` / `substring(s FROM 0 FOR n)` / `substr(s, 0, n)` |
| [`sql480`](#sql480--group-by-null--group-by-true--group-by-foo) | `GROUP BY NULL` / `GROUP BY TRUE` / `GROUP BY 'foo'` |
| [`sql481`](#sql481--positionneedle-in---strpos-needle) | `position(<needle> in '')` / `strpos('', <needle>)` |
| [`sql482`](#sql482--having-constant) | `HAVING <constant>` |
| [`sql483`](#sql483--split-parts-delim-0) | `split_part(<s>, <delim>, 0)` |
| [`sql484`](#sql484--over-partition-by-constant-) | `OVER (PARTITION BY <constant> ...)` |
| [`sql485`](#sql485--regexp-split-to-arrays--regexp-split-to-tables--regexp-matchs--regexp-matchess-) | `regexp_split_to_array(s, '')`, `regexp_split_to_table(s, '')`, `regexp_match(s, '')`, `regexp_matches(s, '')` |
| [`sql486`](#sql486--select-distinct---select-distinct-t) | `SELECT DISTINCT *` / `SELECT DISTINCT t.*` |
| [`sql487`](#sql487--array-lengtharr-0-array-lowerarr-0-array-upperarr-0-or-any-negative-dimension) | `array_length(arr, 0)`, `array_lower(arr, 0)`, `array_upper(arr, 0)`, or any negative dimension |
| [`sql488`](#sql488--jsonb-path-existsqueryquery-arrayquery-firstmatchcol-path) | `jsonb_path_exists/query/query_array/query_first/match(col, '<path>')` |
| [`sql489`](#sql489--where-col--0--n-col---0--n-col--1--n-col--1--n-and-the-commutative-0--col-1--col) | `WHERE col + 0 = N`, `col - 0 = N`, `col * 1 = N`, `col / 1 = N` (and the commutative `0 + col`, `1 * col`) |
| [`sql490`](#sql490--col------col) | `col \|\| ''` / `'' \|\| col` |
| [`sql491`](#sql491--having-1--1-tautology--having-1--2-contradiction--having-a--b) | `HAVING 1 = 1` (tautology) / `HAVING 1 = 2` (contradiction) / `HAVING 'a' = 'b'` |
| [`sql492`](#sql492--col-not-in--null-) | `col NOT IN (..., NULL, ...)` |
| [`sql493`](#sql493--coalescenot-null-col-) | `COALESCE(<not-null-col>, ...)` |
| [`sql494`](#sql494--jsonb-settarget--value--jsonb-set-lax--jsonb-insert-with-an-empty-path-array) | `jsonb_set(target, '{}', value)` / `jsonb_set_lax` / `jsonb_insert` with an empty path array |
| [`sql495`](#sql495--where-col--allarray-literal) | `WHERE col = ALL(<array-literal>)` |
| [`sql496`](#sql496--update-t-set-col--default-where-col-has-no-default-definition) | `UPDATE t SET col = DEFAULT` where `col` has no DEFAULT definition |
| [`sql497`](#sql497--array-aggdistinct-a-order-by-b-and-similar) | `array_agg(DISTINCT a ORDER BY b)` and similar |
| [`sql498`](#sql498--where-col-similar-to-pattern) | `WHERE col SIMILAR TO 'pattern'` |
| [`sql499`](#sql499--where-tsvector-col--plain-text) | `WHERE tsvector_col @@ 'plain text'` |
| [`sql500`](#sql500--date-col1---date-col2) | `date_col1 - date_col2` |
| [`sql501`](#sql501--order-by-not-null-col-nulls-firstlast) | `ORDER BY not_null_col NULLS FIRST\|LAST` |
| [`sql502`](#sql502--where-timestamptz-col-op-timestamp-lit) | `WHERE timestamptz_col <op> TIMESTAMP 'lit'` |
| [`sql503`](#sql503--where-non-jsonb-col--key----) | `WHERE non_jsonb_col ? 'key'` / `?\|` / `?&` |
| [`sql504`](#sql504--int-col--int-literal) | `<int_col> / <int_literal>` |
| [`sql505`](#sql505--text-col---key-------) | `<text_col> -> 'key'` / `->>` / `#>` / `#>>` |
| [`sql506`](#sql506--arraynull--arraynull-null-) | `ARRAY[NULL]` / `ARRAY[NULL, NULL, ...]` |
| [`sql507`](#sql507--execute-sql--var) | `EXECUTE '<sql>' \|\| <var>` |
| [`sql508`](#sql508--where-col-like-col--ilike--not-like--not-ilike-and-the-posix-regex-equivalents-------) | `WHERE col LIKE col` / `ILIKE` / `NOT LIKE` / `NOT ILIKE` and the POSIX-regex equivalents `~ / ~* / !~ / !~*` |
| [`sql509`](#sql509--explicit-pg-temptable-or-pg-temp-ntable-reference-temporary-tables-live-in-a-per-backend) | explicit `pg_temp.<table>` (or `pg_temp_<N>.<table>`) reference. Temporary tables live in a per-backend... |
| [`sql510`](#sql510--where-col-similar-to-col--not-similar-to-col) | `WHERE col SIMILAR TO col` / `NOT SIMILAR TO col` |
| [`sql511`](#sql511--where-col--col--col--col--col--col) | `WHERE col @> col` / `col <@ col` / `col && col` |
| [`sql512`](#sql512--table-level-pk--unique--fk-source-constraint-references-a-column-that-isnt-declared-on-this-table-pg) | table-level PK / UNIQUE / FK source constraint references a column that isn't declared on this table. PG... |
| [`sql513`](#sql513--function-call-arg-count-validation) | function call arg-count validation |
| [`sql514`](#sql514--empty-expression-parentheses-where-an-expression-is-required-catches-the-post-refactor-pattern-where-a) | empty expression parentheses where an expression is required. Catches the post-refactor pattern where a... |
| [`sql515`](#sql515--where-col-in-1--where-col-not-in-1) | `WHERE col IN (1)` / `WHERE col NOT IN (1)` |
| [`sql516`](#sql516--update-t-set-col--col) | `UPDATE t SET col = col` |
| [`sql517`](#sql517--join--on-1--1) | `JOIN ... ON 1 = 1` |
| [`sql518`](#sql518--case-when-cond-then-true-else-false-end) | `CASE WHEN cond THEN TRUE ELSE FALSE END` |
| [`sql519`](#sql519--where-a--1-or-a--2-or-a--3) | `WHERE a = 1 OR a = 2 OR a = 3` |
| [`sql520`](#sql520--where-lowercol--abc--where-uppercol-like-abc) | `WHERE lower(col) = 'ABC'` / `WHERE upper(col) LIKE 'abc%'` |
| [`sql521`](#sql521--col--anyarray1--col--allarrayx) | `col = ANY(ARRAY[1])` / `col <> ALL(ARRAY['x'])` |
| [`sql522`](#sql522--a-left-join-b-on--where-bcol--x) | `a LEFT JOIN b ON ... WHERE b.col = 'x'` |
| [`sql523`](#sql523--where-col-is-null-or-col-is-not-null) | `WHERE col IS NULL OR col IS NOT NULL` |
| [`sql524`](#sql524--col-like-) | `col LIKE '%'` |
| [`sql525`](#sql525--exists-select--limit-1) | `EXISTS (SELECT ... LIMIT 1)` |
| [`sql526`](#sql526--where-col--1-and-col--2) | `WHERE col = 1 AND col = 2` |
| [`sql527`](#sql527--where-col--5-and-col--3) | `WHERE col > 5 AND col < 3` |
| [`sql528`](#sql528--replaces-x-x) | `REPLACE(s, x, x)` |
| [`sql529`](#sql529--having-count--0) | `HAVING COUNT(*) > 0` |
| [`sql530`](#sql530--coalescecoalescea-b-c) | `COALESCE(COALESCE(a, b), c)` |
| [`sql531`](#sql531--select-name-as-name) | `SELECT name AS name` |
| [`sql532`](#sql532--select) | `SELECT |
| [`sql533`](#sql533--col-between-5-and-5) | `col BETWEEN 5 AND 5` |
| [`sql534`](#sql534--greatestx-x--leasta-b-a) | `GREATEST(x, x)` / `LEAST(a, b, a)` |
| [`sql535`](#sql535--where-a--1-and-a--2-and-a--3) | `WHERE a <> 1 AND a <> 2 AND a <> 3` |
| [`sql536`](#sql536--insert--on-conflict--do-update-set-col--col) | `INSERT ... ON CONFLICT ... DO UPDATE SET col = col` |
| [`sql537`](#sql537--not-a--b) | `NOT (a = b)` |
| [`sql538`](#sql538--roundx-0--truncx-0) | `ROUND(x, 0)` / `TRUNC(x, 0)` |
| [`sql539`](#sql539--select-distinctcol-other--or-countdistinctcol) | `SELECT DISTINCT(col), other ...` (or `COUNT(DISTINCT(col))`) |
| [`sql540`](#sql540--where-lengths--0--lengths--0) | `WHERE length(s) = 0` / `length(s) > 0` |
| [`sql541`](#sql541--a-boolean-literal-operand-that-forces-the-whole-condition-to-a-constant) | a boolean literal operand that forces the whole condition to a constant |
| [`sql542`](#sql542--nowdate--current-timestampdate) | `now()::date` / `current_timestamp::date` |
| [`sql543`](#sql543--group-by-count--group-by-sumx) | `GROUP BY count(*)` / `GROUP BY sum(x)` |
| [`sql544`](#sql544--where-col--5-and-col--5) | `WHERE col >= 5 AND col <= 5` |
| [`sql545`](#sql545--where-extractmonth-from-x--13--extractdow-from-x--7) | `WHERE EXTRACT(MONTH FROM x) = 13` / `EXTRACT(DOW FROM x) = 7` |
| [`sql546`](#sql546--where-x--7--7) | `WHERE x % 7 = 7` |
| [`sql547`](#sql547--where-array-lengtharr-1--0) | `WHERE array_length(arr, 1) = 0` |
| [`sql548`](#sql548--col--allarray1-2-3) | `col <> ALL(ARRAY[1, 2, 3])` |
| [`sql549`](#sql549--from-users-as-users--join-orders-orders) | `FROM users AS users` / `JOIN orders orders` |
| [`sql550`](#sql550--where-x--5-and-x--3) | `WHERE x > 5 AND x > 3` |
| [`sql551`](#sql551--redundantly-nested-functions-whose-outer-call-subsumes-the-inner-one--upperlowerx--lowerupperx) | redundantly nested functions whose outer call subsumes the inner one: * `upper(lower(x))` / `lower(upper(x))`... |
| [`sql552`](#sql552--where-absx--0--cardinalityarr---1) | `WHERE abs(x) < 0` / `cardinality(arr) = -1` |
| [`sql553`](#sql553--create-table-t-col-int-default-null) | `CREATE TABLE t (col int DEFAULT NULL)` |
| [`sql554`](#sql554--the-operator-spellings-of-like) | the operator spellings of LIKE |
| [`sql555`](#sql555--where-active-is-true--where-active-is-false) | `WHERE active IS TRUE` / `WHERE active IS FALSE` |
| [`sql556`](#sql556--col--anyarray1-2-3) | `col = ANY(ARRAY[1, 2, 3])` |
| [`sql557`](#sql557--create-table-t-id-int-id-text) | `CREATE TABLE t (id int, id text)` |
| [`sql558`](#sql558--a-create-table-with-more-than-one-primary-key-definition-eg) | a `CREATE TABLE` with more than one PRIMARY KEY definition (e.g |
| [`sql559`](#sql559--create-index-idx-on-t-a-b-a) | `CREATE INDEX idx ON t (a, b, a)` |
| [`sql560`](#sql560--foreign-key-a-b-references-t-c) | `FOREIGN KEY (a, b) REFERENCES t (c)` |
| [`sql561`](#sql561--select--limit-all) | `SELECT ... LIMIT ALL` |
| [`sql562`](#sql562--col-int-default-select-maxid-from-t) | `col int DEFAULT (SELECT max(id) FROM t)` |
| [`sql563`](#sql563--col--anyarray1-2-1) | `col = ANY(ARRAY[1, 2, 1])` |
| [`sql564`](#sql564--create-table-t-a-int-null-not-null) | `CREATE TABLE t (a int NULL NOT NULL)` |
| [`sql565`](#sql565--col---col-always-0-and-col--col-always-1-or-a-division-by-zero-error-when-col-is-0-subtracting) | `col - col` (always 0) and `col / col` (always 1, or a division-by-zero error when `col` is 0). Subtracting... |
| [`sql566`](#sql566--where-x--x--1) | `WHERE x = x + 1` |
| [`sql567`](#sql567--common-built-in-functions-called-with-too-few-arguments) | common built-in functions called with too few arguments |
| [`sql568`](#sql568--col--abc) | `col ~ 'abc'` |
| [`sql569`](#sql569--exists-select--order-by-) | `EXISTS (SELECT ... ORDER BY ...)` |
| [`sql570`](#sql570--exists-select-distinct-) | `EXISTS (SELECT DISTINCT ...)` |
| [`sql571`](#sql571--create-role-app-password-hunter2) | `CREATE ROLE app PASSWORD 'hunter2'` |
| [`sql572`](#sql572--create-role-deploy-superuser--alter-role-app-superuser) | `CREATE ROLE deploy SUPERUSER` / `ALTER ROLE app SUPERUSER` |
| [`sql573`](#sql573--create-role-etl-bypassrls--alter-role-app-bypassrls) | `CREATE ROLE etl BYPASSRLS` / `ALTER ROLE app BYPASSRLS` |
| [`sql574`](#sql574--alter-table-t-disable-row-level-security) | `ALTER TABLE t DISABLE ROW LEVEL SECURITY` |
| [`sql575`](#sql575--create-policy-p-on-t-using-true-or-with-check-true) | `CREATE POLICY p ON t USING (true)` (or `WITH CHECK (true)`) |
| [`sql576`](#sql576--alter-table-t-disable-trigger-all) | `ALTER TABLE t DISABLE TRIGGER ALL` |
| [`sql577`](#sql577--create-view-v-as-select--order-by-x) | `CREATE VIEW v AS SELECT ... ORDER BY x` |
| [`sql578`](#sql578--create-rule-) | `CREATE RULE ...` |
| [`sql579`](#sql579---with-autovacuum-enabled--false--alter-table-t-set-autovacuum-enabled--off) | `... WITH (autovacuum_enabled = false)` / `ALTER TABLE t SET (autovacuum_enabled = off)` |
| [`sql580`](#sql580--create-unlogged-table--or-alter-table--set-unlogged) | `CREATE UNLOGGED TABLE ...` (or `ALTER TABLE ... SET UNLOGGED`) |
| [`sql581`](#sql581--a-json-column-type-or-json-cast-jsonb-is-almost-always-the-better-choice-its-stored-decomposed) | a `json` column type (or `::json` cast). `jsonb` is almost always the better choice: it's stored decomposed... |
| [`sql582`](#sql582--the-money-column-type) | the `money` column type |
| [`sql583`](#sql583--exists-select--group-by-x-with-no-having) | `EXISTS (SELECT ... GROUP BY x)` with no HAVING |
| [`sql584`](#sql584--the-internal-pg-catalog-type-aliases-int4-int8-float8-serial4--in-ddl) | the internal `pg_catalog` type aliases (`int4`, `int8`, `float8`, `serial4`, ...) in DDL |
| [`sql585`](#sql585--a-cluster-command-it-physically-rewrites-the-whole-table-in-index-order-under-an-access-exclusive-lock) | a `CLUSTER` command. It physically rewrites the whole table in index order under an ACCESS EXCLUSIVE lock... |
| [`sql586`](#sql586--vacuum-full-rewrites-the-entire-table-and-its-indexes-into-new-files-under-an-access-exclusive-lock) | `VACUUM FULL` rewrites the entire table (and its indexes) into new files under an ACCESS EXCLUSIVE lock... |
| [`sql587`](#sql587--alter-table-t-add-column-c-uuid-default-gen-random-uuid) | `ALTER TABLE t ADD COLUMN c uuid DEFAULT gen_random_uuid()` |
| [`sql588`](#sql588--alter-table-t-add-primary-key---add-unique-) | `ALTER TABLE t ADD PRIMARY KEY (...)` / `ADD UNIQUE (...)` |
| [`sql589`](#sql589--alter-table-t-add-constraint-fk-foreign-key-a-references-b-c-without-not-valid) | `ALTER TABLE t ADD CONSTRAINT fk FOREIGN KEY (a) REFERENCES b (c)` without `NOT VALID` |
| [`sql590`](#sql590--a-reindex-without-concurrently) | a `REINDEX` without `CONCURRENTLY` |
| [`sql591`](#sql591--values-1-2-3-4-5) | `VALUES (1, 2), (3, 4, 5)` |
| [`sql592`](#sql592--where-1--where-0) | `WHERE 1` / `WHERE 0` |
| [`sql593`](#sql593--limit-10-20) | `LIMIT 10, 20` |
| [`sql594`](#sql594--insert--on-duplicate-key-update-) | `INSERT ... ON DUPLICATE KEY UPDATE ...` |
| [`sql595`](#sql595--replace-into-t-) | `REPLACE INTO t ...` |
| [`sql596`](#sql596--mysql-only-functions-that-dont-exist-in-postgresql) | MySQL-only functions that don't exist in PostgreSQL |
| [`sql597`](#sql597--col-regexp-pat--col-rlike-pat) | `col REGEXP 'pat'` / `col RLIKE 'pat'` |
| [`sql598`](#sql598--use-mydb) | `USE mydb` |
| [`sql599`](#sql599--int-unsigned--bigint-unsigned) | `int unsigned` / `bigint unsigned` |
| [`sql600`](#sql600---col-) | `` `col` `` |
| [`sql601`](#sql601--varchar2n--nvarchar2n) | `VARCHAR2(n)` / `NVARCHAR2(n)` |
| [`sql602`](#sql602--decodeexpr-search-result----default) | `DECODE(expr, search, result [, ...] [, default])` |
| [`sql603`](#sql603---minus-) | `... MINUS ...` |
| [`sql604`](#sql604--clob--nclob) | `CLOB` / `NCLOB` |
| [`sql605`](#sql605--an-inline-foreign-key-column-declared-not-null-but-with-an-on-delete-set-null--on-update-set-null) | an inline foreign-key column declared `NOT NULL` but with an `ON DELETE SET NULL` / `ON UPDATE SET NULL`... |
| [`sql606`](#sql606--a-check-constraint-whose-expression-contains-a-subquery-eg) | a `CHECK` constraint whose expression contains a subquery (e.g |
| [`sql607`](#sql607--a-lengthprecision-modifier-on-a-type-that-doesnt-accept-one) | a length/precision modifier on a type that doesn't accept one |
| [`sql608`](#sql608--create-unique-index) | `CREATE UNIQUE INDEX |
| [`sql609`](#sql609--select-distinct--for-update) | `SELECT DISTINCT ... FOR UPDATE` |
| [`sql610`](#sql610--select--over---for-update) | `SELECT ... OVER (...) ... FOR UPDATE` |
| [`sql611`](#sql611--update--order-by--delete--order-by) | `UPDATE ... ORDER BY` / `DELETE ... ORDER BY` |
| [`sql612`](#sql612--an-aggregate-function-in-a-returning-list) | an aggregate function in a `RETURNING` list |
| [`sql613`](#sql613--col--generated-always-as-expr-without-the-stored-keyword-or-written--virtual-postgresql-only) | `col ... GENERATED ALWAYS AS (expr)` without the `STORED` keyword (or written `... VIRTUAL`). PostgreSQL only... |
| [`sql614`](#sql614--a-mysql-style-inline-key---index--definition-inside-create-table-postgresql-doesnt-allow) | a MySQL-style inline `KEY ...` / `INDEX ...` definition inside `CREATE TABLE`. PostgreSQL doesn't allow... |
| [`sql615`](#sql615--the-with-oids-table-option) | the `WITH OIDS` table option |
| [`sql616`](#sql616--a-mysql-character-set---charset-clause-per-column-or-per-table-postgresql-has-no-per-column-or) | a MySQL `CHARACTER SET ...` / `CHARSET=...` clause (per-column or per-table). PostgreSQL has no per-column or... |
| [`sql617`](#sql617--natural-join-and-natural-leftrightfull-join-a-natural-join-implicitly-joins-on-every-pair-of) | `NATURAL JOIN` (and `NATURAL LEFT/RIGHT/FULL JOIN`). A natural join implicitly joins on *every* pair of... |
| [`sql618`](#sql618--fetch-first-n-rows-with-ties-without-an-order-by) | `FETCH FIRST n ROWS WITH TIES` without an `ORDER BY` |
| [`sql619`](#sql619--date-truncunit--where-unit-is-a-string-literal-that-isnt-one-of-postgresqls-recognised) | `date_trunc('<unit>', ...)` where `<unit>` is a string literal that isn't one of PostgreSQL's recognised... |
| [`sql620`](#sql620--mysql--sql-server-date-arithmetic-functions-that-dont-exist-in-postgresql) | MySQL / SQL Server date arithmetic functions that don't exist in PostgreSQL |
| [`sql621`](#sql621--the-mysql-ifcond-then-else-function) | the MySQL `IF(cond, then, else)` function |
| [`sql622`](#sql622--mysql-only-string-functions-that-dont-exist-in-postgresql) | MySQL-only string functions that don't exist in PostgreSQL |
| [`sql623`](#sql623--a-mysql-inline-enumab-column-type) | a MySQL inline `ENUM('a','b',...)` column type |
| [`sql624`](#sql624--the-mysql-column-attribute-on-update-current-timestamp-auto-touch-a-timestamp-column-on-every-row-update) | the MySQL column attribute `ON UPDATE CURRENT_TIMESTAMP` (auto-touch a timestamp column on every row update) |
| [`sql625`](#sql625--the-mysql-zerofill-column-attribute-left-pads-a-numeric-column-with-zeros-on-display-and-implies) | the MySQL `ZEROFILL` column attribute (left-pads a numeric column with zeros on display, and implies... |
| [`sql626`](#sql626--mysql-only-query-modifiers--hints-that-have-no-postgresql-equivalent-and-are-syntax-errors-in-pg) | MySQL-only query modifiers / hints that have no PostgreSQL equivalent and are syntax errors in PG |
| [`sql627`](#sql627--the-mysql-infix-operators-xor-logical-exclusive-or-and-div-integer-division) | the MySQL infix operators `XOR` (logical exclusive-or) and `DIV` (integer division) |
| [`sql628`](#sql628--scalar-functions-from-oracle--sql-server--mysql-that-dont-exist-in-postgresql) | scalar functions from Oracle / SQL Server / MySQL that don't exist in PostgreSQL |
| [`sql629`](#sql629--sql-server-t-sql-data-types-that-dont-exist-in-postgresql) | SQL Server (T-SQL) data types that don't exist in PostgreSQL |
| [`sql630`](#sql630--sql-server-t-sql-identity--guid-functions-that-dont-exist-in-postgresql) | SQL Server (T-SQL) identity / GUID functions that don't exist in PostgreSQL |
| [`sql631`](#sql631--last-value--nth-value-over-a-window-that-has-an-order-by-but-no-explicit-frame-clause-the) | `last_value(...)` / `nth_value(...)` over a window that has an `ORDER BY` but no explicit frame clause. The... |
| [`sql632`](#sql632--the-server-side-large-object-file-functions-lo-importpath-and-lo-exportoid-path-they-readwrite) | the server-side large-object file functions `lo_import('path')` and `lo_export(oid, 'path')`. They read/write... |
| [`sql633`](#sql633--server-side-filesystem-functions-pg-read-file-pg-read-binary-file-pg-ls-dir-and-pg-stat-file) | server-side filesystem functions `pg_read_file`, `pg_read_binary_file`, `pg_ls_dir`, and `pg_stat_file` |
| [`sql634`](#sql634--gen-saltmd5--des--xdes-from-pgcrypto-these-algorithms-are-weak-for-password-hashing) | `gen_salt('md5' \| 'des' \| 'xdes')` from pgcrypto. These algorithms are weak for password hashing |
| [`sql635`](#sql635--a-pragma--statement) | a `PRAGMA ...` statement |
| [`sql636`](#sql636--the-sqlite-autoincrement-keyword-one-word-eg) | the SQLite `AUTOINCREMENT` keyword (one word, e.g |
| [`sql637`](#sql637--the-sqlite-glob-operator-case-sensitive-unix-glob-pattern-match-eg) | the SQLite `GLOB` operator (case-sensitive, Unix-glob pattern match, e.g |
| [`sql638`](#sql638--sqlite-only-functions-that-dont-exist-in-postgresql) | SQLite-only functions that don't exist in PostgreSQL |
| [`sql639`](#sql639--more-cross-dialect-string-functions-absent-from-postgresql) | more cross-dialect string functions absent from PostgreSQL |
| [`sql640`](#sql640--mysql-date-part-functions-with-no-postgresql-equivalent) | MySQL date-part functions with no PostgreSQL equivalent |
| [`sql641`](#sql641--a-default-of-the-special-relative-datetime-strings-now-today-tomorrow-or-yesterday) | a `DEFAULT` of the special relative date/time strings `'now'`, `'today'`, `'tomorrow'`, or `'yesterday'`.... |
| [`sql642`](#sql642--mysql-file-io-syntax) | MySQL file-I/O syntax |
| [`sql643`](#sql643--oracle-scalar-functions-absent-from-postgresql) | Oracle scalar functions absent from PostgreSQL |
| [`sql644`](#sql644--mysql-date-arithmetic-functions-absent-from-postgresql) | MySQL date-arithmetic functions absent from PostgreSQL |
| [`sql645`](#sql645--a-set-returning-function-generate-series-unnest-jsonb-array-elements-regexp-split-to-table) | a set-returning function (`generate_series`, `unnest`, `jsonb_array_elements`, `regexp_split_to_table`... |
| [`sql646`](#sql646--countdistinct--or-any-aggregate-with-distinct--postgresql-doesnt-support-distinct--inside-an) | `count(DISTINCT *)` (or any aggregate with `DISTINCT *`). PostgreSQL doesn't support `DISTINCT *` inside an... |
| [`sql647`](#sql647--col-in-select-a-b) | `<col> IN (SELECT a, b, |
| [`sql648`](#sql648--tablesample-system-p--tablesample-bernoulli-p-where-the-literal-sampling-percentage-p-is-outside-0) | `TABLESAMPLE SYSTEM (p)` / `TABLESAMPLE BERNOULLI (p)` where the literal sampling percentage `p` is outside `0 |
| [`sql649`](#sql649--insert--on-conflict-do-update--with-no-conflict-target-do-update-needs-to-know-which-unique) | `INSERT ... ON CONFLICT DO UPDATE ...` with no conflict target. `DO UPDATE` needs to know *which* unique... |
| [`sql650`](#sql650--a-row-constructor-comparison-with-unequal-arity-eg) | a row-constructor comparison with unequal arity, e.g |
| [`sql651`](#sql651--a-set-returning-function-generate-series-unnest-jsonb-array-elements--in-a-group-by) | a set-returning function (`generate_series`, `unnest`, `jsonb_array_elements`, ...) in a `GROUP BY`... |
| [`sql652`](#sql652--two-common-table-expressions-in-the-same-with-clause-share-a-name-eg) | two common-table expressions in the same `WITH` clause share a name, e.g |
| [`sql653`](#sql653--an-aggregate-function-inside-a-check-constraint-eg) | an aggregate function inside a `CHECK` constraint, e.g |
| [`sql654`](#sql654--an-aggregate-function-in-a-create-index-expression-eg) | an aggregate function in a `CREATE INDEX` expression, e.g |
| [`sql655`](#sql655--a-multi-column-update-assignment-whose-column-list-and-value-list-have-different-lengths-eg) | a multi-column UPDATE assignment whose column list and value list have different lengths, e.g |
| [`sql656`](#sql656--a-truncate-statement-with-a-where-clause-truncate-removes-all-rows-of-a-table-and-accepts-no-row-filter) | a `TRUNCATE` statement with a `WHERE` clause. TRUNCATE removes *all* rows of a table and accepts no row filter |
| [`sql657`](#sql657--an-order-by-that-appears-after-limit--offset--fetch-at-the-top-level-of-a-query) | an `ORDER BY` that appears after `LIMIT` / `OFFSET` / `FETCH` at the top level of a query |
| [`sql658`](#sql658--both-a-limit-clause-and-a-fetch-firstnext--rows-clause-in-the-same-query-level-theyre-two-spellings) | both a `LIMIT` clause and a `FETCH FIRST/NEXT ... ROWS` clause in the same query level. They're two spellings... |
| [`sql659`](#sql659--a-where-clause-that-appears-after-group-by-at-the-top-level-of-a-query-sql-fixes-the-order-as--where) | a `WHERE` clause that appears after `GROUP BY` at the top level of a query. SQL fixes the order as `... WHERE... |
| [`sql660`](#sql660--a-cross-join-with-an-on-or-using-clause) | a `CROSS JOIN` with an `ON` or `USING` clause |
| [`sql661`](#sql661--a-window-only-function-row-number-rank-dense-rank-lag-lead-ntile-first-value-) | a window-only function (`row_number`, `rank`, `dense_rank`, `lag`, `lead`, `ntile`, `first_value`, ...)... |
| [`sql662`](#sql662--select-distinct-on-expr-without-parentheses-around-the-expression-list) | `SELECT DISTINCT ON <expr>` without parentheses around the expression list |
| [`sql663`](#sql663--an-order-by--limit--offset--fetch-clause-that-appears-before-a-set-operation-union-) | an `ORDER BY` / `LIMIT` / `OFFSET` / `FETCH` clause that appears before a set operation (`UNION` /... |
| [`sql664`](#sql664--a-having-clause-that-appears-before-group-by-at-the-top-level) | a `HAVING` clause that appears before `GROUP BY` at the top level |
| [`sql665`](#sql665--an-update-whose-where-clause-comes-before-set-eg) | an `UPDATE` whose `WHERE` clause comes before `SET`, e.g |
| [`sql666`](#sql666--insert-ignore-into-) | `INSERT IGNORE INTO ...` |
| [`sql667`](#sql667--mysqls-insert-into-t-set-a--1-b--2-assignment-list-syntax) | MySQL's `INSERT INTO t SET a = 1, b = 2` assignment-list syntax |
| [`sql668`](#sql668--a-delete-whose-first-token-isnt-from-eg) | a `DELETE` whose first token isn't `FROM`, e.g |
| [`sql669`](#sql669--mysqls-select) | MySQL's `SELECT |
| [`sql670`](#sql670--a-mysql-show-tables--show-databases--show-columns--show-create-table-) | a MySQL `SHOW TABLES` / `SHOW DATABASES` / `SHOW COLUMNS` / `SHOW CREATE TABLE` / |
| [`sql671`](#sql671--a-describe-t--desc-t-statement-mysql--oracle-table-introspection) | a `DESCRIBE t` / `DESC t` statement (MySQL / Oracle table introspection) |
| [`sql672`](#sql672--mysqls-alter-table) | MySQL's `ALTER TABLE |
| [`sql673`](#sql673--x-between-null-and-y--x-between-y-and-null) | `x BETWEEN NULL AND y` / `x BETWEEN y AND NULL` |
| [`sql674`](#sql674--a-ranking-window-function-with-an-explicit-frame-clause-eg-row-number-over-order-by-x-rows-between) | a ranking window function with an explicit frame clause, e.g. `ROW_NUMBER() OVER (ORDER BY x ROWS BETWEEN... |
| [`sql675`](#sql675--select-distinct--union-select-) | `SELECT DISTINCT ... UNION SELECT ...` |
| [`sql676`](#sql676--countdistinct-1--countdistinct-x) | `COUNT(DISTINCT 1)` / `COUNT(DISTINCT 'x')` |
| [`sql677`](#sql677--x--1--modx-1) | `x % 1` / `MOD(x, 1)` |
| [`sql678`](#sql678--the-mysql-zero-date-literal-0000-00-00-or-0000-00-00-000000-mysql-accepts-it-as-a-placeholder) | the MySQL "zero date" literal `'0000-00-00'` (or `'0000-00-00 00:00:00'`). MySQL accepts it as a placeholder... |
| [`sql679`](#sql679--lefts-0--rights-0) | `left(s, 0)` / `right(s, 0)` |
| [`sql680`](#sql680--substrings-from-n-for-0--substrs-n-0) | `substring(s FROM n FOR 0)` / `substr(s, n, 0)` |
| [`sql681`](#sql681--x--0--0--x) | `x * 0` / `0 * x` |
| [`sql682`](#sql682--coalescecount-0) | `COALESCE(COUNT(...), 0)` |
| [`sql683`](#sql683--case-when-true-then---case-when-false-then-) | `CASE WHEN TRUE THEN ...` / `CASE WHEN FALSE THEN ...` |
| [`sql684`](#sql684--greatesta-null-b--leastx-null) | `GREATEST(a, NULL, b)` / `LEAST(x, NULL)` |
| [`sql685`](#sql685--power1-x-always-returns-1) | `power(1, x)` always returns 1 |
| [`sql686`](#sql686--not-not-x--not-not-x) | `NOT NOT x` / `NOT (NOT x)` |
| [`sql687`](#sql687--coalescex-) | `COALESCE('x', ...)` |
| [`sql688`](#sql688--concat-wsnull-a-b) | `concat_ws(NULL, a, b)` |
| [`sql689`](#sql689--col--col) | `col % col` |
| [`sql690`](#sql690--sqrt-1) | `sqrt(-1)` |
| [`sql691`](#sql691--mindistinct-x--maxdistinct-x) | `min(DISTINCT x)` / `max(DISTINCT x)` |
| [`sql692`](#sql692--ln0--ln-1--log0--log-5) | `ln(0)` / `ln(-1)` / `log(0)` / `log(-5)` |
| [`sql693`](#sql693--log1-x) | `log(1, x)` |
| [`sql694`](#sql694--acos2--asin-3) | `acos(2)` / `asin(-3)` |
| [`sql695`](#sql695--an-aggregate-call-nested-directly-inside-another-aggregate-eg) | an aggregate call nested directly inside another aggregate, e.g |
| [`sql696`](#sql696--countcoalescex-0) | `count(coalesce(x, 0))` |
| [`sql697`](#sql697--degreesradiansx--radiansdegreesx) | `degrees(radians(x))` / `radians(degrees(x))` |
| [`sql698`](#sql698--chr0) | `chr(0)` |
| [`sql699`](#sql699--lpads-0--rpads-0) | `lpad(s, 0)` / `rpad(s, 0)` |
| [`sql700`](#sql700--setseed2) | `setseed(2)` |
| [`sql701`](#sql701--nullifa-b--nullif1-2) | `NULLIF('a', 'b')` / `NULLIF(1, 2)` |
| [`sql702`](#sql702--coalescex-0-is-null) | `COALESCE(x, 0) IS NULL` |
| [`sql703`](#sql703--ntile0--ntile-2) | `ntile(0)` / `ntile(-2)` |
| [`sql704`](#sql704--nth-valuex-0--nth-valuex--1) | `nth_value(x, 0)` / `nth_value(x, -1)` |
| [`sql705`](#sql705--width-bucketx-lo-hi-0) | `width_bucket(x, lo, hi, 0)` |
| [`sql706`](#sql706--array-to-stringarr-null) | `array_to_string(arr, NULL)` |
| [`sql707`](#sql707--lagx-0--leadx-0) | `lag(x, 0)` / `lead(x, 0)` |
| [`sql708`](#sql708--lpads-n-null--rpads-n-null) | `lpad(s, n, NULL)` / `rpad(s, n, NULL)` |
| [`sql709`](#sql709--jsonb-typeofx--int) | `jsonb_typeof(x) = 'int'` |
| [`sql710`](#sql710--coalescex-0-is-not-null) | `COALESCE(x, 0) IS NOT NULL` |
| [`sql711`](#sql711--make-date2024-13-1) | `make_date(2024, 13, 1)` |
| [`sql712`](#sql712--make-time25-0-0) | `make_time(25, 0, 0)` |
| [`sql713`](#sql713--x--0--0--x) | `x & 0` / `0 & x` |
| [`sql714`](#sql714--col--col--col--col) | `col & col` / `col \| col` |
| [`sql715`](#sql715--starts-withx-) | `starts_with(x, '')` |
| [`sql716`](#sql716--translates--to) | `translate(s, '', to)` |
| [`sql717`](#sql717--to-charx-) | `to_char(x, '')` |
| [`sql718`](#sql718--repeats-1) | `repeat(s, 1)` |
| [`sql719`](#sql719--create-sequence--increment-0-or-increment-by-0-also-in-alter-sequence) | `CREATE SEQUENCE ... INCREMENT 0` (or `INCREMENT BY 0`, also in `ALTER SEQUENCE`) |
| [`sql720`](#sql720--power0--1) | `power(0, -1)` |
| [`sql721`](#sql721--make-timestamp2024-13-1-0-0-0) | `make_timestamp(2024, 13, 1, 0, 0, 0)` |
| [`sql722`](#sql722--factorial-1) | `factorial(-1)` |
| [`sql723`](#sql723--array-cata---array-catarray-a) | `array_cat(a, '{}')` / `array_cat(ARRAY[], a)` |
| [`sql724`](#sql724--numeric2000--decimal0-0) | `NUMERIC(2000)` / `DECIMAL(0, 0)` |
| [`sql725`](#sql725--random--1--random--0) | `random() >= 1` / `random() < 0` |
| [`sql726`](#sql726--ascii) | `ascii('')` |
| [`sql727`](#sql727--explnx--lnexpx) | `exp(ln(x))` / `ln(exp(x))` |
| [`sql728`](#sql728--x--0--0--x) | `x \| 0` / `0 \| x` |
| [`sql729`](#sql729--x--0--x--0) | `x << 0` / `x >> 0` |
| [`sql730`](#sql730--chr2000000--chr-1) | `chr(2000000)` / `chr(-1)` |
| [`sql731`](#sql731--ln1--log1) | `ln(1)` / `log(1)` |
| [`sql732`](#sql732--acosh0--atanh1) | `acosh(0)` / `atanh(1)` |
| [`sql733`](#sql733--a-string-literal-containing-password) | a string literal containing `password=...` |
| [`sql734`](#sql734--x-ilike-plain) | `x ILIKE 'plain'` |
| [`sql735`](#sql735--exists-select-count-from-) | `EXISTS (SELECT count(*) FROM ...)` |
| [`sql736`](#sql736--width-bucketx-5-5-10) | `width_bucket(x, 5, 5, 10)` |
| [`sql737`](#sql737--date-bin0-seconds-ts-origin) | `date_bin('0 seconds', ts, origin)` |
| [`sql738`](#sql738--comparing-a-never-negative-function-against-a-negative-value-or--0-so-the-predicate-never-matches) | comparing a never-negative function against a negative value (or `< 0`), so the predicate never matches |
| [`sql739`](#sql739--xintint--a--btexttext) | `x::int::int` / `(a \|\| b)::text::text` |
| [`sql740`](#sql740--not-true--not-false) | `NOT TRUE` / `NOT FALSE` |
| [`sql741`](#sql741--x---1--modx--1) | `x % -1` / `MOD(x, -1)` |
| [`sql742`](#sql742--array-removearr-null) | `array_remove(arr, NULL)` |
| [`sql743`](#sql743--array-replacearr-x-x) | `array_replace(arr, x, x)` |
| [`sql744`](#sql744--array-positionarr-x--0) | `array_position(arr, x) = 0` |
| [`sql745`](#sql745--date-partyearr-ts) | `date_part('yearr', ts)` |
| [`sql746`](#sql746--int4range5-1--numrange10-2) | `int4range(5, 1)` / `numrange(10, 2)` |
| [`sql747`](#sql747--percentile-cont15-within-group-) | `percentile_cont(1.5) WITHIN GROUP (...)` |
| [`sql748`](#sql748--encodedata-base32) | `encode(data, 'base32')` |
| [`sql749`](#sql749--daterange2024-01-01-2023-01-01) | `daterange('2024-01-01', '2023-01-01')` |
| [`sql750`](#sql750--b1021) | `B'1021'` |
| [`sql751`](#sql751--x1g) | `X'1G'` |
| [`sql752`](#sql752--like-a-escape-) | `LIKE 'a%' ESCAPE '\\!'` |
| [`sql753`](#sql753--setweighttsv-e) | `setweight(tsv, 'E')` |
| [`sql754`](#sql754--to-tsqueryquick-brown-fox) | `to_tsquery('quick brown fox')` |
| [`sql755`](#sql755--countdistinct-a-b) | `count(DISTINCT a, b)` |
| [`sql756`](#sql756--string-aggx--jsonb-object-aggk) | `string_agg(x)` / `jsonb_object_agg(k)` |
| [`sql757`](#sql757--a-partitioned-tables-primary-key-does-not-include-every-partition-key-column-postgresql-requires-every) | a partitioned table's PRIMARY KEY does not include every partition key column. PostgreSQL requires every... |
| [`sql758`](#sql758--for-values-from-x-to-y-where-the-lower-partition-bound-is-not-strictly-less-than-the-upper-bound) | `FOR VALUES FROM (x) TO (y)` where the lower partition bound is not strictly less than the upper bound.... |
| [`sql759`](#sql759--partition-by-rangelisthash-some-volatile-fncol) | `PARTITION BY RANGE/LIST/HASH (some_volatile_fn(col))` |
| [`sql760`](#sql760--partition-by-rangelisthash-a-a) | `PARTITION BY RANGE/LIST/HASH (a, a)` |
| [`sql761`](#sql761--alter-table--detach-partition--concurrently-inside-an-explicit-transaction-like-drop-index) | `ALTER TABLE ... DETACH PARTITION ... CONCURRENTLY` inside an explicit transaction. Like `DROP INDEX... |
| [`sql762`](#sql762--for-values-with-modulus-m-remainder-r-where-the-remainder-is-not-less-than-the-modulus) | `FOR VALUES WITH (MODULUS m, REMAINDER r)` where the remainder is not less than the modulus |
| [`sql763`](#sql763--json-existsdoc-literal-where-the-literal-path-string-does-not-start-with-) | `JSON_EXISTS(doc, 'literal')` where the literal path string does not start with `$` |
| [`sql764`](#sql764--json-value) | `JSON_VALUE( |
| [`sql765`](#sql765--json-query-with-wrapper--omit-quotes) | `JSON_QUERY(... WITH WRAPPER ... OMIT QUOTES)` |
| [`sql766`](#sql766--json-table-columns-a--a-) | `JSON_TABLE(... COLUMNS (a ..., a ...))` |
| [`sql767`](#sql767--col-is-json-where-the-catalog-already-types-col-as-json-or-jsonb) | `col IS JSON` where the catalog already types `col` as `json` or `jsonb` |
| [`sql768`](#sql768--expr-is-json-object-and-same-expr-is-json-array-or-any-two-different-is-json-kinds-directly-anded) | `<expr> IS JSON OBJECT AND <same expr> IS JSON ARRAY` (or any two different IS JSON kinds directly ANDed... |
| [`sql769`](#sql769--cycle) | `CYCLE |
| [`sql770`](#sql770--the-recursive-term-references-the-cte-itself-more-than-once) | the recursive term references the CTE itself more than once |
| [`sql771`](#sql771--the-recursive-term-contains-an-aggregate-function-call) | the recursive term contains an aggregate function call |
| [`sql772`](#sql772--the-recursive-term-contains-a-top-level-order-by-limit-or-distinct) | the recursive term contains a top-level ORDER BY, LIMIT, or DISTINCT |
| [`sql773`](#sql773--the-recursive-terms-self-reference-sits-on-the-nullable-side-of-an-outer-join) | the recursive term's self-reference sits on the nullable side of an outer join |
| [`sql774`](#sql774--exclude-using-am-col-with-op-col-with-op) | `EXCLUDE USING <am> (col WITH op, col WITH op)` |
| [`sql775`](#sql775--exclude-using-btreehashbringin-) | `EXCLUDE USING btree/hash/brin/gin (...)` |
| [`sql776`](#sql776--exclude-using-gist-col-with--on-a-single-column-with-only-the--operator) | `EXCLUDE USING gist (col WITH =)` on a single column with only the `=` operator |
| [`sql777`](#sql777--create-domain--check-expr-where-expr-never-references-value) | `CREATE DOMAIN ... CHECK (expr)` where `expr` never references `VALUE` |
| [`sql778`](#sql778--create-domain--check-value-op-literal-default-literal-where-the-default-literal-plainly-fails) | `CREATE DOMAIN ... CHECK (VALUE <op> <literal>) DEFAULT <literal>` where the DEFAULT literal plainly fails... |
| [`sql779`](#sql779--create-type--as-a-int-a-text) | `CREATE TYPE ... AS (a int, a text)` |
| [`sql780`](#sql780--jsonb-path-existsdoc-a--1--2) | `jsonb_path_exists(doc, '$.a ? (1 == 2)')` |
| [`sql781`](#sql781--jsonb-array-lengtha1jsonb) | `jsonb_array_length('{"a":1}'::jsonb)` |
| [`sql782`](#sql782--a1jsonb---0) | `'{"a":1}'::jsonb - 0` |
| [`sql783`](#sql783--jsonb-build-objectnull-1-) | `jsonb_build_object(NULL, 1, ...)` |
| [`sql784`](#sql784--grouping-sets-ab-ab) | `GROUPING SETS ((a,b), (a,b))` |
| [`sql785`](#sql785--groupingx-where-x-does-not-appear-anywhere-in-the-statements-group-by-clause) | `GROUPING(x)` where `x` does not appear anywhere in the statement's GROUP BY clause |
| [`sql786`](#sql786--rollup-a-a--cube-a-a) | `ROLLUP (a, a)` / `CUBE (a, a)` |
| [`sql787`](#sql787--a-parenthesized-select--subquery-correlated-to-the-outer-query-references-a-qualified-column-whose) | a parenthesized `(SELECT ...)` subquery, correlated to the outer query (references a qualified column whose... |
| [`sql788`](#sql788--a-lateral--subquery-references-a-table-alias-thats-introduced-later-in-the-same-fromjoin-list) | a `LATERAL (...)` subquery references a table alias that's introduced later in the same FROM/JOIN list |
| [`sql789`](#sql789--a-full-outer-join-b-on--where-bcol--x) | `a FULL [OUTER] JOIN b ON ... WHERE b.col = 'x'` |
| [`sql790`](#sql790--col-type-not-null-unique-nulls-not-distinct) | `col type NOT NULL UNIQUE NULLS NOT DISTINCT` |
| [`sql791`](#sql791--create-statistics-name-ndistinct-or-dependencies-with-fewer-than-2-columnsexpressions-in-the-on) | `CREATE STATISTICS name (ndistinct)` (or `dependencies`) with fewer than 2 columns/expressions in the `ON`... |
| [`sql792`](#sql792--create-statistics--on-a-a-from-t) | `CREATE STATISTICS ... ON a, a FROM t` |
| [`sql793`](#sql793--an-unconditional-when-matched-then-clause-appears-before-another-when-matched-and--then-clause-in) | an unconditional `WHEN MATCHED THEN` clause appears before another `WHEN MATCHED [AND ...] THEN` clause in... |
| [`sql794`](#sql794--when-not-matched-then-insert--values-targetcol-) | `WHEN NOT MATCHED THEN INSERT ... VALUES (target.col, ...)` |
| [`sql795`](#sql795--create-publication--for-tables-in-schema-s-s) | `CREATE PUBLICATION ... FOR TABLES IN SCHEMA s, s` |
| [`sql796`](#sql796--create-subscription--with-create-slot--false-with-no-slot-name) | `CREATE SUBSCRIPTION ... WITH (create_slot = false)` with no `slot_name` |
| [`sql797`](#sql797--create-publication--for-table-a-a) | `CREATE PUBLICATION ... FOR TABLE a, a` |
| [`sql798`](#sql798--a-bare-loop--end-loop-not-forwhile-whose-body-contains-no-exit-return-or-raise-anywhere) | a bare `LOOP ... END LOOP` (not FOR/WHILE) whose body contains no `EXIT`, `RETURN`, or `RAISE` anywhere |
| [`sql799`](#sql799--a-for-i-in--loop-variable-name-shadows-a-column-that-exists-somewhere-in-the-connected-catalog) | a `FOR i IN ...` loop variable name shadows a column that exists somewhere in the connected catalog |
| [`sql800`](#sql800--exception-when-others-then-with-an-empty-or-null-only-body) | `EXCEPTION WHEN OTHERS THEN` with an empty or `NULL;`-only body |
| [`sql801`](#sql801--execute-dynamic-sql-using-a-b-where-the-highest-n-placeholder-referenced-in-the-dynamic-sql-text) | `EXECUTE <dynamic sql> USING a, b` where the highest `$N` placeholder referenced in the dynamic SQL text... |
| [`sql802`](#sql802--execute-literal-select-into-a-b-where-the-statically-known-select-list-column-count-doesnt-match-the) | `EXECUTE '<literal SELECT>' INTO a, b` where the statically-known SELECT-list column count doesn't match the... |
| [`sql803`](#sql803--raise-notice-appears-inside-a-loop-body-bare-loop-for--loop-or-while--loop) | `RAISE NOTICE` appears inside a loop body (bare `LOOP`, `FOR ... LOOP`, or `WHILE ... LOOP`) |
| [`sql804`](#sql804--a-plpgsql-declare-x-type-variable-thats-never-referenced-anywhere-after-begin) | a PL/pgSQL `DECLARE x type;` variable that's never referenced anywhere after `BEGIN` |

## Rules

### `sql001` — table referenced by FROM / JOIN / UPDATE / DELETE / INSERT INTO does not exist in the catalog

table referenced by FROM / JOIN / UPDATE / DELETE / INSERT INTO does not exist in the catalog.

<sub>`dsl-analysis/src/rules/unresolved_table.rs`</sub>

### `sql002` — column reference does not exist in any in-scope table

column reference does not exist in any in-scope table.

<sub>`dsl-analysis/src/rules/unknown_column.rs`</sub>

### `sql003` — unqualified column reference exists in more than one in-scope table; the user must qualify it

unqualified column reference exists in more than one in-scope table; the user must qualify it.

<sub>`dsl-analysis/src/rules/ambiguous_column.rs`</sub>

### `sql010` — UNION / INTERSECT / EXCEPT column-count mismatch

UNION / INTERSECT / EXCEPT column-count mismatch. Each arm of a set operation must project the same number of columns. The internal AST does not model UNION yet so we tokenise the statement text, split on the top-level UNION / INTERSECT / EXCEPT keywords, and count comma-separated projection expressions in each arm. Sub-queries inside an arm have their parens skipped so an arm with a column list `SELECT a, (SELECT max(b) FROM t), c` is counted as 3.

<sub>`dsl-analysis/src/rules/union_column_count.rs`</sub>

### `sql013` — UPDATE or DELETE without a WHERE clause

UPDATE or DELETE without a WHERE clause. Almost always a bug waiting to clear out a whole table.

<sub>`dsl-analysis/src/rules/mutating_without_where.rs`</sub>

### `sql014` — implicit cross join

implicit cross join. `FROM a, b WHERE ...` without a join predicate between `a` and `b` produces a Cartesian product. Usually a missing `ON` clause.

<sub>`dsl-analysis/src/rules/implicit_cross_join.rs`</sub>

### `sql015` — comparison with NULL using `=` or `<>` (or `!=`). Always yields NULL; the user almost always meant `IS NULL`...

comparison with NULL using `=` or `<>` (or `!=`). Always yields NULL; the user almost always meant `IS NULL` / `IS NOT NULL`. Detection is text-level on the statement source slice -- our Expr type stringifies binary ops, so a structural walk doesn't help.

<sub>`dsl-analysis/src/rules/null_comparison.rs`</sub>

### `sql016` — `INSERT INTO t SELECT *` is arity-fragile

`INSERT INTO t SELECT *` is arity-fragile. A schema change to the source table silently corrupts the destination. Always project columns explicitly. Detection runs on the statement source slice because our Insert AST does not carry the inner SELECT today.

<sub>`dsl-analysis/src/rules/select_star_insert.rs`</sub>

### `sql017` — SELECT mixes aggregates with bare column references but the bare columns are not all listed in GROUP BY

SELECT mixes aggregates with bare column references but the bare columns are not all listed in GROUP BY. Postgres treats this as an error at execution time; we surface it at edit time. Heuristic on the statement text: 1. Find the projection slice (between SELECT and FROM). 2. Collect aggregate calls (`count(`, `sum(`, `avg(`, `min(`, `max(`, `array_agg(`, `string_agg(`, `json_agg(`, `bool_or(`, `bool_and(`). 3. Collect bare column references in the projection (identifiers not followed by `(`, not inside an aggregate). 4. If aggregates exist and any bare column is not present in the GROUP BY column list (case-insensitive whole-word match), flag.

<sub>`dsl-analysis/src/rules/group_by_required.rs`</sub>

### `sql018` — `NOT IN (subquery)` is dangerous when the subquery can return NULL. Postgres treats `x NOT IN (NULL)` as...

`NOT IN (subquery)` is dangerous when the subquery can return NULL. Postgres treats `x NOT IN (NULL)` as UNKNOWN, so the predicate never matches and the outer query silently returns zero rows. Heuristic: flag every literal `NOT IN (` followed by `SELECT`. We don't try to prove the subquery is null-free -- the recommendation is always to use `NOT EXISTS` or filter the subquery with `IS NOT NULL`.

<sub>`dsl-analysis/src/rules/not_in_subquery.rs`</sub>

### `sql020` — deprecated / non-recommended function call

deprecated / non-recommended function call. These are calls Postgres still accepts but where the preferred form differs. Surfaced as a Hint so it doesn't crowd real issues.

<sub>`dsl-analysis/src/rules/deprecated_function.rs`</sub>

### `sql021` — prefer the declared alias over the bare table name

prefer the declared alias over the bare table name. When a statement declares `FROM users AS u`, references to columns should go through the alias (`u.id`), not through the raw table (`users.id`). The aliased form is shorter, survives table renames, and avoids ambiguity in multi-table SELECTs. We surface this as a hint (not a hard error) so the diagnostic doesn't fight users who deliberately spelled the table name.

<sub>`dsl-analysis/src/rules/prefer_alias.rs`</sub>

### `sql030` — trigger function body has no RETURN

trigger function body has no RETURN. Any `CREATE FUNCTION ... RETURNS TRIGGER` must end every reachable control-flow path with `RETURN NEW;`, `RETURN OLD;`, or `RETURN NULL;`. Without it Postgres fires "control reached end of trigger procedure without RETURN" at runtime. v1 is a text-level approximation: we treat the function as buggy when the body contains no `RETURN ` keyword at all. Branch-aware analysis (every IF/ELSE arm has a RETURN) comes in a follow-up.

<sub>`dsl-analysis/src/rules/missing_trigger_return.rs`</sub>

### `sql031` — `RETURN <literal>` type doesn't match declared `RETURNS <type>`. Catches the easy literal-return mismatches...

`RETURN <literal>` type doesn't match declared `RETURNS <type>`. Catches the easy literal-return mismatches: - `RETURN 'string';` in `RETURNS INT` -> Error - `RETURN 1;` in `RETURNS TEXT` -> Error - `RETURN true;` in `RETURNS INT` -> Error Skips when the return value is anything other than a bare literal (column, expression, function call) -- those need real type inference, deferred to a follow-up rule.

<sub>`dsl-analysis/src/rules/return_type_literal.rs`</sub>

### `sql032` — bare `RETURN;` inside a function that declares a non-void return type

bare `RETURN;` inside a function that declares a non-void return type. Postgres requires `RETURN <expr>;` whenever the function isn't `RETURNS void`. A bare `RETURN;` is only legal in OUT-parameter procedures or void functions; everywhere else it's a runtime trap.

<sub>`dsl-analysis/src/rules/bare_return_typed.rs`</sub>

### `sql036` — `RAISE EXCEPTION` (or NOTICE/WARNING/etc.) format string `%` placeholder count doesn't match the supplied...

`RAISE EXCEPTION` (or NOTICE/WARNING/etc.) format string `%` placeholder count doesn't match the supplied argument count. Postgres errors with `too few parameters specified for RAISE` / `too many parameters specified for RAISE` at runtime. Catch it at edit time.

<sub>`dsl-analysis/src/rules/raise_arg_count.rs`</sub>

### `sql037` — `SELECT

`SELECT ... INTO var [, var2]` row-shape doesn't match the SELECT projection count. Postgres raises `query has too many/few columns` at runtime. Catch at edit time by counting projection commas vs INTO variable commas.

<sub>`dsl-analysis/src/rules/select_into_shape.rs`</sub>

### `sql038` — `INSERT INTO t (a, b) VALUES (1)`

`INSERT INTO t (a, b) VALUES (1)` -- column-list length must match the VALUES tuple length. Postgres raises `INSERT has more/fewer expressions than target columns`. We catch at edit time via direct text scan since the parser exposes only the column list, not the VALUES count.

<sub>`dsl-analysis/src/rules/insert_col_value_count.rs`</sub>

### `sql039` — `INSERT INTO t (col1, col2) VALUES (lit1, lit2)` literal types must match the target column types

`INSERT INTO t (col1, col2) VALUES (lit1, lit2)` literal types must match the target column types. Conservative: only flags literal kinds we can classify with high confidence (string / integer / float / boolean / NULL). Anything else (function call, expression, cast) is skipped.

<sub>`dsl-analysis/src/rules/insert_type_literal.rs`</sub>

### `sql040` — an `IMMUTABLE` function body calls a known `VOLATILE` built-in

an `IMMUTABLE` function body calls a known `VOLATILE` built-in. Postgres trusts the author's volatility annotation; planning + index optimisations rely on it. Violating purity by calling `now()`, `random()`, `gen_random_uuid()`, `clock_timestamp()`, etc. from an IMMUTABLE function silently produces wrong query plans.

<sub>`dsl-analysis/src/rules/immutable_calls_volatile.rs`</sub>

### `sql041` — `LANGUAGE sql` function body references `NEW` or `OLD`

`LANGUAGE sql` function body references `NEW` or `OLD`. NEW / OLD are PL/pgSQL trigger row aliases. A pure-SQL function has no notion of them and Postgres rejects the call at runtime. Flag at edit time so the user sees it before the deploy.

<sub>`dsl-analysis/src/rules/sql_lang_uses_new_old.rs`</sub>

### `sql042` — `UPDATE <table> SET <col> = ...` where `<col>` is not in the target table's catalog definition

`UPDATE <table> SET <col> = ...` where `<col>` is not in the target table's catalog definition. Sibling of sql002 (unknown column inside SELECT). UPDATE statements reach the catalog via `UpdateStmt.table` and assignments expose the target column name, so checking the assignments against the catalog's column list is straightforward.

<sub>`dsl-analysis/src/rules/update_set_unknown_col.rs`</sub>

### `sql043` — `DELETE FROM <tbl>` without WHERE inside a function body

`DELETE FROM <tbl>` without WHERE inside a function body. The base `sql013` rule catches DELETE-without-WHERE at top level already. This rule narrows the focus: inside a PL/pgSQL function the mistake is even more likely to wipe the table on every call. Warn.

<sub>`dsl-analysis/src/rules/delete_no_where_in_fn.rs`</sub>

### `sql044` — `EXIT` / `CONTINUE` used outside a LOOP / WHILE / FOR block

`EXIT` / `CONTINUE` used outside a LOOP / WHILE / FOR block. Postgres rejects this with `EXIT cannot be used outside a loop` at parse time on the server. We surface it sooner so the user sees the red squiggle inside the editor.

<sub>`dsl-analysis/src/rules/exit_outside_loop.rs`</sub>

### `sql045` — unreachable code after an unconditional `RETURN` or `RAISE EXCEPTION`. Postgres won't error on dead code but...

unreachable code after an unconditional `RETURN` or `RAISE EXCEPTION`. Postgres won't error on dead code but it's almost always a bug -- either the author forgot to remove obsolete code or guarded the return wrongly. Hint severity.

<sub>`dsl-analysis/src/rules/unreachable_after_return.rs`</sub>

### `sql046` — `CREATE TABLE` without a PRIMARY KEY. Heap tables without a primary key cause replication, ORM, and audit...

`CREATE TABLE` without a PRIMARY KEY. Heap tables without a primary key cause replication, ORM, and audit pain. Warn so the author adds one explicitly (and suppresses with a comment when intentionally omitting it -- e.g. log tables).

<sub>`dsl-analysis/src/rules/missing_primary_key.rs`</sub>

### `sql048` — `INSERT INTO t VALUES (...)` without a column list. Positional INSERT works but is fragile

`INSERT INTO t VALUES (...)` without a column list. Positional INSERT works but is fragile -- adding or reordering columns in the target table silently changes which column receives which value. Warn to push users toward `INSERT INTO t (c1, c2) VALUES (...)`.

<sub>`dsl-analysis/src/rules/insert_no_columns.rs`</sub>

### `sql050` — a column or table identifier in CREATE TABLE matches a PG reserved keyword. Postgres still accepts it but...

a column or table identifier in CREATE TABLE matches a PG reserved keyword. Postgres still accepts it but forces every later reference to be double-quoted -- a guaranteed paper-cut.

<sub>`dsl-analysis/src/rules/reserved_word_identifier.rs`</sub>

### `sql051` — `LIMIT` without `ORDER BY` produces non-deterministic rows

`LIMIT` without `ORDER BY` produces non-deterministic rows. PG's planner is free to return any subset matching the predicate when no ORDER BY pins the row order. Warn so the author makes the ordering explicit (or adds a comment if they really want the random sample).

<sub>`dsl-analysis/src/rules/limit_without_order.rs`</sub>

### `sql052` — `LIKE 'plain string'`

`LIKE 'plain string'` -- no wildcard means LIKE behaves exactly like `=`, and `=` is faster. Example: `WHERE name LIKE 'alice'` -> use `WHERE name = 'alice'`.

<sub>`dsl-analysis/src/rules/like_without_wildcard.rs`</sub>

### `sql054` — `WHERE x = true` / `WHERE x = false`

`WHERE x = true` / `WHERE x = false` -- redundant boolean comparison. `WHERE active = true` should be `WHERE active`. The shorter form reads better and the planner sometimes picks different paths for boolean expressions in predicate position.

<sub>`dsl-analysis/src/rules/bool_compare_equals.rs`</sub>

### `sql055` — `WHERE (single condition)`

`WHERE (single condition)` -- the parens add noise. Catches the simple case: a single `WHERE ( expr )` where the body has no top-level AND/OR. Multi-clause predicates obviously need grouping; this rule only flags the single-condition case.

<sub>`dsl-analysis/src/rules/redundant_parens.rs`</sub>

### `sql056` — `UNION` (deduplicates) is often slower than `UNION ALL` and used by mistake

`UNION` (deduplicates) is often slower than `UNION ALL` and used by mistake. Hint: when the author wrote `UNION` without explicit `DISTINCT` reasoning, suggest considering `UNION ALL` for cases where duplicate rows are impossible (different tables, disjoint predicates, etc.). We can't fully prove disjointness, so this is a soft Hint that reminds the author to think about it.

<sub>`dsl-analysis/src/rules/union_vs_all.rs`</sub>

### `sql058` — `CASE WHEN

`CASE WHEN ... THEN ... ELSE ... END` with exactly one WHEN arm. PL/pgSQL `IF`, or PG's `coalesce`/`nullif`/`iif`-like helpers read better. Hint.

<sub>`dsl-analysis/src/rules/case_single_when.rs`</sub>

### `sql061` — bare `NULL` inside `VALUES (...)` without an explicit cast

bare `NULL` inside `VALUES (...)` without an explicit cast. PG infers NULL's type from context. In a multi-row VALUES block, an untyped NULL on the first row can pin the column to TEXT and force later rows to cast. Hint: `NULL::<type>` instead.

<sub>`dsl-analysis/src/rules/null_in_values.rs`</sub>

### `sql062` — `SAVEPOINT x` declared but never `RELEASE`d (or rolled back to)

`SAVEPOINT x` declared but never `RELEASE`d (or rolled back to). Long-lived savepoints leak resources and confuse readers. v1 scope: in the same buffer, every `SAVEPOINT name` should have a matching `RELEASE [SAVEPOINT] name` or `ROLLBACK TO [SAVEPOINT] name`. Inter-file flows are out of scope.

<sub>`dsl-analysis/src/rules/savepoint_no_release.rs`</sub>

### `sql064` — `

`... JOIN tbl` not followed by `ON`/`USING` and not preceded by `CROSS` / `NATURAL`. The pg_query backend can't reliably distinguish CROSS-JOIN from a missing ON, so this rule uses text analysis.

<sub>`dsl-analysis/src/rules/join_no_on.rs`</sub>

### `sql065` — `GROUP BY 1, 2`

`GROUP BY 1, 2` -- positional grouping is brittle. A projection-list edit silently changes the grouping. Hint: use the column expression (or its alias) instead.

<sub>`dsl-analysis/src/rules/group_by_position.rs`</sub>

### `sql068` — BEGIN / COMMIT pair wrapping a single statement

BEGIN / COMMIT pair wrapping a single statement -- the transaction adds nothing. Each statement already runs in its own implicit transaction. Hint.

<sub>`dsl-analysis/src/rules/single_stmt_transaction.rs`</sub>

### `sql069` — column declared `NOT NULL` but `DEFAULT NULL`

column declared `NOT NULL` but `DEFAULT NULL`. `NOT NULL DEFAULT NULL` is contradictory; the very first row insert that omits the column will violate the NOT NULL constraint. Error.

<sub>`dsl-analysis/src/rules/null_default_not_null.rs`</sub>

### `sql072` — `SELECT ... FOR UPDATE` without a WHERE clause locks every row of the target table

`SELECT ... FOR UPDATE` without a WHERE clause locks every row of the target table -- almost always a footgun.

<sub>`dsl-analysis/src/rules/select_for_update_no_where.rs`</sub>

### `sql074` — `WHERE x IN (a, b, c, ...)` with > 50 items

`WHERE x IN (a, b, c, ...)` with > 50 items. Long IN-lists defeat the planner; suggest a temp table or `= ANY(ARRAY[...])`.

<sub>`dsl-analysis/src/rules/long_in_list.rs`</sub>

### `sql075` — column declared as `TIME WITH TIME ZONE` (alias `TIMETZ`). PG docs recommend against TIMETZ

column declared as `TIME WITH TIME ZONE` (alias `TIMETZ`). PG docs recommend against TIMETZ -- it's almost never what you want. Use `TIMESTAMP WITH TIME ZONE` (`TIMESTAMPTZ`) instead. Hint.

<sub>`dsl-analysis/src/rules/time_with_timezone.rs`</sub>

### `sql076` — `LIMIT -1` / `OFFSET -1`

`LIMIT -1` / `OFFSET -1` -- PG rejects negative values.

<sub>`dsl-analysis/src/rules/negative_limit_offset.rs`</sub>

### `sql081` — `ORDER BY random()`

`ORDER BY random()` -- slow, no index can help, runs a sort over the entire result set. Hint: TABLESAMPLE BERNOULLI for sampling.

<sub>`dsl-analysis/src/rules/order_by_random.rs`</sub>

### `sql083` — `INSERT INTO t (id, ...)` referencing the primary key without `ON CONFLICT`

`INSERT INTO t (id, ...)` referencing the primary key without `ON CONFLICT` -- the second call with the same id fails. Hint: add `ON CONFLICT (id) DO NOTHING` or `ON CONFLICT (id) DO UPDATE` when idempotency is desired.

<sub>`dsl-analysis/src/rules/insert_no_on_conflict.rs`</sub>

### `sql084` — `COUNT(1)` is equivalent to `COUNT(*)`

`COUNT(1)` is equivalent to `COUNT(*)` -- prefer `COUNT(*)` which reads more naturally and matches every style guide.

<sub>`dsl-analysis/src/rules/count_one_vs_star.rs`</sub>

### `sql085` — `NULLIF(x, x)` always returns NULL

`NULLIF(x, x)` always returns NULL -- pointless. Error.

<sub>`dsl-analysis/src/rules/nullif_same_args.rs`</sub>

### `sql087` — `x BETWEEN <high> AND <low>`

`x BETWEEN <high> AND <low>` -- bounds are flipped and the expression matches nothing. Catches numeric literal cases (constant low > constant high) and string-literal cases (lex-order swap, which is correct for TEXT columns and ISO-format date/timestamp literals).

<sub>`dsl-analysis/src/rules/between_reversed.rs`</sub>

### `sql088` — `LIKE '%foo'`

`LIKE '%foo'` -- leading wildcard prevents B-tree index use. Suggest `text_pattern_ops` index, `pg_trgm`, or full-text search.

<sub>`dsl-analysis/src/rules/like_leading_wildcard.rs`</sub>

### `sql089` — two `RAISE EXCEPTION` calls back-to-back

two `RAISE EXCEPTION` calls back-to-back -- the second is unreachable because the first aborts the transaction.

<sub>`dsl-analysis/src/rules/multi_raise_exception.rs`</sub>

### `sql090` — PG 17 added `GROUP BY ALL` shorthand

PG 17 added `GROUP BY ALL` shorthand. Flag it as a Hint so callers know about the portability cost (works only on PG 17+).

<sub>`dsl-analysis/src/rules/group_by_all.rs`</sub>

### `sql091` — `COMMENT ON ... IS ''`

`COMMENT ON ... IS ''` -- empty comment string. PG accepts it but it usually means the author forgot to write the doc. Hint.

<sub>`dsl-analysis/src/rules/empty_comment.rs`</sub>

### `sql093` — `SELECT DISTINCT count(...) FROM t`

`SELECT DISTINCT count(...) FROM t` -- DISTINCT after an aggregate without GROUP BY is almost always redundant or wrong. Aggregates already collapse rows; DISTINCT on a single-row result does nothing.

<sub>`dsl-analysis/src/rules/distinct_with_aggregate.rs`</sub>

### `sql094` — `CASE` expressions nested more than 3 deep

`CASE` expressions nested more than 3 deep -- usually signals a lookup table or function refactor is needed.

<sub>`dsl-analysis/src/rules/deep_case_nesting.rs`</sub>

### `sql095` — `x IS NOT DISTINCT FROM NULL` is just `x IS NULL`; the other form is `x IS DISTINCT FROM NULL` ≡ `x IS NOT...

`x IS NOT DISTINCT FROM NULL` is just `x IS NULL`; the other form is `x IS DISTINCT FROM NULL` ≡ `x IS NOT NULL`. Both confuse readers -- suggest the shorter form.

<sub>`dsl-analysis/src/rules/is_distinct_null.rs`</sub>

### `sql096` — `INSERT INTO t VALUES (1, 2, );`

`INSERT INTO t VALUES (1, 2, );` -- trailing comma before the closing paren in a VALUES tuple. PG rejects this at parse time.

<sub>`dsl-analysis/src/rules/trailing_comma_values.rs`</sub>

### `sql097` — `SELECT col FROM nothing`

`SELECT col FROM nothing` -- i.e. `SELECT x;` without a FROM clause and without an aggregate. Usually a typo.

<sub>`dsl-analysis/src/rules/select_no_from_no_agg.rs`</sub>

### `sql098` — more than one `WHERE` clause in the same statement (outside parentheses/subqueries). Usually a copy/paste...

more than one `WHERE` clause in the same statement (outside parentheses/subqueries). Usually a copy/paste mistake -- PG rejects at parse time.

<sub>`dsl-analysis/src/rules/multi_where.rs`</sub>

### `sql099` — `ORDER BY 1, 2`

`ORDER BY 1, 2` -- positional ORDER BY is fragile because changing the SELECT list silently changes the sort.

<sub>`dsl-analysis/src/rules/order_by_position.rs`</sub>

### `sql101` — `SELECT DISTINCT ON (x) ... FROM t` without an `ORDER BY` that starts with `x`

`SELECT DISTINCT ON (x) ... FROM t` without an `ORDER BY` that starts with `x` -- which row PG returns is undefined.

<sub>`dsl-analysis/src/rules/distinct_on_no_order.rs`</sub>

### `sql104` — `CHAR(n)` / `CHARACTER(n)`

`CHAR(n)` / `CHARACTER(n)` -- fixed-width type that right-pads with spaces. PG docs explicitly recommend VARCHAR or TEXT.

<sub>`dsl-analysis/src/rules/char_n_type.rs`</sub>

### `sql105` — `TRUNCATE t` without `CASCADE`

`TRUNCATE t` without `CASCADE` -- if any FK points at `t`, the command fails at runtime.

<sub>`dsl-analysis/src/rules/truncate_no_cascade.rs`</sub>

### `sql107` — comparing a `jsonb` column to a text literal without `::text` / `::jsonb`

comparing a `jsonb` column to a text literal without `::text` / `::jsonb` -- the comparison is always false because PG treats the literal as jsonb.

<sub>`dsl-analysis/src/rules/jsonb_no_cast.rs`</sub>

### `sql109` — `length(text_col)` returns *bytes*. Use `char_length` for characters

`length(text_col)` returns *bytes*. Use `char_length` for characters -- the bytes/chars distinction bites with non-ASCII.

<sub>`dsl-analysis/src/rules/char_length_vs_length.rs`</sub>

### `sql111` — `LOCK TABLE` outside an explicit transaction has no effect beyond the single statement

`LOCK TABLE` outside an explicit transaction has no effect beyond the single statement -- usually a bug.

<sub>`dsl-analysis/src/rules/lock_table_no_tx.rs`</sub>

### `sql112` — `generate_series(...)` in a FROM clause without an alias ends up named `generate_series` which makes queries...

`generate_series(...)` in a FROM clause without an alias ends up named `generate_series` which makes queries hard to read.

<sub>`dsl-analysis/src/rules/generate_series_no_alias.rs`</sub>

### `sql113` — `TIMESTAMP` without time zone

`TIMESTAMP` without time zone -- ambiguous across sessions. Prefer `TIMESTAMPTZ` (`TIMESTAMP WITH TIME ZONE`).

<sub>`dsl-analysis/src/rules/timestamp_without_tz.rs`</sub>

### `sql115` — `jsonb_set(col, path, val)`

`jsonb_set(col, path, val)` -- 4th arg defaults to `true` (create-if-missing). But explicit `false` silently drops updates when the key isn't already present. Flag explicit `false`.

<sub>`dsl-analysis/src/rules/jsonb_set_no_create.rs`</sub>

### `sql116` — bare `NUMERIC` / `DECIMAL`

bare `NUMERIC` / `DECIMAL` -- unbounded precision is fine but rarely intentional. Most use-cases want NUMERIC(p,s).

<sub>`dsl-analysis/src/rules/numeric_no_precision.rs`</sub>

### `sql117` — `INSERT INTO t (col) VALUES ('true')` where `col` is boolean

`INSERT INTO t (col) VALUES ('true')` where `col` is boolean -- the literal `'true'` is text, not bool. Catches the missing `::boolean` cast.

<sub>`dsl-analysis/src/rules/boolean_in_text_column.rs`</sub>

### `sql118` — `SELECT ... INTO foo FROM t` at the top level is **DDL**

`SELECT ... INTO foo FROM t` at the top level is **DDL** -- it creates a new table `foo`. Usually the user meant PL/pgSQL variable assignment (which only works inside `$$ ... $$`).

<sub>`dsl-analysis/src/rules/select_into_outside_plpgsql.rs`</sub>

### `sql119` — `SET TRANSACTION ISOLATION LEVEL ...` must be the **first** statement after `BEGIN`

`SET TRANSACTION ISOLATION LEVEL ...` must be the **first** statement after `BEGIN` -- otherwise PG ignores it. Catches the mistake of putting it after a SELECT.

<sub>`dsl-analysis/src/rules/transaction_isolation_no_set.rs`</sub>

### `sql120` — `SELECT DISTINCT ... GROUP BY ...`

`SELECT DISTINCT ... GROUP BY ...` -- GROUP BY already produces unique rows, the DISTINCT is dead weight.

<sub>`dsl-analysis/src/rules/distinct_after_group_by.rs`</sub>

### `sql121` — comparing a text expression to an int literal in WHERE. Common bug

comparing a text expression to an int literal in WHERE. Common bug -- PG will cast text -> int row-by-row and discard the index. Catches `t.id_text = 123` style patterns where the left side is wrapped in a text function.

<sub>`dsl-analysis/src/rules/cast_text_to_int_in_where.rs`</sub>

### `sql122` — `LIKE` inside a query without explicit `COLLATE`

`LIKE` inside a query without explicit `COLLATE` -- the collation comes from the column or the session, which has burned teams on multi-locale deployments. Hint to add `COLLATE "C"` or `COLLATE "und-x-icu"` for predictable behaviour.

<sub>`dsl-analysis/src/rules/like_with_no_collation.rs`</sub>

### `sql123` — `\n`, `\t`, `\\` inside a plain `'...'` string. PG 9.1+ defaults to `standard_conforming_strings = on`

`\n`, `\t`, `\\` inside a plain `'...'` string. PG 9.1+ defaults to `standard_conforming_strings = on` -- the backslash is literal, not an escape. Use `E'...'` if the user wants escapes.

<sub>`dsl-analysis/src/rules/backslash_in_string.rs`</sub>

### `sql124` — `WITH t AS (SELECT

`WITH t AS (SELECT ... FROM t)` self-references `t` but lacks the `RECURSIVE` keyword. PG will refuse to execute it.

<sub>`dsl-analysis/src/rules/cte_missing_recursive.rs`</sub>

### `sql125` — `EXPLAIN ANALYZE INSERT/UPDATE/DELETE`

`EXPLAIN ANALYZE INSERT/UPDATE/DELETE` -- ANALYZE actually runs the query so the DML mutates the table. Often surprises people debugging in prod. Suggest wrapping in BEGIN ... ROLLBACK.

<sub>`dsl-analysis/src/rules/explain_analyze_in_dml.rs`</sub>

### `sql126` — DML inside a PL/pgSQL function without a subsequent `GET DIAGNOSTICS rows = ROW_COUNT`

DML inside a PL/pgSQL function without a subsequent `GET DIAGNOSTICS rows = ROW_COUNT` -- callers usually want to know whether the UPDATE/DELETE actually touched anything. Flag as Hint.

<sub>`dsl-analysis/src/rules/row_count_after_dml.rs`</sub>

### `sql127` — `UPDATE t SET ... FROM other` without a WHERE that joins `t` and `other`

`UPDATE t SET ... FROM other` without a WHERE that joins `t` and `other` -- the FROM becomes a cross product and every row of `t` gets touched.

<sub>`dsl-analysis/src/rules/update_from_no_pk_filter.rs`</sub>

### `sql128` — `GRANT ... TO PUBLIC`

`GRANT ... TO PUBLIC` -- grants the privilege to *every* current and future role. Almost always a mistake.

<sub>`dsl-analysis/src/rules/grant_to_public.rs`</sub>

### `sql130` — multiple `TRUNCATE` statements in one transaction. PG supports `TRUNCATE a, b, c` directly

multiple `TRUNCATE` statements in one transaction. PG supports `TRUNCATE a, b, c` directly -- batching gets a single AccessExclusiveLock acquisition and one rewrite.

<sub>`dsl-analysis/src/rules/multiple_truncate_in_tx.rs`</sub>

### `sql131` — `RAISE NOTICE 'value is %s'`

`RAISE NOTICE 'value is %s'` -- but no `,` providing the argument. PG prints the placeholder as-is and probably swallows the error if the user expected interpolation.

<sub>`dsl-analysis/src/rules/raise_message_no_args.rs`</sub>

### `sql132` — `SELECT ... FOR UPDATE` inside the recursive arm of a CTE is forbidden by PG

`SELECT ... FOR UPDATE` inside the recursive arm of a CTE is forbidden by PG -- the planner rejects it.

<sub>`dsl-analysis/src/rules/select_for_update_in_recursive_cte.rs`</sub>

### `sql133` — `GRANT ... WITH GRANT OPTION` lets the grantee re-grant the privilege chain to anyone else

`GRANT ... WITH GRANT OPTION` lets the grantee re-grant the privilege chain to anyone else -- almost always too broad for application roles.

<sub>`dsl-analysis/src/rules/grant_with_grant_option.rs`</sub>

### `sql134` — `VACUUM` cannot run inside an explicit transaction block

`VACUUM` cannot run inside an explicit transaction block -- PG raises an error at runtime.

<sub>`dsl-analysis/src/rules/vacuum_in_transaction.rs`</sub>

### `sql135` — `SET ROLE x` inside a transaction without a matching `RESET ROLE`

`SET ROLE x` inside a transaction without a matching `RESET ROLE` -- the elevated role leaks past the COMMIT into the pooled connection's lifetime.

<sub>`dsl-analysis/src/rules/set_role_no_reset.rs`</sub>

### `sql136` — `COPY t FROM 'file'` without a `FORMAT` clause

`COPY t FROM 'file'` without a `FORMAT` clause -- defaults to `text` which has subtle escaping rules. Hint to make the format explicit.

<sub>`dsl-analysis/src/rules/copy_no_format.rs`</sub>

### `sql137` — bare `LISTEN <channel>` in a session that never `UNLISTEN`s

bare `LISTEN <channel>` in a session that never `UNLISTEN`s -- the backend accumulates queued notifications indefinitely.

<sub>`dsl-analysis/src/rules/listen_unbounded.rs`</sub>

### `sql138` — `SELECT DISTINCT (col)::text FROM t`

`SELECT DISTINCT (col)::text FROM t` -- casting to text inside DISTINCT throws away the typed comparison and runs a string compare, almost always wrong.

<sub>`dsl-analysis/src/rules/cast_text_in_distinct.rs`</sub>

### `sql139` — `UNIQUE` on a nullable column with `NULLS DISTINCT` (the PG default)

`UNIQUE` on a nullable column with `NULLS DISTINCT` (the PG default) -- multiple NULL rows are allowed. Usually surprising. Suggest `UNIQUE NULLS NOT DISTINCT` (PG 15+) or making the column `NOT NULL`.

<sub>`dsl-analysis/src/rules/unique_on_nullable.rs`</sub>

### `sql140` — `CREATE TRIGGER ... AFTER INSERT ... WHEN (OLD.x ...)`

`CREATE TRIGGER ... AFTER INSERT ... WHEN (OLD.x ...)` -- INSERT triggers have no OLD row. PG raises an error at runtime.

<sub>`dsl-analysis/src/rules/trigger_when_uses_old_in_insert.rs`</sub>

### `sql141` — `ALTER TYPE x ADD VALUE 'y'` cannot run inside an explicit transaction block

`ALTER TYPE x ADD VALUE 'y'` cannot run inside an explicit transaction block -- PG aborts the statement.

<sub>`dsl-analysis/src/rules/alter_type_add_value_in_tx.rs`</sub>

### `sql142` — `CREATE [OR REPLACE] FUNCTION ... IMMUTABLE` whose body issues DDL (CREATE, ALTER, DROP, TRUNCATE)

`CREATE [OR REPLACE] FUNCTION ... IMMUTABLE` whose body issues DDL (CREATE, ALTER, DROP, TRUNCATE) -- IMMUTABLE promises deterministic output for any given input and is *not* allowed to mutate the database. PG plan caches IMMUTABLE results.

<sub>`dsl-analysis/src/rules/ddl_in_immutable.rs`</sub>

### `sql143` — `INSERT/UPDATE/DELETE ... RETURNING ...` inside a PL/pgSQL block without `INTO <vars>` or `STRICT`

`INSERT/UPDATE/DELETE ... RETURNING ...` inside a PL/pgSQL block without `INTO <vars>` or `STRICT` -- the returned row is silently discarded. Almost always a bug.

<sub>`dsl-analysis/src/rules/returning_no_assign.rs`</sub>

### `sql144` — `CREATE TRIGGER ... AFTER DELETE ... WHEN (NEW.x ...)`

`CREATE TRIGGER ... AFTER DELETE ... WHEN (NEW.x ...)` -- DELETE triggers have no NEW row. Mirror of sql140.

<sub>`dsl-analysis/src/rules/trigger_when_uses_new_in_delete.rs`</sub>

### `sql145` — column `DEFAULT now()` (or any volatile expression) freezes the value at insert time, which is usually fine

column `DEFAULT now()` (or any volatile expression) freezes the value at insert time, which is usually fine -- but DEFAULT random() / nextval() / etc. inside CREATE TABLE produces a fresh value per row at insert. Surface as a Hint so the user is aware the default is recomputed per row.

<sub>`dsl-analysis/src/rules/column_default_volatile.rs`</sub>

### `sql146` — `VARCHAR` / `CHARACTER VARYING` without an explicit length. Unbounded VARCHAR is effectively TEXT but with...

`VARCHAR` / `CHARACTER VARYING` without an explicit length. Unbounded VARCHAR is effectively TEXT but with the awkward type name -- prefer `TEXT` when no cap is wanted, or spell the cap when it is.

<sub>`dsl-analysis/src/rules/character_varying_no_limit.rs`</sub>

### `sql148` — array subscript `arr[0]` or `arr[-1]`

array subscript `arr[0]` or `arr[-1]` -- PG arrays are 1-based by default. `arr[0]` returns NULL, never the first element.

<sub>`dsl-analysis/src/rules/array_subscript_zero.rs`</sub>

### `sql149` — `UPDATE t SET x = x`

`UPDATE t SET x = x` -- assigning a column to itself. The row gets an unnecessary write (and a trigger fires) for no semantic change.

<sub>`dsl-analysis/src/rules/update_set_no_change.rs`</sub>

### `sql150` — `CASE WHEN

`CASE WHEN ... THEN ... END` without an `ELSE` branch. Unmatched rows return NULL silently. Hint to add `ELSE` explicitly so the author's intent is on the page.

<sub>`dsl-analysis/src/rules/case_no_else.rs`</sub>

### `sql151` — `SELECT ... FROM t, generate_series(t.col, 10)`

`SELECT ... FROM t, generate_series(t.col, 10)` -- the function reads from `t.col` but no `LATERAL` keyword. PG rejects: "missing FROM-clause entry for table t".

<sub>`dsl-analysis/src/rules/missing_lateral.rs`</sub>

### `sql152` — `BEGIN` for a transaction that needs to UPDATE/DELETE many rows without an explicit `LOCK TABLE` or `FOR...

`BEGIN` for a transaction that needs to UPDATE/DELETE many rows without an explicit `LOCK TABLE` or `FOR UPDATE` -- can lead to lost updates when there's concurrent traffic. Hint to consider explicit lock-mode for write-heavy transactions. Conservative heuristic: only flag when the transaction body contains UPDATE/DELETE without a WHERE on a unique key.

<sub>`dsl-analysis/src/rules/begin_no_lock_mode.rs`</sub>

### `sql153` — `now() + 1`, `created_at + 30`

`now() + 1`, `created_at + 30` -- integer added to a timestamp uses *days*, which is rarely what's meant. Use an explicit `interval '1 day'` / `interval '30 minutes'`.

<sub>`dsl-analysis/src/rules/timestamp_int_arithmetic.rs`</sub>

### `sql154` — `SELECT count(*) FROM t WHERE ...` (no GROUP BY) returns **one row** even when the WHERE matches nothing

`SELECT count(*) FROM t WHERE ...` (no GROUP BY) returns **one row** even when the WHERE matches nothing -- count() is an aggregate over the empty set = 0. Common gotcha when porting from per-row languages where "no rows" expected.

<sub>`dsl-analysis/src/rules/count_star_returns_one.rs`</sub>

### `sql155` — `TRUNCATE t RETURNING ...`

`TRUNCATE t RETURNING ...` -- TRUNCATE does not support RETURNING. PG rejects at parse time.

<sub>`dsl-analysis/src/rules/returning_with_truncate.rs`</sub>

### `sql156` — `SELECT ... INTO STRICT var` inside PL/pgSQL without a surrounding EXCEPTION block. STRICT raises...

`SELECT ... INTO STRICT var` inside PL/pgSQL without a surrounding EXCEPTION block. STRICT raises NO_DATA_FOUND or TOO_MANY_ROWS on miss -- an uncaught raise aborts the transaction.

<sub>`dsl-analysis/src/rules/select_into_strict_no_exception.rs`</sub>

### `sql157` — `RAISE EXCEPTION ... USING ERRCODE = my_var`

`RAISE EXCEPTION ... USING ERRCODE = my_var` -- an unquoted identifier as the errcode value is almost always a typo for a SQLSTATE string literal like `'P0001'` or `'23505'`.

<sub>`dsl-analysis/src/rules/raise_using_errcode.rs`</sub>

### `sql158` — `PERFORM <select>` inside PL/pgSQL where the SELECT calls no function with side effects

`PERFORM <select>` inside PL/pgSQL where the SELECT calls no function with side effects -- the result is silently discarded. Suggest dropping PERFORM (cheap NO-OP) or using the result.

<sub>`dsl-analysis/src/rules/perform_for_pure_select.rs`</sub>

### `sql159` — `CREATE TRIGGER ... FOR EACH STATEMENT ... NEW`

`CREATE TRIGGER ... FOR EACH STATEMENT ... NEW` -- only row-level triggers have NEW/OLD. Statement-level triggers cannot reference them.

<sub>`dsl-analysis/src/rules/trigger_stmt_uses_new.rs`</sub>

### `sql160` — `pg_advisory_lock(...)` (session-level) without a matching `pg_advisory_unlock(...)` in the same source

`pg_advisory_lock(...)` (session-level) without a matching `pg_advisory_unlock(...)` in the same source. Session locks persist beyond the transaction and leak across pool reuse.

<sub>`dsl-analysis/src/rules/advisory_lock_no_unlock.rs`</sub>

### `sql164` — `'foo' || 1` or `'a' + 1`

`'foo' || 1` or `'a' + 1` -- string literal + int. PG requires explicit cast; the implicit one bites when porting from MySQL.

<sub>`dsl-analysis/src/rules/text_int_arithmetic.rs`</sub>

### `sql166` — `ROW(x)` with a single element

`ROW(x)` with a single element -- PG treats it as a row, but `(x)` is just `x`. Worth flagging when the user writes the explicit ROW form with one element because it's almost always pasted from a multi-element template.

<sub>`dsl-analysis/src/rules/row_constructor_single.rs`</sub>

### `sql167` — `CREATE INDEX

`CREATE INDEX ... ON t (pk_col)` where `pk_col` is the primary key of `t`. PRIMARY KEY already creates a unique B-tree index, so the explicit one is duplicate storage + maintenance cost.

<sub>`dsl-analysis/src/rules/redundant_index_on_pk.rs`</sub>

### `sql168` — `CREATE UNIQUE INDEX

`CREATE UNIQUE INDEX ... ON t (cols)` where `t` already has a UNIQUE constraint on the same column set. PG already enforces uniqueness via the constraint's implicit index.

<sub>`dsl-analysis/src/rules/redundant_unique_index.rs`</sub>

### `sql169` — `ALTER TABLE x OWNER TO some_role`

`ALTER TABLE x OWNER TO some_role` -- when a live catalog is connected, validate that `some_role` exists in `pg_roles`. Otherwise silently runs and PG errors at exec.

<sub>`dsl-analysis/src/rules/owner_to_unknown_role.rs`</sub>

### `sql170` — `x := <lit>` inside a PL/pgSQL body where the literal kind disagrees with x's declared type. Catches `DECLARE...

`x := <lit>` inside a PL/pgSQL body where the literal kind disagrees with x's declared type. Catches `DECLARE x INT; ... x := 'str';` and similar at edit time -- Postgres errors at execution. Conservative: only literal kinds we can classify with high confidence (string / integer / float / boolean / NULL); skips function calls / expressions / casts.

<sub>`dsl-analysis/src/rules/plpgsql_assign_type.rs`</sub>

### `sql171` — `UPDATE t SET <col> = <literal>` where the literal kind disagrees with the column's catalog type

`UPDATE t SET <col> = <literal>` where the literal kind disagrees with the column's catalog type. Mirror of sql039 for the SET assignment path. Conservative: only literal kinds we can classify with high confidence (string / integer / float / boolean / NULL). Function calls, expressions, casts, subqueries -> skipped.

<sub>`dsl-analysis/src/rules/update_set_type_literal.rs`</sub>

### `sql172` — `<col> = <literal>` (or `<>`, `>`, `<`, `>=`, `<=`) where the literal kind disagrees with the column's...

`<col> = <literal>` (or `<>`, `>`, `<`, `>=`, `<=`) where the literal kind disagrees with the column's catalog type. Fires in WHERE / HAVING / ON predicates and in CHECK constraint bodies. Conservative literal classification (str / int / float / bool / null); skips function calls / casts / subqueries / column-vs-column comparisons. The cursor / WHERE / etc. spans aren't parsed by pg_query for predicate-level details, so this is a text scan that splits on AND / OR + binary operators.

<sub>`dsl-analysis/src/rules/where_type_literal.rs`</sub>

### `sql173` — workspace CREATE TABLE diverges from the live catalog

workspace CREATE TABLE diverges from the live catalog. When the user has a live connection AND a CREATE TABLE for the same name in the buffer, compare column sets. Columns in the buffer but missing from live -> drift error on the missing-side. Columns in live but missing from buffer -> hint that the table has extra columns the DDL doesn't declare. Skips when no live catalog (live tables empty) OR no buffer CreateTable matches.

<sub>`dsl-analysis/src/rules/schema_drift.rs`</sub>

### `sql174` — `COUNT(col)` where `col` is nullable

`COUNT(col)` where `col` is nullable. Skips NULL rows which the user may not have intended. Suggest `COUNT(*)` or `COUNT(col) FILTER (WHERE col IS NOT NULL)` to make the intent explicit.

<sub>`dsl-analysis/src/rules/count_nullable.rs`</sub>

### `sql175` — `SELECT ... FROM <view> FOR UPDATE`

`SELECT ... FROM <view> FOR UPDATE` -- views can't be locked, PG errors at runtime ("FOR UPDATE cannot be applied to the relation 'v'"). Flag the FOR UPDATE / FOR SHARE clause at edit time when the FROM target is a TableKind::View or MaterializedView in the catalog.

<sub>`dsl-analysis/src/rules/for_update_on_view.rs`</sub>

### `sql176` — `WHERE col IS NULL` where the catalog says `col` is NOT NULL

`WHERE col IS NULL` where the catalog says `col` is NOT NULL. The predicate can never be true so the query returns zero rows.

<sub>`dsl-analysis/src/rules/is_null_on_not_null.rs`</sub>

### `sql177` — `INSERT INTO t (a, ...) VALUES (NULL, ...)` where `a` is NOT NULL and has no default

`INSERT INTO t (a, ...) VALUES (NULL, ...)` where `a` is NOT NULL and has no default. PG errors at runtime with `null value in column "a" violates not-null constraint`. Catch at edit time.

<sub>`dsl-analysis/src/rules/null_into_not_null.rs`</sub>

### `sql178` — Writing to a `GENERATED ALWAYS` column. PG rejects writes to identity/stored generated columns: * `INSERT...

Writing to a `GENERATED ALWAYS` column. PG rejects writes to identity/stored generated columns: * `INSERT INTO t (id, ...) VALUES (...)` where `id` is GENERATED ALWAYS AS IDENTITY -- requires `OVERRIDING SYSTEM VALUE`. * `INSERT INTO t (full_name, ...) VALUES (...)` where `full_name` is GENERATED ALWAYS AS (expr) STORED -- *cannot* be overridden; the column must be omitted entirely. * `UPDATE t SET id = ...` (identity) or `SET full_name = ...` (stored) -- both are runtime errors. Detection uses the catalog: `col.default` carries the `GENERATED ALWAYS AS IDENTITY` text for identity columns, and `col.generated` is set to the expression for STORED generated columns.

<sub>`dsl-analysis/src/rules/insert_into_generated.rs`</sub>

### `sql179` — `SAVEPOINT s;` outside a transaction errors with 25P01 ("SAVEPOINT can only be used in transaction blocks")

`SAVEPOINT s;` outside a transaction errors with 25P01 ("SAVEPOINT can only be used in transaction blocks"). Heuristic: walk back from the SAVEPOINT keyword counting BEGIN / START TRANSACTION vs COMMIT / ROLLBACK. If the balance is zero or negative, no active tx -> flag.

<sub>`dsl-analysis/src/rules/savepoint_outside_tx.rs`</sub>

### `sql180` — `TRUNCATE` inside a trigger function body

`TRUNCATE` inside a trigger function body. PG rejects with `cannot TRUNCATE inside a function`. Heuristic: the statement's source span lives inside a $$ ... $$ block of a CREATE FUNCTION ... RETURNS TRIGGER.

<sub>`dsl-analysis/src/rules/truncate_in_trigger.rs`</sub>

### `sql181` — `INSERT INTO t (name) VALUES ('long-string')` where `name` is declared `VARCHAR(n)` and the literal exceeds n

`INSERT INTO t (name) VALUES ('long-string')` where `name` is declared `VARCHAR(n)` and the literal exceeds n. PG truncates silently in some modes, errors with 22001 in strict mode. Flag at edit time so the user knows.

<sub>`dsl-analysis/src/rules/varchar_length.rs`</sub>

### `sql182` — `INSERT INTO t (d) VALUES ('garbage')` where `d` is DATE / TIMESTAMP / TIMESTAMPTZ / TIME and the string...

`INSERT INTO t (d) VALUES ('garbage')` where `d` is DATE / TIMESTAMP / TIMESTAMPTZ / TIME and the string literal doesn't parse as that type. Lightweight regex check at edit time; PG raises 22007 / 22008 at runtime.

<sub>`dsl-analysis/src/rules/date_literal_format.rs`</sub>

### `sql183` — `INSERT INTO t (id) VALUES ('not-a-uuid')` where `id` is UUID. PG raises 22P02 at runtime. Accept only: *...

`INSERT INTO t (id) VALUES ('not-a-uuid')` where `id` is UUID. PG raises 22P02 at runtime. Accept only: * 8-4-4-4-12 hex (dashed canonical form) * 32 hex chars (no dashes) -- PG also accepts this * Surrounding braces `{...}`

<sub>`dsl-analysis/src/rules/uuid_literal_format.rs`</sub>

### `sql184` — integer literal larger than the column's declared type can hold (`SMALLINT` max 32767, `INT` max 2147483647)

integer literal larger than the column's declared type can hold (`SMALLINT` max 32767, `INT` max 2147483647). PG raises 22003 at runtime. Catch at edit time.

<sub>`dsl-analysis/src/rules/int_range.rs`</sub>

### `sql185` — `REFERENCES other(missing)` where `missing` isn't a column on `other`

`REFERENCES other(missing)` where `missing` isn't a column on `other`. PG raises 42703 at runtime. Walks the CREATE TABLE constraints + the catalog to validate every FK target column exists.

<sub>`dsl-analysis/src/rules/fk_unknown_column.rs`</sub>

### `sql186` — `ALTER TABLE t DROP COLUMN id` where another catalog table has a FK that references `t(id)`

`ALTER TABLE t DROP COLUMN id` where another catalog table has a FK that references `t(id)`. PG refuses without CASCADE. Surface the dependency at edit time.

<sub>`dsl-analysis/src/rules/drop_column_fk.rs`</sub>

### `sql187` — `JOIN other USING (col)`

`JOIN other USING (col)` -- col must exist on BOTH sides of the join. PG raises 42703 at runtime when missing. Flag at edit time when the catalog has both tables but `col` isn't a column of at least one.

<sub>`dsl-analysis/src/rules/using_clause_columns.rs`</sub>

### `sql188` — `COMMENT ON TABLE bogus IS '...'` where bogus isn't a known catalog table

`COMMENT ON TABLE bogus IS '...'` where bogus isn't a known catalog table. PG raises 42P01 at runtime. Also catches COMMENT ON COLUMN bogus.col / FUNCTION bogus / TYPE bogus.

<sub>`dsl-analysis/src/rules/comment_on_unknown.rs`</sub>

### `sql189` — `ALTER TABLE t ALTER COLUMN c TYPE <new_type>` where `c`'s catalog type doesn't auto-cast to `<new_type>` and...

`ALTER TABLE t ALTER COLUMN c TYPE <new_type>` where `c`'s catalog type doesn't auto-cast to `<new_type>` and the statement lacks `USING`. PG raises 42804 at runtime. Conservative: only flag when both source + target are known type families AND the cast isn't trivially safe (same family, widening numeric, etc).

<sub>`dsl-analysis/src/rules/alter_column_type.rs`</sub>

### `sql190` — `INSERT INTO t (...) ... ON CONFLICT (col, ...) DO ...` where `(col, ...)` is not the target of any PRIMARY...

`INSERT INTO t (...) ... ON CONFLICT (col, ...) DO ...` where `(col, ...)` is not the target of any PRIMARY KEY / UNIQUE constraint or unique index on `t`. PG raises 42P10 "there is no unique or exclusion constraint matching the ON CONFLICT spec". Skip when `ON CONFLICT ON CONSTRAINT <name>` or no column list is provided -- those forms target an explicit constraint or are DO NOTHING with no inference.

<sub>`dsl-analysis/src/rules/on_conflict_no_unique.rs`</sub>

### `sql191` — `ROWS BETWEEN <n> FOLLOWING AND <m> PRECEDING` or any frame where the start bound is strictly later than the...

`ROWS BETWEEN <n> FOLLOWING AND <m> PRECEDING` or any frame where the start bound is strictly later than the end bound. PG raises 22023 "frame starting from following row cannot end with current row" (or equivalent) at runtime. Cheap textual scan over BETWEEN ... AND ... pairs.

<sub>`dsl-analysis/src/rules/window_frame_reversed.rs`</sub>

### `sql192` — `SELECT

`SELECT ... FROM a JOIN b ... FOR UPDATE OF x` where `x` is not in the FROM list (neither table name nor alias). PG raises 42P01 "relation `x` in FOR UPDATE clause not found in FROM clause" at runtime.

<sub>`dsl-analysis/src/rules/for_update_of_unknown.rs`</sub>

### `sql193` — `GENERATED ALWAYS AS (expr) STORED` where `expr` calls a known-volatile function (random / now /...

`GENERATED ALWAYS AS (expr) STORED` where `expr` calls a known-volatile function (random / now / clock_timestamp / uuid / nextval / etc). PG raises 42P17 "generation expression is not immutable" at CREATE TABLE time. Textual: scans CREATE TABLE bodies for `GENERATED ALWAYS AS (...) STORED` and looks for volatile call names in the parenthesised expression.

<sub>`dsl-analysis/src/rules/generated_uses_volatile.rs`</sub>

### `sql194` — `TRUNCATE foo` (no CASCADE) when another table has an FK referencing `foo`

`TRUNCATE foo` (no CASCADE) when another table has an FK referencing `foo`. PG raises 0A000 "cannot truncate a table referenced in a foreign key constraint" at runtime. Uses the merged catalog to find inbound FK references to the truncated table. Skips when the statement already includes CASCADE.

<sub>`dsl-analysis/src/rules/truncate_with_fk.rs`</sub>

### `sql195` — `CAST('lit' AS <type>)` or `'lit'::<type>` where `lit` can't be parsed as `<type>`

`CAST('lit' AS <type>)` or `'lit'::<type>` where `lit` can't be parsed as `<type>`. PG raises 22P02 at runtime. Only fires for cheap, lossless local checks: - INT family: non-integer literals - NUMERIC / FLOAT family: non-numeric literals - UUID: not 8-4-4-4-12 hex - BOOLEAN: not in {true,false,t,f,1,0,yes,no} - DATE: not YYYY-MM-DD - TIMESTAMP: not YYYY-MM-DD HH:MM[:SS][+TZ]

<sub>`dsl-analysis/src/rules/cast_literal_invalid.rs`</sub>

### `sql196` — `REFERENCES other(col)` where `other.col` is not the target of a PRIMARY KEY or UNIQUE constraint / unique...

`REFERENCES other(col)` where `other.col` is not the target of a PRIMARY KEY or UNIQUE constraint / unique index. PG raises 42830 "there is no unique constraint matching given keys for referenced table" at CREATE TABLE.

<sub>`dsl-analysis/src/rules/fk_target_not_unique.rs`</sub>

### `sql197` — `array_length(col, ...)`, `unnest(col)`, `cardinality(col)`, `array_to_string(col, ...)`...

`array_length(col, ...)`, `unnest(col)`, `cardinality(col)`, `array_to_string(col, ...)`, `array_position(col, ...)` where `col` resolves to a scalar (non-array) catalog column. PG raises 42883 "function does not exist" at runtime (no array overload). Conservative: only flags bare column references inside the array function's first argument. Subqueries and computed exprs are skipped.

<sub>`dsl-analysis/src/rules/array_fn_on_scalar.rs`</sub>

### `sql198` — inline column CHECK references a different column

inline column CHECK references a different column. e.g. `start_at DATE CHECK (end_at > start_at)`. PG raises 0A000 "cannot use column reference in DEFAULT/CHECK constraint" if the CHECK is column-level (single inline constraint after column type). Promote it to table-level CHECK instead. Conservative: only fires when the inline CHECK expression contains an identifier that doesn't match the owning column.

<sub>`dsl-analysis/src/rules/inline_check_other_col.rs`</sub>

### `sql199` — `<col> <type> DEFAULT <expr>` where `<expr>` references another column on the same table

`<col> <type> DEFAULT <expr>` where `<expr>` references another column on the same table. PG raises 0A000 "cannot use column reference in DEFAULT expression" at CREATE TABLE. Conservative scan: walks the column list of CREATE TABLE, locates each column's `DEFAULT` clause, then checks bare identifiers in the expression against the set of sibling column names.

<sub>`dsl-analysis/src/rules/default_references_column.rs`</sub>

### `sql200` — `JOIN LATERAL (SELECT

`JOIN LATERAL (SELECT ... FROM x) y ON ...` where the inner subquery does NOT reference any outer alias. LATERAL is a no-op there and can be safely removed for clarity. Detect by: * Find each `LATERAL` keyword + parenthesised body. * Collect FROM/JOIN aliases that appear before the LATERAL. * If none of those alias tokens appear inside the LATERAL body, emit a warning to drop LATERAL.

<sub>`dsl-analysis/src/rules/lateral_no_ref.rs`</sub>

### `sql201` — `CREATE FUNCTION

`CREATE FUNCTION ... SECURITY DEFINER ...` without an explicit `SET search_path = ...` clause. Tracks CVE-2018-1058 escalation: a SECURITY DEFINER function inherits the caller's search_path, letting a hostile schema shadow `public.fn(...)`. PG docs recommend pinning search_path to `pg_catalog, pg_temp`.

<sub>`dsl-analysis/src/rules/secdef_no_search_path.rs`</sub>

### `sql202` — PL/pgSQL trigger function body references `OLD.*` inside an INSERT trigger or `NEW.*` inside a DELETE...

PL/pgSQL trigger function body references `OLD.*` inside an INSERT trigger or `NEW.*` inside a DELETE trigger. PG raises "record `old` has no field `xyz`" -- the row alias is undefined. Heuristic: each CREATE TRIGGER statement names the function it invokes plus the event(s) (INSERT/UPDATE/DELETE). We map the trigger fn -> events, then re-scan every CREATE FUNCTION body to flag forbidden NEW/OLD references for its registered events. Two-phase pass: this rule only fires when both CREATE TRIGGER and CREATE FUNCTION appear in the same buffer, which is the common workspace layout.

<sub>`dsl-analysis/src/rules/trigger_wrong_row_alias.rs`</sub>

### `sql203` — `RAISE 'msg'` inside a PL/pgSQL body without a level keyword (NOTICE/INFO/LOG/WARNING/EXCEPTION/DEBUG). PG...

`RAISE 'msg'` inside a PL/pgSQL body without a level keyword (NOTICE/INFO/LOG/WARNING/EXCEPTION/DEBUG). PG defaults to EXCEPTION which aborts the surrounding transaction -- almost never the intended behaviour when the author wrote `RAISE 'debug %', x`. Heuristic: word-bounded RAISE followed directly by a string literal (skipping whitespace) instead of a level keyword.

<sub>`dsl-analysis/src/rules/raise_no_level.rs`</sub>

### `sql204` — `UPDATE users u SET other.col = ...`

`UPDATE users u SET other.col = ...` -- the qualifier on the SET target doesn't match the updated table (`u` or `users`). PG raises 42703 / 42P01 -- only the updated table is in scope on the SET left-hand side. Catches the common bug where folks alias the update target then accidentally use a JOINed table alias.

<sub>`dsl-analysis/src/rules/update_set_alias_mismatch.rs`</sub>

### `sql205` — `NOTIFY <channel>` where no `LISTEN <channel>` appears in the same buffer. Dead channel

`NOTIFY <channel>` where no `LISTEN <channel>` appears in the same buffer. Dead channel -- subscriber side missing. Best- effort: covers buffers that contain both producer + consumer SQL (common in repo-managed schema dumps + migration files).

<sub>`dsl-analysis/src/rules/notify_unlistened.rs`</sub>

### `sql206` — `INSERT INTO t (a, b) VALUES ((SELECT 1, 2))`

`INSERT INTO t (a, b) VALUES ((SELECT 1, 2))` -- the scalar-subquery returns 2 columns where one was expected. Or `INSERT INTO t SELECT 1, 2, 3` where t has only 2 columns. PG raises 42601 / 42P10. Heuristic: counts commas at top level in the subquery projection list.

<sub>`dsl-analysis/src/rules/insert_subquery_col_count.rs`</sub>

### `sql207` — `COALESCE(x)` with a single argument is a no-op

`COALESCE(x)` with a single argument is a no-op -- it returns x unchanged. Almost always a copy-paste bug from a multi-arg COALESCE. Same applies to GREATEST / LEAST / CONCAT. `CONCAT_WS(sep, value)` is also a no-op (the separator is never used when there's only one value to join).

<sub>`dsl-analysis/src/rules/coalesce_single_arg.rs`</sub>

### `sql208` — `EXTRACT(<field> FROM <expr>)` where `<field>` is not in the PG-supported list

`EXTRACT(<field> FROM <expr>)` where `<field>` is not in the PG-supported list. PG raises 22023 / 0AP01 at runtime. Common typos like `EXTRACT(yearr FROM ts)` or wrong casing handled by lowercase comparison.

<sub>`dsl-analysis/src/rules/extract_unknown_field.rs`</sub>

### `sql209` — `COPY t TO 'file.csv'` or `COPY t FROM 'file.csv'`

`COPY t TO 'file.csv'` or `COPY t FROM 'file.csv'` -- server-side file access requires PG superuser (or pg_{read,write}_server_files membership). Almost always the author wanted client-side `\copy` (psql) or STDIN/STDOUT. Suggest swap.

<sub>`dsl-analysis/src/rules/copy_file_path.rs`</sub>

### `sql210` — `REINDEX [CONCURRENTLY] (TABLE|INDEX) pg_<x>`

`REINDEX [CONCURRENTLY] (TABLE|INDEX) pg_<x>` -- system catalog reindex. PG rejects CONCURRENTLY on system catalogs (only superuser can do plain REINDEX SYSTEM). Catches accidental targets against pg_catalog / pg_toast / information_schema.

<sub>`dsl-analysis/src/rules/reindex_system.rs`</sub>

### `sql211` — bare `ROLLBACK;` / `COMMIT;` with no preceding BEGIN / START TRANSACTION in the source

bare `ROLLBACK;` / `COMMIT;` with no preceding BEGIN / START TRANSACTION in the source. PG emits a WARNING ("there is no transaction in progress") and the statement is a no-op.

<sub>`dsl-analysis/src/rules/rollback_outside_tx.rs`</sub>

### `sql212` — top-level `SELECT * INTO foo FROM bar`

top-level `SELECT * INTO foo FROM bar` -- DDL form creates a NEW table `foo`. If `foo` already exists in the catalog, PG raises 42P07 at runtime. Skip when inside a $$...$$ body (SELECT INTO inside PL/pgSQL is an assignment, handled by sql118).

<sub>`dsl-analysis/src/rules/select_into_existing.rs`</sub>

### `sql213` — `CREATE INDEX

`CREATE INDEX ... (expr)` where `expr` calls a known- volatile function (random / now / clock_timestamp / nextval / gen_random_uuid / etc). PG raises 42P17 "functions in index expression must be marked IMMUTABLE" at runtime.

<sub>`dsl-analysis/src/rules/index_expr_volatile.rs`</sub>

### `sql214` — `CREATE INDEX CONCURRENTLY` (or `DROP INDEX CONCURRENTLY`) inside an explicit transaction block

`CREATE INDEX CONCURRENTLY` (or `DROP INDEX CONCURRENTLY`) inside an explicit transaction block. PG raises 25001 "CREATE INDEX CONCURRENTLY cannot run inside a transaction block" at runtime. Counts BEGIN/START TRANSACTION minus COMMIT/ROLLBACK in the source before this statement.

<sub>`dsl-analysis/src/rules/index_concurrently_in_tx.rs`</sub>

### `sql215` — `GROUP BY ROLLUP(a)` / `CUBE(a)` with a single grouping column

`GROUP BY ROLLUP(a)` / `CUBE(a)` with a single grouping column. ROLLUP(a) ≡ GROUPING SETS ((a), ()), CUBE(a) likewise, which is a one-extra-row trick rarely intended. Suggest GROUPING SETS or remove the wrapper.

<sub>`dsl-analysis/src/rules/rollup_cube_single.rs`</sub>

### `sql216` — `INSERT INTO t VALUES (1,2), (1,2,3)`

`INSERT INTO t VALUES (1,2), (1,2,3)` -- the rows in a VALUES list disagree on column count. PG raises 42601 / 42P10 at parse / execute time. Text-scan: locate the VALUES keyword, split top-level paren-wrapped tuples, count commas at the tuple's depth-1 level.

<sub>`dsl-analysis/src/rules/values_row_width.rs`</sub>

### `sql217` — `SELECT ... LEFT JOIN ... FOR UPDATE`

`SELECT ... LEFT JOIN ... FOR UPDATE` -- the FOR UPDATE locks rows in every joined table even when LEFT JOIN matched no row on the right side. PG returns NULL on the right but still tries to lock; with `OF <alias>` you can restrict but the default form is rarely what the author meant. Suggest FOR UPDATE OF <left> to scope the lock or switch to INNER JOIN.

<sub>`dsl-analysis/src/rules/for_update_left_join.rs`</sub>

### `sql218` — `CASE WHEN ... THEN 1 ... WHEN ... THEN 'foo' ... END`

`CASE WHEN ... THEN 1 ... WHEN ... THEN 'foo' ... END` -- branches return literals of incompatible families (integer + string + boolean). PG raises 42804 at parse time. Local literal sniff only -- expressions / column refs are accepted as unknown.

<sub>`dsl-analysis/src/rules/case_branch_types.rs`</sub>

### `sql219` — `COMMIT` / `ROLLBACK` inside a PL/pgSQL FUNCTION body

`COMMIT` / `ROLLBACK` inside a PL/pgSQL FUNCTION body. PG only allows transaction control statements inside PROCEDUREs; functions get 2D000 "invalid transaction termination" at runtime.

<sub>`dsl-analysis/src/rules/commit_in_function.rs`</sub>

### `sql220` — `WITH RECURSIVE t(...) AS (<single SELECT>) ...`

`WITH RECURSIVE t(...) AS (<single SELECT>) ...` -- the recursive CTE body must use UNION [ALL] to combine the anchor + recursive parts. A single SELECT is structurally non-recursive and the RECURSIVE keyword serves no purpose. PG raises at parse when the body actually self-references; this rule catches the more common case where the author wrote RECURSIVE then forgot the recursion.

<sub>`dsl-analysis/src/rules/recursive_cte_no_union.rs`</sub>

### `sql221` — `ARRAY[1, 'foo']`

`ARRAY[1, 'foo']` -- mixed-type literal constructor. PG raises 42804 "ARRAY types ... and ... cannot be matched" at parse time. Catches the common mistake of bracket-constructed arrays that mix int + text + bool literals.

<sub>`dsl-analysis/src/rules/array_mixed_types.rs`</sub>

### `sql222` — `SELECT * FROM (SELECT ... LIMIT N) FOR UPDATE`

`SELECT * FROM (SELECT ... LIMIT N) FOR UPDATE` -- the outer FOR UPDATE locks every row matched by the inner SELECT, not just the first N. The intended form is `SELECT ... FOR UPDATE LIMIT N` directly inside the inner query. PG silently does the wrong thing here so the lint is the only signal.

<sub>`dsl-analysis/src/rules/limit_for_update_subq.rs`</sub>

### `sql223` — `jsonb_set(col, 'key', '"val"')`

`jsonb_set(col, 'key', '"val"')` -- path must be an array literal `{key}` or `{a,b,c}` not a bare string. PG raises 22P02 "malformed array literal" at runtime.

<sub>`dsl-analysis/src/rules/jsonb_set_path_format.rs`</sub>

### `sql224` — `SET CONSTRAINTS ALL DEFERRED` (or any SET CONSTRAINTS form) outside an explicit transaction block. The...

`SET CONSTRAINTS ALL DEFERRED` (or any SET CONSTRAINTS form) outside an explicit transaction block. The effect is transaction-scoped, so issuing it autocommit means PG resets the constraint mode immediately afterwards -- no-op.

<sub>`dsl-analysis/src/rules/set_constraints_outside_tx.rs`</sub>

### `sql225` — `COMMENT ON ... IS NULL` (or `IS ''`) when the target already has a non-empty catalog comment. PG accepts this

`COMMENT ON ... IS NULL` (or `IS ''`) when the target already has a non-empty catalog comment. PG accepts this -- it deletes the comment silently -- but it's almost never intentional. Suggest making the intent explicit (drop-then-recreate) or remove the statement.

<sub>`dsl-analysis/src/rules/comment_clears_existing.rs`</sub>

### `sql226` — `DROP TABLE foo CASCADE` (or DROP TYPE/etc CASCADE) when the catalog shows 3+ direct dependents (FK...

`DROP TABLE foo CASCADE` (or DROP TYPE/etc CASCADE) when the catalog shows 3+ direct dependents (FK references + views + triggers + indexes). Surface how many objects will be dropped so the author can re-confirm.

<sub>`dsl-analysis/src/rules/drop_cascade_chain.rs`</sub>

### `sql227` — `EXISTS (SELECT * FROM ...)`

`EXISTS (SELECT * FROM ...)` -- the projection is discarded; `SELECT 1` is the conventional form and reads more clearly (and avoids the planner expanding * unnecessarily on wide rows in some PG versions).

<sub>`dsl-analysis/src/rules/exists_select_star.rs`</sub>

### `sql228` — `x = ANY (SELECT 1, 2 FROM ...)`

`x = ANY (SELECT 1, 2 FROM ...)` -- the subquery on the RHS of an ANY/ALL/IN must return exactly one column. PG raises 42601 at parse time. Counts top-level commas in the subquery projection.

<sub>`dsl-analysis/src/rules/any_all_multicol.rs`</sub>

### `sql229` — `WITH foo AS (UPDATE/INSERT/DELETE ...) SELECT * FROM foo` where the data-modifying CTE has no RETURNING...

`WITH foo AS (UPDATE/INSERT/DELETE ...) SELECT * FROM foo` where the data-modifying CTE has no RETURNING clause. PG raises 0A000 "WITH clause containing a data-modifying statement must have a RETURNING clause" when the outer query references the CTE.

<sub>`dsl-analysis/src/rules/cte_dml_no_returning.rs`</sub>

### `sql230` — `CREATE INDEX ... USING GIN (col)` where `col` is a plain scalar (text/int/etc)

`CREATE INDEX ... USING GIN (col)` where `col` is a plain scalar (text/int/etc) -- GIN supports array, jsonb, tsvector, and trgm-extension operator classes. PG raises 42704 "data type X has no default operator class for access method gin" when none of the GIN ops applies.

<sub>`dsl-analysis/src/rules/gin_on_scalar.rs`</sub>

### `sql231` — `NULLS FIRST` / `NULLS LAST` outside an ORDER BY clause

`NULLS FIRST` / `NULLS LAST` outside an ORDER BY clause. PG raises 42601 at parse time. Catches the pattern where the author wrote a DISTINCT or SELECT clause and bolted NULLS FIRST on by mistake.

<sub>`dsl-analysis/src/rules/nulls_first_last_no_order.rs`</sub>

### `sql232` — `<jsonb col> @> 'foo'` (or `<@`) where the RHS is a plain text literal without `::jsonb`

`<jsonb col> @> 'foo'` (or `<@`) where the RHS is a plain text literal without `::jsonb`. PG implicitly casts the literal at runtime; the explicit `::jsonb` cast nudges the planner and reads better. Hint, not error.

<sub>`dsl-analysis/src/rules/jsonb_contains_no_cast.rs`</sub>

### `sql233` — `CREATE MATERIALIZED VIEW mv

`CREATE MATERIALIZED VIEW mv ... WITH NO DATA;` followed by `SELECT ... FROM mv` somewhere later in the buffer. PG raises 55000 "materialized view is not populated" when queried before a REFRESH MATERIALIZED VIEW. Catches the omission.

<sub>`dsl-analysis/src/rules/mv_no_data_query.rs`</sub>

### `sql234` — `WHERE col IN ()`

`WHERE col IN ()` -- literal empty IN list. PG raises 42601 at parse time. Common when generating IN-list from an empty parameter array without guarding.

<sub>`dsl-analysis/src/rules/empty_in_list.rs`</sub>

### `sql235` — `pg_sleep(n)` inside an explicit transaction block

`pg_sleep(n)` inside an explicit transaction block. The sleeping backend keeps every lock + snapshot acquired so far, easily stalls other writers, and consumes a slot. Hint at the risk and suggest sleeping outside the tx.

<sub>`dsl-analysis/src/rules/pg_sleep_in_tx.rs`</sub>

### `sql236` — `AFTER` trigger function returns NEW/OLD row

`AFTER` trigger function returns NEW/OLD row -- PG discards the value for AFTER triggers (only BEFORE / INSTEAD OF can mutate the row via the RETURNed record). Suggest `RETURN NULL` to clarify intent. Cross-references CREATE TRIGGER ... AFTER ... EXECUTE FUNCTION <fn> with the CREATE FUNCTION body in the same buffer.

<sub>`dsl-analysis/src/rules/after_trigger_return_row.rs`</sub>

### `sql237` — A shell command (pg_dump, psql, pg_restore, createdb, dropdb) appears as the first token of a statement. PG...

A shell command (pg_dump, psql, pg_restore, createdb, dropdb) appears as the first token of a statement. PG raises a syntax error -- the author probably pasted a terminal command into the SQL buffer by mistake.

<sub>`dsl-analysis/src/rules/shell_command_in_sql.rs`</sub>

### `sql238` — `<arr> = ARRAY[..., NULL, ...]`

`<arr> = ARRAY[..., NULL, ...]` -- = on arrays treats NULL elements as never-equal (returns NULL not TRUE). Almost always the author wanted `IS NOT DISTINCT FROM` for full equality including NULL elements.

<sub>`dsl-analysis/src/rules/array_eq_with_null.rs`</sub>

### `sql239` — `ALTER TABLE t DROP COLUMN c` where `c` was declared in a `CREATE TABLE t (

`ALTER TABLE t DROP COLUMN c` where `c` was declared in a `CREATE TABLE t (... c ...)` earlier in the same buffer. The migration cancels itself; the author probably meant to drop the create-table column instead.

<sub>`dsl-analysis/src/rules/alter_drop_just_created.rs`</sub>

### `sql240` — `SAVEPOINT s; ... SAVEPOINT s;`

`SAVEPOINT s; ... SAVEPOINT s;` -- declaring the same savepoint name twice inside one transaction. PG allows it: the second SAVEPOINT shadows the first (so ROLLBACK TO s rolls back only to the inner). Almost always a copy-paste mistake.

<sub>`dsl-analysis/src/rules/savepoint_name_reuse.rs`</sub>

### `sql241` — `CREATE [OR REPLACE] VIEW v AS SELECT * FROM t`

`CREATE [OR REPLACE] VIEW v AS SELECT * FROM t` -- the view's column set is frozen at CREATE time. Adding a column to t later does NOT appear in v, and dropping a column from t breaks the view at the next OR REPLACE. Hint: list columns explicitly.

<sub>`dsl-analysis/src/rules/view_select_star.rs`</sub>

### `sql242` — `DROP SCHEMA foo` (no CASCADE / RESTRICT)

`DROP SCHEMA foo` (no CASCADE / RESTRICT) -- PG defaults to RESTRICT and fails with 2BP01 "schema X is not empty" when any object lives inside. Make it explicit so the author confirms their intent.

<sub>`dsl-analysis/src/rules/drop_schema_no_cascade.rs`</sub>

### `sql243` — `FROM (VALUES (1, 2)) WHERE ...`

`FROM (VALUES (1, 2)) WHERE ...` -- a VALUES-derived relation needs an alias plus a column list. PG raises 42601 "subquery in FROM must have an alias" without it.

<sub>`dsl-analysis/src/rules/values_subq_no_alias.rs`</sub>

### `sql244` — `CHECK (TRUE)` / `CHECK (1=1)` / `CHECK (1)` constraint is trivially satisfied

`CHECK (TRUE)` / `CHECK (1=1)` / `CHECK (1)` constraint is trivially satisfied -- it enforces nothing. Almost always placeholder code that escaped review.

<sub>`dsl-analysis/src/rules/check_always_true.rs`</sub>

### `sql245` — `FROM pg_class` (bare) instead of `FROM pg_catalog.pg_class`

`FROM pg_class` (bare) instead of `FROM pg_catalog.pg_class`. search_path resolution lets attackers shadow pg_class with a user-schema table; explicit `pg_catalog.` prefix is the safe pattern (CVE-2018-1058). Same applies to common pg_catalog relations.

<sub>`dsl-analysis/src/rules/pg_catalog_no_schema.rs`</sub>

### `sql246` — `INSERT

`INSERT ... ON CONFLICT DO NOTHING` (without the column list / constraint name to scope it). Without an inference target PG swallows ANY constraint violation: PK clash, UNIQUE, EXCLUDE, even CHECK. Almost always the author wanted to ignore only the specific dup-key case. Suggest naming the conflict target.

<sub>`dsl-analysis/src/rules/on_conflict_do_nothing.rs`</sub>

### `sql247` — `pg_advisory_lock(1)` (or `pg_advisory_xact_lock(1)`) with a hard-coded literal key. PG advisory locks are...

`pg_advisory_lock(1)` (or `pg_advisory_xact_lock(1)`) with a hard-coded literal key. PG advisory locks are global per key, so two unrelated code paths each calling with `1` will serialize on each other -- a hidden mutex. Hint: derive the key from the resource you actually need to serialize on.

<sub>`dsl-analysis/src/rules/advisory_lock_literal_key.rs`</sub>

### `sql248` — `ALTER TABLE t ADD COLUMN c <type> NOT NULL` (no DEFAULT). On PG<11 PG rewrites the whole table to fill the...

`ALTER TABLE t ADD COLUMN c <type> NOT NULL` (no DEFAULT). On PG<11 PG rewrites the whole table to fill the new column -- AccessExclusiveLock for the duration, which is risky on big tables. On PG11+ a constant DEFAULT avoids the rewrite, but NOT NULL alone with no default still fails if any row already exists. Hint: add a DEFAULT or split into two steps (ADD nullable -> backfill -> ALTER SET NOT NULL).

<sub>`dsl-analysis/src/rules/add_column_notnull_no_default.rs`</sub>

### `sql249` — `INSERT INTO t DEFAULT VALUES`

`INSERT INTO t DEFAULT VALUES` -- requires every column to be NOT NULL with a DEFAULT, GENERATED, or nullable. Catches the common case where the catalog table has a NOT NULL column without DEFAULT (and not a serial / generated identity), which PG raises 23502 at runtime.

<sub>`dsl-analysis/src/rules/default_values_no_default_col.rs`</sub>

### `sql250` — `SELECT count(*) FROM t FOR UPDATE`

`SELECT count(*) FROM t FOR UPDATE` -- PG raises 0A000 "FOR UPDATE is not allowed with aggregate functions" / "with GROUP BY clause" at parse time. Catches the pattern where lock intent is bolted onto an aggregate query.

<sub>`dsl-analysis/src/rules/for_update_aggregate.rs`</sub>

### `sql251` — `SELECT * FROM t ORDER BY 1`

`SELECT * FROM t ORDER BY 1` -- positional ORDER BY on a `*` projection is brittle: adding or reordering columns changes which column the sort happens on. Hint: name the column.

<sub>`dsl-analysis/src/rules/star_with_order_by_position.rs`</sub>

### `sql252` — `SELECT * FROM (SELECT ... ORDER BY x) sub`

`SELECT * FROM (SELECT ... ORDER BY x) sub` -- the outer SELECT is free to re-order, so the inner ORDER BY is a no-op unless paired with LIMIT/OFFSET/FETCH. The author probably wanted to sort the final result, not the intermediate.

<sub>`dsl-analysis/src/rules/order_by_in_subquery.rs`</sub>

### `sql253` — `x NOT IN (SELECT col FROM t)` where `col` is nullable

`x NOT IN (SELECT col FROM t)` where `col` is nullable. If the subquery returns even one NULL, the whole `NOT IN` predicate evaluates to UNKNOWN -> filtered out. Almost always a bug. Suggest `NOT EXISTS` or filter NULLs in the subquery.

<sub>`dsl-analysis/src/rules/not_in_nullable.rs`</sub>

### `sql254` — `ALTER TABLE t SET TABLESPACE ts` rewrites the entire table on disk and holds AccessExclusiveLock for the...

`ALTER TABLE t SET TABLESPACE ts` rewrites the entire table on disk and holds AccessExclusiveLock for the duration. On large tables this is a sustained outage. Hint: use ALTER TABLE ... SET TABLESPACE ... NOWAIT or schedule a maintenance window.

<sub>`dsl-analysis/src/rules/alter_set_tablespace.rs`</sub>

### `sql255` — `ROW_NUMBER() OVER ()` / `RANK() OVER ()` / `LAG() OVER ()` without an ORDER BY in the window definition

`ROW_NUMBER() OVER ()` / `RANK() OVER ()` / `LAG() OVER ()` without an ORDER BY in the window definition. The ranking / position is undefined and changes between executions. PG accepts it, but the result is non-deterministic.

<sub>`dsl-analysis/src/rules/window_no_order.rs`</sub>

### `sql256` — `current_setting('foo')`

`current_setting('foo')` -- if the GUC isn't set, PG raises 42704. The 2-arg form `current_setting('foo', true)` returns NULL instead, which is almost always what callers want when reading optional settings. Hint: pass `missing_ok=true`.

<sub>`dsl-analysis/src/rules/current_setting_no_missing_ok.rs`</sub>

### `sql257` — `DO $$ BEGIN SELECT now(); END $$;`

`DO $$ BEGIN SELECT now(); END $$;` -- inside a DO block a bare `SELECT` discards its result (DO doesn't return rows). The author probably meant PERFORM (to evaluate side effects) or RAISE NOTICE (to print) or SELECT ... INTO <var>.

<sub>`dsl-analysis/src/rules/do_block_bare_select.rs`</sub>

### `sql258` — `SET LOCAL <foo> = <val>` outside an explicit transaction block

`SET LOCAL <foo> = <val>` outside an explicit transaction block. SET LOCAL scopes to the tx, so issued in autocommit it's a no-op + immediate reset. Catches the migration file that calls SET LOCAL search_path then forgets to wrap in BEGIN/COMMIT.

<sub>`dsl-analysis/src/rules/set_local_outside_tx.rs`</sub>

### `sql259` — `SET ROLE <foo>` inside a CREATE FUNCTION body. Almost never intentional

`SET ROLE <foo>` inside a CREATE FUNCTION body. Almost never intentional -- SET ROLE persists past the function call (it's session-scoped, not function-scoped) so the caller's role is silently mutated. Use SECURITY DEFINER to run as the function owner, or wrap in `SET LOCAL ROLE` within a BEGIN/COMMIT.

<sub>`dsl-analysis/src/rules/set_role_in_function.rs`</sub>

### `sql260` — `DROP FUNCTION foo` without an argument signature. On PG14+ this works when there's only one overload, but it...

`DROP FUNCTION foo` without an argument signature. On PG14+ this works when there's only one overload, but it fails if any second overload exists -- the drop becomes ambiguous. Hint: always pass the arg list to make the migration deterministic.

<sub>`dsl-analysis/src/rules/drop_function_no_args.rs`</sub>

### `sql261` — `MERGE INTO t USING src ON ... ;`

`MERGE INTO t USING src ON ... ;` -- needs at least one WHEN MATCHED / WHEN NOT MATCHED clause; PG raises 42601 at parse.

<sub>`dsl-analysis/src/rules/merge_missing_when.rs`</sub>

### `sql262` — `CREATE EXTENSION pg_stat_statements` (without IF NOT EXISTS)

`CREATE EXTENSION pg_stat_statements` (without IF NOT EXISTS). Migration scripts almost always want the idempotent form. Hint: add IF NOT EXISTS.

<sub>`dsl-analysis/src/rules/extension_no_if_not_exists.rs`</sub>

### `sql263` — `SELECT * FROM (SELECT DISTINCT ON (k) ... FROM t) sub` without an ORDER BY inside the subquery. DISTINCT ON...

`SELECT * FROM (SELECT DISTINCT ON (k) ... FROM t) sub` without an ORDER BY inside the subquery. DISTINCT ON picks the "first" row per group based on the inner ORDER BY -- without it PG picks an arbitrary row and the result is non-deterministic.

<sub>`dsl-analysis/src/rules/distinct_on_subq_no_order.rs`</sub>

### `sql264` — `UPDATE pg_class SET ...` / `DELETE FROM pg_class` and other direct DML against `pg_catalog` system tables

`UPDATE pg_class SET ...` / `DELETE FROM pg_class` and other direct DML against `pg_catalog` system tables. Requires `allow_system_table_mods = on` + superuser and is almost always a footgun (corrupts the catalog). Block it with an Error so the author has to actively dismiss.

<sub>`dsl-analysis/src/rules/system_catalog_dml.rs`</sub>

### `sql265` — `CREATE TABLE t (..., c TIMESTAMP DEFAULT now(), ...)`

`CREATE TABLE t (..., c TIMESTAMP DEFAULT now(), ...)` -- now() returns `timestamptz` so PG silently converts to local timezone for storage in a non-TZ column. Subsequent reads then drift by tz changes. Suggest TIMESTAMPTZ for the column or `(now() AT TIME ZONE 'UTC')` for the default.

<sub>`dsl-analysis/src/rules/now_default_on_timestamp.rs`</sub>

### `sql266` — `jsonb_build_object(k1, v1, k2)`

`jsonb_build_object(k1, v1, k2)` -- argument count must be even (alternating key/value). PG raises 22023 at runtime. Same for `json_build_object`.

<sub>`dsl-analysis/src/rules/jsonb_build_odd_args.rs`</sub>

### `sql267` — `a = b = c` chained comparison

`a = b = c` chained comparison. SQL doesn't have Python- style chaining; this parses as `(a = b) = c` which compares a boolean to c. Almost always a logic bug. Hint: `a = b AND b = c`.

<sub>`dsl-analysis/src/rules/chained_comparison.rs`</sub>

### `sql268` — `(SELECT ... ORDER BY a) UNION (SELECT ...)`

`(SELECT ... ORDER BY a) UNION (SELECT ...)` -- ORDER BY inside a UNION branch is allowed only on the LAST branch (and applies to the whole UNION). PG raises 42601 when an earlier branch has ORDER BY without LIMIT/OFFSET.

<sub>`dsl-analysis/src/rules/union_inner_order_by.rs`</sub>

### `sql269` — `WHERE EXTRACT(YEAR FROM ts) = 2024` or `WHERE date_part('year', ts) = 2024`

`WHERE EXTRACT(YEAR FROM ts) = 2024` or `WHERE date_part('year', ts) = 2024` -- wrapping a timestamp column in EXTRACT / date_part prevents the planner from using a btree index. Suggest a range predicate (`ts >= '2024-01-01' AND ts < '2025-01-01'`) so the index applies. Skip when the operand isn't a real column (e.g. CURRENT_DATE, now()) -- no index to block in that case.

<sub>`dsl-analysis/src/rules/extract_on_indexable.rs`</sub>

### `sql270` — `format('hello world')`

`format('hello world')` -- call to format() with no `%` placeholders. Result is identical to the input string and the function call overhead is wasted. Hint: pass the string literally.

<sub>`dsl-analysis/src/rules/format_no_placeholders.rs`</sub>

### `sql271` — `DECLARE c CURSOR WITH HOLD FOR ...` outside an explicit transaction

`DECLARE c CURSOR WITH HOLD FOR ...` outside an explicit transaction. WITH HOLD only matters if the cursor needs to survive the tx that opened it; in autocommit mode there is no tx so PG either errors or the HOLD is a no-op (depends on PG version).

<sub>`dsl-analysis/src/rules/cursor_with_hold_no_tx.rs`</sub>

### `sql272` — `CREATE INDEX

`CREATE INDEX ... USING GIST (col)` where `col`'s catalog type doesn't have a default GIST operator class. Common cases: plain int/text/uuid. PG raises 42704 unless the btree_gist extension is installed and the opclass is explicit.

<sub>`dsl-analysis/src/rules/gist_on_scalar.rs`</sub>

### `sql273` — `CHECK (FALSE)` / `CHECK (0)` constraint rejects every row

`CHECK (FALSE)` / `CHECK (0)` constraint rejects every row. Almost certainly a placeholder that escaped review.

<sub>`dsl-analysis/src/rules/check_always_false.rs`</sub>

### `sql274` — `SELECT ... INTO TEMP foo FROM bar` (or INTO TEMPORARY) where `foo` is also a real catalog table. PG allows it

`SELECT ... INTO TEMP foo FROM bar` (or INTO TEMPORARY) where `foo` is also a real catalog table. PG allows it -- the temp shadows the base for the session -- but it almost always breaks subsequent queries that thought they were hitting the base table.

<sub>`dsl-analysis/src/rules/select_into_temp_shadows.rs`</sub>

### `sql275` — `SET TRANSACTION ...` (READ ONLY / READ WRITE / ISOLATION LEVEL) inside a CREATE FUNCTION body

`SET TRANSACTION ...` (READ ONLY / READ WRITE / ISOLATION LEVEL) inside a CREATE FUNCTION body. Function bodies run inside the caller's open tx and cannot mutate transaction characteristics mid-flight. PG raises 25001 "SET TRANSACTION ISOLATION LEVEL must be called before any query" at runtime.

<sub>`dsl-analysis/src/rules/set_transaction_in_function.rs`</sub>

### `sql276` — `INTERVAL 1 DAY` style (no quotes)

`INTERVAL 1 DAY` style (no quotes) -- that's the MySQL literal form. PG requires `INTERVAL '1 day'`. Catches the common mistake of porting MySQL SQL verbatim.

<sub>`dsl-analysis/src/rules/mysql_interval_syntax.rs`</sub>

### `sql277` — `COMMENT ON FUNCTION foo IS '...'` without argument signature. Same hazard as DROP FUNCTION

`COMMENT ON FUNCTION foo IS '...'` without argument signature. Same hazard as DROP FUNCTION -- fails when multiple overloads exist (PG 42725). Hint: always pass the arg-type list.

<sub>`dsl-analysis/src/rules/comment_fn_no_args.rs`</sub>

### `sql278` — `<expr> / 0` literal division by zero

`<expr> / 0` literal division by zero. PG raises 22012 at runtime. Catches the common typo / placeholder.

<sub>`dsl-analysis/src/rules/literal_div_zero.rs`</sub>

### `sql279` — `COMMENT ON CONSTRAINT pk_users IS '...'`

`COMMENT ON CONSTRAINT pk_users IS '...'` -- needs the `ON <table>` qualifier (e.g. `ON users`). PG raises 42601 at parse time without it.

<sub>`dsl-analysis/src/rules/comment_constraint_no_on.rs`</sub>

### `sql280` — `ALTER TABLE t ADD CONSTRAINT c CHECK (...)` without `NOT VALID`

`ALTER TABLE t ADD CONSTRAINT c CHECK (...)` without `NOT VALID`. PG scans every existing row to validate, holding AccessExclusiveLock the whole time. On big tables that's a sustained outage. Pattern: ADD CONSTRAINT ... NOT VALID + later `VALIDATE CONSTRAINT` (only ShareUpdateExclusiveLock).

<sub>`dsl-analysis/src/rules/alter_add_check_no_not_valid.rs`</sub>

### `sql281` — `ALTER TABLE t ALTER COLUMN c SET NOT NULL`

`ALTER TABLE t ALTER COLUMN c SET NOT NULL` -- PG scans every row to verify nullability + holds AccessExclusiveLock. On big tables: outage. Recommended pattern: add CHECK (c IS NOT NULL) NOT VALID, validate it in the background, then SET NOT NULL (which on PG12+ short-circuits when an equivalent CHECK is already VALID).

<sub>`dsl-analysis/src/rules/alter_set_not_null_scan.rs`</sub>

### `sql282` — `WHERE 1=1 AND ...` / `WHERE TRUE AND ...`

`WHERE 1=1 AND ...` / `WHERE TRUE AND ...` -- the leading tautology is a placeholder common in dynamic-SQL generators where every real condition gets prepended with `AND`. In hand-written static SQL it's just noise.

<sub>`dsl-analysis/src/rules/where_true_placeholder.rs`</sub>

### `sql283` — `ANALYZE` (or `ANALYZE t`) inside an explicit transaction

`ANALYZE` (or `ANALYZE t`) inside an explicit transaction. ANALYZE acquires ShareUpdateExclusiveLock per table; bundled in a long-running tx those locks are held until COMMIT and block autovacuum / other ANALYZE concurrently. PG accepts it, but almost always you want ANALYZE outside the tx.

<sub>`dsl-analysis/src/rules/analyze_in_tx.rs`</sub>

### `sql284` — `TG_OP`, `TG_TABLE_NAME`, `TG_RELID`, `TG_NAME`, `TG_WHEN`, `TG_LEVEL`, `TG_NARGS`, `TG_ARGV` referenced...

`TG_OP`, `TG_TABLE_NAME`, `TG_RELID`, `TG_NAME`, `TG_WHEN`, `TG_LEVEL`, `TG_NARGS`, `TG_ARGV` referenced inside a CREATE FUNCTION body that doesn't return TRIGGER. PG raises 42703 at runtime -- the TG_* vars are only bound in trigger functions.

<sub>`dsl-analysis/src/rules/tg_var_in_non_trigger.rs`</sub>

### `sql285` — `DROP ROLE foo` / `DROP USER foo` without a preceding `REASSIGN OWNED BY foo` + `DROP OWNED BY foo`

`DROP ROLE foo` / `DROP USER foo` without a preceding `REASSIGN OWNED BY foo` + `DROP OWNED BY foo`. PG raises 2BP01 when the role still owns any object (or has any privileges). Hint: run the reassign/drop-owned pair first.

<sub>`dsl-analysis/src/rules/drop_role_no_reassign.rs`</sub>

### `sql286` — `ALTER TYPE x ADD VALUE 'new' BEFORE 'bogus'` where `bogus` is not one of `x`'s enum labels

`ALTER TYPE x ADD VALUE 'new' BEFORE 'bogus'` where `bogus` is not one of `x`'s enum labels. PG raises 22023 at parse. Catches typos in the anchor label. Source-text scan: harvests every `CREATE TYPE x AS ENUM (...)` to know each enum's labels, then validates the BEFORE/AFTER anchor.

<sub>`dsl-analysis/src/rules/alter_type_label_unknown.rs`</sub>

### `sql287` — `REVOKE ... CASCADE` on a privilege the grantee may have re-granted. CASCADE recursively revokes from every...

`REVOKE ... CASCADE` on a privilege the grantee may have re-granted. CASCADE recursively revokes from every onward grantee -- a chain reaction. Hint: confirm intent or use `RESTRICT` (the default) so failures are explicit.

<sub>`dsl-analysis/src/rules/revoke_cascade.rs`</sub>

### `sql288` — `CREATE INDEX ON t (col)`

`CREATE INDEX ON t (col)` -- PG auto-generates a name like `t_col_idx`, but the name is hard to reference for later DROP / REINDEX and gets ugly with expression indexes. Hint: name the index explicitly.

<sub>`dsl-analysis/src/rules/index_no_name.rs`</sub>

### `sql289` — `CREATE TABLE ... INHERITS (parent)`

`CREATE TABLE ... INHERITS (parent)` -- table inheritance predates partitioning and has surprising semantics (UNIQUE/PK aren't enforced across children, FK only references parent rows). For partitioning use cases, declarative partitioning (PG10+) is the recommended path: `CREATE TABLE child PARTITION OF parent ...`.

<sub>`dsl-analysis/src/rules/table_inherits.rs`</sub>

### `sql290` — `percentile_cont(0.5)` / `percentile_disc(0.5)` / `mode()` without the required `WITHIN GROUP (ORDER BY ...)`...

`percentile_cont(0.5)` / `percentile_disc(0.5)` / `mode()` without the required `WITHIN GROUP (ORDER BY ...)` clause. These are ordered-set aggregates; PG raises 42883 at parse without WITHIN GROUP.

<sub>`dsl-analysis/src/rules/percentile_no_within.rs`</sub>

### `sql291` — `GRANT ALL PRIVILEGES ON ...` (or bare `GRANT ALL`)

`GRANT ALL PRIVILEGES ON ...` (or bare `GRANT ALL`) -- the principle of least privilege says enumerate. Hint: list the specific privileges (SELECT / INSERT / UPDATE / DELETE / USAGE / EXECUTE / TRIGGER / etc).

<sub>`dsl-analysis/src/rules/grant_all_too_broad.rs`</sub>

### `sql292` — `LIMIT 0` returns zero rows

`LIMIT 0` returns zero rows. Sometimes used to fetch the column metadata of a query without the rows, but more often a leftover placeholder. Worth a Hint to confirm intent.

<sub>`dsl-analysis/src/rules/limit_zero.rs`</sub>

### `sql293` — `NULLIF(1, 'foo')`

`NULLIF(1, 'foo')` -- args must be comparable. PG raises 42883 (operator does not exist) at runtime. Same for GREATEST/LEAST with mixed literal types.

<sub>`dsl-analysis/src/rules/nullif_type_mismatch.rs`</sub>

### `sql294` — `BEGIN;` (or `START TRANSACTION;`) when an earlier BEGIN in the source hasn't been COMMITed / ROLLBACKed

`BEGIN;` (or `START TRANSACTION;`) when an earlier BEGIN in the source hasn't been COMMITed / ROLLBACKed. PG emits WARNING "there is already a transaction in progress". The author probably meant SAVEPOINT for a nested rollback unit.

<sub>`dsl-analysis/src/rules/nested_begin.rs`</sub>

### `sql295` — `COPY ... WITH (HEADER, FORMAT TEXT)`

`COPY ... WITH (HEADER, FORMAT TEXT)` -- the HEADER option is only valid for CSV format. PG raises 42601 at parse.

<sub>`dsl-analysis/src/rules/copy_header_no_csv.rs`</sub>

### `sql296` — `REINDEX` (TABLE / INDEX / SCHEMA / DATABASE) inside an open transaction. PG holds AccessExclusiveLock for...

`REINDEX` (TABLE / INDEX / SCHEMA / DATABASE) inside an open transaction. PG holds AccessExclusiveLock for the whole tx duration -- a sustained outage on busy tables. Run REINDEX outside BEGIN/COMMIT, or use CONCURRENTLY (PG12+, and outside tx -- see sql214).

<sub>`dsl-analysis/src/rules/reindex_in_tx.rs`</sub>

### `sql297` — `NOTIFY chan, '<huge literal>'`

`NOTIFY chan, '<huge literal>'` -- PG caps NOTIFY payload at NAMEDATALEN-bound length (default 8000 bytes); larger payloads raise 22023 at runtime. Catches the obvious case where the literal in the SQL exceeds the limit.

<sub>`dsl-analysis/src/rules/notify_payload_too_large.rs`</sub>

### `sql298` — CREATE TABLE / FUNCTION / TYPE / INDEX / TRIGGER / CONSTRAINT name longer than 63 bytes

CREATE TABLE / FUNCTION / TYPE / INDEX / TRIGGER / CONSTRAINT name longer than 63 bytes. PG silently truncates to NAMEDATALEN-1 (default 63) and emits a NOTICE, so distinct "long_name_abc" / "long_name_xyz" can collide.

<sub>`dsl-analysis/src/rules/identifier_too_long.rs`</sub>

### `sql299` — `PRIMARY KEY (a, a)` / `UNIQUE (a, a)`

`PRIMARY KEY (a, a)` / `UNIQUE (a, a)` -- duplicate column in the key. PG raises 42P16 at parse time.

<sub>`dsl-analysis/src/rules/pk_duplicate_col.rs`</sub>

### `sql300` — `SELECT a, b, FROM t`

`SELECT a, b, FROM t` -- trailing comma in projection list. PG raises 42601 at parse. Catches the very common typo from copy-pasting projection items.

<sub>`dsl-analysis/src/rules/select_trailing_comma.rs`</sub>

### `sql301` — `COPY ... FROM PROGRAM 'cmd'` / `COPY ... TO PROGRAM 'cmd'`

`COPY ... FROM PROGRAM 'cmd'` / `COPY ... TO PROGRAM 'cmd'` -- runs a shell command as the PG server OS user. Requires superuser and is a massive RCE risk if reachable from user-supplied SQL. Flag it loudly.

<sub>`dsl-analysis/src/rules/copy_program_exec.rs`</sub>

### `sql302` — `DROP TABLE foo` (or DROP INDEX/VIEW/TRIGGER/etc) without `IF EXISTS`

`DROP TABLE foo` (or DROP INDEX/VIEW/TRIGGER/etc) without `IF EXISTS`. Migrations and rollback scripts almost always want the idempotent form; otherwise rerun raises 42P01.

<sub>`dsl-analysis/src/rules/drop_table_no_if_exists.rs`</sub>

### `sql303` — `ARRAY[]` (empty constructor) without a `::type[]` cast

`ARRAY[]` (empty constructor) without a `::type[]` cast. PG raises 42P18 "cannot determine type of empty array". Suggest e.g. `ARRAY[]::int[]`.

<sub>`dsl-analysis/src/rules/empty_array_no_cast.rs`</sub>

### `sql304` — CREATE TABLE foo (..., parent_id REFERENCES foo(id))

CREATE TABLE foo (..., parent_id REFERENCES foo(id)) -- self-referential FK without DEFERRABLE. INSERT into a chain requires inserting parents before children; DEFERRABLE INITIALLY DEFERRED lets you insert in any order inside a tx.

<sub>`dsl-analysis/src/rules/self_fk_no_deferrable.rs`</sub>

### `sql305` — `FROM information_schema.<view>`

`FROM information_schema.<view>` -- the standard SQL introspection views are usually 10-100x slower than the equivalent `pg_catalog` queries because they're built on cross-schema joins. Hint: for any non-portable script, query `pg_catalog` directly.

<sub>`dsl-analysis/src/rules/information_schema_perf.rs`</sub>

### `sql306` — `WHERE id IN (1, 1, 2)`

`WHERE id IN (1, 1, 2)` -- duplicate literal in IN list. Planner dedups but the query is larger + harder to read.

<sub>`dsl-analysis/src/rules/in_list_duplicates.rs`</sub>

### `sql307` — `UPDATE ... LIMIT N` / `DELETE ... LIMIT N`

`UPDATE ... LIMIT N` / `DELETE ... LIMIT N` -- PG does not support LIMIT on UPDATE or DELETE (only SELECT). PG raises 42601 at parse. MySQL allows it; common port mistake. Suggest `UPDATE ... WHERE ctid IN (SELECT ctid FROM t WHERE ... LIMIT N)`.

<sub>`dsl-analysis/src/rules/update_delete_limit.rs`</sub>

### `sql308` — `TIMESTAMP(7)` / `TIME(7)` / `TIMESTAMPTZ(7)` etc

`TIMESTAMP(7)` / `TIME(7)` / `TIMESTAMPTZ(7)` etc. PG caps date/time precision at 6 (microseconds). Higher precisions are silently capped. Hint: drop to (6) or omit.

<sub>`dsl-analysis/src/rules/timestamp_precision_over.rs`</sub>

### `sql309` — `REVOKE SELECT ON foo;`

`REVOKE SELECT ON foo;` -- missing `FROM <role>`. PG raises 42601 at parse time. Catches the typo where author ported GRANT syntax but forgot to flip TO to FROM.

<sub>`dsl-analysis/src/rules/revoke_missing_from.rs`</sub>

### `sql310` — line starts with `\<letter>`

line starts with `\<letter>` -- psql meta-command (`\d`, `\dt`, `\l`, `\timing`, `\copy`, etc). Only psql parses these; the SQL server raises 42601. Common when copy-pasting from a psql session into a file or app.

<sub>`dsl-analysis/src/rules/psql_backslash.rs`</sub>

### `sql311` — `string_agg(col, ',')` / `array_agg(col)` / `json_agg(col)` / `jsonb_agg(col)` without an `ORDER BY` clause...

`string_agg(col, ',')` / `array_agg(col)` / `json_agg(col)` / `jsonb_agg(col)` without an `ORDER BY` clause inside the aggregate -- concatenation order is non-deterministic and depends on the plan.

<sub>`dsl-analysis/src/rules/string_agg_no_order.rs`</sub>

### `sql312` — column declared `SERIAL` / `BIGSERIAL` / `SMALLSERIAL`

column declared `SERIAL` / `BIGSERIAL` / `SMALLSERIAL`. These are the legacy pre-PG10 form. On PG10+ the preferred form is `GENERATED ALWAYS AS IDENTITY` (SQL standard, no leaked sequence permissions, no ownership coupling, can be UPDATEd via OVERRIDING SYSTEM VALUE).

<sub>`dsl-analysis/src/rules/serial_vs_identity.rs`</sub>

### `sql313` — `CREATE TABLE t (...) COMMENT 'msg'`

`CREATE TABLE t (...) COMMENT 'msg'` -- MySQL inline- comment syntax. PG requires `COMMENT ON TABLE t IS 'msg'` as a separate statement. Catches the common port mistake.

<sub>`dsl-analysis/src/rules/mysql_table_comment.rs`</sub>

### `sql314` — `AUTO_INCREMENT`

`AUTO_INCREMENT` -- MySQL column attribute. PG has no AUTO_INCREMENT; use `SERIAL` / `BIGSERIAL` (legacy) or `GENERATED ALWAYS AS IDENTITY` (preferred, PG10+).

<sub>`dsl-analysis/src/rules/mysql_auto_increment.rs`</sub>

### `sql315` — `ENGINE=InnoDB` / `ENGINE=MyISAM` / similar

`ENGINE=InnoDB` / `ENGINE=MyISAM` / similar -- MySQL storage-engine attribute. PG rejects with 42601.

<sub>`dsl-analysis/src/rules/mysql_engine.rs`</sub>

### `sql316` — MySQL-only types (TINYINT, MEDIUMINT, LONGTEXT, etc)

MySQL-only types (TINYINT, MEDIUMINT, LONGTEXT, etc). PG accepts INTEGER/SMALLINT/TEXT instead.

<sub>`dsl-analysis/src/rules/mysql_types.rs`</sub>

### `sql317` — `[identifier]` (square-bracket quoting)

`[identifier]` (square-bracket quoting) -- MSSQL/T-SQL syntax. PG uses double quotes. Avoids false positives on array subscripts by requiring the bracket content to look like an identifier (no operators, single token, no digits-only).

<sub>`dsl-analysis/src/rules/mssql_bracket_quote.rs`</sub>

### `sql318` — `SELECT TOP 10 ...`

`SELECT TOP 10 ...` -- MSSQL/Sybase syntax. PG uses `SELECT ... LIMIT 10`. Catches a common port mistake.

<sub>`dsl-analysis/src/rules/mssql_top.rs`</sub>

### `sql319` — `ISNULL(x, y)` (MSSQL/MySQL) / `NVL(x, y)` (Oracle) / `IFNULL(x, y)` (MySQL)

`ISNULL(x, y)` (MSSQL/MySQL) / `NVL(x, y)` (Oracle) / `IFNULL(x, y)` (MySQL) -- non-PG NULL-coalesce functions. PG has `COALESCE(x, y, ...)` (SQL standard) and `NULLIF(x, y)`.

<sub>`dsl-analysis/src/rules/non_pg_null_fns.rs`</sub>

### `sql320` — `GETDATE()` / `SYSDATE` / `GETUTCDATE()`

`GETDATE()` / `SYSDATE` / `GETUTCDATE()` -- non-PG current-time forms. PG uses `now()` (or `CURRENT_TIMESTAMP`).

<sub>`dsl-analysis/src/rules/non_pg_date_fns.rs`</sub>

### `sql321` — standalone `GO`

standalone `GO` -- MSSQL batch separator. PG raises 42601 (`syntax error at or near "GO"`).

<sub>`dsl-analysis/src/rules/mssql_go.rs`</sub>

### `sql322` — `BEGIN TRAN`

`BEGIN TRAN` -- MSSQL shorthand for BEGIN TRANSACTION. PG only accepts `BEGIN`, `BEGIN TRANSACTION`, or `BEGIN WORK`.

<sub>`dsl-analysis/src/rules/mssql_begin_tran.rs`</sub>

### `sql323` — `SELECT ... FROM DUAL`

`SELECT ... FROM DUAL` -- Oracle's dummy single-row table. PG doesn't have DUAL; `SELECT 1;` works without FROM.

<sub>`dsl-analysis/src/rules/oracle_dual.rs`</sub>

### `sql324` — `ROWNUM`

`ROWNUM` -- Oracle pseudo-column. PG has no ROWNUM; use `LIMIT N` (paging top-N) or `ROW_NUMBER() OVER (...)` (ranking) instead.

<sub>`dsl-analysis/src/rules/oracle_rownum.rs`</sub>

### `sql325` — `CONNECT BY PRIOR ...`

`CONNECT BY PRIOR ...` -- Oracle hierarchical query. PG has no CONNECT BY; use `WITH RECURSIVE` instead.

<sub>`dsl-analysis/src/rules/oracle_connect_by.rs`</sub>

### `sql326` — `a.id = b.id(+)`

`a.id = b.id(+)` -- Oracle's pre-ANSI outer-join hint. PG uses ANSI `LEFT JOIN` / `RIGHT JOIN` instead.

<sub>`dsl-analysis/src/rules/oracle_outer_join.rs`</sub>

### `sql327` — `CREATE TABLE foo (...)` without an explicit schema qualifier. Style hint: every CREATE TABLE in a...

`CREATE TABLE foo (...)` without an explicit schema qualifier. Style hint: every CREATE TABLE in a multi-schema project should spell out which schema the table belongs to. Otherwise the table lands in whatever `search_path` happens to be first -- usually `public`, but breaks if a migration runs with a different default.

<sub>`dsl-analysis/src/rules/create_table_no_schema.rs`</sub>

### `sql328` — REVOKE in a buffer that has no matching GRANT

REVOKE in a buffer that has no matching GRANT. Style/safety: a stand-alone REVOKE migration depends on whoever ran the original GRANT. When the buffer contains the GRANT/REVOKE pair the intent is obvious; a lone REVOKE usually means the migration author has assumed a prior state that may not hold.

<sub>`dsl-analysis/src/rules/revoke_without_grant.rs`</sub>

### `sql329` — `substring(text FROM <number>)` without a matching `FOR`. PG returns the rest of the string from the start...

`substring(text FROM <number>)` without a matching `FOR`. PG returns the rest of the string from the start position when FOR is omitted, which is rarely what the author wanted -- almost every sighting in code review turns out to be a typo for `FOR n`. Make it explicit.

<sub>`dsl-analysis/src/rules/substring_from_no_for.rs`</sub>

### `sql331` — `DROP INDEX CONCURRENTLY` inside an explicit transaction

`DROP INDEX CONCURRENTLY` inside an explicit transaction. Like `CREATE INDEX CONCURRENTLY`, the CONCURRENTLY drop variant cannot run inside a BEGIN/COMMIT block. PG raises 25001 at runtime. Flag when the same buffer mixes a CONCURRENTLY drop with a BEGIN.

<sub>`dsl-analysis/src/rules/drop_index_concurrently_in_tx.rs`</sub>

### `sql332` — `pg_terminate_backend(...)` / `pg_cancel_backend(...)` invoked from an unprivileged buffer

`pg_terminate_backend(...)` / `pg_cancel_backend(...)` invoked from an unprivileged buffer. PG requires the caller to be a superuser (or have `pg_signal_backend` on PG13+). Useful to flag because the failure mode is silent (function returns `false`).

<sub>`dsl-analysis/src/rules/pg_terminate_backend.rs`</sub>

### `sql333` — `ON UPDATE CASCADE` on a column referenced as a primary key. ON UPDATE CASCADE is rarely the right choice on...

`ON UPDATE CASCADE` on a column referenced as a primary key. ON UPDATE CASCADE is rarely the right choice on a PK column -- PK values are supposed to be immutable. Almost always means the author confused ON UPDATE with ON DELETE intent. Warn.

<sub>`dsl-analysis/src/rules/on_update_cascade_pk.rs`</sub>

### `sql334` — `SELECT setseed(...)` without a nearby deterministic guard

`SELECT setseed(...)` without a nearby deterministic guard. `setseed()` is a hidden source of plan instability: subsequent calls to `random()` become deterministic, which is great for tests but dangerous in shared-session contexts because the seed leaks across queries. Hint when a buffer calls setseed without an obvious test marker (`BEGIN;` / `SET LOCAL` / comment-pragma).

<sub>`dsl-analysis/src/rules/setseed_no_determinism_guard.rs`</sub>

### `sql335` — explicit `TABLESPACE <name>` clause in a buffer that likely runs as a non-superuser migration

explicit `TABLESPACE <name>` clause in a buffer that likely runs as a non-superuser migration. PG only allows TABLESPACE on objects the caller owns + can create-in-tblspc; cloud-hosted PG usually rejects non-default tablespaces outright. Hint that this will break in many deployment targets.

<sub>`dsl-analysis/src/rules/tablespace_specified.rs`</sub>

### `sql336` — `bytea` literal `'\\xFF'` without the `E''` escape-string prefix

`bytea` literal `'\\xFF'` without the `E''` escape-string prefix. PG defaults to standard-conforming strings on PG9.1+, so a bare backslash is *literal*, not an escape. Hex-bytea literals need `'\xFF'::bytea` or `E'\\xFF'`.

<sub>`dsl-analysis/src/rules/bytea_literal_no_escape.rs`</sub>

### `sql337` — `GROUP BY` references a SELECT-list alias instead of the original column

`GROUP BY` references a SELECT-list alias instead of the original column. PG accepts this since 9.0 but the SQL standard says alias names aren't in scope for GROUP BY; many other engines reject it. Hint to use the underlying column expression.

<sub>`dsl-analysis/src/rules/group_by_alias.rs`</sub>

### `sql338` — `CREATE TABLE x PARTITION OF parent (LIKE base INCLUDING INDEXES ...)` INCLUDING INDEXES inside a PARTITION...

`CREATE TABLE x PARTITION OF parent (LIKE base INCLUDING INDEXES ...)` INCLUDING INDEXES inside a PARTITION OF body is silently ignored by PG: partition tables can't declare independent indexes that way -- the parent's index template attaches them. Flag so the author knows the clause is a no-op.

<sub>`dsl-analysis/src/rules/like_include_indexes_partition.rs`</sub>

### `sql339` — `TRUNCATE` inside a PL/pgSQL function body that also has an `EXCEPTION` block

`TRUNCATE` inside a PL/pgSQL function body that also has an `EXCEPTION` block. PL/pgSQL EXCEPTION wraps the body in a subxact; TRUNCATE acquires an ACCESS EXCLUSIVE lock that doesn't roll back cleanly inside subxacts and can leave the catalog in a state where the row visibility is wrong for the rest of the transaction. Hint.

<sub>`dsl-analysis/src/rules/truncate_in_plpgsql_exception.rs`</sub>

### `sql340` — `NEW.id := <expr>` inside a `BEFORE INSERT` trigger body

`NEW.id := <expr>` inside a `BEFORE INSERT` trigger body. When the target table has a SERIAL / IDENTITY PK, assigning NEW.id before INSERT silently bypasses the sequence default. Usually a bug: either the trigger should use a different column or it should call `nextval()` explicitly so the sequence stays in sync.

<sub>`dsl-analysis/src/rules/new_assign_pk_in_before_insert.rs`</sub>

### `sql341` — `INSERT INTO t (col) VALUES (ARRAY[...])` where the array element family doesn't match the target column's...

`INSERT INTO t (col) VALUES (ARRAY[...])` where the array element family doesn't match the target column's element family, e.g. `text_col := ARRAY[1, 2, 3]`. Conservative: only fires when both element + column families are known and disagree.

<sub>`dsl-analysis/src/rules/array_elem_vs_col.rs`</sub>

### `sql342` — `BOOL_AND(col)` / `BOOL_OR(col)` / `EVERY(col)` on a nullable boolean column

`BOOL_AND(col)` / `BOOL_OR(col)` / `EVERY(col)` on a nullable boolean column. PG silently ignores NULL inputs, so the result hides the fact that some rows had no opinion. Suggest COALESCE(col, false) or an explicit IS NULL filter.

<sub>`dsl-analysis/src/rules/bool_agg_nullable.rs`</sub>

### `sql343` — `percent_rank() OVER (ORDER BY <col>)` / `cume_dist() OVER (ORDER BY <col>)` where `<col>` is a non-numeric...

`percent_rank() OVER (ORDER BY <col>)` / `cume_dist() OVER (ORDER BY <col>)` where `<col>` is a non-numeric, non-temporal type. The window function still runs but yields lexicographic ranking, which is rarely what was meant.

<sub>`dsl-analysis/src/rules/percentile_non_numeric_order.rs`</sub>

### `sql344` — `ORDER BY <col> USING <op>` where the column's type family is one of the families that lacks a meaningful...

`ORDER BY <col> USING <op>` where the column's type family is one of the families that lacks a meaningful total order (json/jsonb/bytea/uuid). PG accepts the syntax but the comparison is lexicographic on the wire representation -- almost never the intent.

<sub>`dsl-analysis/src/rules/order_by_using_noncomparable.rs`</sub>

### `sql345` — `ALTER TABLE t RENAME COLUMN old TO new` while some `CREATE VIEW v AS SELECT ...` in the same buffer...

`ALTER TABLE t RENAME COLUMN old TO new` while some `CREATE VIEW v AS SELECT ...` in the same buffer references both table `t` and column name `old`. PG cascades the rename for views defined with an explicit column list, but inline `SELECT old` references silently become invalid and the view stops compiling on next pg_dump / DEFINITION refresh.

<sub>`dsl-analysis/src/rules/rename_column_breaks_view.rs`</sub>

### `sql346` — `CREATE INDEX

`CREATE INDEX ... USING BRIN` on a table the live catalog says has fewer than 10k rows. BRIN is built for large append-only tables (logs, time series). On a small table the index returns whole heap pages and the planner picks seq-scan anyway.

<sub>`dsl-analysis/src/rules/brin_small_table.rs`</sub>

### `sql347` — `ALTER TABLE t ENABLE|DISABLE TRIGGER ...`

`ALTER TABLE t ENABLE|DISABLE TRIGGER ...`. Takes an ACCESS EXCLUSIVE lock on the target table, which blocks every read AND every write until the catalog mutation commits. Hint about running during low traffic or wrapping in `lock_timeout`.

<sub>`dsl-analysis/src/rules/alter_trigger_lock.rs`</sub>

### `sql348` — function call whose name isn't in the live catalog, the built-in dsl-knowledge function table, or a...

function call whose name isn't in the live catalog, the built-in dsl-knowledge function table, or a buffer-local CREATE FUNCTION. Helps catch typos and missing schema-qualified imports. Conservative -- skips anything that looks like a keyword form, a CTE name reference, an explicit cast, or a method-style suffix call that isn't actually a function (e.g. `count(*)`).

<sub>`dsl-analysis/src/rules/unknown_function.rs`</sub>

### `sql349` — `INSERT INTO t (col_list)` lists a column not in the target table's catalog

`INSERT INTO t (col_list)` lists a column not in the target table's catalog. Catches typos in INSERT statements.

<sub>`dsl-analysis/src/rules/insert_unknown_column.rs`</sub>

### `sql350` — `INSERT/UPDATE/DELETE

`INSERT/UPDATE/DELETE ... RETURNING <list>` lists a column not on the target table. Mirrors sql349 + sql002 coverage gaps.

<sub>`dsl-analysis/src/rules/returning_unknown_column.rs`</sub>

### `sql351` — `DELETE/UPDATE FROM t WHERE bogus`

`DELETE/UPDATE FROM t WHERE bogus` -- WHERE column not found on the target table. Fills the sql002 gap (which is SELECT-only).

<sub>`dsl-analysis/src/rules/dml_where_unknown_column.rs`</sub>

### `sql402` — duplicate FROM/JOIN alias in a single SELECT. Example: `SELECT * FROM users a, orders a`

duplicate FROM/JOIN alias in a single SELECT. Example: `SELECT * FROM users a, orders a` -- PG rejects this with "table name 'a' specified more than once". We catch it earlier so the user fixes the alias before running.

<sub>`dsl-analysis/src/rules/duplicate_alias.rs`</sub>

### `sql403` — ORDER BY references a column that doesn't exist in any in-scope table or projection alias. PG models ORDER BY...

ORDER BY references a column that doesn't exist in any in-scope table or projection alias. PG models ORDER BY as part of the SELECT but our AST doesn't expose it -- so we text-scan inside the statement's source range. We only flag bare `<ident>` or `<qualifier>.<ident>` items (skipping expressions, function calls, positional `ORDER BY 1`, and items that resolve to a projection alias) to keep this honest.

<sub>`dsl-analysis/src/rules/order_by_unknown_column.rs`</sub>

### `sql404` — GROUP BY references a column that doesn't exist

GROUP BY references a column that doesn't exist. Mirrors `order_by_unknown_column` but bounded by HAVING / ORDER BY / LIMIT / OFFSET / FOR / FETCH / WINDOW. Projection aliases are accepted (PG allows them and we have a separate stylistic rule, sql337, for the portability concern). Items wrapped in ROLLUP/CUBE/GROUPING SETS or `(a, b)` grouping expressions fall through naturally because parse_simple_ident rejects anything with parens or commas inside an item.

<sub>`dsl-analysis/src/rules/group_by_unknown_column.rs`</sub>

### `sql405` — HAVING references a column that doesn't exist

HAVING references a column that doesn't exist. HAVING is an expression (not a comma-separated list like GROUP BY or ORDER BY), so the scanner walks every word-shaped token inside the clause and checks each as either a bare or qualified column ref. Tokens that are function names (followed by `(`), SQL keywords / boolean / null literals, type names commonly used in casts, or projection aliases are skipped.

<sub>`dsl-analysis/src/rules/having_unknown_column.rs`</sub>

### `sql406` — duplicate column in an INSERT column list or UPDATE SET assignment list. - `INSERT INTO t (a, b, a) VALUES...

duplicate column in an INSERT column list or UPDATE SET assignment list. - `INSERT INTO t (a, b, a) VALUES (...)` -- PG: column "a" specified more than once - `UPDATE t SET a = 1, a = 2` -- PG: multiple assignments to same column "a" Both forms are pure typos and the user almost certainly meant to reference two different columns, so we surface them as errors.

<sub>`dsl-analysis/src/rules/duplicate_dml_column.rs`</sub>

### `sql407` — `WHERE 1=2` / `WHERE FALSE` / `WHERE 1<>1`

`WHERE 1=2` / `WHERE FALSE` / `WHERE 1<>1` -- the entire predicate is a trivially-false literal comparison; the query returns zero rows regardless of input. Usually a leftover from copy-paste-and-edit ("kill the rows for now") or a debugging placeholder that escaped review. PG happily accepts and executes it; we surface a warning so it surfaces in review.

<sub>`dsl-analysis/src/rules/where_always_false.rs`</sub>

### `sql408` — `WHERE col = col` (or `<col> OP <col>` for the same column on both sides)

`WHERE col = col` (or `<col> OP <col>` for the same column on both sides). The predicate is either a tautology (`=`, `<=`, `>=`) or trivially-false (`<`, `>`, `<>`, `!=`), modulo NULL. Almost always a typo for `col = other_col` or `col = literal`.

<sub>`dsl-analysis/src/rules/where_column_self_compare.rs`</sub>

### `sql409` — `WHERE col BETWEEN col AND ...` or `WHERE col BETWEEN ... AND col`

`WHERE col BETWEEN col AND ...` or `WHERE col BETWEEN ... AND col` -- one of the bounds is the same column being tested, so the predicate collapses. `col BETWEEN col AND high` is equivalent to `col <= high`; `col BETWEEN low AND col` is equivalent to `col >= low`. Almost always a typo for two real bounds.

<sub>`dsl-analysis/src/rules/between_self_bound.rs`</sub>

### `sql410` — `SELECT id, id FROM ...`

`SELECT id, id FROM ...` -- a column appears twice in the SELECT list. PG accepts this (the output has two identically-named columns) but it's almost always a copy-paste typo. The duplicate also breaks code that builds dicts/structs keyed by column name.

<sub>`dsl-analysis/src/rules/duplicate_select_projection.rs`</sub>

### `sql411` — `LIMIT 1 OFFSET N` (with N > 0) without ORDER BY picks a deliberately non-first row, but without ORDER BY...

`LIMIT 1 OFFSET N` (with N > 0) without ORDER BY picks a deliberately non-first row, but without ORDER BY there's no defined notion of "the Nth row" -- the planner is free to return anything. Distinct from sql051 which exempts `LIMIT 1` (the common "any one matching row" idiom): the OFFSET makes the intent position-sensitive so the missing ORDER BY is the bug.

<sub>`dsl-analysis/src/rules/limit_one_offset_no_order.rs`</sub>

### `sql412` — `ORDER BY id, id` / `GROUP BY id, id`

`ORDER BY id, id` / `GROUP BY id, id` -- a column appears more than once in the clause. The repeat does nothing (ORDER BY is already deterministic on the first occurrence; GROUP BY repeats are redundant), and is almost always a typo for two different columns. Distinct directions (`ORDER BY id ASC, id DESC`) are still flagged because the second sort key is unreachable -- the first ordering already pins every row.

<sub>`dsl-analysis/src/rules/duplicate_order_or_group_item.rs`</sub>

### `sql413` — `expr || NULL` / `NULL || expr`

`expr || NULL` / `NULL || expr` -- the `||` operator returns NULL when either operand is NULL, so any literal NULL in a string-concatenation chain silently drops the whole expression to NULL. Use `concat()` (NULL-as-empty-string) or `coalesce(part, '')` when that's actually what you want.

<sub>`dsl-analysis/src/rules/concat_with_null_literal.rs`</sub>

### `sql414` — `WHERE col IN (col, ...)` or `... col NOT IN (col, ...)`

`WHERE col IN (col, ...)` or `... col NOT IN (col, ...)` -- the column appears in its own IN list. For non-NULL rows the membership is unconditionally true (or unconditionally false in the NOT IN form), so the predicate collapses. Almost always a typo for a different column or a literal.

<sub>`dsl-analysis/src/rules/in_list_self_member.rs`</sub>

### `sql415` — `col::T` or `CAST(col AS T)` where T is the column's catalog data type

`col::T` or `CAST(col AS T)` where T is the column's catalog data type -- the cast is a no-op and adds visual noise (and sometimes hides the wrong type from review). Drop the cast.

<sub>`dsl-analysis/src/rules/cast_same_type.rs`</sub>

### `sql416` — `CASE WHEN ... THEN x ... WHEN ... THEN x ELSE x END`

`CASE WHEN ... THEN x ... WHEN ... THEN x ELSE x END` -- every branch (including ELSE when present) returns the same value, so the whole CASE expression collapses to that value. Either the conditions are unintentional or the constant is.

<sub>`dsl-analysis/src/rules/case_all_branches_same.rs`</sub>

### `sql417` — `COALESCE(a, a, ...)` or `COALESCE(a, NULL, ...)`

`COALESCE(a, a, ...)` or `COALESCE(a, NULL, ...)` -- the duplicate / NULL argument is dead. COALESCE short-circuits on the first non-NULL arg; a later identical arg (when the first returned NULL the second will too, assuming determinism) or a NULL literal never contributes. Almost always a typo.

<sub>`dsl-analysis/src/rules/coalesce_dead_arg.rs`</sub>

### `sql418` — `SELECT DISTINCT pk_col FROM t`

`SELECT DISTINCT pk_col FROM t` -- DISTINCT is redundant when the projection contains the columns of a PRIMARY KEY or UNIQUE constraint (rows are already distinct on those columns). Drop the DISTINCT to avoid the implicit sort PG performs to deduplicate.

<sub>`dsl-analysis/src/rules/distinct_on_unique.rs`</sub>

### `sql419` — `NULLIF(x, NULL)` and `NULLIF(NULL, x)` are pointless

`NULLIF(x, NULL)` and `NULLIF(NULL, x)` are pointless -- `NULLIF(x, NULL)` collapses to just `x` (NULL compared to anything is NULL, so the equality never holds), and `NULLIF(NULL, x)` is always NULL. Likely a typo or unfinished thought.

<sub>`dsl-analysis/src/rules/nullif_with_null_literal.rs`</sub>

### `sql420` — `WHERE col = ANY(ARRAY[col, ...])`

`WHERE col = ANY(ARRAY[col, ...])` -- the column appears in its own ANY-array, which (like sql414's IN-list) makes the membership unconditionally true for non-NULL rows. Same for the `ALL` variant which becomes tautologically true only when every other entry equals the column too. Likely a typo.

<sub>`dsl-analysis/src/rules/any_array_self_member.rs`</sub>

### `sql421` — `WHERE age > 0 AND age > 0`

`WHERE age > 0 AND age > 0` -- duplicate conjunct. Splits the WHERE predicate on top-level AND/OR (paren-aware), normalizes each piece (strip outer parens, collapse whitespace, lowercase), and flags repeats. The dup is wasted parse/plan work and is almost always a copy-paste typo for two distinct predicates.

<sub>`dsl-analysis/src/rules/duplicate_where_predicate.rs`</sub>

### `sql422` — `WHERE X AND NOT X`

`WHERE X AND NOT X` -- the same predicate AND its negation is always false; the query returns zero rows. Almost certainly a typo or unfinished refactor.

<sub>`dsl-analysis/src/rules/where_pred_and_negation.rs`</sub>

### `sql423` — `col ~ '^prefix'` (or `~* '^prefix'`) where the regex is just an anchored literal prefix could be rewritten...

`col ~ '^prefix'` (or `~* '^prefix'`) where the regex is just an anchored literal prefix could be rewritten as `col LIKE 'prefix%'` (or `ILIKE`). The LIKE form is sargable when the column has a btree `text_pattern_ops` (or default `text_ops` for the C locale) index; the regex form usually isn't.

<sub>`dsl-analysis/src/rules/regex_prefix_could_be_like.rs`</sub>

### `sql424` — `WHERE count(*) > 1`

`WHERE count(*) > 1` -- aggregate function in WHERE. PG raises 42803 "aggregate functions are not allowed in WHERE"; the user almost certainly wanted HAVING (after a GROUP BY) or to move the aggregate into a subquery.

<sub>`dsl-analysis/src/rules/aggregate_in_where.rs`</sub>

### `sql425` — window function in WHERE / HAVING / JOIN ON

window function in WHERE / HAVING / JOIN ON. PG raises 42P20 ("window functions are not allowed in WHERE"). Move the window into a subquery and filter the result, or use HAVING for aggregates.

<sub>`dsl-analysis/src/rules/window_in_where.rs`</sub>

### `sql426` — `SELECT DISTINCT id FROM users ORDER BY age`

`SELECT DISTINCT id FROM users ORDER BY age` -- PG raises "for SELECT DISTINCT, ORDER BY expressions must appear in select list". The DISTINCT deduplicates the projection, so any sort key must be derivable from those columns. Add the column to the projection or drop the DISTINCT.

<sub>`dsl-analysis/src/rules/distinct_order_by_must_be_in_projection.rs`</sub>

### `sql427` — `WHERE date(ts) = '2024-01-01'` / `WHERE ts::date = ...` / `WHERE CAST(ts AS date) = ...`

`WHERE date(ts) = '2024-01-01'` / `WHERE ts::date = ...` / `WHERE CAST(ts AS date) = ...` -- wrapping a column in a function call or cast prevents the btree index on that column from being used. Use a range predicate or build a functional index instead.

<sub>`dsl-analysis/src/rules/wrap_blocks_index.rs`</sub>

### `sql428` — `MAX(*) / SUM(*) / AVG(*)` etc

`MAX(*) / SUM(*) / AVG(*)` etc. -- only `count(*)` and `count_if(*)`-style aggregates accept `*`. PG rejects e.g. `function max(*) does not exist`; we surface this at parse time.

<sub>`dsl-analysis/src/rules/aggregate_star_only_count.rs`</sub>

### `sql429` — `WHERE col == 1` (C-style) and `WHERE col <=> 1` (MySQL null-safe equal)

`WHERE col == 1` (C-style) and `WHERE col <=> 1` (MySQL null-safe equal) -- PG accepts neither. PG's `==` raises "operator does not exist", and `<=>` raises a similar error (the spaceship operator is MySQL-specific). Real fix: `=` for C-style typos, and `IS NOT DISTINCT FROM` for NULL-safe equality.

<sub>`dsl-analysis/src/rules/invalid_equality_operator.rs`</sub>

### `sql430` — `SELECT *, col FROM t`

`SELECT *, col FROM t` -- mixing `*` with an explicit column name duplicates that column in the output (PG returns every column AND `col`). Almost always a typo or a stray paste; either drop the `*` or drop the named column.

<sub>`dsl-analysis/src/rules/select_star_with_named_columns.rs`</sub>

### `sql431` — `SELECT

`SELECT ... FOR UPDATE` combined with `UNION` / `INTERSECT` / `EXCEPT`. PG raises 0A000 "FOR UPDATE is not allowed with UNION/INTERSECT/EXCEPT operation" both for the trailing-FOR-UPDATE shape and the per-arm shape `(SELECT ... FOR UPDATE) UNION (...)`. Hoist the row-locking query into a CTE / outer wrapper and apply FOR UPDATE there.

<sub>`dsl-analysis/src/rules/for_update_in_setop.rs`</sub>

### `sql432` — `CASE WHEN p THEN a WHEN p THEN b END`

`CASE WHEN p THEN a WHEN p THEN b END` -- two WHEN branches share the same condition. PG evaluates the first match only, so the later branch is unreachable dead code. Either drop the duplicate or fix the condition. Also covers searched `CASE x WHEN 1 THEN .. WHEN 1 THEN .. END` where the constant WHEN value is duplicated.

<sub>`dsl-analysis/src/rules/case_duplicate_when.rs`</sub>

### `sql433` — `ORDER BY NULL` / `ORDER BY TRUE` / `ORDER BY 'foo'`

`ORDER BY NULL` / `ORDER BY TRUE` / `ORDER BY 'foo'` -- sorting by a constant is a no-op in PG (every row gets the same sort key). Almost always a MySQL idiom: MySQL used `ORDER BY NULL` to suppress the implicit sort GROUP BY imposed. PG has no implicit sort, so the clause is dead. Either drop it or sort by a real column. Positional `ORDER BY 1` is a real column reference (1st projection) and is *not* flagged here -- that's sql099's territory.

<sub>`dsl-analysis/src/rules/order_by_constant.rs`</sub>

### `sql434` — `WHERE col IS NOT NULL AND col = 5`

`WHERE col IS NOT NULL AND col = 5` -- the `IS NOT NULL` check is redundant because `col = 5` (or any strict comparison / IN / LIKE / BETWEEN / regex) already requires `col` to be NOT NULL (PG returns NULL, not FALSE, for `NULL = anything`, and rows where the WHERE predicate evaluates to NULL are discarded). The redundant check costs no rows but adds noise and confuses readers about whether NULLs were ever expected.

<sub>`dsl-analysis/src/rules/redundant_is_not_null.rs`</sub>

### `sql435` — `WHERE col IS NULL AND col = 5` (or any strict op, or `col IS NOT NULL`)

`WHERE col IS NULL AND col = 5` (or any strict op, or `col IS NOT NULL`) -- the conjunction is a contradiction; the query returns zero rows. PG returns NULL (not FALSE) for `NULL = anything`, and rows where the WHERE predicate evaluates to NULL are discarded, so the IS NULL branch demands the column is NULL while the strict op demands it isn't. Almost always a typo (the user meant OR), an unfinished refactor, or a copy-paste from a different column.

<sub>`dsl-analysis/src/rules/where_is_null_contradiction.rs`</sub>

### `sql436` — `sum(row_number() OVER (...))`

`sum(row_number() OVER (...))` -- window function nested inside an aggregate. PG raises 42P20: "window function calls cannot be nested inside an aggregate function call". The fix is usually to wrap the windowed query in a subquery / CTE and aggregate over its output, not to combine the two in one expression.

<sub>`dsl-analysis/src/rules/window_in_aggregate.rs`</sub>

### `sql437` — `WHERE NULL IN (1, 2, 3)`

`WHERE NULL IN (1, 2, 3)` -- the LHS literal is NULL, so the IN expression evaluates to NULL (not TRUE/FALSE) regardless of the list contents. PG treats a NULL WHERE result as failure, so the row is dropped -- the whole query returns nothing. Almost certainly a typo (the user meant a column name on the LHS) or a leftover placeholder. Also covers `NULL NOT IN (...)` which similarly always evaluates to NULL.

<sub>`dsl-analysis/src/rules/null_in_list.rs`</sub>

### `sql438` — `id int GENERATED ALWAYS AS IDENTITY DEFAULT 0`

`id int GENERATED ALWAYS AS IDENTITY DEFAULT 0` -- a column defined as a SQL-standard identity column can NOT also have an explicit DEFAULT clause; the identity sequence IS the default. PG raises 42601 ("multiple default values specified for column"). The same goes for `GENERATED BY DEFAULT AS IDENTITY DEFAULT ...`. Almost always a leftover from a refactor away from a SERIAL/sequence pattern -- drop the DEFAULT.

<sub>`dsl-analysis/src/rules/generated_identity_with_default.rs`</sub>

### `sql439` — `DATE '2024-13-01'` / `TIMESTAMP '2024-02-30'`

`DATE '2024-13-01'` / `TIMESTAMP '2024-02-30'` -- typed date/time literals with an out-of-range month or day. PG raises 22008 "date/time field value out of range" at parse / execution time depending on the path. Catches obvious calendar mistakes (month > 12, day > 31, day > days-in-month). Leap year is respected (Feb 29 valid only in leap years).

<sub>`dsl-analysis/src/rules/invalid_date_literal.rs`</sub>

### `sql440` — `INTERVAL '2 mans'`

`INTERVAL '2 mans'` -- the unit word is not a recognized PG interval unit. PG raises 22007 "invalid input syntax for type interval" at execution. Almost always a typo (`mans` -> `months`, `weak` -> `weeks`, `yeers` -> `years`). The check only fires for the `<number> <word>` shape; ISO 8601 (`P1Y2M3D`), bare `HH:MM:SS`, and bare numbers are left alone.

<sub>`dsl-analysis/src/rules/invalid_interval_unit.rs`</sub>

### `sql441` — `WHERE EXISTS (SELECT 1 FROM other_table)`

`WHERE EXISTS (SELECT 1 FROM other_table)` -- the inner subquery does not reference any column from the OUTER statement, so the EXISTS is uncorrelated and degenerates to "does `other_table` have any rows" (a single boolean check repeated for every outer row). Almost always a typo (forgot the join predicate), an unfinished refactor, or a misuse of EXISTS where `LIMIT 1` would do. Heuristic: walk each top-level WHERE `EXISTS (...)` / `NOT EXISTS (...)`. If the subquery body contains no token matching `<outer_alias>.` (case-insensitive) for any outer binding's alias OR table name, flag.

<sub>`dsl-analysis/src/rules/uncorrelated_exists.rs`</sub>

### `sql442` — `regexp_replace(s, pattern, replacement)`

`regexp_replace(s, pattern, replacement)` -- PG's default behavior is to replace only the FIRST match, not all matches. The global-replace behavior (matching common-language intuition and most other regex libraries) requires an explicit 4th-arg flag string containing `g` (`regexp_replace(s, pattern, replacement, 'g')`). Same goes for `regexp_replace(s, pat, repl, 'i')` -- case-insensitive but still single-replace. Fires on calls with 3 args (no flag arg) or 4 args where the flag is a string literal NOT containing `g`. Skip when the flag arg is a non-literal (variable / column) since we can't determine its contents at edit time.

<sub>`dsl-analysis/src/rules/regexp_replace_no_global.rs`</sub>

### `sql443` — `substring(s, start, -3)`

`substring(s, start, -3)` -- a negative literal length argument. PG raises 22011 "negative substring length not allowed" at runtime. Almost always a typo (the user inverted the sign or confused start/length). When the length arg is a non-literal column / variable, we can't tell, so we stay silent.

<sub>`dsl-analysis/src/rules/substring_negative_length.rs`</sub>

### `sql444` — `generate_series(1, 10, 0)`

`generate_series(1, 10, 0)` -- a literal zero step is a runtime error in PG (22023 "step size cannot equal zero"). Also covers a step whose sign points the wrong way for the start/end range (e.g. `generate_series(10, 1, 1)` produces an empty set because the step moves the cursor further from the end). Fires only when args are integer literals.

<sub>`dsl-analysis/src/rules/generate_series_bad_step.rs`</sub>

### `sql445` — `array_position(arr, NULL)`

`array_position(arr, NULL)` -- always returns NULL because PG's equality is NULL-rejecting (`NULL = anything` is NULL, not TRUE), and array_position uses equality to find the needle. To find NULL inside an array, the user wants `(SELECT i FROM generate_subscripts(arr, 1) WHERE arr[i] IS NULL)` or `arr @> ARRAY[NULL]::<elem-type>[]` style checks. Same goes for `array_positions(arr, NULL)`.

<sub>`dsl-analysis/src/rules/array_position_null.rs`</sub>

### `sql446` — `position('' in s)` / `strpos(s, '')`

`position('' in s)` / `strpos(s, '')` -- searching for the empty string. PG returns 1 for every non-NULL `s` (the empty string is found at position 1). The expression is a constant 1 and almost certainly a leftover placeholder where the user meant to fill in the actual substring.

<sub>`dsl-analysis/src/rules/position_empty_substring.rs`</sub>

### `sql447` — `power(x, 0)` always returns 1 and `power(x, 1)` always returns x

`power(x, 0)` always returns 1 and `power(x, 1)` always returns x. Both are tautologies that almost always indicate a typo, a leftover placeholder, or a misunderstanding of which arg is the base vs the exponent (PG signature is `power(base, exponent)`).

<sub>`dsl-analysis/src/rules/power_trivial_exponent.rs`</sub>

### `sql448` — `lpad('hi', -3, '0')`

`lpad('hi', -3, '0')` -- negative literal length. PG returns an empty string for negative `length` (truncates from the right by `-length` chars; with a negative greater than the input length the result is empty). Almost always a sign-flip typo. Same for `rpad`.

<sub>`dsl-analysis/src/rules/lpad_rpad_negative.rs`</sub>

### `sql449` — `jsonb_build_object('k', 1, 'k', 2)`

`jsonb_build_object('k', 1, 'k', 2)` -- duplicate key. PG silently overwrites the earlier value with the later one in the resulting JSON object, so the first pair is dead. Almost always a copy-paste typo. Same for `json_build_object`.

<sub>`dsl-analysis/src/rules/jsonb_build_object_duplicate_key.rs`</sub>

### `sql450` — `NUMERIC(p, s)` (or `DECIMAL(p, s)`) with `s > p`

`NUMERIC(p, s)` (or `DECIMAL(p, s)`) with `s > p`. PG raises 22023 "NUMERIC scale N must be between 0 and precision P" at parse time. The numeric type's invariant is that the scale ( digits after the decimal point) cannot exceed the precision ( total digits), so the column / cast can never store a value. Likely a swapped-arg typo (the user meant `NUMERIC(s, p)`). Also flags `NUMERIC(0, ...)` since precision must be positive.

<sub>`dsl-analysis/src/rules/numeric_scale_exceeds_precision.rs`</sub>

### `sql451` — `VARCHAR(0)` / `CHAR(0)` / `CHARACTER(0)` / `CHARACTER VARYING(0)`

`VARCHAR(0)` / `CHAR(0)` / `CHARACTER(0)` / `CHARACTER VARYING(0)` -- a zero-length string type. PG accepts the declaration, but the column can only ever store the empty string (any non-empty input raises 22001 "value too long"). Almost always a typo (the user meant `VARCHAR(10)` etc.) or a placeholder left in a refactor.

<sub>`dsl-analysis/src/rules/varchar_char_zero_length.rs`</sub>

### `sql452` — `repeat(s, 0)` or `repeat(s, -3)`

`repeat(s, 0)` or `repeat(s, -3)` -- a literal zero or negative count always produces the empty string. The call is a constant `''` that almost always indicates a typo (the user meant 1 / a real count) or a leftover placeholder.

<sub>`dsl-analysis/src/rules/repeat_trivial_count.rs`</sub>

### `sql453` — `array_length(arr)`

`array_length(arr)` -- missing dimension argument. PG's signature is `array_length(anyarray, integer)`; the single-arg form does not exist and the query raises 42883 "function array_length(<type>[]) does not exist" at parse / execution. Pass `1` as the dimension for the common 1D case, or use `cardinality(arr)` (which takes no dim arg and returns 0 for empty arrays).

<sub>`dsl-analysis/src/rules/array_length_missing_dim.rs`</sub>

### `sql454` — `to_timestamp(s, 'HH:MM')`

`to_timestamp(s, 'HH:MM')` -- in PG's datetime template language, `MM` means MONTH (not minute). `MI` is the minute token. Users coming from strftime / Java SimpleDateFormat / Python / Ruby routinely write `HH:MM` thinking it means HH:minute, but PG silently parses (and TO_CHAR formats) it as HH:MONTH. The result is wrong values without any runtime error. Same gotcha applies to `MM:SS` (where the user meant `MI:SS`). Covers to_timestamp, to_char, and to_date format-string literals.

<sub>`dsl-analysis/src/rules/to_timestamp_hh_mm_confusion.rs`</sub>

### `sql455` — `WHERE X OR NOT X`

`WHERE X OR NOT X` -- a predicate ORed with its own negation is a tautology (always TRUE for non-NULL X; NULL when X is NULL, which WHERE then drops). Either the user meant a different second branch, or the whole `OR` clause should be removed. Mirror of sql422 (the AND-version that yields always- FALSE).

<sub>`dsl-analysis/src/rules/where_pred_or_negation.rs`</sub>

### `sql456` — `WHERE smallint_col = 100000`

`WHERE smallint_col = 100000` -- the literal exceeds the column type's range. PG raises 22003 "smallint out of range" at execution; the comparison can never match because the value literally doesn't fit. Almost always a copy-paste mistake from a wider-type context. Implementation note: our Expr AST exposes the WHERE clause as a flat `Expr::List` of column references without operator/literal pairs, so this rule does a text scan of the WHERE body looking for `<col> <op> <intlit>` triples.

<sub>`dsl-analysis/src/rules/int_literal_out_of_range.rs`</sub>

### `sql457` — `SELECT a, b FROM t GROUP BY 3`

`SELECT a, b FROM t GROUP BY 3` -- the positional reference points past the projection list. PG raises 42703 "GROUP BY position N is not in select list" at parse. Same for `ORDER BY 5` when only 2 projections exist, and for `GROUP BY 0` (positions are 1-based).

<sub>`dsl-analysis/src/rules/positional_out_of_range.rs`</sub>

### `sql458` — `SUM(bool_col)` / `AVG(bool_col)`

`SUM(bool_col)` / `AVG(bool_col)` -- PG raises 42883 "function sum(boolean) does not exist". Users almost always want either `count(*) FILTER (WHERE bool_col)` (PG-idiomatic count of trues) or `sum(bool_col::int)` (boolean->int cast). Implementation: our parser collapses projection expressions to a flat `Expr::List` of column refs, so a structural AST walk can't see the surrounding sum/avg call. Use a text scan.

<sub>`dsl-analysis/src/rules/sum_avg_of_boolean.rs`</sub>

### `sql459` — `COUNT(col)` where `col` is declared NOT NULL

`COUNT(col)` where `col` is declared NOT NULL -- the expression is identical to `COUNT(*)`. Both yield the same row count, but `COUNT(*)` is the conventional spelling and lets the planner skip column-extraction work. (sql174 handles the inverse "nullable column" case where COUNT(col) silently skips NULL rows -- that's a semantic bug; this one is only a clarity issue.) Does NOT fire on COUNT(DISTINCT col) -- semantic difference.

<sub>`dsl-analysis/src/rules/count_notnull_column.rs`</sub>

### `sql460` — `SELECT id FROM t HAVING id > 5`

`SELECT id FROM t HAVING id > 5` -- HAVING without GROUP BY and without any aggregate function in the predicate. PG silently runs the predicate against the whole-table single group, which yields the same set of rows as a plain WHERE but after aggregation has (notionally) happened. Almost always a typo of "WHERE", or a leftover from removing a GROUP BY without relocating the predicate.

<sub>`dsl-analysis/src/rules/having_without_aggregate.rs`</sub>

### `sql461` — `array_remove(NULL, 1)` / `array_position(NULL, 1)` / `cardinality(NULL)`

`array_remove(NULL, 1)` / `array_position(NULL, 1)` / `cardinality(NULL)` -- the array argument is a NULL literal. PG's array functions are STRICT for the array operand, so the call always returns NULL. Users routinely expect "treat NULL like an empty array" but PG does not. Use `COALESCE(arr, '{}')` to make the empty-array intent explicit. Notable exception: `array_append(NULL, x)` and `array_prepend(x, NULL)` are NOT covered -- PG treats them as constructing a new 1-element array, which is the user's likely intent.

<sub>`dsl-analysis/src/rules/array_func_null_array.rs`</sub>

### `sql462` — `x + NULL` (or `-`, `*`, `/`, `%`)

`x + NULL` (or `-`, `*`, `/`, `%`) -- arithmetic with a literal NULL operand always returns NULL. Almost always a typo or a leftover placeholder (the user dropped a real value). In WHERE / ON the row will silently disappear; in projection the row will quietly show NULL.

<sub>`dsl-analysis/src/rules/null_arithmetic.rs`</sub>

### `sql463` — `IF TG_OP = 'inserted' THEN ...`

`IF TG_OP = 'inserted' THEN ...` -- PG's TG_OP returns one of exactly four uppercase strings: `INSERT`, `UPDATE`, `DELETE`, `TRUNCATE`. Any other literal makes the comparison always FALSE, so the branch is silently dead. Most common typo is lowercase (`'insert'`) or past-tense (`'inserted'`). Also handles `TG_OP IN ('INSERT', 'updated', ...)`.

<sub>`dsl-analysis/src/rules/tg_op_invalid_literal.rs`</sub>

### `sql464` — `x IS DISTINCT FROM x`

`x IS DISTINCT FROM x` -- always FALSE (the NULL-safe equality says x is NOT distinct from itself even when NULL). Likewise `x IS NOT DISTINCT FROM x` is always TRUE. Almost always a copy-paste typo for two different operands.

<sub>`dsl-analysis/src/rules/is_distinct_self.rs`</sub>

### `sql465` — `concat_ws('', a, b, c)`

`concat_ws('', a, b, c)` -- empty separator is the same as calling `concat(a, b, c)`. Both functions skip NULL arguments, so the only role of `concat_ws`'s first arg is the separator; when it's empty there's no separator and the call is identical to plain `concat`. Use `concat` for clarity.

<sub>`dsl-analysis/src/rules/concat_ws_empty_sep.rs`</sub>

### `sql466` — `... OFFSET 0`

`... OFFSET 0` -- skipping zero rows is a no-op. Almost always a leftover from a parameterized template (`OFFSET $offset` where offset=0) or a placeholder. Drop the clause for clarity. Note: PG occasionally uses `OFFSET 0` as an optimization fence to prevent subquery unnesting (a deliberate trick); we still emit a Hint since the common case is unintentional. Also covers SQL-standard `OFFSET 0 ROWS` / `OFFSET 0 ROW`.

<sub>`dsl-analysis/src/rules/offset_zero.rs`</sub>

### `sql467` — `replace(s, '', x)` / `split_part(s, '', n)`

`replace(s, '', x)` / `split_part(s, '', n)` -- the needle / delimiter argument is the empty string. PG semantics: - `replace(s, '', x)` returns `s` unchanged (the empty needle is never "found"). - `split_part(s, '', n)` returns `s` for n=1, '' otherwise. Both are effectively no-ops and almost always a leftover placeholder.

<sub>`dsl-analysis/src/rules/empty_needle_string_fn.rs`</sub>

### `sql468` — `GREATEST(NULL, NULL)` / `LEAST(NULL, NULL, NULL)`

`GREATEST(NULL, NULL)` / `LEAST(NULL, NULL, NULL)` -- all arguments are literal NULL. PG returns NULL (when every arg is NULL, both functions return NULL). The call is a constant NULL and almost always a leftover placeholder. (PG legitimately skips NULLs when at least one non-NULL value is present, so we only flag the all-NULL case.)

<sub>`dsl-analysis/src/rules/greatest_least_all_null.rs`</sub>

### `sql469` — `NOT (col IS NULL)` and `NOT col IS NULL`

`NOT (col IS NULL)` and `NOT col IS NULL` -- less idiomatic than `col IS NOT NULL`. Both forms are semantically equivalent in PG but the negated form is harder to scan and a common pattern after a refactor that dropped the inner predicate. Same with the inverse `NOT (col IS NOT NULL)` -> `col IS NULL`.

<sub>`dsl-analysis/src/rules/not_is_null.rs`</sub>

### `sql470` — `NOT (col IN (...))` / `NOT (col LIKE ...)` / `NOT (col BETWEEN ...)`

`NOT (col IN (...))` / `NOT (col LIKE ...)` / `NOT (col BETWEEN ...)` -- less idiomatic than `col NOT IN (...)` / `col NOT LIKE ...` / `col NOT BETWEEN ...`. PG accepts all forms but the explicit NOT-prefix is conventional and easier to scan. Pairs with sql469 which handles the NOT (IS NULL) variant.

<sub>`dsl-analysis/src/rules/not_paren_predicate.rs`</sub>

### `sql471` — `WHERE x IN (SELECT DISTINCT y FROM t)`

`WHERE x IN (SELECT DISTINCT y FROM t)` -- the DISTINCT inside an IN-subquery is wasted work. The IN operator already treats the subquery's result as a set: row equality against any occurrence of `y` succeeds regardless of how many times `y` appears. Drop the DISTINCT to let the planner pick the better plan (often a hash semi-join). Same for `NOT IN (SELECT DISTINCT ...)`.

<sub>`dsl-analysis/src/rules/distinct_inside_in_subquery.rs`</sub>

### `sql472` — `EXTRACT(dow FROM '1 day'::interval)`

`EXTRACT(dow FROM '1 day'::interval)` -- `dow`, `doy`, `week`, `isodow`, `isoyear`, `julian`, `timezone*` are not valid fields for an INTERVAL operand. PG raises 22023 "unit X not supported for type interval" at execution. Only fires when the EXTRACT's FROM expression is recognizably an interval literal (`INTERVAL '...'` keyword form or `'...'::interval` cast form). Other types (date / timestamp / time) take a wider valid-field set and are not checked here.

<sub>`dsl-analysis/src/rules/extract_invalid_interval_field.rs`</sub>

### `sql473` — `col = ANY(ARRAY[]::int[])`

`col = ANY(ARRAY[]::int[])` -- empty array on the RHS of an ANY-comparison. PG returns FALSE (no row matches); the predicate filters out everything. The `ALL` variant returns TRUE (vacuously true) which is also almost always a bug. Also covers the bare empty-array literal `'{}'::<type>[]`.

<sub>`dsl-analysis/src/rules/any_all_empty_array.rs`</sub>

### `sql474` — `WHERE 'a' = 'a'` (tautology), `WHERE 2 = 2` (tautology), `WHERE 'a' = 'b'` (contradiction)

`WHERE 'a' = 'a'` (tautology), `WHERE 2 = 2` (tautology), `WHERE 'a' = 'b'` (contradiction) -- a constant on both sides of an equality is independent of any row's data. Tautologies are noise; contradictions silently return zero rows. Pairs with sql282 which handles the narrow `WHERE 1=1` placeholder case and sql407 which handles `1 = 0` numeric contradictions.

<sub>`dsl-analysis/src/rules/where_literal_literal.rs`</sub>

### `sql475` — `INSERT INTO t SELECT ... FROM t`

`INSERT INTO t SELECT ... FROM t` -- the SELECT reads from the same table being inserted into. Each execution doubles the row count (or grows it unboundedly when the new rows feed the next iteration via triggers). Almost always a typo for a different source table, or it should be guarded by an `ON CONFLICT DO NOTHING` / `WHERE NOT EXISTS (...)` predicate to keep idempotent.

<sub>`dsl-analysis/src/rules/insert_self_select.rs`</sub>

### `sql476` — `CASE col WHEN NULL THEN ...`

`CASE col WHEN NULL THEN ...` -- in PG's simple-CASE form the WHEN value is compared with `=`, and `col = NULL` evaluates to NULL (not TRUE), so the branch never matches. The user almost certainly meant the searched form `CASE WHEN col IS NULL THEN ...` instead. (Searched CASE handles NULLs explicitly via IS / IS NOT operators.)

<sub>`dsl-analysis/src/rules/case_when_null.rs`</sub>

### `sql477` — `col @> '{}'::jsonb` / `col @> '[]'::jsonb` / `col @> ARRAY[]::int[]`

`col @> '{}'::jsonb` / `col @> '[]'::jsonb` / `col @> ARRAY[]::int[]` -- containment against an empty container is vacuously TRUE for every non-NULL value (the empty container is a subset of everything). The predicate has no filter effect and is almost always a leftover placeholder.

<sub>`dsl-analysis/src/rules/contains_empty_container.rs`</sub>

### `sql478` — `col <@ '{}'::jsonb` / `col <@ '[]'::jsonb` / `col <@ ARRAY[]::int[]`

`col <@ '{}'::jsonb` / `col <@ '[]'::jsonb` / `col <@ ARRAY[]::int[]` -- "col is contained-by an empty container" is the inverse of sql477's containment case: the predicate is TRUE only when `col` itself is empty (or NULL is filtered by the comparison). It almost never expresses what the author meant -- the intent was probably `col = '{}'::jsonb`, `col IS NULL`, or to remove the placeholder filter entirely.

<sub>`dsl-analysis/src/rules/contained_by_empty.rs`</sub>

### `sql479` — `substring(s, 0, n)` / `substring(s FROM 0 FOR n)` / `substr(s, 0, n)`

`substring(s, 0, n)` / `substring(s FROM 0 FOR n)` / `substr(s, 0, n)` -- PostgreSQL's `substring` is 1-indexed, but a 0 (or negative) FROM argument silently truncates the FOR count by `1 - start`. So `substring('abc', 0, 2)` returns `'a'`, not `'ab'` -- a classic off-by-one. Almost always the author meant `1` as the start.

<sub>`dsl-analysis/src/rules/substring_zero_start.rs`</sub>

### `sql480` — `GROUP BY NULL` / `GROUP BY TRUE` / `GROUP BY 'foo'`

`GROUP BY NULL` / `GROUP BY TRUE` / `GROUP BY 'foo'` -- grouping by a constant collapses every row into a single bucket, which is semantically equivalent to having no GROUP BY at all (when the projection is purely aggregates). Almost always a leftover or a mistaken attempt to group by a column whose name was typed as a string literal. Counterpart to sql433 for the GROUP BY clause. Positional `GROUP BY 1` is a real column reference (1st projection) and is *not* flagged here -- that's sql065's territory.

<sub>`dsl-analysis/src/rules/group_by_constant.rs`</sub>

### `sql481` — `position(<needle> in '')` / `strpos('', <needle>)`

`position(<needle> in '')` / `strpos('', <needle>)` -- searching in an empty haystack. PG returns 0 for every non-NULL `needle` (nothing matches in an empty string). The expression is a constant 0 and almost certainly a placeholder where the real string should go. Counterpart to sql446 (which catches the empty-needle case, always returning 1).

<sub>`dsl-analysis/src/rules/position_empty_haystack.rs`</sub>

### `sql482` — `HAVING <constant>`

`HAVING <constant>` -- a constant HAVING is either pointless (`HAVING TRUE`) or empties the result (`HAVING FALSE` / `HAVING NULL`). Counterpart to the WHERE always-true/false family for the HAVING clause.

<sub>`dsl-analysis/src/rules/having_constant.rs`</sub>

### `sql483` — `split_part(<s>, <delim>, 0)`

`split_part(<s>, <delim>, 0)` -- PG raises a runtime error: `field position must not be zero` (since the n argument is 1-indexed, with negative values counting from the end in PG 14+). The literal `0` will always blow up at execution time.

<sub>`dsl-analysis/src/rules/split_part_zero_field.rs`</sub>

### `sql484` — `OVER (PARTITION BY <constant> ...)`

`OVER (PARTITION BY <constant> ...)` -- partitioning by a constant expression collapses every row into a single window, which is equivalent to having no PARTITION BY at all. Counterpart to sql480 (GROUP BY constant) and sql433 (ORDER BY constant) for the window-function PARTITION BY clause.

<sub>`dsl-analysis/src/rules/partition_by_constant.rs`</sub>

### `sql485` — `regexp_split_to_array(s, '')`, `regexp_split_to_table(s, '')`, `regexp_match(s, '')`, `regexp_matches(s, '')`

`regexp_split_to_array(s, '')`, `regexp_split_to_table(s, '')`, `regexp_match(s, '')`, `regexp_matches(s, '')` -- an empty regex pattern matches at every position, so: * split functions return an array/set of single characters * match functions return `{""}` (an array containing one empty string) Almost always a placeholder bug; the user meant a real pattern.

<sub>`dsl-analysis/src/rules/regexp_empty_pattern.rs`</sub>

### `sql486` — `SELECT DISTINCT *` / `SELECT DISTINCT t.*`

`SELECT DISTINCT *` / `SELECT DISTINCT t.*` -- DISTINCT on a whole-row projection is almost always a workaround for a join that produced duplicates, not the intended filter. It forces a full-row sort/hash and silently hides the underlying join bug. Prefer fixing the join (EXISTS subquery, narrower SELECT list, or aggregation) instead of deduplicating the result.

<sub>`dsl-analysis/src/rules/distinct_star.rs`</sub>

### `sql487` — `array_length(arr, 0)`, `array_lower(arr, 0)`, `array_upper(arr, 0)`, or any negative dimension

`array_length(arr, 0)`, `array_lower(arr, 0)`, `array_upper(arr, 0)`, or any negative dimension -- PG array dimensions are 1-based; an out-of-range dimension makes the function silently return NULL. Almost always a typo for `1`.

<sub>`dsl-analysis/src/rules/array_dim_zero.rs`</sub>

### `sql488` — `jsonb_path_exists/query/query_array/query_first/match(col, '<path>')`

`jsonb_path_exists/query/query_array/query_first/match(col, '<path>')` -- when the path is a string literal, it MUST start with `$` (optionally prefixed by `strict ` or `lax `). PG raises a runtime parse error otherwise (e.g. `ERROR: syntax error at end of jsonpath input`).

<sub>`dsl-analysis/src/rules/jsonpath_missing_anchor.rs`</sub>

### `sql489` — `WHERE col + 0 = N`, `col - 0 = N`, `col * 1 = N`, `col / 1 = N` (and the commutative `0 + col`, `1 * col`)

`WHERE col + 0 = N`, `col - 0 = N`, `col * 1 = N`, `col / 1 = N` (and the commutative `0 + col`, `1 * col`) -- wrapping a column in an arithmetic identity defeats a btree index on that column. The expression is equal to `col` itself; remove the no-op operand.

<sub>`dsl-analysis/src/rules/where_arith_identity.rs`</sub>

### `sql490` — `col || ''` / `'' || col`

`col || ''` / `'' || col` -- concatenating with the empty string is a no-op (the expression equals `col`). Almost always either a placeholder where a real literal should go or a leftover from refactoring. Drop the empty operand. Note: sql413 catches `col || NULL` (returns NULL). sql490 is the empty-string-literal counterpart, which has different semantics (no-op, not NULL).

<sub>`dsl-analysis/src/rules/concat_empty_string.rs`</sub>

### `sql491` — `HAVING 1 = 1` (tautology) / `HAVING 1 = 2` (contradiction) / `HAVING 'a' = 'b'`

`HAVING 1 = 1` (tautology) / `HAVING 1 = 2` (contradiction) / `HAVING 'a' = 'b'` -- equality (or `<>`/`!=`) between two constant literals in HAVING is independent of row data. Counterpart to sql474 for the HAVING clause.

<sub>`dsl-analysis/src/rules/having_literal_literal.rs`</sub>

### `sql492` — `col NOT IN (..., NULL, ...)`

`col NOT IN (..., NULL, ...)` -- a NULL anywhere in the list of a NOT IN predicate makes the entire predicate evaluate to NULL for every row (because the desugared form is `col <> v1 AND col <> v2 AND col <> NULL`, and the last conjunct is NULL, so the whole AND is NULL). NULL in WHERE is filtered out, so the query returns ZERO rows -- regardless of `col`'s actual values. Classic gotcha. Also flags `col IN (NULL)` as a sole element (always NULL -> 0 rows).

<sub>`dsl-analysis/src/rules/not_in_null_list.rs`</sub>

### `sql493` — `COALESCE(<not-null-col>, ...)`

`COALESCE(<not-null-col>, ...)` -- when the first argument is a NOT NULL column, COALESCE always returns it; the remaining defaults are dead code. Drop the wrapper or move the NOT NULL guarantee somewhere visible.

<sub>`dsl-analysis/src/rules/coalesce_not_null.rs`</sub>

### `sql494` — `jsonb_set(target, '{}', value)` / `jsonb_set_lax` / `jsonb_insert` with an empty path array

`jsonb_set(target, '{}', value)` / `jsonb_set_lax` / `jsonb_insert` with an empty path array -- PG walks the path into the target and replaces/inserts at the leaf. An empty path has no leaf to update, so the call returns the original target unchanged. Almost always a placeholder where the real path should go.

<sub>`dsl-analysis/src/rules/jsonb_set_empty_path.rs`</sub>

### `sql495` — `WHERE col = ALL(<array-literal>)`

`WHERE col = ALL(<array-literal>)` -- `= ALL` requires col to equal *every* element. With 2+ literal elements that aren't all identical, the predicate is always FALSE. With all identical elements, it's equivalent to a single `col = <elem>`. Almost always the author meant `= ANY` (i.e. IN).

<sub>`dsl-analysis/src/rules/eq_all_array.rs`</sub>

### `sql496` — `UPDATE t SET col = DEFAULT` where `col` has no DEFAULT definition

`UPDATE t SET col = DEFAULT` where `col` has no DEFAULT definition. PG resets `col` to its default expression: * column is NOT NULL with no default -> runtime error ("null value in column violates not-null constraint") * column is nullable with no default -> silently becomes NULL, usually not what the author intended. pg_query exposes the `DEFAULT` keyword as `Expr::Other("")`, so we text-scan the SET clause for `<col> = DEFAULT` patterns.

<sub>`dsl-analysis/src/rules/set_default_no_default.rs`</sub>

### `sql497` — `array_agg(DISTINCT a ORDER BY b)` and similar

`array_agg(DISTINCT a ORDER BY b)` and similar -- PG requires that, when DISTINCT is used inside an aggregate, every ORDER BY expression must also appear in the aggregate's argument list. Mismatch raises a runtime error: `in an aggregate with DISTINCT, ORDER BY expressions must appear in argument list` Covers array_agg / string_agg / json_agg / jsonb_agg / json_object_agg / jsonb_object_agg / xmlagg.

<sub>`dsl-analysis/src/rules/agg_distinct_order_mismatch.rs`</sub>

### `sql498` — `WHERE col SIMILAR TO 'pattern'`

`WHERE col SIMILAR TO 'pattern'` -- the PG-specific SIMILAR TO operator is a third-rail SQL-standard regex variant that's neither LIKE nor POSIX regex. It's slower than POSIX regex (the `~` operator) in many cases and rarely understood outside PG. Prefer: * `LIKE '...'` for simple wildcard (`%` / `_`) patterns * `~ '...'` (POSIX regex) for full regular expressions

<sub>`dsl-analysis/src/rules/similar_to_deprecated.rs`</sub>

### `sql499` — `WHERE tsvector_col @@ 'plain text'`

`WHERE tsvector_col @@ 'plain text'` -- PG coerces the string literal to `tsquery`, which has its own syntax (operators `& | ! :`). A literal like `'foo bar'` (with a space) raises a runtime syntax error; a single-word literal like `'foo'` works but is rarely the author's intent. Wrap user-input-style text with `plainto_tsquery(...)` (whitespace -> AND) or `websearch_to_tsquery(...)` (google-style); use `to_tsquery(...)` only when the literal really is tsquery syntax.

<sub>`dsl-analysis/src/rules/tsvector_text_literal.rs`</sub>

### `sql500` — `date_col1 - date_col2`

`date_col1 - date_col2` -- PG returns `integer` (days), not `interval`. Confusing for authors expecting an interval; the result also doesn't compose with interval-arithmetic. If you want an interval, use `age(d1, d2)`; if you want days, alias the result so the units are explicit (e.g. `AS days`). Note: `timestamp - timestamp` returns an interval (no warning); `date - interval` returns a timestamp (no warning).

<sub>`dsl-analysis/src/rules/date_minus_date.rs`</sub>

### `sql501` — `ORDER BY not_null_col NULLS FIRST|LAST`

`ORDER BY not_null_col NULLS FIRST|LAST` -- the NULLS clause is redundant because the column can never be NULL. Drop the `NULLS ...` to make the intent (and the query plan) cleaner.

<sub>`dsl-analysis/src/rules/order_nulls_on_not_null.rs`</sub>

### `sql502` — `WHERE timestamptz_col <op> TIMESTAMP 'lit'`

`WHERE timestamptz_col <op> TIMESTAMP 'lit'` -- comparing a `timestamptz` column to a plain `TIMESTAMP` literal makes PG coerce the literal to timestamptz using the *session* timezone. The same query then returns different rows depending on session TZ, which is almost never intended. Prefer the explicit form: `TIMESTAMPTZ '<lit>'` (with offset) or `'lit'::timestamptz` so the timezone is unambiguous.

<sub>`dsl-analysis/src/rules/timestamp_lit_on_tstz_col.rs`</sub>

### `sql503` — `WHERE non_jsonb_col ? 'key'` / `?|` / `?&`

`WHERE non_jsonb_col ? 'key'` / `?|` / `?&` -- the key-exists family of operators (`?`, `?|`, `?&`) is only defined for `jsonb`, not for `json` or `text`. Using them on the wrong type raises a runtime error: `operator does not exist: <type> ? text`.

<sub>`dsl-analysis/src/rules/jsonb_question_on_non_jsonb.rs`</sub>

### `sql504` — `<int_col> / <int_literal>`

`<int_col> / <int_literal>` -- integer/integer division in PG truncates toward zero (e.g. `5 / 2` is `2`, not `2.5`). If the author meant float division, cast one side: `col::float / 2` or `col / 2.0`. Catches the common case where the LHS is a known integer column and the RHS is a bare integer literal.

<sub>`dsl-analysis/src/rules/integer_division_truncation.rs`</sub>

### `sql505` — `<text_col> -> 'key'` / `->>` / `#>` / `#>>`

`<text_col> -> 'key'` / `->>` / `#>` / `#>>` -- the JSON extraction operators are defined only for `json` and `jsonb`. On a `text` (or other non-JSON) column PG raises a runtime error: `operator does not exist: text -> unknown`. Add a `::jsonb` cast if the column actually holds JSON-shaped text, or use the correct column.

<sub>`dsl-analysis/src/rules/json_extract_on_text.rs`</sub>

### `sql506` — `ARRAY[NULL]` / `ARRAY[NULL, NULL, ...]`

`ARRAY[NULL]` / `ARRAY[NULL, NULL, ...]` -- when every element of the array constructor is the bare NULL keyword, PG cannot determine the element type and may fall back to `text[]` or raise `cannot determine type of empty array`. The result type depends on context (or session config) and is rarely what the author intended. Cast either an element (`NULL::int`) or the whole array (`ARRAY[NULL]::int[]`) to fix the type.

<sub>`dsl-analysis/src/rules/array_all_null.rs`</sub>

### `sql507` — `EXECUTE '<sql>' || <var>`

`EXECUTE '<sql>' || <var>` -- building dynamic SQL by string-concatenating a parameter is a SQL-injection vector. Use `EXECUTE ... USING <var>` for value parameters, or `format('%I' / '%L', ...)` for identifier / literal interpolation that survives malicious input.

<sub>`dsl-analysis/src/rules/execute_string_concat.rs`</sub>

### `sql508` — `WHERE col LIKE col` / `ILIKE` / `NOT LIKE` / `NOT ILIKE` and the POSIX-regex equivalents `~ / ~* / !~ / !~*`

`WHERE col LIKE col` / `ILIKE` / `NOT LIKE` / `NOT ILIKE` and the POSIX-regex equivalents `~ / ~* / !~ / !~*` -- a column compared against itself is almost always a copy-paste typo for two distinct columns. The expression is also semantically degenerate: `col LIKE col` is TRUE for every non-NULL row regardless of pattern (and NULL for NULL rows), so the predicate has no filter effect. `NOT LIKE` is the always-FALSE inverse.

<sub>`dsl-analysis/src/rules/self_like.rs`</sub>

### `sql509` — explicit `pg_temp.<table>` (or `pg_temp_<N>.<table>`) reference. Temporary tables live in a per-backend...

explicit `pg_temp.<table>` (or `pg_temp_<N>.<table>`) reference. Temporary tables live in a per-backend internal schema whose name (`pg_temp_<backend_id>`) is backend-specific and gets aliased as `pg_temp` in the search_path. Just write the table name unqualified -- PG resolves it via search_path automatically. Explicit qualification leaks an implementation detail into the SQL and can break across sessions or restarts.

<sub>`dsl-analysis/src/rules/pg_temp_explicit.rs`</sub>

### `sql510` — `WHERE col SIMILAR TO col` / `NOT SIMILAR TO col`

`WHERE col SIMILAR TO col` / `NOT SIMILAR TO col` -- companion to sql508 for the SIMILAR TO operator. A column compared against itself is always TRUE (for non-NULL rows) for the positive form and always FALSE for the negated form, regardless of what's in the column. Almost always a copy-paste typo for two distinct columns.

<sub>`dsl-analysis/src/rules/self_similar.rs`</sub>

### `sql511` — `WHERE col @> col` / `col <@ col` / `col && col`

`WHERE col @> col` / `col <@ col` / `col && col` -- containment / overlap of a column with itself is always TRUE for non-NULL rows (every value contains and is contained by itself; every non-empty array overlaps itself). Almost always a copy-paste typo for two distinct operands. Companion to sql508 and sql510 for the array / jsonb operator family.

<sub>`dsl-analysis/src/rules/self_containment.rs`</sub>

### `sql512` — table-level PK / UNIQUE / FK source constraint references a column that isn't declared on this table. PG...

table-level PK / UNIQUE / FK source constraint references a column that isn't declared on this table. PG raises 42703 at `CREATE TABLE` time. Catches typos like: CREATE TABLE t ( id int, CONSTRAINT pk_t PRIMARY KEY (idd) -- typo ); sql185 covers the *target* side of an FK (referenced table's column). This rule covers the *source* side (the column on the table being defined). CHECK bodies are out of scope -- they accept arbitrary expressions and would need full expression resolution.

<sub>`dsl-analysis/src/rules/constraint_unknown_column.rs`</sub>

### `sql513` — function call arg-count validation

function call arg-count validation. Text-scans the statement body for `<name>(...)` invocations, looks the function up in the catalog, and warns when the arity doesn't match the declared signature: - too few args (less than required) -> warning - too many args (more than declared, non-variadic) -> warning Why text scan instead of AST: the pg_query backend flattens FuncCall args into their column refs and does not emit Expr::Call. A text scan with paren tracking captures every call site reliably, including nested calls and cast-expression contexts. Empty catalog (no live DB + no offline-derived functions) -> silent.

<sub>`dsl-analysis/src/rules/function_arg_validation.rs`</sub>

### `sql514` — empty expression parentheses where an expression is required. Catches the post-refactor pattern where a...

empty expression parentheses where an expression is required. Catches the post-refactor pattern where a `WHEN`, `IF`, `WHERE`, `NOT`, `AND`, `OR`, `IN`, `ANY`, `ALL` etc. ends up with an empty `()` group -- almost always a typo or half-deleted condition. IF NOT () THEN ... -- meant `IF NOT (cond)` CASE WHEN () THEN ... -- empty WHEN clause WHERE id IN () -- PG rejects empty IN list at runtime foo BETWEEN () AND () -- empty BETWEEN bound Text-scanned (cheap + works regardless of AST shape). Skips bodies of string literals + comments via strip_comments_strings.

<sub>`dsl-analysis/src/rules/empty_expression_paren.rs`</sub>

### `sql515` — `WHERE col IN (1)` / `WHERE col NOT IN (1)`

`WHERE col IN (1)` / `WHERE col NOT IN (1)` -- an IN list with a single element. Equivalent to `col = 1` / `col <> 1` but longer and a hair slower to read; almost always a leftover from a list that was templated down to one value. Suggests the direct comparison. Skips genuine multi-element lists, subqueries (`IN (SELECT ...)`), `VALUES` lists, empty lists (sql234 owns those), and `IN (NULL)` -- the last would rewrite to `= NULL`, which is its own (wrong) thing.

<sub>`dsl-analysis/src/rules/in_list_single_value.rs`</sub>

### `sql516` — `UPDATE t SET col = col`

`UPDATE t SET col = col` -- assigning a column to itself is a no-op. It still dirties the row (fires triggers, bumps xmax, writes a new tuple version), so it's wasteful at best and usually a copy-paste slip where the right-hand side should have referenced a different column or expression. Only the textually-identical `col = col` / `t.col = t.col` form is flagged; `col = col + 1`, `a = b.a`, casts, etc. are left alone.

<sub>`dsl-analysis/src/rules/update_self_assignment.rs`</sub>

### `sql517` — `JOIN ... ON 1 = 1`

`JOIN ... ON 1 = 1` -- the join condition is a numeric constant tautology, so the join produces a full cartesian product. That's almost always a placeholder someone forgot to fill in, or an accidental cross join. Suggests an explicit `CROSS JOIN` (if intended) or a real predicate. Only the *numeric* `n = n` form is flagged, never `ON TRUE` -- the latter is the idiomatic, intentional condition for `LEFT JOIN LATERAL (...)`.

<sub>`dsl-analysis/src/rules/join_on_constant_tautology.rs`</sub>

### `sql518` — `CASE WHEN cond THEN TRUE ELSE FALSE END`

`CASE WHEN cond THEN TRUE ELSE FALSE END` -- a single-branch CASE that just maps a condition to a boolean. It's equivalent to `(cond) IS TRUE` (the `IS TRUE` matters: the CASE returns FALSE, not NULL, when `cond` is NULL). The `THEN FALSE ELSE TRUE` form is `(cond) IS NOT TRUE`. Collapsing it is shorter and reads better. Conservative: only the searched single-WHEN form with boolean literals in both arms is flagged; multi-branch, nested, or simple (`CASE x WHEN`) forms are left alone.

<sub>`dsl-analysis/src/rules/case_boolean_redundant.rs`</sub>

### `sql519` — `WHERE a = 1 OR a = 2 OR a = 3`

`WHERE a = 1 OR a = 2 OR a = 3` -- a chain of equality tests on the same column, OR-ed together. Equivalent to `a IN (1, 2, 3)`, which is shorter, clearer, and lets the planner build a single index probe / hash lookup instead of evaluating each disjunct. Fires at 3+ values on one column to stay quiet on trivial two-way ORs. Scoped to WHERE / ON / HAVING bodies; only clean `col = <value>` disjuncts count, so a term carrying `AND`, an inequality, or a function on the LHS breaks the run rather than producing a wrong suggestion.

<sub>`dsl-analysis/src/rules/or_chain_to_in.rs`</sub>

### `sql520` — `WHERE lower(col) = 'ABC'` / `WHERE upper(col) LIKE 'abc%'`

`WHERE lower(col) = 'ABC'` / `WHERE upper(col) LIKE 'abc%'` -- a case-folding function compared against a string literal of the opposite case. `lower(...)` only ever returns lowercase, so it can never equal a literal containing an uppercase ASCII letter (and vice-versa for `upper(...)`). The predicate is dead: it matches zero rows. Almost always a bug -- the literal should have been written in the folded case.

<sub>`dsl-analysis/src/rules/case_fold_impossible_compare.rs`</sub>

### `sql521` — `col = ANY(ARRAY[1])` / `col <> ALL(ARRAY['x'])`

`col = ANY(ARRAY[1])` / `col <> ALL(ARRAY['x'])` -- the array has a single element, so the quantifier is pointless: `op ANY(ARRAY[v])` and `op ALL(ARRAY[v])` both reduce to `col op v`. Usually a list templated down to one value. Suggests the direct comparison. (Parallels sql515 for the `IN (v)` spelling.)

<sub>`dsl-analysis/src/rules/any_all_single_element_array.rs`</sub>

### `sql522` — `a LEFT JOIN b ON ... WHERE b.col = 'x'`

`a LEFT JOIN b ON ... WHERE b.col = 'x'` -- a positive WHERE predicate on the *nullable* (right) side of a LEFT JOIN silently turns it into an INNER JOIN: the NULL-extended rows from unmatched left rows fail the filter and disappear. Almost always a bug -- either the condition belongs in the ON clause (to keep it an outer join) or the join should be an explicit INNER JOIN. Conservative: only a conjunct that *begins* with `alias.col <predicate>` is flagged, and any conjunct mentioning NULL (the legitimate `b.col IS NULL` anti-join / `... OR b.col IS NULL` guard) or containing a top-level OR is skipped, so the idiomatic outer-join-preserving forms never fire.

<sub>`dsl-analysis/src/rules/left_join_defeated_by_where.rs`</sub>

### `sql523` — `WHERE col IS NULL OR col IS NOT NULL`

`WHERE col IS NULL OR col IS NOT NULL` -- the two halves cover every possible value of `col`, so the disjunction is always true and the whole predicate is a no-op filter. Usually a leftover from refactoring a real condition, or a misunderstanding of three-valued logic. (Pairs with sql435, which catches the always-false `IS NULL AND <strict op>` form.)

<sub>`dsl-analysis/src/rules/is_null_or_is_not_null.rs`</sub>

### `sql524` — `col LIKE '%'`

`col LIKE '%'` -- a pattern that is nothing but `%` matches every non-NULL value, so the predicate does no filtering (it's at most an `IS NOT NULL`). `col NOT LIKE '%'` is the opposite: it matches no non-NULL row, so the query returns nothing. Both are almost always a placeholder that was never filled in (e.g. a search box defaulting to `%`).

<sub>`dsl-analysis/src/rules/like_all_wildcard.rs`</sub>

### `sql525` — `EXISTS (SELECT ... LIMIT 1)`

`EXISTS (SELECT ... LIMIT 1)` -- the LIMIT is dead weight. EXISTS short-circuits as soon as the subquery yields a single row, so capping it changes nothing about the result and only adds noise (and a needless node the planner has to reason about). Drop the LIMIT.

<sub>`dsl-analysis/src/rules/exists_with_limit.rs`</sub>

### `sql526` — `WHERE col = 1 AND col = 2`

`WHERE col = 1 AND col = 2` -- the same column is required to equal two different constants at once, so the predicate is always false and the query returns nothing. Also catches `col = 1 AND col <> 1` (same value demanded and forbidden). Usually a copy-paste slip or a bad codegen template. (Pairs with sql407, which handles literal-only `1 = 2`.)

<sub>`dsl-analysis/src/rules/eq_contradiction.rs`</sub>

### `sql527` — `WHERE col > 5 AND col < 3`

`WHERE col > 5 AND col < 3` -- the lower and upper bounds on a column don't overlap, so the range is empty and the query returns nothing. Type-independent: only flagged when the bounds are empty for *any* numeric domain (`lo > hi`, or `lo == hi` with a strict `<`/`>` on either side), so `col > 5 AND col < 6` (empty for ints, not for numerics) is left alone.

<sub>`dsl-analysis/src/rules/impossible_range.rs`</sub>

### `sql528` — `REPLACE(s, x, x)`

`REPLACE(s, x, x)` -- the search and replacement strings are identical, so the call returns `s` unchanged. A no-op, almost always a copy-paste slip where the replacement should differ (e.g. `REPLACE(s, '-', '')`). Same idea as NULLIF(x, x) (sql085).

<sub>`dsl-analysis/src/rules/replace_same_from_to.rs`</sub>

### `sql529` — `HAVING COUNT(*) > 0`

`HAVING COUNT(*) > 0` -- a group only exists if it has at least one row, so `COUNT(*)` is always >= 1 and the predicate is always true. It filters nothing; the GROUP BY already guarantees non-empty groups. Common when someone reaches for HAVING expecting WHERE-style row filtering. Restricted to `COUNT(*)` / `COUNT(1)` (which count rows, not non-NULL values) and to comparisons that hold for every integer >= 1.

<sub>`dsl-analysis/src/rules/having_count_always_true.rs`</sub>

### `sql530` — `COALESCE(COALESCE(a, b), c)`

`COALESCE(COALESCE(a, b), c)` -- a COALESCE whose argument is itself a COALESCE. The two collapse into one `COALESCE(a, b, c)`, which is shorter and lets the planner stop at the first non-NULL without an extra nesting level. Usually an artifact of incremental edits.

<sub>`dsl-analysis/src/rules/coalesce_nested.rs`</sub>

### `sql531` — `SELECT name AS name`

`SELECT name AS name` -- aliasing a column to its own name. The `AS name` is dead: the output column is already called `name`. Also covers `SELECT u.name AS name` (the unqualified output name already matches). Pure noise; drop the alias. Only a bare column reference whose alias equals the column's base name is flagged -- `lower(name) AS name` (a real rename) and `name AS full_name` are left alone.

<sub>`dsl-analysis/src/rules/redundant_column_alias.rs`</sub>

### `sql532` — `SELECT

`SELECT ... UNION SELECT ...` where two branches of a set operation are textually identical. `UNION` dedups the duplicate away (so it reduces to one branch) and `UNION ALL` repeats every row twice; either way it's almost always a copy-paste slip where one branch should have differed. Also covers INTERSECT / EXCEPT.

<sub>`dsl-analysis/src/rules/setop_identical_branches.rs`</sub>

### `sql533` — `col BETWEEN 5 AND 5`

`col BETWEEN 5 AND 5` -- the lower and upper bounds are the same value, so the range degenerates to `col = 5`. `col NOT BETWEEN 5 AND 5` is `col <> 5`. Writing it as a range obscures the intent and is usually a placeholder left half-edited. Only simple identical literal/identifier bounds are flagged.

<sub>`dsl-analysis/src/rules/between_equal_bounds.rs`</sub>

### `sql534` — `GREATEST(x, x)` / `LEAST(a, b, a)`

`GREATEST(x, x)` / `LEAST(a, b, a)` -- a duplicate argument. The max / min is unaffected by repeating a value, so the extra argument is dead. `GREATEST(x, x)` in particular just returns `x`. Usually a typo for a different second argument. (Mirrors sql417 for COALESCE.)

<sub>`dsl-analysis/src/rules/greatest_least_dup_arg.rs`</sub>

### `sql535` — `WHERE a <> 1 AND a <> 2 AND a <> 3`

`WHERE a <> 1 AND a <> 2 AND a <> 3` -- a chain of not-equal tests on the same column, AND-ed together. Equivalent to `a NOT IN (1, 2, 3)`, which is shorter and clearer. The mirror of sql519 (`= ... OR` -> `IN`). Fires at 3+ values on one column to stay quiet on trivial two-way chains.

<sub>`dsl-analysis/src/rules/neq_chain_to_not_in.rs`</sub>

### `sql536` — `INSERT ... ON CONFLICT ... DO UPDATE SET col = col`

`INSERT ... ON CONFLICT ... DO UPDATE SET col = col` -- the upsert assigns a column to its own (pre-conflict) value, a no-op. The intent was almost certainly `SET col = EXCLUDED.col` (take the incoming value). The INSERT path of sql516 (which only sees plain UPDATE) misses this, so it gets its own check.

<sub>`dsl-analysis/src/rules/on_conflict_self_assignment.rs`</sub>

### `sql537` — `NOT (a = b)`

`NOT (a = b)` -- negating a single comparison is clearer written with the negated operator: `a <> b`. Likewise `NOT (a < b)` -> `a >= b`, etc. Complements sql470 (which handles `NOT (col IN/LIKE/BETWEEN ...)`). Only a lone comparison inside the parens is rewritten; anything with AND/OR/IN/LIKE/BETWEEN/IS is left alone.

<sub>`dsl-analysis/src/rules/not_paren_comparison.rs`</sub>

### `sql538` — `ROUND(x, 0)` / `TRUNC(x, 0)`

`ROUND(x, 0)` / `TRUNC(x, 0)` -- the explicit scale of 0 is redundant; the single-argument `ROUND(x)` / `TRUNC(x)` already rounds / truncates to zero decimal places. Harmless but noise, and a `, 0` often signals a half-finished edit (the author meant a real scale).

<sub>`dsl-analysis/src/rules/round_trunc_zero_scale.rs`</sub>

### `sql539` — `SELECT DISTINCT(col), other ...` (or `COUNT(DISTINCT(col))`)

`SELECT DISTINCT(col), other ...` (or `COUNT(DISTINCT(col))`) -- `DISTINCT` written as if it were a function. The parentheses are misleading: `DISTINCT` is a keyword that deduplicates the *entire* row / aggregate input, not just the parenthesised expression. The query may still be correct, but readers (and the author) routinely misread it as "distinct on this one column".

<sub>`dsl-analysis/src/rules/distinct_looks_like_function.rs`</sub>

### `sql540` — `WHERE length(s) = 0` / `length(s) > 0`

`WHERE length(s) = 0` / `length(s) > 0` -- comparing a string's length to zero is an indirect, non-sargable way to ask "is it empty?". `length(s) = 0` is `s = ''` and `length(s) > 0` is `s <> ''` (for non-NULL `s`). The direct form is clearer and can use an index on `s`.

<sub>`dsl-analysis/src/rules/length_compare_zero.rs`</sub>

### `sql541` — a boolean literal operand that forces the whole condition to a constant

a boolean literal operand that forces the whole condition to a constant -- `... OR TRUE` (always true, matches everything) or `... AND FALSE` (always false, matches nothing). Both are almost always a debugging placeholder that escaped review and silently changes which rows are affected. (The harmless `AND TRUE` / `OR FALSE` no-ops are left to sql282.) Precedence-aware: `OR TRUE` dominates regardless (OR binds loosest), but `AND FALSE` only forces the result false when there is no top-level `OR`. A literal that is the side of a comparison (`col = TRUE`) is never mistaken for a standalone operand.

<sub>`dsl-analysis/src/rules/boolean_literal_dominates.rs`</sub>

### `sql542` — `now()::date` / `current_timestamp::date`

`now()::date` / `current_timestamp::date` -- casting the current timestamp to a date is exactly what `CURRENT_DATE` returns, more directly and without a per-row cast. Also covers `localtimestamp::date`. A small readability / idiom hint.

<sub>`dsl-analysis/src/rules/now_cast_to_date.rs`</sub>

### `sql543` — `GROUP BY count(*)` / `GROUP BY sum(x)`

`GROUP BY count(*)` / `GROUP BY sum(x)` -- an aggregate function in the GROUP BY list. Postgres rejects this at execution with 42803 ("aggregate functions are not allowed in GROUP BY"). Usually a confusion with HAVING, or a column that was meant to be a plain expression.

<sub>`dsl-analysis/src/rules/group_by_aggregate.rs`</sub>

### `sql544` — `WHERE col >= 5 AND col <= 5`

`WHERE col >= 5 AND col <= 5` -- inclusive lower and upper bounds on the same value. The range admits exactly one value, so it's just `col = 5`, written more directly. (sql527 owns the *empty* cases like `> 5 AND < 5`; this is the single-point case it deliberately leaves alone.)

<sub>`dsl-analysis/src/rules/range_is_equality.rs`</sub>

### `sql545` — `WHERE EXTRACT(MONTH FROM x) = 13` / `EXTRACT(DOW FROM x) = 7`

`WHERE EXTRACT(MONTH FROM x) = 13` / `EXTRACT(DOW FROM x) = 7` -- comparing an EXTRACT (or date_part) field to a value outside that field's range, so the predicate never matches. `DOW` (0-6, Sunday = 0) tripping people who expect 1-7 is the classic case -- they want `ISODOW`.

<sub>`dsl-analysis/src/rules/extract_value_out_of_range.rs`</sub>

### `sql546` — `WHERE x % 7 = 7`

`WHERE x % 7 = 7` -- the result of `x % N` is always in the range `(-N, N)`, so comparing it to a value whose magnitude is `>= N` can never be true. `x % 2 = 2`, `id % 10 = 10`, etc. are dead predicates -- usually an off-by-one (the author wanted `% N = 0` or a different divisor).

<sub>`dsl-analysis/src/rules/modulo_out_of_range.rs`</sub>

### `sql547` — `WHERE array_length(arr, 1) = 0`

`WHERE array_length(arr, 1) = 0` -- a wrong empty-array test. `array_length` returns NULL for an empty array (and `>= 1` otherwise), so it is never 0; the predicate never matches. Use `cardinality(arr) = 0`, `arr = '{}'`, or `array_length(arr, 1) IS NULL` instead.

<sub>`dsl-analysis/src/rules/array_length_zero_check.rs`</sub>

### `sql548` — `col <> ALL(ARRAY[1, 2, 3])`

`col <> ALL(ARRAY[1, 2, 3])` -- equivalent to `col NOT IN (1, 2, 3)`, which is shorter and the idiom most readers expect. (sql495 handles the buggy `= ALL`; sql521 the single-element case -- this is the multi-element `<> ALL` style suggestion.)

<sub>`dsl-analysis/src/rules/neq_all_array_to_not_in.rs`</sub>

### `sql549` — `FROM users AS users` / `JOIN orders orders`

`FROM users AS users` / `JOIN orders orders` -- aliasing a table to its own name. The alias adds nothing; drop it (or pick a short alias like `u`). Pure noise, and a tell-tale sign of a half-applied rename.

<sub>`dsl-analysis/src/rules/table_self_alias.rs`</sub>

### `sql550` — `WHERE x > 5 AND x > 3`

`WHERE x > 5 AND x > 3` -- two bounds in the same direction on one column. Only the tighter one matters (`x > 5` here); the looser bound is dead weight. Usually a leftover from editing a range. (sql527 owns the contradictory opposite-direction case; this is the same-direction one.)

<sub>`dsl-analysis/src/rules/redundant_range_bound.rs`</sub>

### `sql551` — redundantly nested functions whose outer call subsumes the inner one: * `upper(lower(x))` / `lower(upper(x))`...

redundantly nested functions whose outer call subsumes the inner one: * `upper(lower(x))` / `lower(upper(x))` / `upper(upper(x))` -- the outer case-fold wins; the inner one does nothing. * `trim(trim(x))` / `btrim(btrim(x))` / `abs(abs(x))` -- idempotent, so the second application is a no-op. * `reverse(reverse(x))` -- two reverses cancel; it's just `x`.

<sub>`dsl-analysis/src/rules/redundant_nested_function.rs`</sub>

### `sql552` — `WHERE abs(x) < 0` / `cardinality(arr) = -1`

`WHERE abs(x) < 0` / `cardinality(arr) = -1` -- comparing a function whose result is always non-negative against a negative value (or `< 0`), so the predicate never matches. Covers abs / length-family / cardinality / bit_length. (sql540 owns the `length(s) = 0` empty-string case; this is the genuinely-impossible negative case.)

<sub>`dsl-analysis/src/rules/nonneg_func_negative_compare.rs`</sub>

### `sql553` — `CREATE TABLE t (col int DEFAULT NULL)`

`CREATE TABLE t (col int DEFAULT NULL)` -- a nullable column already defaults to NULL, so `DEFAULT NULL` is redundant noise. (sql069 owns the contradictory `NOT NULL DEFAULT NULL`; this is the plain nullable case.)

<sub>`dsl-analysis/src/rules/default_null_redundant.rs`</sub>

### `sql554` — the operator spellings of LIKE

the operator spellings of LIKE -- `~~`, `~~*`, `!~~`, `!~~*` -- in place of `LIKE` / `ILIKE` / `NOT LIKE` / `NOT ILIKE`. They're the internal operators PG uses to implement those keywords; valid, but obscure and easily confused with the regex operators (`~`, `~*`). Prefer the keyword.

<sub>`dsl-analysis/src/rules/like_operator_form.rs`</sub>

### `sql555` — `WHERE active IS TRUE` / `WHERE active IS FALSE`

`WHERE active IS TRUE` / `WHERE active IS FALSE` -- in a boolean predicate the `IS TRUE` is redundant (`WHERE active`) and `IS FALSE` is just `NOT active`. Scoped to WHERE / ON / HAVING so a SELECT-list `x IS TRUE` (which legitimately produces a boolean value) is left alone. `IS NOT TRUE` / `IS NOT FALSE` are NOT flagged -- their NULL handling does not reduce to a plain expression. (Parallels sql054 for `= true`.)

<sub>`dsl-analysis/src/rules/is_true_redundant.rs`</sub>

### `sql556` — `col = ANY(ARRAY[1, 2, 3])`

`col = ANY(ARRAY[1, 2, 3])` -- equivalent to `col IN (1, 2, 3)`, the idiom most readers reach for first. (sql521 handles the single-element case; sql548 the `<> ALL` -> `NOT IN` mirror; this is multi-element `= ANY`.)

<sub>`dsl-analysis/src/rules/eq_any_array_to_in.rs`</sub>

### `sql557` — `CREATE TABLE t (id int, id text)`

`CREATE TABLE t (id int, id text)` -- the same column name appears twice in the column list. Postgres rejects it at DDL time with 42701 ("column \"id\" specified more than once"). Almost always a copy-paste slip.

<sub>`dsl-analysis/src/rules/create_table_dup_column.rs`</sub>

### `sql558` — a `CREATE TABLE` with more than one PRIMARY KEY definition (e.g

a `CREATE TABLE` with more than one PRIMARY KEY definition (e.g. an inline `id int PRIMARY KEY` plus a table-level `PRIMARY KEY (...)`, or two inline ones). A table may have only one primary key; Postgres rejects it with 42P16 ("multiple primary keys for table ... are not allowed"). For a composite key, list the columns in a single `PRIMARY KEY (a, b)`.

<sub>`dsl-analysis/src/rules/multiple_primary_keys.rs`</sub>

### `sql559` — `CREATE INDEX idx ON t (a, b, a)`

`CREATE INDEX idx ON t (a, b, a)` -- the same column (or expression) listed twice in an index. Postgres rejects it with 42701 ("column \"a\" specified more than once"). The repeat is dead weight even when accepted; almost always a typo for a different column.

<sub>`dsl-analysis/src/rules/index_dup_column.rs`</sub>

### `sql560` — `FOREIGN KEY (a, b) REFERENCES t (c)`

`FOREIGN KEY (a, b) REFERENCES t (c)` -- the referencing and referenced column lists have different lengths. Postgres rejects this with 42830 ("number of referencing and referenced columns for foreign key disagree"). The two lists must line up one-to-one.

<sub>`dsl-analysis/src/rules/fk_column_count_mismatch.rs`</sub>

### `sql561` — `SELECT ... LIMIT ALL`

`SELECT ... LIMIT ALL` -- `LIMIT ALL` is the explicit spelling of "no limit", exactly the same as omitting the clause. It's harmless but pure noise; drop it.

<sub>`dsl-analysis/src/rules/limit_all_redundant.rs`</sub>

### `sql562` — `col int DEFAULT (SELECT max(id) FROM t)`

`col int DEFAULT (SELECT max(id) FROM t)` -- a subquery in a column DEFAULT. Postgres rejects it ("cannot use subquery in DEFAULT expression"): a default is evaluated per-row without access to other rows/tables. Use a trigger or compute the value in the INSERT instead.

<sub>`dsl-analysis/src/rules/default_subquery.rs`</sub>

### `sql563` — `col = ANY(ARRAY[1, 2, 1])`

`col = ANY(ARRAY[1, 2, 1])` -- a duplicate element in an ANY / ALL array literal. The planner dedups it, but it bloats the query and is usually a copy-paste typo for a different value. (The `IN (...)` spelling is covered by sql306.)

<sub>`dsl-analysis/src/rules/any_array_duplicate.rs`</sub>

### `sql564` — `CREATE TABLE t (a int NULL NOT NULL)`

`CREATE TABLE t (a int NULL NOT NULL)` -- a column declared both explicitly nullable (`NULL`) and `NOT NULL`. Postgres rejects the contradiction with 42601 ("conflicting NULL/NOT NULL declarations").

<sub>`dsl-analysis/src/rules/null_not_null_conflict.rs`</sub>

### `sql565` — `col - col` (always 0) and `col / col` (always 1, or a division-by-zero error when `col` is 0). Subtracting...

`col - col` (always 0) and `col / col` (always 1, or a division-by-zero error when `col` is 0). Subtracting or dividing a column by itself is a constant -- almost always a typo for a different operand.

<sub>`dsl-analysis/src/rules/self_arithmetic.rs`</sub>

### `sql566` — `WHERE x = x + 1`

`WHERE x = x + 1` -- a column compared to itself plus/minus a non-zero constant. It reduces to `0 = 1`, so it's always false and the query returns nothing. Almost always a typo (a different column, or the wrong side of an update expression).

<sub>`dsl-analysis/src/rules/col_eq_col_offset.rs`</sub>

### `sql567` — common built-in functions called with too few arguments

common built-in functions called with too few arguments -- e.g. `to_char(x)` (needs a format), `lpad(s)` (needs a length), `split_part(s, d)` (needs a field index). The single/short forms don't exist, so Postgres raises 42883 ("function ... does not exist"). These built-ins aren't in the catalog, so sql513's signature check doesn't see them.

<sub>`dsl-analysis/src/rules/builtin_too_few_args.rs`</sub>

### `sql568` — `col ~ 'abc'`

`col ~ 'abc'` -- a regex match against a pattern with no regex metacharacters at all. It just tests "contains the substring abc", which is `col LIKE '%abc%'` (or `ILIKE` / `NOT LIKE` for `~*` / `!~`). The LIKE form is clearer and can use a `text_pattern_ops` index. (sql423 handles the anchored `^prefix` form; this is the no-metacharacter substring case.)

<sub>`dsl-analysis/src/rules/regex_literal_could_be_like.rs`</sub>

### `sql569` — `EXISTS (SELECT ... ORDER BY ...)`

`EXISTS (SELECT ... ORDER BY ...)` -- ordering inside an EXISTS subquery is dead weight. EXISTS only asks "is there at least one row?", which is independent of order, so the planner discards the sort anyway. Drop the ORDER BY. (Companion to sql525, which handles LIMIT in EXISTS.)

<sub>`dsl-analysis/src/rules/exists_order_by.rs`</sub>

### `sql570` — `EXISTS (SELECT DISTINCT ...)`

`EXISTS (SELECT DISTINCT ...)` -- the DISTINCT is dead weight. EXISTS only checks whether at least one row exists; deduplicating the rows first can't change that (and costs a sort/hash). Drop the DISTINCT. (Companion to sql525 / sql569 for LIMIT / ORDER BY in EXISTS.)

<sub>`dsl-analysis/src/rules/exists_distinct.rs`</sub>

### `sql571` — `CREATE ROLE app PASSWORD 'hunter2'`

`CREATE ROLE app PASSWORD 'hunter2'` -- a plaintext password literal in DDL. The value lands in the server log, `pg_stat_activity`, `.psql_history`, and any migration file in version control. Use `\password` (psql prompts and sends a pre-hashed value) or supply a precomputed SCRAM-SHA-256 verifier instead.

<sub>`dsl-analysis/src/rules/plaintext_password.rs`</sub>

### `sql572` — `CREATE ROLE deploy SUPERUSER` / `ALTER ROLE app SUPERUSER`

`CREATE ROLE deploy SUPERUSER` / `ALTER ROLE app SUPERUSER` -- granting the SUPERUSER attribute. A superuser bypasses every permission check (and can read/write any file the server account can), so it should be reserved for the bootstrap/admin role. Grant the specific privileges (or attributes like CREATEDB / CREATEROLE) the role actually needs.

<sub>`dsl-analysis/src/rules/role_superuser.rs`</sub>

### `sql573` — `CREATE ROLE etl BYPASSRLS` / `ALTER ROLE app BYPASSRLS`

`CREATE ROLE etl BYPASSRLS` / `ALTER ROLE app BYPASSRLS` -- the BYPASSRLS attribute lets the role skip every row-level-security policy on every table. That quietly defeats RLS for that role; grant it only to trusted admin/maintenance roles, and prefer per-table policies otherwise.

<sub>`dsl-analysis/src/rules/role_bypassrls.rs`</sub>

### `sql574` — `ALTER TABLE t DISABLE ROW LEVEL SECURITY`

`ALTER TABLE t DISABLE ROW LEVEL SECURITY` -- turns off RLS enforcement on the table, so every policy stops applying and all rows become visible/writable to anyone with table privileges. Sometimes intentional (maintenance), but it's a security-relevant change worth a second look -- often a leftover from debugging.

<sub>`dsl-analysis/src/rules/disable_row_level_security.rs`</sub>

### `sql575` — `CREATE POLICY p ON t USING (true)` (or `WITH CHECK (true)`)

`CREATE POLICY p ON t USING (true)` (or `WITH CHECK (true)`) -- a row-level-security policy whose qualifier is trivially true grants access to every row, which defeats the point of enabling RLS. Usually a placeholder that was never filled in with a real ownership/tenant check.

<sub>`dsl-analysis/src/rules/policy_using_true.rs`</sub>

### `sql576` — `ALTER TABLE t DISABLE TRIGGER ALL`

`ALTER TABLE t DISABLE TRIGGER ALL` -- disables *every* trigger on the table, including the internal RI triggers that enforce foreign keys. Bulk loads done this way can leave dangling references and skip audit / business-logic triggers. Prefer `DISABLE TRIGGER USER` (keeps FK checks), or re-validate constraints afterwards. Easy to leave on by accident.

<sub>`dsl-analysis/src/rules/disable_trigger_all.rs`</sub>

### `sql577` — `CREATE VIEW v AS SELECT ... ORDER BY x`

`CREATE VIEW v AS SELECT ... ORDER BY x` -- an ORDER BY in a (non-materialized) view definition is not guaranteed to survive: when you `SELECT ... FROM v` the planner is free to re-order, so the sort is wasted work and a false promise. Sort in the queries that read the view instead. (ORDER BY ... LIMIT is a deliberate top-N and is left alone; MATERIALIZED views, where the order is materialized once, are also skipped.)

<sub>`dsl-analysis/src/rules/view_order_by.rs`</sub>

### `sql578` — `CREATE RULE ...`

`CREATE RULE ...` -- the PostgreSQL rule system rewrites queries at parse time and has notoriously surprising semantics (multiple evaluation of volatile functions, interactions with RETURNING, etc.). The PG docs steer new code toward triggers (for row-level side effects) or updatable views with INSTEAD OF triggers. Reserve rules for the rare case they're genuinely needed.

<sub>`dsl-analysis/src/rules/create_rule_legacy.rs`</sub>

### `sql579` — `... WITH (autovacuum_enabled = false)` / `ALTER TABLE t SET (autovacuum_enabled = off)`

`... WITH (autovacuum_enabled = false)` / `ALTER TABLE t SET (autovacuum_enabled = off)` -- turning off autovacuum for a table. Unless a scheduled manual VACUUM/ANALYZE replaces it, the table accumulates dead tuples and stale statistics indefinitely, degrading both bloat and plans. Almost always a temporary tweak that got left in.

<sub>`dsl-analysis/src/rules/autovacuum_disabled.rs`</sub>

### `sql580` — `CREATE UNLOGGED TABLE ...` (or `ALTER TABLE ... SET UNLOGGED`)

`CREATE UNLOGGED TABLE ...` (or `ALTER TABLE ... SET UNLOGGED`) -- unlogged tables skip the WAL, so they're fast to write but their entire contents are TRUNCATED on a crash or unclean restart, and they aren't replicated to standbys. Fine for scratch/cache data, but a data-loss surprise if the table holds anything you expect to keep.

<sub>`dsl-analysis/src/rules/unlogged_table.rs`</sub>

### `sql581` — a `json` column type (or `::json` cast). `jsonb` is almost always the better choice: it's stored decomposed...

a `json` column type (or `::json` cast). `jsonb` is almost always the better choice: it's stored decomposed (so it supports GIN indexes and the containment / path operators), and it dedups keys. Plain `json` only preserves the exact input text and whitespace -- rarely what you want for a stored value. Word-bounded matching skips `jsonb`, `json_*` functions, and `to_json`.

<sub>`dsl-analysis/src/rules/json_prefer_jsonb.rs`</sub>

### `sql582` — the `money` column type

the `money` column type. It carries a fixed, locale-dependent fractional precision, its text output depends on `lc_monetary`, and arithmetic with it is awkward (no clean multiply/divide by fractions). Store currency as `numeric(p, s)` (or integer minor units) instead.

<sub>`dsl-analysis/src/rules/money_type.rs`</sub>

### `sql583` — `EXISTS (SELECT ... GROUP BY x)` with no HAVING

`EXISTS (SELECT ... GROUP BY x)` with no HAVING -- the GROUP BY is dead weight. EXISTS only checks for at least one row; grouping the rows first can't change whether any exist (a non-empty input always yields at least one group). With a HAVING it *can* matter, so that case is left alone. (Companion to sql525 / sql569 / sql570 for LIMIT / ORDER BY / DISTINCT in EXISTS.)

<sub>`dsl-analysis/src/rules/exists_group_by.rs`</sub>

### `sql584` — the internal `pg_catalog` type aliases (`int4`, `int8`, `float8`, `serial4`, ...) in DDL

the internal `pg_catalog` type aliases (`int4`, `int8`, `float8`, `serial4`, ...) in DDL. They're valid but read as implementation detail; the SQL-standard spellings (`integer`, `bigint`, `double precision`, ...) are clearer and what the docs use. Scoped to DDL / `::` casts so a column or alias coincidentally named `int4` isn't flagged.

<sub>`dsl-analysis/src/rules/internal_type_alias.rs`</sub>

### `sql585` — a `CLUSTER` command. It physically rewrites the whole table in index order under an ACCESS EXCLUSIVE lock...

a `CLUSTER` command. It physically rewrites the whole table in index order under an ACCESS EXCLUSIVE lock, blocking every read and write for the entire duration -- on a large table that's a long outage. The ordering also isn't maintained afterwards. For online use, reach for `pg_repack`.

<sub>`dsl-analysis/src/rules/cluster_locks_table.rs`</sub>

### `sql586` — `VACUUM FULL` rewrites the entire table (and its indexes) into new files under an ACCESS EXCLUSIVE lock...

`VACUUM FULL` rewrites the entire table (and its indexes) into new files under an ACCESS EXCLUSIVE lock, blocking all reads and writes until it finishes -- and it needs free disk space roughly equal to the table. Plain `VACUUM` reclaims space online; `pg_repack` compacts without the long lock. Reserve `VACUUM FULL` for a planned maintenance window.

<sub>`dsl-analysis/src/rules/vacuum_full_locks.rs`</sub>

### `sql587` — `ALTER TABLE t ADD COLUMN c uuid DEFAULT gen_random_uuid()`

`ALTER TABLE t ADD COLUMN c uuid DEFAULT gen_random_uuid()` -- a non-constant default on ADD COLUMN forces a full table rewrite under an ACCESS EXCLUSIVE lock (the constant-default fast path only applies to a literal). On a large table that's a long outage. Add the column with no default, backfill in batches, then `SET DEFAULT`. (sql145 covers the per-row-recompute semantics; this is the rewrite/lock cost.)

<sub>`dsl-analysis/src/rules/add_column_volatile_default.rs`</sub>

### `sql588` — `ALTER TABLE t ADD PRIMARY KEY (...)` / `ADD UNIQUE (...)`

`ALTER TABLE t ADD PRIMARY KEY (...)` / `ADD UNIQUE (...)` -- adding a primary-key or unique constraint builds its backing index while holding an ACCESS EXCLUSIVE lock, blocking writes (and the build itself) for the whole duration. On a large table, build the index off-lock first -- `CREATE UNIQUE INDEX CONCURRENTLY ...` -- then `ADD CONSTRAINT ... USING INDEX ...`. (Skipped when it already attaches a prebuilt index.)

<sub>`dsl-analysis/src/rules/alter_add_key_lock.rs`</sub>

### `sql589` — `ALTER TABLE t ADD CONSTRAINT fk FOREIGN KEY (a) REFERENCES b (c)` without `NOT VALID`

`ALTER TABLE t ADD CONSTRAINT fk FOREIGN KEY (a) REFERENCES b (c)` without `NOT VALID`. Adding a validated foreign key scans every existing row to check it, holding a lock that blocks writes on both tables for the duration. Add it `NOT VALID` (cheap, only new rows are checked), then `VALIDATE CONSTRAINT` in a separate step (takes only a SHARE UPDATE EXCLUSIVE lock). (Companion to sql280 for CHECK constraints.)

<sub>`dsl-analysis/src/rules/add_fk_not_valid.rs`</sub>

### `sql590` — a `REINDEX` without `CONCURRENTLY`

a `REINDEX` without `CONCURRENTLY`. Plain REINDEX takes an ACCESS EXCLUSIVE lock on the table (or index's table), blocking all reads and writes until the rebuild finishes. Since PG12, `REINDEX INDEX CONCURRENTLY` / `REINDEX TABLE CONCURRENTLY` rebuild online with only a SHARE UPDATE EXCLUSIVE lock.

<sub>`dsl-analysis/src/rules/reindex_not_concurrent.rs`</sub>

### `sql591` — `VALUES (1, 2), (3, 4, 5)`

`VALUES (1, 2), (3, 4, 5)` -- the rows of a multi-row VALUES list have different lengths. Postgres rejects this with 21000 ("VALUES lists must all be the same length"). Usually a missing or extra column in one row. (sql038 checks each tuple against the INSERT column list; this checks the tuples against each other.)

<sub>`dsl-analysis/src/rules/values_inconsistent_length.rs`</sub>

### `sql592` — `WHERE 1` / `WHERE 0`

`WHERE 1` / `WHERE 0` -- a bare integer where a boolean is required. This is a MySQL idiom (`WHERE 1` = always true); PostgreSQL has a real boolean type and rejects it with 42804 ("argument of WHERE must be type boolean, not type integer"). Use `WHERE true` / `WHERE false`, or a real predicate.

<sub>`dsl-analysis/src/rules/where_bare_integer.rs`</sub>

### `sql593` — `LIMIT 10, 20`

`LIMIT 10, 20` -- MySQL's `LIMIT offset, count` syntax. PostgreSQL doesn't accept it and raises a syntax error; the equivalent is `LIMIT 20 OFFSET 10` (note the order flips -- count first). A common slip when porting MySQL queries.

<sub>`dsl-analysis/src/rules/mysql_limit_comma.rs`</sub>

### `sql594` — `INSERT ... ON DUPLICATE KEY UPDATE ...`

`INSERT ... ON DUPLICATE KEY UPDATE ...` -- MySQL's upsert syntax. PostgreSQL doesn't have it; the equivalent is `INSERT ... ON CONFLICT (<conflict columns>) DO UPDATE SET ...` (or `DO NOTHING`). Note PG requires you to name the conflict target.

<sub>`dsl-analysis/src/rules/mysql_on_duplicate_key.rs`</sub>

### `sql595` — `REPLACE INTO t ...`

`REPLACE INTO t ...` -- MySQL's REPLACE statement (a DELETE of any conflicting row followed by an INSERT). PostgreSQL has no REPLACE; use `INSERT ... ON CONFLICT (<cols>) DO UPDATE SET ...` for a true upsert, or an explicit DELETE + INSERT if you really want the delete-then-insert semantics (which also fire ON DELETE triggers / cascades).

<sub>`dsl-analysis/src/rules/mysql_replace_into.rs`</sub>

### `sql596` — MySQL-only functions that don't exist in PostgreSQL

MySQL-only functions that don't exist in PostgreSQL -- e.g. `GROUP_CONCAT`, `DATE_FORMAT`, `STR_TO_DATE`, `UNIX_TIMESTAMP`. PG raises 42883 ("function ... does not exist"). Each has a standard PG counterpart. (NULL-coalesce functions like IFNULL/NVL are sql319's job.)

<sub>`dsl-analysis/src/rules/mysql_functions.rs`</sub>

### `sql597` — `col REGEXP 'pat'` / `col RLIKE 'pat'`

`col REGEXP 'pat'` / `col RLIKE 'pat'` -- MySQL's regex-match operators. PostgreSQL doesn't have them; use the POSIX regex operators `~` (case-sensitive), `~*` (case-insensitive), and `!~` / `!~*` for the negated forms. Word-bounded so `regexp_match` / `regexp_replace` (real PG functions) aren't touched.

<sub>`dsl-analysis/src/rules/mysql_regexp_operator.rs`</sub>

### `sql598` — `USE mydb`

`USE mydb` -- the MySQL / SQL Server command to switch the current database. PostgreSQL has no `USE` statement: a connection is bound to one database for its lifetime. Switch with the psql meta-command `\c dbname`, or point the connection string at the target database.

<sub>`dsl-analysis/src/rules/use_statement.rs`</sub>

### `sql599` — `int unsigned` / `bigint unsigned`

`int unsigned` / `bigint unsigned` -- MySQL's UNSIGNED integer modifier. PostgreSQL has no unsigned integer types and rejects the keyword. To enforce non-negativity, keep the signed type and add `CHECK (col >= 0)`, or step up to a wider type (`bigint` for an unsigned `int`) if you need the extra range.

<sub>`dsl-analysis/src/rules/mysql_unsigned.rs`</sub>

### `sql600` — `` `col` ``

`` `col` `` -- backtick-quoted identifiers. Backticks are MySQL's identifier quoting; PostgreSQL quotes identifiers with double quotes (`"col"`) and rejects backticks as a syntax error. Backticks that appear inside a single-quoted string literal are skipped (there they're ordinary characters, not identifier delimiters).

<sub>`dsl-analysis/src/rules/backtick_identifier.rs`</sub>

### `sql601` — `VARCHAR2(n)` / `NVARCHAR2(n)`

`VARCHAR2(n)` / `NVARCHAR2(n)` -- Oracle's variable-length string types. PostgreSQL doesn't have them; use `varchar(n)` (or `text` for no length cap). Word-bounded so an identifier such as `nvarchar2_col` isn't matched.

<sub>`dsl-analysis/src/rules/oracle_varchar2.rs`</sub>

### `sql602` — `DECODE(expr, search, result [, ...] [, default])`

`DECODE(expr, search, result [, ...] [, default])` -- Oracle's if-then-else function. PostgreSQL has no such function; use a `CASE` expression (or `COALESCE`/`NULLIF` for the simple cases). PostgreSQL *does* have a two-argument `decode(text, format)` for binary decoding (base64/hex), so only calls with three or more top-level arguments -- the Oracle signature -- are flagged.

<sub>`dsl-analysis/src/rules/oracle_decode.rs`</sub>

### `sql603` — `... MINUS ...`

`... MINUS ...` -- Oracle's set-difference operator. PostgreSQL spells it `EXCEPT` (and `EXCEPT ALL` to keep duplicates). Word-bounded so an identifier like `minus_balance` isn't matched.

<sub>`dsl-analysis/src/rules/oracle_minus.rs`</sub>

### `sql604` — `CLOB` / `NCLOB`

`CLOB` / `NCLOB` -- Oracle's large-character-object types. PostgreSQL has no length-limited character LOBs; `text` holds strings of any size. (Oracle `BLOB` -> PG `bytea` is handled with the MySQL BLOB lint.) Word-bounded so identifiers like `nclob_data` aren't matched.

<sub>`dsl-analysis/src/rules/oracle_lob_types.rs`</sub>

### `sql605` — an inline foreign-key column declared `NOT NULL` but with an `ON DELETE SET NULL` / `ON UPDATE SET NULL`...

an inline foreign-key column declared `NOT NULL` but with an `ON DELETE SET NULL` / `ON UPDATE SET NULL` referential action. When the referenced row changes, PostgreSQL tries to write NULL into the column and the NOT NULL constraint rejects it at runtime -- the cascade can never succeed. Drop the NOT NULL, or use `SET DEFAULT` / `RESTRICT` / `CASCADE`.

<sub>`dsl-analysis/src/rules/fk_set_null_not_null.rs`</sub>

### `sql606` — a `CHECK` constraint whose expression contains a subquery (e.g

a `CHECK` constraint whose expression contains a subquery (e.g. `CHECK (col IN (SELECT ...))`). PostgreSQL forbids subqueries in CHECK expressions and rejects the statement (0A000). Enforce cross-row rules with a trigger or a foreign key instead.

<sub>`dsl-analysis/src/rules/check_subquery.rs`</sub>

### `sql607` — a length/precision modifier on a type that doesn't accept one

a length/precision modifier on a type that doesn't accept one -- e.g. `text(50)`, `bytea(16)`, `jsonb(1)`, `boolean(1)`, `uuid(16)`. PostgreSQL rejects the statement ("type modifier is not allowed for type ..."). These types are unbounded (or fixed-width); drop the modifier, or use `varchar(n)` if you actually want a length-limited string. Only a purely numeric modifier is treated as an error, so a same-named constructor call with non-numeric arguments is never misread.

<sub>`dsl-analysis/src/rules/type_no_modifier.rs`</sub>

### `sql608` — `CREATE UNIQUE INDEX

`CREATE UNIQUE INDEX ... USING <am>` where `<am>` is a non-B-tree access method (hash, gin, gist, brin, spgist). Only B-tree supports unique indexes; PostgreSQL rejects the others with "access method \"...\" does not support unique indexes". Drop UNIQUE, or drop the USING clause to get the default B-tree.

<sub>`dsl-analysis/src/rules/unique_index_non_btree.rs`</sub>

### `sql609` — `SELECT DISTINCT ... FOR UPDATE`

`SELECT DISTINCT ... FOR UPDATE` -- PostgreSQL raises 0A000 "FOR UPDATE is not allowed with DISTINCT clause" at parse time. Row locking needs a plain row source; a DISTINCT (which collapses rows) has no single row to lock. Drop DISTINCT, or lock in a separate query over the base table.

<sub>`dsl-analysis/src/rules/for_update_distinct.rs`</sub>

### `sql610` — `SELECT ... OVER (...) ... FOR UPDATE`

`SELECT ... OVER (...) ... FOR UPDATE` -- PostgreSQL raises 0A000 "FOR UPDATE is not allowed with window functions" at parse time. A window function computes over a frame of rows, so there's no single base row to lock. Drop the lock, or compute the window in a subquery and lock the outer plain query.

<sub>`dsl-analysis/src/rules/for_update_window.rs`</sub>

### `sql611` — `UPDATE ... ORDER BY` / `DELETE ... ORDER BY`

`UPDATE ... ORDER BY` / `DELETE ... ORDER BY` -- PostgreSQL's UPDATE and DELETE don't accept a top-level ORDER BY (it raises 42601 at parse). MySQL allows `ORDER BY ... LIMIT` on UPDATE/DELETE; this is a common port mistake. To affect a bounded, ordered subset, target rows via a subquery: `DELETE FROM t WHERE ctid IN (SELECT ctid FROM t ORDER BY ... LIMIT n)`. Only a depth-0 ORDER BY is flagged, so an ORDER BY inside a subquery (e.g. `SET x = (SELECT ... ORDER BY ... LIMIT 1)`) is left alone.

<sub>`dsl-analysis/src/rules/update_delete_order_by.rs`</sub>

### `sql612` — an aggregate function in a `RETURNING` list

an aggregate function in a `RETURNING` list -- e.g. `INSERT ... RETURNING count(*)`. PostgreSQL forbids set functions in RETURNING and raises 42803 ("aggregate functions are not allowed in RETURNING"). RETURNING yields one row per affected row, so there's nothing to aggregate over; wrap the DML in a CTE and aggregate the result instead. Only aggregates at the RETURNING level (paren depth 0) are flagged, so an aggregate inside a scalar subquery in the list is left alone.

<sub>`dsl-analysis/src/rules/returning_aggregate.rs`</sub>

### `sql613` — `col ... GENERATED ALWAYS AS (expr)` without the `STORED` keyword (or written `... VIRTUAL`). PostgreSQL only...

`col ... GENERATED ALWAYS AS (expr)` without the `STORED` keyword (or written `... VIRTUAL`). PostgreSQL only supports STORED generated columns; a missing or VIRTUAL specification is a syntax error ("only STORED generated columns are supported"). MySQL and SQL Server default to virtual columns, so this is a frequent port mistake -- append `STORED`.

<sub>`dsl-analysis/src/rules/generated_column_not_stored.rs`</sub>

### `sql614` — a MySQL-style inline `KEY ...` / `INDEX ...` definition inside `CREATE TABLE`. PostgreSQL doesn't allow...

a MySQL-style inline `KEY ...` / `INDEX ...` definition inside `CREATE TABLE`. PostgreSQL doesn't allow secondary indexes in the table body -- only PRIMARY KEY / UNIQUE / FOREIGN KEY constraints -- and rejects the statement. Create the index separately with `CREATE INDEX ... ON t (...)` after the table.

<sub>`dsl-analysis/src/rules/mysql_inline_index.rs`</sub>

### `sql615` — the `WITH OIDS` table option

the `WITH OIDS` table option. System OID columns on user tables were removed in PostgreSQL 12, and `WITH OIDS` (in CREATE TABLE or `ALTER TABLE ... SET WITH OIDS`) now raises 42601. Remove the clause; if you relied on a hidden row identifier, add an explicit identity/serial column.

<sub>`dsl-analysis/src/rules/with_oids.rs`</sub>

### `sql616` — a MySQL `CHARACTER SET ...` / `CHARSET=...` clause (per-column or per-table). PostgreSQL has no per-column or...

a MySQL `CHARACTER SET ...` / `CHARSET=...` clause (per-column or per-table). PostgreSQL has no per-column or per-table character sets -- the encoding is fixed per database -- so these clauses are syntax errors (42601). Drop them; use `COLLATE "..."` for collation, and set the encoding at `CREATE DATABASE ... ENCODING` time. `CHARACTER SET` is matched as a two-word phrase so it never collides with `CHARACTER VARYING` (a valid spelling of `varchar`).

<sub>`dsl-analysis/src/rules/mysql_character_set.rs`</sub>

### `sql617` — `NATURAL JOIN` (and `NATURAL LEFT/RIGHT/FULL JOIN`). A natural join implicitly joins on *every* pair of...

`NATURAL JOIN` (and `NATURAL LEFT/RIGHT/FULL JOIN`). A natural join implicitly joins on *every* pair of same-named columns, so adding, renaming, or dropping a column on either side silently changes the join condition -- a frequent source of surprise breakage. Spell the join out with `JOIN ... ON ...` or `JOIN ... USING (...)`. Complements sql064 (`JOIN` without `ON`), which deliberately skips NATURAL.

<sub>`dsl-analysis/src/rules/natural_join.rs`</sub>

### `sql618` — `FETCH FIRST n ROWS WITH TIES` without an `ORDER BY`

`FETCH FIRST n ROWS WITH TIES` without an `ORDER BY`. WITH TIES returns the extra rows that tie with the last row *according to the ORDER BY*; with no ORDER BY there's no defined ordering, so PostgreSQL rejects it (42601, "WITH TIES cannot be specified without ORDER BY clause"). Add an ORDER BY, or use plain `ROWS ONLY` / `LIMIT`. Conservative: only flags when the statement has no ORDER BY at all.

<sub>`dsl-analysis/src/rules/with_ties_no_order.rs`</sub>

### `sql619` — `date_trunc('<unit>', ...)` where `<unit>` is a string literal that isn't one of PostgreSQL's recognised...

`date_trunc('<unit>', ...)` where `<unit>` is a string literal that isn't one of PostgreSQL's recognised fields. At runtime PG raises 22023 ("unit \"...\" not recognized for type timestamp..."). Catches typos like `'minutes'` (plural) or `'mon'` before they reach production. Only a literal first argument is checked; a column/parameter unit is left alone.

<sub>`dsl-analysis/src/rules/date_trunc_invalid_unit.rs`</sub>

### `sql620` — MySQL / SQL Server date arithmetic functions that don't exist in PostgreSQL

MySQL / SQL Server date arithmetic functions that don't exist in PostgreSQL -- `DATEDIFF`, `DATEADD`, `TIMESTAMPDIFF`, `DATEPART`. PG raises 42883 ("function ... does not exist"). PostgreSQL does date math with native operators and EXTRACT instead. (Complements sql596 / the non-PG date fns rule, which cover GETDATE/SYSDATE/DATE_FORMAT and friends.)

<sub>`dsl-analysis/src/rules/non_pg_date_diff_fns.rs`</sub>

### `sql621` — the MySQL `IF(cond, then, else)` function

the MySQL `IF(cond, then, else)` function. PostgreSQL has no scalar `IF()` function (42883); use a `CASE WHEN cond THEN ... ELSE ... END` expression (or `COALESCE` / `NULLIF` for the simple shapes). Carefully distinguished from PL/pgSQL's `IF ... THEN` control statement: the function form has a comma-separated argument list and is *not* followed by `THEN`.

<sub>`dsl-analysis/src/rules/mysql_if_function.rs`</sub>

### `sql622` — MySQL-only string functions that don't exist in PostgreSQL

MySQL-only string functions that don't exist in PostgreSQL -- `LCASE`, `UCASE`, `SUBSTRING_INDEX`, `FIND_IN_SET`. PG raises 42883; each has a standard PG counterpart. Complements sql596 (GROUP_CONCAT/DATE_FORMAT/...) and sql620 (DATEDIFF/...).

<sub>`dsl-analysis/src/rules/mysql_string_functions.rs`</sub>

### `sql623` — a MySQL inline `ENUM('a','b',...)` column type

a MySQL inline `ENUM('a','b',...)` column type. PostgreSQL has no inline enum: you declare a named type with `CREATE TYPE x AS ENUM (...)` and reference it, or model the constraint with `CHECK (col IN ('a','b'))`. The inline form is a syntax error in PG (42601). `ENUM(` is matched word-bounded; PostgreSQL never uses `ENUM(...)` as an expression, so there's nothing legitimate to confuse it with.

<sub>`dsl-analysis/src/rules/mysql_enum_inline.rs`</sub>

### `sql624` — the MySQL column attribute `ON UPDATE CURRENT_TIMESTAMP` (auto-touch a timestamp column on every row update)

the MySQL column attribute `ON UPDATE CURRENT_TIMESTAMP` (auto-touch a timestamp column on every row update). PostgreSQL has no such column attribute and rejects it; implement it with a `BEFORE UPDATE` trigger that sets the column to `now()`. Only `ON UPDATE` immediately followed by a current-time function is flagged, so foreign-key actions (`ON UPDATE CASCADE` / `SET NULL` / `RESTRICT` / `NO ACTION`) are left alone.

<sub>`dsl-analysis/src/rules/mysql_on_update_timestamp.rs`</sub>

### `sql625` — the MySQL `ZEROFILL` column attribute (left-pads a numeric column with zeros on display, and implies...

the MySQL `ZEROFILL` column attribute (left-pads a numeric column with zeros on display, and implies UNSIGNED). PostgreSQL has no display attributes -- storage and presentation are separate -- so the keyword is a syntax error. Format with `to_char(n, 'FM0000')` / `lpad(...)` at query time instead. Sibling of the UNSIGNED lint.

<sub>`dsl-analysis/src/rules/mysql_zerofill.rs`</sub>

### `sql626` — MySQL-only query modifiers / hints that have no PostgreSQL equivalent and are syntax errors in PG

MySQL-only query modifiers / hints that have no PostgreSQL equivalent and are syntax errors in PG -- `SQL_CALC_FOUND_ROWS`, `STRAIGHT_JOIN`, `SQL_NO_CACHE`, `SQL_CACHE`, `HIGH_PRIORITY`, `LOW_PRIORITY`, `DELAYED`. These are all word-bounded, single-token keywords that PostgreSQL never uses, so they're safe to flag on sight.

<sub>`dsl-analysis/src/rules/mysql_query_modifiers.rs`</sub>

### `sql627` — the MySQL infix operators `XOR` (logical exclusive-or) and `DIV` (integer division)

the MySQL infix operators `XOR` (logical exclusive-or) and `DIV` (integer division). Neither is a PostgreSQL operator, so both raise a syntax error. Replace `a XOR b` with `a <> b` (booleans) or `a # b` (bitwise), and `a DIV b` with `a / b` (integer operands) or `div(a, b)`.

<sub>`dsl-analysis/src/rules/mysql_xor_div.rs`</sub>

### `sql628` — scalar functions from Oracle / SQL Server / MySQL that don't exist in PostgreSQL

scalar functions from Oracle / SQL Server / MySQL that don't exist in PostgreSQL -- `LISTAGG`, `INSTR`, `CHARINDEX`, `IIF`, `NVL2`, `LEN`. PG raises 42883; each has a standard PG counterpart. Complements sql596 / sql620 / sql622 (other non-PG functions) and the NVL / ISNULL / IFNULL lint (sql628 adds the three-argument `NVL2`, which that rule doesn't cover).

<sub>`dsl-analysis/src/rules/cross_dialect_scalar_fns.rs`</sub>

### `sql629` — SQL Server (T-SQL) data types that don't exist in PostgreSQL

SQL Server (T-SQL) data types that don't exist in PostgreSQL -- `NVARCHAR`, `NCHAR`, `DATETIME2`, `DATETIMEOFFSET`, `SMALLDATETIME`, `UNIQUEIDENTIFIER`, `VARBINARY`, `NTEXT`, `IMAGE`, `SYSNAME`. PG rejects them (42704). Each maps onto a native PG type. Complements the MySQL-types (sql316) and Oracle-types lints.

<sub>`dsl-analysis/src/rules/tsql_types.rs`</sub>

### `sql630` — SQL Server (T-SQL) identity / GUID functions that don't exist in PostgreSQL

SQL Server (T-SQL) identity / GUID functions that don't exist in PostgreSQL -- `NEWID`, `NEWSEQUENTIALID`, `SCOPE_IDENTITY`, `IDENT_CURRENT`. PG raises 42883. Generate UUIDs with `gen_random_uuid()`, and read a freshly inserted serial/identity value with `RETURNING`, `lastval()`, or `currval('seq')`.

<sub>`dsl-analysis/src/rules/tsql_identity_fns.rs`</sub>

### `sql631` — `last_value(...)` / `nth_value(...)` over a window that has an `ORDER BY` but no explicit frame clause. The...

`last_value(...)` / `nth_value(...)` over a window that has an `ORDER BY` but no explicit frame clause. The default frame is `RANGE BETWEEN UNBOUNDED PRECEDING AND CURRENT ROW`, so `last_value` returns the *current* row's value, not the partition's last -- a classic footgun. Add an explicit frame, e.g. `ROWS BETWEEN UNBOUNDED PRECEDING AND UNBOUNDED FOLLOWING`. `first_value` is intentionally not flagged: the default frame already starts at the partition start, so it returns the correct value.

<sub>`dsl-analysis/src/rules/last_value_default_frame.rs`</sub>

### `sql632` — the server-side large-object file functions `lo_import('path')` and `lo_export(oid, 'path')`. They read/write...

the server-side large-object file functions `lo_import('path')` and `lo_export(oid, 'path')`. They read/write files on the *database server's* filesystem with the postgres OS user's privileges and require superuser -- a privilege-escalation and data-exfiltration vector. Move bytes through the client (`\lo_import` / `\lo_export` in psql, or bytea over the wire) instead.

<sub>`dsl-analysis/src/rules/large_object_file_access.rs`</sub>

### `sql633` — server-side filesystem functions `pg_read_file`, `pg_read_binary_file`, `pg_ls_dir`, and `pg_stat_file`

server-side filesystem functions `pg_read_file`, `pg_read_binary_file`, `pg_ls_dir`, and `pg_stat_file`. They expose the database server's filesystem (reading arbitrary files / listing directories) with the postgres OS user's privileges and are restricted to superusers and the `pg_read_server_files` role. Exposing them through application SQL is a data-exfiltration / privilege-escalation vector. Complements sql632 (lo_import / lo_export).

<sub>`dsl-analysis/src/rules/server_file_read_fns.rs`</sub>

### `sql634` — `gen_salt('md5' | 'des' | 'xdes')` from pgcrypto. These algorithms are weak for password hashing

`gen_salt('md5' | 'des' | 'xdes')` from pgcrypto. These algorithms are weak for password hashing -- DES truncates to 8 characters and MD5 is fast and broken. Use `gen_salt('bf', <rounds>)` (Blowfish/bcrypt) so each hash is deliberately slow to brute-force.

<sub>`dsl-analysis/src/rules/weak_gen_salt.rs`</sub>

### `sql635` — a `PRAGMA ...` statement

a `PRAGMA ...` statement. PRAGMA is SQLite's mechanism for reading and setting database options (e.g. `PRAGMA foreign_keys = ON`). PostgreSQL has no PRAGMA and rejects it; configure the session with `SET`, the cluster with `ALTER SYSTEM` / `postgresql.conf`, and note that foreign keys are always enforced in PG.

<sub>`dsl-analysis/src/rules/sqlite_pragma.rs`</sub>

### `sql636` — the SQLite `AUTOINCREMENT` keyword (one word, e.g

the SQLite `AUTOINCREMENT` keyword (one word, e.g. `id INTEGER PRIMARY KEY AUTOINCREMENT`). PostgreSQL doesn't accept it; use `GENERATED ALWAYS AS IDENTITY` (preferred) or `serial`/`bigserial`. Note a plain `bigint PRIMARY KEY GENERATED ... AS IDENTITY` already auto-assigns without SQLite's rowid-reuse semantics. Sibling of the MySQL AUTO_INCREMENT lint (sql314).

<sub>`dsl-analysis/src/rules/sqlite_autoincrement.rs`</sub>

### `sql637` — the SQLite `GLOB` operator (case-sensitive, Unix-glob pattern match, e.g

the SQLite `GLOB` operator (case-sensitive, Unix-glob pattern match, e.g. `name GLOB 'foo*'`). PostgreSQL has no GLOB operator. Use `LIKE` with `%`/`_` wildcards (case-sensitive by default), or the POSIX regex operator `~` / `~*`.

<sub>`dsl-analysis/src/rules/sqlite_glob.rs`</sub>

### `sql638` — SQLite-only functions that don't exist in PostgreSQL

SQLite-only functions that don't exist in PostgreSQL -- `STRFTIME`, `JULIANDAY`, `TYPEOF`, `PRINTF`, `LAST_INSERT_ROWID`. PG raises 42883; each has a native counterpart. Complements sql596 / sql620 / sql622 / sql628 / sql630 (other non-PG functions).

<sub>`dsl-analysis/src/rules/sqlite_functions.rs`</sub>

### `sql639` — more cross-dialect string functions absent from PostgreSQL

more cross-dialect string functions absent from PostgreSQL -- `HEX`, `UNHEX` (MySQL/SQLite), `SPACE` (MySQL/T-SQL), `QUOTE` (MySQL/SQLite). PG raises 42883; each has a native counterpart. Complements sql622 / sql628.

<sub>`dsl-analysis/src/rules/cross_dialect_string_fns2.rs`</sub>

### `sql640` — MySQL date-part functions with no PostgreSQL equivalent

MySQL date-part functions with no PostgreSQL equivalent -- `DAYOFWEEK`, `DAYOFMONTH`, `DAYOFYEAR`, `WEEKDAY`, `WEEKOFYEAR`, `MONTHNAME`, `DAYNAME`, `QUARTER`. PG raises 42883; extract the field with `EXTRACT(field FROM ts)` / `date_part(...)`, and format names with `to_char(ts, 'Month')` / `to_char(ts, 'Day')`.

<sub>`dsl-analysis/src/rules/mysql_datepart_fns.rs`</sub>

### `sql641` — a `DEFAULT` of the special relative date/time strings `'now'`, `'today'`, `'tomorrow'`, or `'yesterday'`....

a `DEFAULT` of the special relative date/time strings `'now'`, `'today'`, `'tomorrow'`, or `'yesterday'`. PostgreSQL resolves these special values *when the default expression is created* (at DDL time), not at each INSERT -- so `created_at timestamptz DEFAULT 'now'` freezes every row to the moment the table was defined. Use the functions `now()` / `CURRENT_TIMESTAMP` / `CURRENT_DATE`, which are evaluated per row.

<sub>`dsl-analysis/src/rules/default_relative_datetime_string.rs`</sub>

### `sql642` — MySQL file-I/O syntax

MySQL file-I/O syntax -- `SELECT ... INTO OUTFILE 'path'`, `... INTO DUMPFILE 'path'`, and `LOAD DATA [LOCAL] INFILE 'path' INTO TABLE ...`. PostgreSQL has none of these; bulk-load and dump with `COPY` (server-side, superuser) or `\copy` (client-side, in psql). They also read and write files on the database server, so they're a file-access vector as well as a syntax error.

<sub>`dsl-analysis/src/rules/mysql_file_io.rs`</sub>

### `sql643` — Oracle scalar functions absent from PostgreSQL

Oracle scalar functions absent from PostgreSQL -- `ADD_MONTHS`, `MONTHS_BETWEEN`, `NEXT_DAY`, `SYS_GUID`, `BITAND`. PG raises 42883; each has a native counterpart (interval arithmetic, `age()`, `gen_random_uuid()`, the `&` operator). Complements the Oracle DECODE / ROWNUM / DUAL lints.

<sub>`dsl-analysis/src/rules/oracle_date_misc_fns.rs`</sub>

### `sql644` — MySQL date-arithmetic functions absent from PostgreSQL

MySQL date-arithmetic functions absent from PostgreSQL -- `ADDDATE`, `SUBDATE`, `ADDTIME`, `SUBTIME`, `LAST_DAY`, `SEC_TO_TIME`, `TIME_TO_SEC`, `MAKEDATE`, `MAKETIME`. PG raises 42883; do the math with `interval` arithmetic, `date_trunc`, `extract`, and `make_date` / `make_time` (note the underscores -- those *are* PG functions). Complements sql640.

<sub>`dsl-analysis/src/rules/mysql_date_arith_fns.rs`</sub>

### `sql645` — a set-returning function (`generate_series`, `unnest`, `jsonb_array_elements`, `regexp_split_to_table`...

a set-returning function (`generate_series`, `unnest`, `jsonb_array_elements`, `regexp_split_to_table`, `json_each`, ...) called directly in a `WHERE` clause. PostgreSQL forbids SRFs outside the SELECT list and FROM clause and raises 0A000 ("set-returning functions are not allowed in WHERE"). Move the SRF into the FROM clause (often `LATERAL`) or a subquery and filter its output. SRFs inside a `(SELECT ...)` / `(WITH ...)` subquery within the WHERE are legal and are skipped.

<sub>`dsl-analysis/src/rules/srf_in_where.rs`</sub>

### `sql646` — `count(DISTINCT *)` (or any aggregate with `DISTINCT *`). PostgreSQL doesn't support `DISTINCT *` inside an...

`count(DISTINCT *)` (or any aggregate with `DISTINCT *`). PostgreSQL doesn't support `DISTINCT *` inside an aggregate and raises a syntax error -- `count(DISTINCT col)` needs an explicit column (or list of columns), and `count(*)` already counts every row. This is a frequent mistranslation of "count the distinct rows".

<sub>`dsl-analysis/src/rules/aggregate_distinct_star.rs`</sub>

### `sql647` — `<col> IN (SELECT a, b,

`<col> IN (SELECT a, b, ... FROM ...)` where the subquery's SELECT list has more than one column. An `IN` (or `= ANY`) subquery on a scalar left-hand side must return exactly one column; PostgreSQL raises 42601 ("subquery must return only one column"). Either select a single column, or use a row constructor on the left (`(a, b) IN (SELECT a, b ...)`). Conservative: only fires when the left-hand side is a bare column reference (so row constructors and function-call LHS are never misread) and the SELECT list has an explicit top-level comma.

<sub>`dsl-analysis/src/rules/in_subquery_multi_column.rs`</sub>

### `sql648` — `TABLESAMPLE SYSTEM (p)` / `TABLESAMPLE BERNOULLI (p)` where the literal sampling percentage `p` is outside `0

`TABLESAMPLE SYSTEM (p)` / `TABLESAMPLE BERNOULLI (p)` where the literal sampling percentage `p` is outside `0 .. 100`. PostgreSQL requires the argument to be a percentage in that range and raises 22003 ("sample percentage must be between 0 and 100") at run time.

<sub>`dsl-analysis/src/rules/tablesample_out_of_range.rs`</sub>

### `sql649` — `INSERT ... ON CONFLICT DO UPDATE ...` with no conflict target. `DO UPDATE` needs to know *which* unique...

`INSERT ... ON CONFLICT DO UPDATE ...` with no conflict target. `DO UPDATE` needs to know *which* unique index or constraint the conflict is on, so PostgreSQL requires an inference clause -- `ON CONFLICT (col) DO UPDATE` or `ON CONFLICT ON CONSTRAINT name DO UPDATE` -- and otherwise raises 42601 ("ON CONFLICT DO UPDATE requires inference specification or constraint name"). (`ON CONFLICT DO NOTHING` may omit the target and is left alone.)

<sub>`dsl-analysis/src/rules/on_conflict_do_update_no_target.rs`</sub>

### `sql650` — a row-constructor comparison with unequal arity, e.g

a row-constructor comparison with unequal arity, e.g. `(a, b) = (1, 2, 3)`. Comparing two row constructors requires the same number of fields on each side; PostgreSQL raises 42601 ("unequal number of entries in row expressions"). Conservative: only bare parenthesised lists (not `func(...)` calls) on both sides of `=` / `<>` are considered, and each side must contain a top-level comma (so it's genuinely a row, not a parenthesised scalar).

<sub>`dsl-analysis/src/rules/row_comparison_arity.rs`</sub>

### `sql651` — a set-returning function (`generate_series`, `unnest`, `jsonb_array_elements`, ...) in a `GROUP BY`...

a set-returning function (`generate_series`, `unnest`, `jsonb_array_elements`, ...) in a `GROUP BY`, `HAVING`, or `ORDER BY` clause. Like WHERE (sql645), these clauses don't allow SRFs; PostgreSQL raises 0A000. Put the SRF in the FROM clause (often `LATERAL`) and group/order/filter over its output. SRFs inside a `(SELECT ...)` subquery are legal and skipped.

<sub>`dsl-analysis/src/rules/srf_in_group_order.rs`</sub>

### `sql652` — two common-table expressions in the same `WITH` clause share a name, e.g

two common-table expressions in the same `WITH` clause share a name, e.g. `WITH a AS (...), a AS (...)`. PostgreSQL requires CTE names to be unique within a WITH list and raises 42712 ("WITH query name "a" specified more than once"). Rename one of them.

<sub>`dsl-analysis/src/rules/duplicate_cte_name.rs`</sub>

### `sql653` — an aggregate function inside a `CHECK` constraint, e.g

an aggregate function inside a `CHECK` constraint, e.g. `CHECK (count(*) > 0)`. A CHECK constraint is evaluated per row and cannot see other rows, so PostgreSQL forbids aggregates there and raises 42803 ("aggregate functions are not allowed in check constraints"). Enforce cross-row invariants with a trigger or a separate constraint mechanism. Complements sql606 (subquery in CHECK).

<sub>`dsl-analysis/src/rules/aggregate_in_check.rs`</sub>

### `sql654` — an aggregate function in a `CREATE INDEX` expression, e.g

an aggregate function in a `CREATE INDEX` expression, e.g. `CREATE INDEX ON t (count(x))` or a partial-index predicate `... WHERE sum(x) > 0`. An index expression is evaluated per row, so PostgreSQL forbids aggregates and raises 42803 ("aggregate functions are not allowed in index expressions" / predicates). Index a plain column or an immutable scalar expression instead.

<sub>`dsl-analysis/src/rules/aggregate_in_index.rs`</sub>

### `sql655` — a multi-column UPDATE assignment whose column list and value list have different lengths, e.g

a multi-column UPDATE assignment whose column list and value list have different lengths, e.g. `UPDATE t SET (a, b) = (1, 2, 3)`. PostgreSQL requires the two lists to match and raises 42601 ("number of columns does not match number of values"). Only a literal value list `(...)` is checked; a sub-SELECT value source (`SET (a, b) = (SELECT ...)`) is skipped.

<sub>`dsl-analysis/src/rules/update_set_arity.rs`</sub>

### `sql656` — a `TRUNCATE` statement with a `WHERE` clause. TRUNCATE removes *all* rows of a table and accepts no row filter

a `TRUNCATE` statement with a `WHERE` clause. TRUNCATE removes *all* rows of a table and accepts no row filter -- `TRUNCATE t WHERE ...` is a syntax error (42601). The mistake usually means a conditional `DELETE FROM t WHERE ...` was intended (TRUNCATE can't be filtered).

<sub>`dsl-analysis/src/rules/truncate_with_where.rs`</sub>

### `sql657` — an `ORDER BY` that appears after `LIMIT` / `OFFSET` / `FETCH` at the top level of a query

an `ORDER BY` that appears after `LIMIT` / `OFFSET` / `FETCH` at the top level of a query. SQL fixes the clause order as `... ORDER BY ... LIMIT ... OFFSET ...`, so `LIMIT 5 ORDER BY x` is a syntax error (42601). The author almost certainly meant to order *then* limit. Depth-0 only, so an inner subquery's ORDER BY before the outer LIMIT is fine.

<sub>`dsl-analysis/src/rules/order_by_after_limit.rs`</sub>

### `sql658` — both a `LIMIT` clause and a `FETCH FIRST/NEXT ... ROWS` clause in the same query level. They're two spellings...

both a `LIMIT` clause and a `FETCH FIRST/NEXT ... ROWS` clause in the same query level. They're two spellings of the same row-limit, and PostgreSQL allows only one -- specifying both raises 42601 ("multiple LIMIT options not allowed"). Keep one. Depth-0 only, so a subquery's LIMIT and the outer query's FETCH don't collide.

<sub>`dsl-analysis/src/rules/limit_and_fetch.rs`</sub>

### `sql659` — a `WHERE` clause that appears after `GROUP BY` at the top level of a query. SQL fixes the order as `... WHERE...

a `WHERE` clause that appears after `GROUP BY` at the top level of a query. SQL fixes the order as `... WHERE ... GROUP BY ... HAVING ...`, so `GROUP BY a WHERE b` is a syntax error (42601) -- the row filter belongs before GROUP BY (or, for post-aggregation filtering, use HAVING). Resets at set-operation boundaries, so `... GROUP BY a UNION SELECT ... WHERE b` (a second query's WHERE) is not flagged.

<sub>`dsl-analysis/src/rules/where_after_group_by.rs`</sub>

### `sql660` — a `CROSS JOIN` with an `ON` or `USING` clause

a `CROSS JOIN` with an `ON` or `USING` clause. A cross join is an unconditional Cartesian product and takes no join condition, so `a CROSS JOIN b ON ...` is a syntax error (42601). Either drop the condition (a real cross join) or change `CROSS JOIN` to `[INNER] JOIN ... ON ...`.

<sub>`dsl-analysis/src/rules/cross_join_with_on.rs`</sub>

### `sql661` — a window-only function (`row_number`, `rank`, `dense_rank`, `lag`, `lead`, `ntile`, `first_value`, ...)...

a window-only function (`row_number`, `rank`, `dense_rank`, `lag`, `lead`, `ntile`, `first_value`, ...) called without an `OVER` clause. These functions exist only as window functions, so PostgreSQL raises 42P20 ("window function ... requires an OVER clause"). Add `OVER (...)` (with the appropriate PARTITION BY / ORDER BY). Unlike `count`/`sum`/`min`/..., these names have no aggregate meaning, so a missing OVER is always an error.

<sub>`dsl-analysis/src/rules/window_fn_without_over.rs`</sub>

### `sql662` — `SELECT DISTINCT ON <expr>` without parentheses around the expression list

`SELECT DISTINCT ON <expr>` without parentheses around the expression list. The syntax is `DISTINCT ON (expr [, ...])`; the parentheses are required, so `DISTINCT ON col` is a syntax error (42601). Wrap the expression(s): `DISTINCT ON (col)`.

<sub>`dsl-analysis/src/rules/distinct_on_no_parens.rs`</sub>

### `sql663` — an `ORDER BY` / `LIMIT` / `OFFSET` / `FETCH` clause that appears before a set operation (`UNION` /...

an `ORDER BY` / `LIMIT` / `OFFSET` / `FETCH` clause that appears before a set operation (`UNION` / `INTERSECT` / `EXCEPT`) at the top level, without parentheses -- e.g. `SELECT a FROM t ORDER BY a UNION SELECT b`. Those clauses apply to the whole set operation and must come *after* it (or the individual branch must be parenthesised); otherwise PostgreSQL raises 42601 ("syntax error at or near UNION"). Wrap the branch: `(SELECT a FROM t ORDER BY a LIMIT n) UNION ...`. Complements sql268, which handles the *parenthesised* branch case.

<sub>`dsl-analysis/src/rules/tail_clause_before_setop.rs`</sub>

### `sql664` — a `HAVING` clause that appears before `GROUP BY` at the top level

a `HAVING` clause that appears before `GROUP BY` at the top level. The clause order is `... GROUP BY ... HAVING ...`, so `HAVING x GROUP BY y` is a syntax error (42601). Move HAVING after GROUP BY. Resets at set-operation boundaries so a second query's GROUP BY isn't misread.

<sub>`dsl-analysis/src/rules/having_before_group_by.rs`</sub>

### `sql665` — an `UPDATE` whose `WHERE` clause comes before `SET`, e.g

an `UPDATE` whose `WHERE` clause comes before `SET`, e.g. `UPDATE t WHERE id = 1 SET x = 2`. The required order is `UPDATE t SET ... WHERE ...`; writing WHERE first is a syntax error (42601). Depth-0 only, so a `WHERE` inside a `SET x = (SELECT ... WHERE ...)` subquery is fine.

<sub>`dsl-analysis/src/rules/update_where_before_set.rs`</sub>

### `sql666` — `INSERT IGNORE INTO ...`

`INSERT IGNORE INTO ...` -- MySQL's modifier that silently skips rows which would violate a unique/PK constraint (and downgrades other errors to warnings). PostgreSQL has no `INSERT IGNORE`; express the intent explicitly with `INSERT ... ON CONFLICT DO NOTHING`, which skips only conflicting rows and still raises real errors.

<sub>`dsl-analysis/src/rules/mysql_insert_ignore.rs`</sub>

### `sql667` — MySQL's `INSERT INTO t SET a = 1, b = 2` assignment-list syntax

MySQL's `INSERT INTO t SET a = 1, b = 2` assignment-list syntax. PostgreSQL's INSERT uses a column list and `VALUES` (or a `SELECT`): `INSERT INTO t (a, b) VALUES (1, 2)`. The `SET` form is a syntax error in PG. Only a `SET` reached before any `VALUES` / `SELECT` / `ON CONFLICT` is flagged, so the legitimate `... ON CONFLICT ... DO UPDATE SET ...` is left alone.

<sub>`dsl-analysis/src/rules/mysql_insert_set.rs`</sub>

### `sql668` — a `DELETE` whose first token isn't `FROM`, e.g

a `DELETE` whose first token isn't `FROM`, e.g. `DELETE t1 FROM t1 JOIN t2 ...` (MySQL / SQL Server multi-table delete). PostgreSQL's grammar is `DELETE FROM target [USING ...]`, so the leading table/alias is a syntax error. Rewrite as `DELETE FROM t1 USING t2 WHERE ...`.

<sub>`dsl-analysis/src/rules/delete_alias_before_from.rs`</sub>

### `sql669` — MySQL's `SELECT

MySQL's `SELECT ... LOCK IN SHARE MODE` row-locking clause. PostgreSQL spells a shared row lock `FOR SHARE` (and an exclusive one `FOR UPDATE`). `LOCK IN SHARE MODE` is a syntax error in PG.

<sub>`dsl-analysis/src/rules/mysql_lock_in_share_mode.rs`</sub>

### `sql670` — a MySQL `SHOW TABLES` / `SHOW DATABASES` / `SHOW COLUMNS` / `SHOW CREATE TABLE` /

a MySQL `SHOW TABLES` / `SHOW DATABASES` / `SHOW COLUMNS` / `SHOW CREATE TABLE` / ... introspection statement. PostgreSQL's `SHOW` only displays configuration parameters (`SHOW search_path`, `SHOW ALL`); to list schema objects use the information_schema / pg_catalog views or psql meta-commands (`\dt`, `\d table`, `\l`, `\di`).

<sub>`dsl-analysis/src/rules/mysql_show_statement.rs`</sub>

### `sql671` — a `DESCRIBE t` / `DESC t` statement (MySQL / Oracle table introspection)

a `DESCRIBE t` / `DESC t` statement (MySQL / Oracle table introspection). PostgreSQL has no such statement; inspect a table with the psql meta-command `\d table`, or query `information_schema.columns` / `pg_catalog`. Only the statement-leading form is flagged, so `ORDER BY x DESC` (the sort direction) is never touched.

<sub>`dsl-analysis/src/rules/mysql_describe.rs`</sub>

### `sql672` — MySQL's `ALTER TABLE

MySQL's `ALTER TABLE ... CHANGE [COLUMN]` / `MODIFY [COLUMN]` sub-commands. PostgreSQL spells column changes differently: `MODIFY col <type>` -> `ALTER COLUMN col TYPE <type>` (plus separate `SET/DROP DEFAULT`, `SET/DROP NOT NULL`); `CHANGE old new <type>` -> `RENAME COLUMN old TO new` *and* `ALTER COLUMN new TYPE <type>`. The MySQL keywords are syntax errors in PG.

<sub>`dsl-analysis/src/rules/mysql_alter_change_modify.rs`</sub>

### `sql673` — `x BETWEEN NULL AND y` / `x BETWEEN y AND NULL`

`x BETWEEN NULL AND y` / `x BETWEEN y AND NULL` -- a NULL bound makes the whole range test evaluate to NULL (never TRUE), so the row can never match. Almost always a missing value or a typo; supply a real bound or rewrite the predicate. (Companion to sql011 between_self_bound and the between_reversed / between_equal_bounds checks.)

<sub>`dsl-analysis/src/rules/between_null_bound.rs`</sub>

### `sql674` — a ranking window function with an explicit frame clause, e.g. `ROW_NUMBER() OVER (ORDER BY x ROWS BETWEEN...

a ranking window function with an explicit frame clause, e.g. `ROW_NUMBER() OVER (ORDER BY x ROWS BETWEEN ...)`. ROW_NUMBER, RANK, DENSE_RANK, PERCENT_RANK, CUME_DIST and NTILE assign a value from the whole partition and ignore the frame -- PG rejects the frame outright ("window function ... cannot have a window frame"). Drop the ROWS / RANGE / GROUPS clause; plain `OVER (PARTITION BY ... ORDER BY ...)` is enough.

<sub>`dsl-analysis/src/rules/ranking_fn_with_frame.rs`</sub>

### `sql675` — `SELECT DISTINCT ... UNION SELECT ...`

`SELECT DISTINCT ... UNION SELECT ...` -- a branch begins with DISTINCT but the set operation already deduplicates the combined result. Plain UNION / INTERSECT / EXCEPT remove duplicate rows, so the per-branch DISTINCT is wasted work (an extra sort/hash). Drop it -- or, if duplicates across branches should survive, switch the set op to its ALL form. Conservative: only fires when every top-level set op deduplicates (no `... ALL`), so the redundancy is unconditional. `DISTINCT ON (...)` is left alone -- it selects specific rows and is not made redundant by UNION.

<sub>`dsl-analysis/src/rules/union_branch_distinct.rs`</sub>

### `sql676` — `COUNT(DISTINCT 1)` / `COUNT(DISTINCT 'x')`

`COUNT(DISTINCT 1)` / `COUNT(DISTINCT 'x')` -- counting the distinct values of a constant. A constant has exactly one distinct value, so this returns 1 for any non-empty group (0 for an empty one) -- never what was meant. Either `COUNT(*)` (rows) or `COUNT(DISTINCT col)` (a real column) was almost certainly intended.

<sub>`dsl-analysis/src/rules/count_distinct_constant.rs`</sub>

### `sql677` — `x % 1` / `MOD(x, 1)`

`x % 1` / `MOD(x, 1)` -- the remainder of any integer divided by 1 is always 0, so the expression is a constant. Usually a typo (a different modulus was meant) or leftover from refactoring. (Companion to sql546 modulo_out_of_range and sql565 self_arithmetic.)

<sub>`dsl-analysis/src/rules/modulo_by_one.rs`</sub>

### `sql678` — the MySQL "zero date" literal `'0000-00-00'` (or `'0000-00-00 00:00:00'`). MySQL accepts it as a placeholder...

the MySQL "zero date" literal `'0000-00-00'` (or `'0000-00-00 00:00:00'`). MySQL accepts it as a placeholder for a missing date; PostgreSQL rejects it -- there is no year 0000 / month 00 / day 00, so a cast raises 22008 ("date/time field value out of range"). Use NULL for "no date", or a real sentinel like `'0001-01-01'`.

<sub>`dsl-analysis/src/rules/zero_date_literal.rs`</sub>

### `sql679` — `left(s, 0)` / `right(s, 0)`

`left(s, 0)` / `right(s, 0)` -- taking the first (or last) 0 characters of a string always yields the empty string, so the call is a constant `''`. Almost always a typo (a real length was meant) or leftover from refactoring. (Companion to sql443 substring_negative_length and sql480 substring_zero_length.)

<sub>`dsl-analysis/src/rules/left_right_zero.rs`</sub>

### `sql680` — `substring(s FROM n FOR 0)` / `substr(s, n, 0)`

`substring(s FROM n FOR 0)` / `substr(s, n, 0)` -- a length of 0 always returns the empty string, so the call is a constant `''`. Usually a typo for a real length, or a sign the length was computed to 0 by mistake. (Companion to sql443 substring_negative_length, sql479 substring_zero_start and sql679 left_right_zero.)

<sub>`dsl-analysis/src/rules/substring_zero_length.rs`</sub>

### `sql681` — `x * 0` / `0 * x`

`x * 0` / `0 * x` -- multiplying by the literal 0 is always 0 (NULL when `x` is NULL), so the expression is a constant. Almost always a typo (a different factor was meant) or a disabled term left in by mistake. (Companion to sql489 where_arith_identity and sql565 self_arithmetic.)

<sub>`dsl-analysis/src/rules/multiply_by_zero.rs`</sub>

### `sql682` — `COALESCE(COUNT(...), 0)`

`COALESCE(COUNT(...), 0)` -- COUNT never returns NULL. A grouped or scalar `count(...)` returns 0 for an empty input, not NULL, so wrapping it in COALESCE (almost always with a 0 fallback) is dead weight. Drop the COALESCE and use the COUNT directly. (Companion to sql493 coalesce_not_null for NOT NULL columns.)

<sub>`dsl-analysis/src/rules/coalesce_count_redundant.rs`</sub>

### `sql683` — `CASE WHEN TRUE THEN ...` / `CASE WHEN FALSE THEN ...`

`CASE WHEN TRUE THEN ...` / `CASE WHEN FALSE THEN ...` -- the first branch of a searched CASE tests a constant boolean. `WHEN TRUE` makes the branch unconditional (the rest of the CASE is dead); `WHEN FALSE` makes it unreachable. Either way it's a leftover debugging edit or a forgotten real condition. (Only the leading `CASE WHEN <bool>` is flagged, so simple `CASE x WHEN TRUE` value comparisons are left alone.)

<sub>`dsl-analysis/src/rules/case_constant_when.rs`</sub>

### `sql684` — `GREATEST(a, NULL, b)` / `LEAST(x, NULL)`

`GREATEST(a, NULL, b)` / `LEAST(x, NULL)` -- a literal NULL among the arguments. GREATEST and LEAST skip NULLs when at least one non-NULL value is present, so the NULL argument is dead weight (it can never be the result). Drop it. (sql468 greatest_least_all_null covers the all-NULL case, which returns NULL; sql534 covers duplicate args.)

<sub>`dsl-analysis/src/rules/greatest_least_null_arg.rs`</sub>

### `sql685` — `power(1, x)` always returns 1

`power(1, x)` always returns 1 -- 1 raised to any exponent is 1. The call is a constant, almost always a typo for a real base or a leftover placeholder. (Companion to sql447 power_trivial_exponent, which covers the `power(x, 0)` / `power(x, 1)` exponent side.)

<sub>`dsl-analysis/src/rules/power_base_one.rs`</sub>

### `sql686` — `NOT NOT x` / `NOT (NOT x)`

`NOT NOT x` / `NOT (NOT x)` -- a double negation cancels out. The two NOTs leave the predicate unchanged (`NOT NOT x` is just `x IS TRUE`), so they're dead weight, usually a leftover from editing a condition. Drop both. (Companion to sql088 not_is_null and the not_paren_* checks.)

<sub>`dsl-analysis/src/rules/not_not_double_negation.rs`</sub>

### `sql687` — `COALESCE('x', ...)`

`COALESCE('x', ...)` -- the first argument is a non-NULL constant literal, so COALESCE always returns it and every later argument is dead code. Almost always the operands are in the wrong order (the fallback literal belongs last). (Companion to sql493 coalesce_not_null for NOT NULL columns and sql417 coalesce_dead_arg for duplicate / NULL args.)

<sub>`dsl-analysis/src/rules/coalesce_constant_first.rs`</sub>

### `sql688` — `concat_ws(NULL, a, b)`

`concat_ws(NULL, a, b)` -- a NULL separator makes the whole result NULL. Unlike the value arguments (which concat_ws happily skips when NULL), a NULL *separator* short-circuits the entire call to NULL, which is almost never intended. Use a real separator (e.g. `''` for no separator). (Companion to sql465 concat_ws_empty_sep.)

<sub>`dsl-analysis/src/rules/concat_ws_null_separator.rs`</sub>

### `sql689` — `col % col`

`col % col` -- a column modulo itself is always 0 (or a division-by-zero error when the column is 0, or NULL). The result is a constant, almost always a typo for a different right-hand operand. (Companion to sql565 self_arithmetic for `col - col` / `col / col` and sql677 modulo_by_one.)

<sub>`dsl-analysis/src/rules/modulo_self.rs`</sub>

### `sql690` — `sqrt(-1)`

`sqrt(-1)` -- the square root of a negative literal. PostgreSQL raises 2201F ("cannot take square root of a negative number") at runtime. Almost always a sign typo or a placeholder. (Companion to sql443 substring_negative_length for other negative-literal argument bugs.)

<sub>`dsl-analysis/src/rules/sqrt_negative_literal.rs`</sub>

### `sql691` — `min(DISTINCT x)` / `max(DISTINCT x)`

`min(DISTINCT x)` / `max(DISTINCT x)` -- DISTINCT has no effect on MIN or MAX (the smallest / largest value is the same whether or not duplicates are removed), so it's dead weight that only costs a sort/hash. Drop the DISTINCT. (Companion to sql676 count_distinct_constant.)

<sub>`dsl-analysis/src/rules/min_max_distinct.rs`</sub>

### `sql692` — `ln(0)` / `ln(-1)` / `log(0)` / `log(-5)`

`ln(0)` / `ln(-1)` / `log(0)` / `log(-5)` -- the logarithm of a non-positive literal. PostgreSQL raises 2201E ("cannot take logarithm of a negative number" / "... of zero") at runtime. Almost always a sign typo or a placeholder. (Companion to sql690 sqrt_negative_literal. The two-argument `log(base, x)` form is handled by sql693.)

<sub>`dsl-analysis/src/rules/ln_log_nonpositive_literal.rs`</sub>

### `sql693` — `log(1, x)`

`log(1, x)` -- a logarithm to base 1. PostgreSQL computes `log(b, x)` as `ln(x) / ln(b)`, and `ln(1)` is 0, so base 1 raises a division-by-zero error (22012) at runtime. Almost always a typo for a real base (`log(10, x)`, `log(2, x)`). (Companion to sql692 for the single-arg `log(x)` non-positive case.)

<sub>`dsl-analysis/src/rules/log_base_one.rs`</sub>

### `sql694` — `acos(2)` / `asin(-3)`

`acos(2)` / `asin(-3)` -- the argument to acos/asin is a literal outside the valid domain [-1, 1]. PostgreSQL raises 2201E ("input is out of range") at runtime. Almost always a typo or a value that should have been normalised first. (Companion to sql690 sqrt_negative_literal and sql692 ln_log_nonpositive_literal.)

<sub>`dsl-analysis/src/rules/acos_asin_domain.rs`</sub>

### `sql695` — an aggregate call nested directly inside another aggregate, e.g

an aggregate call nested directly inside another aggregate, e.g. `sum(count(*))` or `max(avg(x))`. PostgreSQL raises 42803 ("aggregate function calls cannot be nested"). The usual fix is a subquery (aggregate the inner result one query level down) or a window function. A nested aggregate that *is* inside a subquery argument is fine and not flagged.

<sub>`dsl-analysis/src/rules/nested_aggregate.rs`</sub>

### `sql696` — `count(coalesce(x, 0))`

`count(coalesce(x, 0))` -- COUNT only skips NULLs, and COALESCE with a non-NULL fallback never produces one, so this counts every row, exactly like `count(*)`. The COALESCE defeats the point of `count(x)` (counting only non-NULL `x`). Either drop the COALESCE to count non-NULLs, or use `count(*)` to count rows. (DISTINCT is left alone -- there COALESCE changes the distinct set. Companion to sql682 coalesce_count_redundant.)

<sub>`dsl-analysis/src/rules/count_of_coalesce.rs`</sub>

### `sql697` — `degrees(radians(x))` / `radians(degrees(x))`

`degrees(radians(x))` / `radians(degrees(x))` -- converting an angle to the other unit and immediately back is the identity, so both calls are dead weight (modulo floating-point rounding). Drop them and use `x` directly. (Companion to sql551 redundant_nested_function.)

<sub>`dsl-analysis/src/rules/degrees_radians_roundtrip.rs`</sub>

### `sql698` — `chr(0)`

`chr(0)` -- PostgreSQL forbids the null character: `chr(0)` raises 54000 ("null character not permitted"). Usually a placeholder or an off-by-one (ASCII codes start at 1 for usable characters). Use a real code point.

<sub>`dsl-analysis/src/rules/chr_zero.rs`</sub>

### `sql699` — `lpad(s, 0)` / `rpad(s, 0)`

`lpad(s, 0)` / `rpad(s, 0)` -- padding (or truncating) a string to length 0 always yields the empty string, so the call is a constant `''`. Almost always a typo for a real width. (Companion to sql448 lpad_rpad_negative, which covers negative lengths, and sql679 left_right_zero.)

<sub>`dsl-analysis/src/rules/lpad_rpad_zero.rs`</sub>

### `sql700` — `setseed(2)`

`setseed(2)` -- the seed argument must be in [-1, 1]. PostgreSQL raises 2202E ("setseed parameter ... is out of range [-1,1]") at runtime for a literal outside that interval. Usually a misunderstanding of the API (the seed is a fraction, not an arbitrary integer). (Distinct from sql334, which flags non-deterministic use of setseed.)

<sub>`dsl-analysis/src/rules/setseed_out_of_range.rs`</sub>

### `sql701` — `NULLIF('a', 'b')` / `NULLIF(1, 2)`

`NULLIF('a', 'b')` / `NULLIF(1, 2)` -- both arguments are distinct constant literals, so the equality can never hold and NULLIF always returns the first one unchanged. The call is dead weight, usually a leftover or a typo. (sql453 nullif_same_args covers equal args; sql419 covers a NULL arg.)

<sub>`dsl-analysis/src/rules/nullif_distinct_literals.rs`</sub>

### `sql702` — `COALESCE(x, 0) IS NULL`

`COALESCE(x, 0) IS NULL` -- when COALESCE's last argument is a non-NULL constant the result can never be NULL, so `IS NULL` is always false and the predicate matches nothing. Almost always a logic slip (the fallback was meant to be something nullable, or the test should be `x IS NULL`). (Companion to sql687 coalesce_constant_first.)

<sub>`dsl-analysis/src/rules/coalesce_is_null_always_false.rs`</sub>

### `sql703` — `ntile(0)` / `ntile(-2)`

`ntile(0)` / `ntile(-2)` -- the bucket count must be positive. PostgreSQL raises 22023 ("argument of ntile must be greater than zero") at runtime for a literal <= 0. Usually a typo or an uninitialised variable that resolved to 0. (Companion to sql704 nth_value_nonpositive.)

<sub>`dsl-analysis/src/rules/ntile_nonpositive.rs`</sub>

### `sql704` — `nth_value(x, 0)` / `nth_value(x, -1)`

`nth_value(x, 0)` / `nth_value(x, -1)` -- the position must be positive. PostgreSQL raises 22023 ("argument of nth_value must be greater than zero") at runtime. Positions are 1-based; `nth_value(x, 1)` is the first row of the frame. (Companion to sql703 ntile_nonpositive.)

<sub>`dsl-analysis/src/rules/nth_value_nonpositive.rs`</sub>

### `sql705` — `width_bucket(x, lo, hi, 0)`

`width_bucket(x, lo, hi, 0)` -- the bucket count (4th argument) must be positive. PostgreSQL raises 22004/22023 ("count must be greater than zero") at runtime for a literal <= 0. Usually a typo or an uninitialised count. (Only the four-argument numeric form is checked.)

<sub>`dsl-analysis/src/rules/width_bucket_nonpositive_count.rs`</sub>

### `sql706` — `array_to_string(arr, NULL)`

`array_to_string(arr, NULL)` -- a NULL delimiter makes the whole result NULL, which is almost never intended. (The optional third null-string argument is different; it's the delimiter, the second arg, that must be non-NULL.) Use `''` for no separator. (Companion to sql688 concat_ws_null_separator.)

<sub>`dsl-analysis/src/rules/array_to_string_null_delimiter.rs`</sub>

### `sql707` — `lag(x, 0)` / `lead(x, 0)`

`lag(x, 0)` / `lead(x, 0)` -- an offset of 0 reads the current row, so the window function just returns `x` itself (no shift). That defeats the purpose of lag/lead and is almost always a typo for offset 1 (or a real offset). (Companion to sql704 nth_value_nonpositive.)

<sub>`dsl-analysis/src/rules/lag_lead_zero_offset.rs`</sub>

### `sql708` — `lpad(s, n, NULL)` / `rpad(s, n, NULL)`

`lpad(s, n, NULL)` / `rpad(s, n, NULL)` -- a NULL fill string makes the whole result NULL whenever padding is actually needed (the input is shorter than `n`), which is almost never intended. Use a real pad string (the default is a space). (Companion to sql699 lpad_rpad_zero.)

<sub>`dsl-analysis/src/rules/lpad_rpad_null_fill.rs`</sub>

### `sql709` — `jsonb_typeof(x) = 'int'`

`jsonb_typeof(x) = 'int'` -- comparing json(b)_typeof to a string that is not one of the values it can return. jsonb_typeof / json_typeof only ever yield 'object', 'array', 'string', 'number', 'boolean' or 'null', so a comparison to anything else is a constant (always false for `=`, always true for `<>`). Usually a wrong type name (`'int'`, `'text'`, `'bool'`).

<sub>`dsl-analysis/src/rules/jsonb_typeof_invalid_literal.rs`</sub>

### `sql710` — `COALESCE(x, 0) IS NOT NULL`

`COALESCE(x, 0) IS NOT NULL` -- when COALESCE's last argument is a non-NULL constant the result can never be NULL, so `IS NOT NULL` is always true and the predicate matches every row. Almost always a logic slip (the guard does nothing). (Mirror of sql702 coalesce_is_null_always_false.)

<sub>`dsl-analysis/src/rules/coalesce_is_not_null_always_true.rs`</sub>

### `sql711` — `make_date(2024, 13, 1)`

`make_date(2024, 13, 1)` -- a month or day literal outside its valid range. `make_date(year, month, day)` needs month in 1..12 and day in 1..31; PostgreSQL raises 22008 ("date field value out of range") at runtime. Usually a transposed month/day or an off-by-one. (Companion to sql712 make_time_invalid.)

<sub>`dsl-analysis/src/rules/make_date_invalid.rs`</sub>

### `sql712` — `make_time(25, 0, 0)`

`make_time(25, 0, 0)` -- an hour, minute or second literal outside its valid range. `make_time(hour, min, sec)` needs hour in 0..23, min in 0..59 and sec in 0..<60; PostgreSQL raises 22008 ("time field value out of range") at runtime. Usually an off-by-one or a transposed field. (Companion to sql711 make_date_invalid.)

<sub>`dsl-analysis/src/rules/make_time_invalid.rs`</sub>

### `sql713` — `x & 0` / `0 & x`

`x & 0` / `0 & x` -- a bitwise AND with 0 is always 0 (NULL when `x` is NULL), so the expression is a constant. Almost always a typo (a different mask was meant) or a disabled term. (Companion to sql681 multiply_by_zero.)

<sub>`dsl-analysis/src/rules/bitand_zero.rs`</sub>

### `sql714` — `col & col` / `col | col`

`col & col` / `col | col` -- a bitwise AND or OR of a column with itself is just the column (both are idempotent). The operand is dead, almost always a typo for a different right-hand side. (Companion to sql565 self_arithmetic and sql689 modulo_self.)

<sub>`dsl-analysis/src/rules/bitwise_self.rs`</sub>

### `sql715` — `starts_with(x, '')`

`starts_with(x, '')` -- every string starts with the empty string, so this is always true (NULL when `x` is NULL). Almost always a leftover placeholder or a prefix that was never filled in. (Companion to sql716 translate_empty_from.)

<sub>`dsl-analysis/src/rules/starts_with_empty_string.rs`</sub>

### `sql716` — `translate(s, '', to)`

`translate(s, '', to)` -- an empty `from` set means there are no characters to map, so translate returns `s` unchanged. The call is a no-op, almost always a leftover or a swapped argument. (Companion to sql467 empty_needle_string_fn for replace/split_part.)

<sub>`dsl-analysis/src/rules/translate_empty_from.rs`</sub>

### `sql717` — `to_char(x, '')`

`to_char(x, '')` -- an empty format string always produces the empty string, regardless of `x`. The call is a constant `''`, almost always a placeholder where the real format pattern was never filled in.

<sub>`dsl-analysis/src/rules/to_char_empty_format.rs`</sub>

### `sql718` — `repeat(s, 1)`

`repeat(s, 1)` -- repeating a string once returns it unchanged, so the call is a no-op. Usually a leftover or a count that should have been larger. (sql452 repeat_trivial_count covers the 0 / negative cases.)

<sub>`dsl-analysis/src/rules/repeat_one.rs`</sub>

### `sql719` — `CREATE SEQUENCE ... INCREMENT 0` (or `INCREMENT BY 0`, also in `ALTER SEQUENCE`)

`CREATE SEQUENCE ... INCREMENT 0` (or `INCREMENT BY 0`, also in `ALTER SEQUENCE`) -- the increment must be non-zero. PostgreSQL rejects it with 22023 ("INCREMENT must not be zero"). Usually a placeholder or a variable that resolved to 0.

<sub>`dsl-analysis/src/rules/sequence_increment_zero.rs`</sub>

### `sql720` — `power(0, -1)`

`power(0, -1)` -- zero raised to a negative power is undefined (it would be 1/0). PostgreSQL raises 2201F ("zero raised to a negative power is undefined") at runtime. Usually a base that should not be 0, or a sign typo on the exponent. (Companion to sql685 power_base_one and sql447 power_trivial_exponent.)

<sub>`dsl-analysis/src/rules/power_zero_negative_exponent.rs`</sub>

### `sql721` — `make_timestamp(2024, 13, 1, 0, 0, 0)`

`make_timestamp(2024, 13, 1, 0, 0, 0)` -- a field outside its valid range in make_timestamp / make_timestamptz. Month must be 1..12, day 1..31, hour 0..23, minute 0..59, second 0..<60; PostgreSQL raises 22008 at runtime otherwise. (Companion to sql711 make_date_invalid and sql712 make_time_invalid.)

<sub>`dsl-analysis/src/rules/make_timestamp_invalid.rs`</sub>

### `sql722` — `factorial(-1)`

`factorial(-1)` -- the factorial of a negative number is undefined. PostgreSQL raises 2201F ("factorial of a negative number is undefined") at runtime for a negative literal. Usually a sign typo. (Companion to sql690 sqrt_negative_literal and sql692 ln_log_nonpositive_literal.)

<sub>`dsl-analysis/src/rules/factorial_negative.rs`</sub>

### `sql723` — `array_cat(a, '{}')` / `array_cat(ARRAY[], a)`

`array_cat(a, '{}')` / `array_cat(ARRAY[], a)` -- concatenating an empty array returns the other operand unchanged, so the call is a no-op. Usually a leftover or an array that was meant to hold elements. (Companion to sql109 concat_empty_string.)

<sub>`dsl-analysis/src/rules/array_cat_empty.rs`</sub>

### `sql724` — `NUMERIC(2000)` / `DECIMAL(0, 0)`

`NUMERIC(2000)` / `DECIMAL(0, 0)` -- the precision must be between 1 and 1000. PostgreSQL rejects a precision outside that range with 22023 ("NUMERIC precision N must be between 1 and 1000"). Usually a typo or a confusion with the column's intended length. (Companion to sql450 numeric_scale_exceeds_precision.)

<sub>`dsl-analysis/src/rules/numeric_precision_out_of_range.rs`</sub>

### `sql725` — `random() >= 1` / `random() < 0`

`random() >= 1` / `random() < 0` -- `random()` returns a value in the half-open interval [0, 1), so a comparison against a constant outside that interval is always false. Usually a misremembered range (people often assume [0, 1] or a percentage). Scale the result (`random() * 100`) or fix the bound.

<sub>`dsl-analysis/src/rules/random_compare_out_of_range.rs`</sub>

### `sql726` — `ascii('')`

`ascii('')` -- the ASCII code of the empty string is always 0 (PostgreSQL returns 0 for an empty input). The call is a constant, almost always a placeholder or a missing character. (Companion to sql698 chr_zero.)

<sub>`dsl-analysis/src/rules/ascii_empty_string.rs`</sub>

### `sql727` — `exp(ln(x))` / `ln(exp(x))`

`exp(ln(x))` / `ln(exp(x))` -- exp and ln are inverses, so a round-trip is the identity (modulo float rounding and the domain x > 0). Both calls are dead weight; use `x` directly. (Companion to sql697 degrees_radians_roundtrip and sql551 redundant_nested_function.)

<sub>`dsl-analysis/src/rules/exp_ln_roundtrip.rs`</sub>

### `sql728` — `x | 0` / `0 | x`

`x | 0` / `0 | x` -- a bitwise OR with 0 leaves the value unchanged, so the operand is dead. Almost always a typo for a real mask or a disabled flag. (Companion to sql713 bitand_zero and sql714 bitwise_self.)

<sub>`dsl-analysis/src/rules/bitor_zero.rs`</sub>

### `sql729` — `x << 0` / `x >> 0`

`x << 0` / `x >> 0` -- shifting by 0 bits leaves the value unchanged, so the shift is a no-op. Usually a typo for a real shift amount. (Companion to sql728 bitor_zero.)

<sub>`dsl-analysis/src/rules/bitshift_zero.rs`</sub>

### `sql730` — `chr(2000000)` / `chr(-1)`

`chr(2000000)` / `chr(-1)` -- a code point outside the valid range. In a UTF-8 database `chr(n)` needs n in 1..=1114111 (0x10FFFF); PostgreSQL raises 54000 ("requested character too large") otherwise. Usually a bad constant or a value that should have been masked. (sql698 chr_zero covers the n = 0 case.)

<sub>`dsl-analysis/src/rules/chr_above_max.rs`</sub>

### `sql731` — `ln(1)` / `log(1)`

`ln(1)` / `log(1)` -- the logarithm of 1 is always 0 in any base, so the call is a constant. Usually a leftover or a misremembered identity. (sql692 ln_log_nonpositive_literal covers the <= 0 error cases; sql693 covers two-argument `log(1, x)`.)

<sub>`dsl-analysis/src/rules/ln_log_one.rs`</sub>

### `sql732` — `acosh(0)` / `atanh(1)`

`acosh(0)` / `atanh(1)` -- an argument outside the function's domain. `acosh(x)` needs x >= 1 and `atanh(x)` needs -1 < x < 1; PostgreSQL raises 2201E ("input is out of range") at runtime otherwise. Usually a value that should have been clamped or normalised. (Companion to sql694 acos_asin_domain.)

<sub>`dsl-analysis/src/rules/acosh_atanh_domain.rs`</sub>

### `sql733` — a string literal containing `password=...`

a string literal containing `password=...` -- a hardcoded credential in a connection string (`dblink(...)`, `postgres_fdw` / `CREATE SERVER ... OPTIONS`, `CREATE USER MAPPING`). The secret lands in the server log, `pg_stat_activity`, and version control. Move it to a `.pgpass` file, a user mapping created out of band, or a secret store. (Companion to sql571 plaintext_password for role DDL.)

<sub>`dsl-analysis/src/rules/connection_string_password.rs`</sub>

### `sql734` — `x ILIKE 'plain'`

`x ILIKE 'plain'` -- an ILIKE pattern with no `%` or `_` wildcard is just a case-insensitive equality test. `lower(x) = lower('plain')` (with a matching functional index) or a citext column is clearer and can use an index. (Mirror of sql052 like_without_wildcard for the ILIKE operator.)

<sub>`dsl-analysis/src/rules/ilike_without_wildcard.rs`</sub>

### `sql735` — `EXISTS (SELECT count(*) FROM ...)`

`EXISTS (SELECT count(*) FROM ...)` -- a subquery whose projection is a bare aggregate (and that has no GROUP BY / HAVING) always returns exactly one row, so the EXISTS is always true (and `NOT EXISTS` always false). Almost always a misunderstanding: use `EXISTS (SELECT 1 FROM ...)` to test for any rows, or compare the count directly. (Companion to sql441 uncorrelated_exists and sql201 exists_select_star.)

<sub>`dsl-analysis/src/rules/exists_aggregate.rs`</sub>

### `sql736` — `width_bucket(x, 5, 5, 10)`

`width_bucket(x, 5, 5, 10)` -- the lower and upper bounds are equal literals. PostgreSQL raises 22023 ("lower bound cannot equal upper bound") at runtime, because an empty range cannot be divided into buckets. Usually a copy-paste slip or a transposed argument. (Companion to sql705 width_bucket_nonpositive_count.)

<sub>`dsl-analysis/src/rules/width_bucket_equal_bounds.rs`</sub>

### `sql737` — `date_bin('0 seconds', ts, origin)`

`date_bin('0 seconds', ts, origin)` -- the stride (first argument) must be a positive interval. PostgreSQL raises 22023 ("stride must be greater than zero") at runtime for a zero or negative literal stride. Usually a placeholder or a computed interval that collapsed to zero.

<sub>`dsl-analysis/src/rules/date_bin_nonpositive_stride.rs`</sub>

### `sql738` — comparing a never-negative function against a negative value (or `< 0`), so the predicate never matches

comparing a never-negative function against a negative value (or `< 0`), so the predicate never matches -- e.g. `WHERE strpos(s, x) = -1` or `array_length(a, 1) < 0`. Covers position / strpos / array_length / jsonb_array_length / ascii. A frequent cross-language bug: these return 0 (or NULL), never -1, for "not found" / "empty". (Extends sql552, which owns abs / length-family / cardinality; sql547 owns `array_length(...) = 0`.)

<sub>`dsl-analysis/src/rules/nonneg_func_negative_compare2.rs`</sub>

### `sql739` — `x::int::int` / `(a || b)::text::text`

`x::int::int` / `(a || b)::text::text` -- two adjacent casts to the same type. The outer cast is a no-op; drop it. Purely syntactic (unlike sql415 cast_same_type, which compares against the column's catalog type), so it fires regardless of what `x` is.

<sub>`dsl-analysis/src/rules/redundant_double_cast.rs`</sub>

### `sql740` — `NOT TRUE` / `NOT FALSE`

`NOT TRUE` / `NOT FALSE` -- negating a boolean literal is a constant (`NOT TRUE` is FALSE, `NOT FALSE` is TRUE). In a predicate it silently forces the branch on or off; almost always a debugging leftover. `IS NOT TRUE` / `IS NOT FALSE` are not flagged -- their NULL handling is meaningful. (Companion to sql686 not_not_double_negation.)

<sub>`dsl-analysis/src/rules/not_boolean_literal.rs`</sub>

### `sql741` — `x % -1` / `MOD(x, -1)`

`x % -1` / `MOD(x, -1)` -- the remainder of any integer divided by -1 is always 0 (just like dividing by 1), so the expression is a constant. Usually a typo for a real modulus. (Companion to sql677 modulo_by_one.)

<sub>`dsl-analysis/src/rules/modulo_by_negative_one.rs`</sub>

### `sql742` — `array_remove(arr, NULL)`

`array_remove(arr, NULL)` -- array_remove uses equality to find elements, and `x = NULL` is never true, so it removes nothing and returns the array unchanged. To strip NULLs use `array(SELECT x FROM unnest(arr) AS x WHERE x IS NOT NULL)`. (Companion to sql445 array_position_null.)

<sub>`dsl-analysis/src/rules/array_remove_null.rs`</sub>

### `sql743` — `array_replace(arr, x, x)`

`array_replace(arr, x, x)` -- the search and replacement values are identical, so every match is replaced with itself and the array is returned unchanged. A no-op, almost always a copy-paste slip where the replacement should differ. (Array analogue of sql528 replace_same_from_to.)

<sub>`dsl-analysis/src/rules/array_replace_same.rs`</sub>

### `sql744` — `array_position(arr, x) = 0`

`array_position(arr, x) = 0` -- array_position returns a 1-based index, or NULL when the element is absent. It is never 0 and never negative, so `= 0`, `< 1`, `<= 0` and comparisons to negatives never match (and NULL never matches anything). A frequent bug: code expecting a 0-based index or a -1 "not found" sentinel. Test with `... IS NULL` / `... IS NOT NULL` instead.

<sub>`dsl-analysis/src/rules/array_position_compare_impossible.rs`</sub>

### `sql745` — `date_part('yearr', ts)`

`date_part('yearr', ts)` -- the field name (first argument) is not a recognised date/time field. PostgreSQL raises 22023 at runtime. This is the function-call form of EXTRACT; sql208 covers the `EXTRACT(field FROM ...)` syntax. Only fires when the field is a string literal.

<sub>`dsl-analysis/src/rules/date_part_unknown_field.rs`</sub>

### `sql746` — `int4range(5, 1)` / `numrange(10, 2)`

`int4range(5, 1)` / `numrange(10, 2)` -- the lower bound is greater than the upper bound. PostgreSQL raises 22000 ("range lower bound must be less than or equal to range upper bound") at runtime. Usually transposed arguments. Fires only on the numeric range constructors with two integer literals (NULL = unbounded is ignored).

<sub>`dsl-analysis/src/rules/range_lower_gt_upper.rs`</sub>

### `sql747` — `percentile_cont(1.5) WITHIN GROUP (...)`

`percentile_cont(1.5) WITHIN GROUP (...)` -- the percentile fraction must be in [0, 1]. PostgreSQL raises 2202E ("percentile value ... is not between 0 and 1") at runtime. Usually a percentage (50) written where a fraction (0.5) was needed. (Companion to sql290 percentile_no_within.)

<sub>`dsl-analysis/src/rules/percentile_fraction_out_of_range.rs`</sub>

### `sql748` — `encode(data, 'base32')`

`encode(data, 'base32')` -- the format (second argument) must be one of `base64`, `hex`, or `escape`. PostgreSQL raises 22023 ("unrecognized encoding") at runtime for anything else. Usually a typo (`base32`, `b64`) or a confusion with another language's API.

<sub>`dsl-analysis/src/rules/encode_invalid_format.rs`</sub>

### `sql749` — `daterange('2024-01-01', '2023-01-01')`

`daterange('2024-01-01', '2023-01-01')` -- the lower date/timestamp bound is later than the upper one. PostgreSQL raises 22000 ("range lower bound must be less than or equal to range upper bound") at runtime. Usually transposed arguments. Fires only when both bounds are ISO date/timestamp string literals. (Companion to sql746 range_lower_gt_upper for numeric ranges.)

<sub>`dsl-analysis/src/rules/daterange_reversed.rs`</sub>

### `sql750` — `B'1021'`

`B'1021'` -- a bit-string literal containing a character other than 0 or 1. PostgreSQL raises 22P03 ("... is not a valid binary digit") at parse. Usually a typo or a decimal value written where a bit pattern was meant. (Companion to sql751 hex_string_invalid_digit.)

<sub>`dsl-analysis/src/rules/bit_string_invalid_digit.rs`</sub>

### `sql751` — `X'1G'`

`X'1G'` -- a hexadecimal string literal containing a non-hex character. PostgreSQL raises 22P03 ("... is not a valid hexadecimal digit") at parse. Usually a typo. (Companion to sql750 bit_string_invalid_digit.)

<sub>`dsl-analysis/src/rules/hex_string_invalid_digit.rs`</sub>

### `sql752` — `LIKE 'a%' ESCAPE '\\!'`

`LIKE 'a%' ESCAPE '\\!'` -- the ESCAPE string must be empty or a single character. PostgreSQL raises 22019 ("invalid escape string ... must be empty or one character") at runtime for a longer literal. Usually a typo or a misunderstanding of the clause.

<sub>`dsl-analysis/src/rules/like_escape_multichar.rs`</sub>

### `sql753` — `setweight(tsv, 'E')`

`setweight(tsv, 'E')` -- the weight label must be one of 'A', 'B', 'C', or 'D' (uppercase). PostgreSQL raises 22023 ("weight must be one of A, B, C, D") at runtime for anything else. Usually a typo or a lowercase label.

<sub>`dsl-analysis/src/rules/setweight_invalid_label.rs`</sub>

### `sql754` — `to_tsquery('quick brown fox')`

`to_tsquery('quick brown fox')` -- to_tsquery expects tsquery syntax (lexemes joined by `&`, `|`, `!`, `<->`), so a plain phrase with spaces and no operator raises a syntax error at runtime. For free text use `plainto_tsquery(...)` (whitespace -> AND), `phraseto_tsquery(...)`, or `websearch_to_tsquery(...)`. (Companion to sql499 tsvector_text_literal.)

<sub>`dsl-analysis/src/rules/to_tsquery_plain_text.rs`</sub>

### `sql755` — `count(DISTINCT a, b)`

`count(DISTINCT a, b)` -- a single-argument aggregate given several comma-separated DISTINCT expressions. count / sum / avg / min / max (etc.) take exactly one argument, so PostgreSQL raises 42883 ("function ... does not exist"). To count distinct combinations use `count(DISTINCT (a, b))` (a row value) or `count(DISTINCT ROW(a, b))`.

<sub>`dsl-analysis/src/rules/count_distinct_multiple_args.rs`</sub>

### `sql756` — `string_agg(x)` / `jsonb_object_agg(k)`

`string_agg(x)` / `jsonb_object_agg(k)` -- a required second argument is missing. string_agg needs a delimiter (`string_agg(x, ',')`) and jsonb_object_agg needs both key and value. The one-argument forms do not exist, so PostgreSQL raises 42883 ("function ... does not exist"). (array_agg / json_agg legitimately take a single argument and are not flagged.)

<sub>`dsl-analysis/src/rules/agg_missing_delimiter.rs`</sub>

### `sql757` — a partitioned table's PRIMARY KEY does not include every partition key column. PostgreSQL requires every...

a partitioned table's PRIMARY KEY does not include every partition key column. PostgreSQL requires every unique constraint (PRIMARY KEY included) on a partitioned table to cover all of the table's partitioning columns -- CREATE TABLE fails with "unique constraint on partitioned table must include all partitioning columns" (0A000) otherwise. Only handles simple column-name partition keys and a single table-level PRIMARY KEY (...) clause; expression partition keys and UNIQUE constraints are out of scope to avoid false positives.

<sub>`dsl-analysis/src/rules/partition_by_no_key_in_pk.rs`</sub>

### `sql758` — `FOR VALUES FROM (x) TO (y)` where the lower partition bound is not strictly less than the upper bound....

`FOR VALUES FROM (x) TO (y)` where the lower partition bound is not strictly less than the upper bound. PostgreSQL rejects an empty partition range at CREATE/ALTER TABLE time ("empty range bound specified for partition"). Only fires on a single-column bound where both sides are literals of the same simple kind (both numeric, or both single-quoted strings) -- multi-column and unbounded (MINVALUE/MAXVALUE) bounds are left alone.

<sub>`dsl-analysis/src/rules/partition_range_bound_reversed.rs`</sub>

### `sql759` — `PARTITION BY RANGE/LIST/HASH (some_volatile_fn(col))`

`PARTITION BY RANGE/LIST/HASH (some_volatile_fn(col))` -- PostgreSQL requires partition key expressions to be immutable and rejects non-immutable functions ("functions in partition key expression must be marked IMMUTABLE"). Flags the common cases where the expression obviously calls a well-known volatile/stable builtin; anything else is left alone (no catalog volatility lookup available for user-defined functions).

<sub>`dsl-analysis/src/rules/partition_by_expression_volatile.rs`</sub>

### `sql760` — `PARTITION BY RANGE/LIST/HASH (a, a)`

`PARTITION BY RANGE/LIST/HASH (a, a)` -- the same column listed twice in the partition key. Always a copy-paste mistake; a repeated column contributes nothing to the partitioning strategy.

<sub>`dsl-analysis/src/rules/partition_by_duplicate_column.rs`</sub>

### `sql761` — `ALTER TABLE ... DETACH PARTITION ... CONCURRENTLY` inside an explicit transaction. Like `DROP INDEX...

`ALTER TABLE ... DETACH PARTITION ... CONCURRENTLY` inside an explicit transaction. Like `DROP INDEX CONCURRENTLY` (sql331), the CONCURRENTLY detach variant cannot run inside a BEGIN/COMMIT block -- PG raises 25001 at runtime. Flags when the same buffer mixes a CONCURRENTLY detach with an earlier BEGIN.

<sub>`dsl-analysis/src/rules/detach_partition_concurrently_in_tx.rs`</sub>

### `sql762` — `FOR VALUES WITH (MODULUS m, REMAINDER r)` where the remainder is not less than the modulus

`FOR VALUES WITH (MODULUS m, REMAINDER r)` where the remainder is not less than the modulus. PostgreSQL requires the remainder to be in [0, modulus) and raises an error ("remainder for hash partition must be less than modulus") otherwise.

<sub>`dsl-analysis/src/rules/hash_partition_modulus_remainder.rs`</sub>

### `sql763` — `JSON_EXISTS(doc, 'literal')` where the literal path string does not start with `$`

`JSON_EXISTS(doc, 'literal')` where the literal path string does not start with `$` -- not a valid SQL/JSON path expression. PostgreSQL raises an error evaluating the path at runtime; the parser accepts any string literal here since path validity isn't checked until execution (verified empirically: no parse rejection).

<sub>`dsl-analysis/src/rules/json_exists_bad_path.rs`</sub>

### `sql764` — `JSON_VALUE(

`JSON_VALUE(... RETURNING <type>)` narrowing the return type with no `ON ERROR` clause. If the extracted value doesn't convert to the target type, JSON_VALUE raises an unhandled runtime error; an explicit `NULL ON ERROR` / `DEFAULT ... ON ERROR` avoids that. Hint-level: valid SQL, just a missing safety net.

<sub>`dsl-analysis/src/rules/json_value_returning_without_on_error.rs`</sub>

### `sql765` — `JSON_QUERY(... WITH WRAPPER ... OMIT QUOTES)`

`JSON_QUERY(... WITH WRAPPER ... OMIT QUOTES)` -- OMIT QUOTES is disallowed together with a wrapper (PostgreSQL raises an error at query time; the parser accepts the combination -- verified empirically). Only the plain `WITH WRAPPER` form is covered -- `WITH CONDITIONAL/UNCONDITIONAL [ARRAY] WRAPPER` variants are out of scope to keep detection conservative.

<sub>`dsl-analysis/src/rules/json_query_wrapper_conflict.rs`</sub>

### `sql766` — `JSON_TABLE(... COLUMNS (a ..., a ...))`

`JSON_TABLE(... COLUMNS (a ..., a ...))` -- the same output column name used twice. PostgreSQL rejects duplicate JSON_TABLE column names. Replaces the spec's original sql766 (empty COLUMNS list), which pg_query rejects as a hard parse error before any LintRule sees it -- verified empirically.

<sub>`dsl-analysis/src/rules/json_table_duplicate_column_name.rs`</sub>

### `sql767` — `col IS JSON` where the catalog already types `col` as `json` or `jsonb`

`col IS JSON` where the catalog already types `col` as `json` or `jsonb` -- always true, the predicate is redundant. `IS JSON OBJECT`/`ARRAY`/`SCALAR` are narrower checks and are left alone (being jsonb-typed doesn't guarantee a specific JSON kind).

<sub>`dsl-analysis/src/rules/is_json_redundant_with_jsonb_column.rs`</sub>

### `sql768` — `<expr> IS JSON OBJECT AND <same expr> IS JSON ARRAY` (or any two different IS JSON kinds directly ANDed...

`<expr> IS JSON OBJECT AND <same expr> IS JSON ARRAY` (or any two different IS JSON kinds directly ANDed together) -- a JSON value is exactly one of object/array/scalar, so requiring two different kinds of the same expression is always false. Only the direct `x IS JSON K1 AND x IS JSON K2` adjacency is matched (nothing else between the two checks) to stay conservative.

<sub>`dsl-analysis/src/rules/is_json_scalar_object_conflict.rs`</sub>

### `sql769` — `CYCLE

`CYCLE ... USING <col>` names a working column that collides with a column already produced by the recursive CTE's own column list. PostgreSQL rejects this as a duplicate column name. `SEARCH ... SET <col>` collision is a separate, narrower case and is out of scope for this rule.

<sub>`dsl-analysis/src/rules/recursive_cte_cycle_column_reused.rs`</sub>

### `sql770` — the recursive term references the CTE itself more than once

the recursive term references the CTE itself more than once. PostgreSQL allows exactly one self-reference in a recursive term ("recursive reference to query ... must not appear more than once").

<sub>`dsl-analysis/src/rules/recursive_cte_missing_base_union.rs`</sub>

### `sql771` — the recursive term contains an aggregate function call

the recursive term contains an aggregate function call -- disallowed ("aggregate functions are not allowed in a recursive query's recursive term").

<sub>`dsl-analysis/src/rules/recursive_term_has_aggregate.rs`</sub>

### `sql772` — the recursive term contains a top-level ORDER BY, LIMIT, or DISTINCT

the recursive term contains a top-level ORDER BY, LIMIT, or DISTINCT -- disallowed in a recursive query's recursive term. Only the term's own top level is checked (via `unwrap_parens` + depth-0 `find_clause`) -- a nested subquery's ORDER BY/LIMIT is legal and left alone.

<sub>`dsl-analysis/src/rules/recursive_term_has_order_or_limit.rs`</sub>

### `sql773` — the recursive term's self-reference sits on the nullable side of an outer join

the recursive term's self-reference sits on the nullable side of an outer join -- disallowed ("recursive reference to query ... must not appear within an outer join").

<sub>`dsl-analysis/src/rules/recursive_cte_outer_join_recursive_side.rs`</sub>

### `sql774` — `EXCLUDE USING <am> (col WITH op, col WITH op)`

`EXCLUDE USING <am> (col WITH op, col WITH op)` -- the same column (or expression) listed twice. Always a copy-paste mistake. Replaces the spec's original sql774 (missing operator after `WITH`), verified unreachable -- pg_query rejects that as a hard parse error before any LintRule sees it.

<sub>`dsl-analysis/src/rules/exclude_using_duplicate_column.rs`</sub>

### `sql775` — `EXCLUDE USING btree/hash/brin/gin (...)`

`EXCLUDE USING btree/hash/brin/gin (...)` -- these access methods do not support exclusion constraints in PostgreSQL (only gist and spgist do). PG rejects the constraint at DDL time.

<sub>`dsl-analysis/src/rules/exclude_using_btree_index_type.rs`</sub>

### `sql776` — `EXCLUDE USING gist (col WITH =)` on a single column with only the `=` operator

`EXCLUDE USING gist (col WITH =)` on a single column with only the `=` operator -- functionally a weaker, slower UNIQUE constraint (GIST equality lookups don't get a btree's lookup speed, so this loses index efficiency for no behavioral gain over UNIQUE).

<sub>`dsl-analysis/src/rules/exclude_using_single_column_eq.rs`</sub>

### `sql777` — `CREATE DOMAIN ... CHECK (expr)` where `expr` never references `VALUE`

`CREATE DOMAIN ... CHECK (expr)` where `expr` never references `VALUE` -- evaluates to the same result for every input, so the constraint either always fires or never does regardless of what's being validated.

<sub>`dsl-analysis/src/rules/domain_check_references_value_missing.rs`</sub>

### `sql778` — `CREATE DOMAIN ... CHECK (VALUE <op> <literal>) DEFAULT <literal>` where the DEFAULT literal plainly fails...

`CREATE DOMAIN ... CHECK (VALUE <op> <literal>) DEFAULT <literal>` where the DEFAULT literal plainly fails the CHECK. Only handles a single `VALUE <op> numeric-literal` comparison (either operand order) and a single numeric DEFAULT literal -- anything more complex is left alone. Warning rather than Error: PostgreSQL's exact validation timing for a domain's own default isn't confirmed.

<sub>`dsl-analysis/src/rules/domain_default_violates_check.rs`</sub>

### `sql779` — `CREATE TYPE ... AS (a int, a text)`

`CREATE TYPE ... AS (a int, a text)` -- duplicate field name. Sibling to the existing create_table_dup_column.

<sub>`dsl-analysis/src/rules/composite_type_dup_field.rs`</sub>

### `sql780` — `jsonb_path_exists(doc, '$.a ? (1 == 2)')`

`jsonb_path_exists(doc, '$.a ? (1 == 2)')` -- the filter's comparison is between two literal numbers, so it evaluates to the same result on every row. Only flags the always-false shapes (`==` with different literals, `!=` with equal literals); an always-true literal filter is a related but separate case, out of scope here.

<sub>`dsl-analysis/src/rules/jsonb_path_exists_static_false.rs`</sub>

### `sql781` — `jsonb_array_length('{"a":1}'::jsonb)`

`jsonb_array_length('{"a":1}'::jsonb)` -- the argument is a jsonb literal whose content is an object, not an array. PostgreSQL raises 22023 ("cannot get array length of a non-array") at runtime; the object-ness is knowable statically here because it's a literal.

<sub>`dsl-analysis/src/rules/jsonb_array_length_on_object_literal.rs`</sub>

### `sql782` — `'{"a":1}'::jsonb - 0`

`'{"a":1}'::jsonb - 0` -- the integer-index form of the `-` operator deletes an array element by position and is only defined for arrays; the literal here is an object. PostgreSQL raises an error ("cannot delete from object using integer index") at runtime. Scoped to jsonb literals only -- a jsonb *column*'s runtime shape (object vs array) isn't visible in its static catalog type, so this can't be checked for columns.

<sub>`dsl-analysis/src/rules/jsonb_minus_integer_on_object.rs`</sub>

### `sql783` — `jsonb_build_object(NULL, 1, ...)`

`jsonb_build_object(NULL, 1, ...)` -- a literal `NULL` in a key position. PostgreSQL raises "null value not allowed for object key" at runtime. Sibling to the existing jsonb_build_object_duplicate_key.

<sub>`dsl-analysis/src/rules/jsonb_build_object_null_key.rs`</sub>

### `sql784` — `GROUPING SETS ((a,b), (a,b))`

`GROUPING SETS ((a,b), (a,b))` -- the same set of grouping columns appears twice (regardless of column order within the set). PostgreSQL doesn't reject this, but it's virtually always a copy-paste mistake.

<sub>`dsl-analysis/src/rules/grouping_sets_duplicate_set.rs`</sub>

### `sql785` — `GROUPING(x)` where `x` does not appear anywhere in the statement's GROUP BY clause

`GROUPING(x)` where `x` does not appear anywhere in the statement's GROUP BY clause -- PostgreSQL raises 42803 ("column ... must appear in GROUP BY clause or be used in an aggregate function"). Checks only that the argument identifier appears somewhere in the GROUP BY clause text (handles GROUPING SETS/ ROLLUP/CUBE without needing to parse their nested structure) -- conservative by construction, never false-positives on a column that's actually grouped.

<sub>`dsl-analysis/src/rules/grouping_function_arg_not_in_group_by.rs`</sub>

### `sql786` — `ROLLUP (a, a)` / `CUBE (a, a)`

`ROLLUP (a, a)` / `CUBE (a, a)` -- the same column listed twice. Replaces the spec's original sql786 (empty ROLLUP/CUBE column list), verified unreachable -- pg_query rejects `ROLLUP ()` / `CUBE ()` as a hard parse error before any LintRule sees it, same class of trap as batches 1, 2, and 4's swaps.

<sub>`dsl-analysis/src/rules/rollup_cube_duplicate_column.rs`</sub>

### `sql787` — a parenthesized `(SELECT ...)` subquery, correlated to the outer query (references a qualified column whose...

a parenthesized `(SELECT ...)` subquery, correlated to the outer query (references a qualified column whose alias isn't defined in the subquery's own FROM), with no aggregate function and no LIMIT clause -- risks "more than one row returned by a subquery used as an expression" at runtime if more than one row matches. EXISTS/IN/ANY/ALL/SOME-wrapped subqueries are exempt (those are valid multi-row contexts). Conservative: subqueries with their own internal JOIN are skipped entirely -- otherwise the subquery's *own* second joined table would look like an outer reference.

<sub>`dsl-analysis/src/rules/correlated_subquery_select_no_limit1_no_agg.rs`</sub>

### `sql788` — a `LATERAL (...)` subquery references a table alias that's introduced later in the same FROM/JOIN list

a `LATERAL (...)` subquery references a table alias that's introduced later in the same FROM/JOIN list -- LATERAL can only see items to its left, so this is out of scope (PG raises "missing FROM-clause entry"). Uses `Scope`'s already-resolved binding positions rather than hand-parsing FROM/JOIN order.

<sub>`dsl-analysis/src/rules/lateral_join_references_later_table.rs`</sub>

### `sql789` — `a FULL [OUTER] JOIN b ON ... WHERE b.col = 'x'`

`a FULL [OUTER] JOIN b ON ... WHERE b.col = 'x'` -- a positive WHERE predicate on either side of a FULL OUTER JOIN silently turns it into a non-full join: the NULL-extended rows fail the filter and disappear. Sibling to the existing left_join_defeated_by_where (sql522), extended to check both sides since both are nullable in a FULL JOIN. Same conservative rules: only a conjunct that *begins* with `alias.col <predicate>` is flagged, and any conjunct mentioning NULL or containing a top-level OR is skipped.

<sub>`dsl-analysis/src/rules/full_outer_join_where_defeats.rs`</sub>

### `sql790` — `col type NOT NULL UNIQUE NULLS NOT DISTINCT`

`col type NOT NULL UNIQUE NULLS NOT DISTINCT` -- the column is already NOT NULL, so NULLS NOT DISTINCT (which only changes how multiple NULLs are treated by the unique constraint) can never apply; it's a no-op clause. Only the column-level inline form is checked -- a table-level `UNIQUE NULLS NOT DISTINCT (col)` constraint would need cross-referencing the column's own NOT NULL, out of scope for this first pass.

<sub>`dsl-analysis/src/rules/unique_nulls_distinct_redundant.rs`</sub>

### `sql791` — `CREATE STATISTICS name (ndistinct)` (or `dependencies`) with fewer than 2 columns/expressions in the `ON`...

`CREATE STATISTICS name (ndistinct)` (or `dependencies`) with fewer than 2 columns/expressions in the `ON` list -- these statistics kinds require at least 2 to be meaningful. PostgreSQL rejects a 1-column ON list for these kinds.

<sub>`dsl-analysis/src/rules/create_statistics_no_columns.rs`</sub>

### `sql792` — `CREATE STATISTICS ... ON a, a FROM t`

`CREATE STATISTICS ... ON a, a FROM t` -- the same column listed twice.

<sub>`dsl-analysis/src/rules/create_statistics_dup_column.rs`</sub>

### `sql793` — an unconditional `WHEN MATCHED THEN` clause appears before another `WHEN MATCHED [AND ...] THEN` clause in...

an unconditional `WHEN MATCHED THEN` clause appears before another `WHEN MATCHED [AND ...] THEN` clause in the same MERGE -- the unconditional branch always wins first, so the later WHEN MATCHED clause can never run.

<sub>`dsl-analysis/src/rules/merge_when_matched_unreachable.rs`</sub>

### `sql794` — `WHEN NOT MATCHED THEN INSERT ... VALUES (target.col, ...)`

`WHEN NOT MATCHED THEN INSERT ... VALUES (target.col, ...)` -- referencing the MERGE target's alias inside the INSERT branch, which only runs when NO target row matched. PostgreSQL rejects this ("invalid reference to FROM-clause entry").

<sub>`dsl-analysis/src/rules/merge_insert_references_target.rs`</sub>

### `sql795` — `CREATE PUBLICATION ... FOR TABLES IN SCHEMA s, s`

`CREATE PUBLICATION ... FOR TABLES IN SCHEMA s, s` -- the same schema listed twice. Replaces the spec's original sql795 (`FOR ALL TABLES, TABLE x`), verified unreachable -- `FOR ALL TABLES` and `FOR TABLE` are mutually exclusive grammar productions; pg_query rejects the comma-combination as a hard parse error.

<sub>`dsl-analysis/src/rules/publication_duplicate_schema.rs`</sub>

### `sql796` — `CREATE SUBSCRIPTION ... WITH (create_slot = false)` with no `slot_name`

`CREATE SUBSCRIPTION ... WITH (create_slot = false)` with no `slot_name` -- PostgreSQL can't infer which replication slot to use when it isn't asked to create one, and raises an error.

<sub>`dsl-analysis/src/rules/subscription_no_slot_name_with_create_false.rs`</sub>

### `sql797` — `CREATE PUBLICATION ... FOR TABLE a, a`

`CREATE PUBLICATION ... FOR TABLE a, a` -- the same table listed twice.

<sub>`dsl-analysis/src/rules/publication_duplicate_table.rs`</sub>

### `sql798` — a bare `LOOP ... END LOOP` (not FOR/WHILE) whose body contains no `EXIT`, `RETURN`, or `RAISE` anywhere

a bare `LOOP ... END LOOP` (not FOR/WHILE) whose body contains no `EXIT`, `RETURN`, or `RAISE` anywhere -- guaranteed infinite loop.

<sub>`dsl-analysis/src/rules/loop_no_exit.rs`</sub>

### `sql799` — a `FOR i IN ...` loop variable name shadows a column that exists somewhere in the connected catalog

a `FOR i IN ...` loop variable name shadows a column that exists somewhere in the connected catalog -- classic PL/pgSQL footgun (ambiguous column vs. variable reference inside the loop body). Checked against the whole `Catalog` rather than a per- statement `Scope`, since a PL/pgSQL function/DO body isn't resolved against a FROM-clause scope the way a bare SELECT is.

<sub>`dsl-analysis/src/rules/for_loop_variable_shadows_column.rs`</sub>

### `sql800` — `EXCEPTION WHEN OTHERS THEN` with an empty or `NULL;`-only body

`EXCEPTION WHEN OTHERS THEN` with an empty or `NULL;`-only body -- silently discards every error. Classic PL/pgSQL anti-pattern.

<sub>`dsl-analysis/src/rules/exception_block_swallows_all.rs`</sub>

### `sql801` — `EXECUTE <dynamic sql> USING a, b` where the highest `$N` placeholder referenced in the dynamic SQL text...

`EXECUTE <dynamic sql> USING a, b` where the highest `$N` placeholder referenced in the dynamic SQL text doesn't match the number of USING arguments. Scans the whole EXECUTE target text for `$N` placeholders regardless of whether the target is a plain string or wrapped in `format(...)` -- `format()`'s own `%s`/`%I`/ `%L` substitutions are a separate mechanism, not counted here.

<sub>`dsl-analysis/src/rules/execute_using_arg_count_mismatch.rs`</sub>

### `sql802` — `EXECUTE '<literal SELECT>' INTO a, b` where the statically-known SELECT-list column count doesn't match the...

`EXECUTE '<literal SELECT>' INTO a, b` where the statically-known SELECT-list column count doesn't match the number of INTO targets. Only fires when the EXECUTE target is a single plain string literal immediately followed by INTO (not `format()`/ concatenation, and not combined with USING) whose content is itself statically a bare `SELECT <items> [FROM ...]`.

<sub>`dsl-analysis/src/rules/execute_into_arity_mismatch.rs`</sub>

### `sql803` — `RAISE NOTICE` appears inside a loop body (bare `LOOP`, `FOR ... LOOP`, or `WHILE ... LOOP`)

`RAISE NOTICE` appears inside a loop body (bare `LOOP`, `FOR ... LOOP`, or `WHILE ... LOOP`) -- a per-iteration notice on a bulk operation is a common, easy-to-miss log-noise/performance footgun. Flags any RAISE NOTICE inside a loop body regardless of further conditional nesting inside that loop -- precise "is this actually unconditional" control-flow analysis is out of scope; this is a nudge, not a certainty.

<sub>`dsl-analysis/src/rules/raise_notice_in_hot_loop.rs`</sub>

### `sql804` — a PL/pgSQL `DECLARE x type;` variable that's never referenced anywhere after `BEGIN`

a PL/pgSQL `DECLARE x type;` variable that's never referenced anywhere after `BEGIN`. Classic dead-code smell. Only handles the simple single top-level `DECLARE ... BEGIN` block shape; nested DECLARE blocks are out of scope.

<sub>`dsl-analysis/src/rules/variable_declared_unused.rs`</sub>

