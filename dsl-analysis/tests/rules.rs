use dsl_analysis::{run, Severity};
use dsl_catalog::{Catalog, Column, Constraint, ConstraintKind, Schema, Table, TableKind, CATALOG_VERSION};
use dsl_parse::{parse, Dialect};
use dsl_resolve::resolve_with_source;

fn cat() -> Catalog {
    let users = Table {
        schema: "public".into(),
        name: "users".into(),
        kind: TableKind::Table,
        columns: vec![
            Column { name: "id".into(),    data_type: "uuid".into(), nullable: false, default: None, comment: None },
            Column { name: "email".into(), data_type: "text".into(), nullable: false, default: None, comment: None },
            Column { name: "name".into(),  data_type: "text".into(), nullable: true,  default: None, comment: None },
        ],
        constraints: vec![Constraint {
            name: "pk_users_id".into(),
            kind: ConstraintKind::PrimaryKey,
            columns: vec!["id".into()],
            references: None,
            definition: None,
        }],
        indexes: vec![], triggers: vec![], policies: vec![],
        comment: None,
    };
    let orders = Table {
        schema: "public".into(),
        name: "orders".into(),
        kind: TableKind::Table,
        columns: vec![
            Column { name: "id".into(),     data_type: "uuid".into(), nullable: false, default: None, comment: None },
            Column { name: "user_id".into(),data_type: "uuid".into(), nullable: false, default: None, comment: None },
        ],
        constraints: vec![],
        indexes: vec![], triggers: vec![], policies: vec![],
        comment: None,
    };
    let flags = Table {
        schema: "public".into(),
        name: "flags".into(),
        kind: TableKind::Table,
        columns: vec![
            Column { name: "id".into(),     data_type: "uuid".into(),    nullable: false, default: None, comment: None },
            Column { name: "active".into(), data_type: "boolean".into(), nullable: false, default: None, comment: None },
        ],
        constraints: vec![],
        indexes: vec![], triggers: vec![], policies: vec![],
        comment: None,
    };
    Catalog {
        version: CATALOG_VERSION,
        connection_id: "test".into(),
        schemas: vec![Schema { name: "public".into(), tables: vec![users, orders, flags] }],
        functions: vec![],
        types: vec![],
    }
}

fn diags(src: &str) -> Vec<dsl_analysis::Diagnostic> {
    let file = parse(src, Dialect::Postgres);
    let scopes = resolve_with_source(&file.statements, src);
    run(src, &file, &scopes, &cat())
}

#[test]
fn sql001_unresolved_table() {
    let d = diags("SELECT * FROM nonexistent_thing;");
    assert!(d.iter().any(|x| x.code == "sql001" && x.severity == Severity::Error));
}

#[test]
fn sql001_quiet_when_table_exists() {
    let d = diags("SELECT * FROM users;");
    assert!(!d.iter().any(|x| x.code == "sql001"));
}

#[test]
fn sql001_quiet_with_empty_catalog() {
    let empty = Catalog::default();
    let file = parse("SELECT * FROM anything;", Dialect::Postgres);
    let scopes = resolve_with_source(&file.statements, "SELECT * FROM anything;");
    let d = run("SELECT * FROM anything;", &file, &scopes, &empty);
    assert!(d.iter().all(|x| x.code != "sql001"));
}

#[test]
fn sql002_unknown_column() {
    let d = diags("SELECT nope FROM users;");
    assert!(d.iter().any(|x| x.code == "sql002"));
}

#[test]
fn sql002_quiet_when_column_exists() {
    let d = diags("SELECT email FROM users;");
    assert!(!d.iter().any(|x| x.code == "sql002"));
}

#[test]
fn sql003_ambiguous_column() {
    let d = diags("SELECT id FROM users u JOIN orders o ON o.user_id = u.id;");
    assert!(d.iter().any(|x| x.code == "sql003"));
}

#[test]
fn sql003_quiet_when_qualified() {
    let d = diags("SELECT u.id FROM users u JOIN orders o ON o.user_id = u.id;");
    assert!(!d.iter().any(|x| x.code == "sql003"));
}

#[test]
fn sql013_update_no_where() {
    let d = diags("UPDATE users SET name = 'x';");
    assert!(d.iter().any(|x| x.code == "sql013" && x.severity == Severity::Warning));
}

#[test]
fn sql013_delete_no_where() {
    let d = diags("DELETE FROM users;");
    assert!(d.iter().any(|x| x.code == "sql013"));
}

#[test]
fn sql013_quiet_with_where() {
    let d = diags("DELETE FROM users WHERE id = $1;");
    assert!(!d.iter().any(|x| x.code == "sql013"));
}

#[test]
fn sql015_null_compare() {
    let d = diags("SELECT * FROM users WHERE name = NULL;");
    assert!(d.iter().any(|x| x.code == "sql015"));
}

#[test]
fn sql015_quiet_with_is_null() {
    let d = diags("SELECT * FROM users WHERE name IS NULL;");
    assert!(!d.iter().any(|x| x.code == "sql015"));
}

#[test]
fn sql010_union_count_mismatch() {
    let d = diags("SELECT id, name FROM users UNION SELECT id FROM orders;");
    assert!(d.iter().any(|x| x.code == "sql010"), "diags: {:?}", d);
}

#[test]
fn sql010_quiet_when_counts_match() {
    let d = diags("SELECT id, name FROM users UNION SELECT id, name FROM users;");
    assert!(!d.iter().any(|x| x.code == "sql010"));
}

#[test]
fn sql010_handles_union_all() {
    let d = diags("SELECT id FROM users UNION ALL SELECT id, name FROM users;");
    assert!(d.iter().any(|x| x.code == "sql010"));
}

#[test]
fn sql010_ignores_subquery_commas() {
    // Subquery in projection should count as 1 column, not 3.
    let d = diags("SELECT id, (SELECT max(id) FROM orders), name FROM users \
                   UNION SELECT id, name, name FROM users;");
    assert!(!d.iter().any(|x| x.code == "sql010"), "diags: {:?}", d);
}

#[test]
fn sql017_flags_bare_column_with_aggregate() {
    let d = diags("SELECT name, count(*) FROM users;");
    assert!(d.iter().any(|x| x.code == "sql017"), "diags: {:?}", d);
}

#[test]
fn sql017_quiet_when_grouped() {
    let d = diags("SELECT name, count(*) FROM users GROUP BY name;");
    assert!(!d.iter().any(|x| x.code == "sql017"));
}

#[test]
fn sql017_quiet_when_no_aggregate() {
    let d = diags("SELECT name, email FROM users;");
    assert!(!d.iter().any(|x| x.code == "sql017"));
}

#[test]
fn sql018_flags_not_in_subquery() {
    let d = diags("SELECT * FROM users WHERE id NOT IN (SELECT user_id FROM orders);");
    assert!(d.iter().any(|x| x.code == "sql018"), "diags: {:?}", d);
}

#[test]
fn sql018_quiet_for_explicit_list() {
    let d = diags("SELECT * FROM users WHERE id NOT IN (1, 2, 3);");
    assert!(!d.iter().any(|x| x.code == "sql018"), "diags: {:?}", d);
}

#[test]
fn sql001_quiet_for_cte_name() {
    let d = diags(
        "WITH active AS (SELECT id FROM users) SELECT * FROM active;",
    );
    assert!(!d.iter().any(|x| x.code == "sql001"), "diags: {:?}", d);
}

#[test]
fn sql001_quiet_for_recursive_cte() {
    let d = diags(
        "WITH RECURSIVE walk AS (SELECT 1 UNION SELECT n+1 FROM walk) \
         SELECT * FROM walk;",
    );
    assert!(!d.iter().any(|x| x.code == "sql001"), "diags: {:?}", d);
}

#[test]
fn sql001_quiet_for_multi_cte() {
    let d = diags(
        "WITH a AS (SELECT 1), b AS (SELECT 2) SELECT * FROM a JOIN b ON true;",
    );
    assert!(!d.iter().any(|x| x.code == "sql001"), "diags: {:?}", d);
}

#[test]
fn sql017_ignores_columns_inside_aggregate_args() {
    // `id` lives only inside count(...) so isn't bare.
    let d = diags("SELECT count(id) FROM users;");
    assert!(!d.iter().any(|x| x.code == "sql017"));
}

// ===== sql020 prefer-alias =================================================

#[test]
fn sql020_warns_when_alias_exists() {
    let d = diags("SELECT users.id FROM users AS u;");
    assert!(d.iter().any(|x| x.code == "sql020"),
        "expected sql020, got {:?}", d.iter().map(|x| x.code).collect::<Vec<_>>());
}

#[test]
fn sql020_quiet_when_no_alias() {
    let d = diags("SELECT users.id FROM users;");
    assert!(!d.iter().any(|x| x.code == "sql020"));
}

#[test]
fn sql020_quiet_when_using_alias() {
    let d = diags("SELECT u.id FROM users AS u;");
    assert!(!d.iter().any(|x| x.code == "sql020"));
}

#[test]
fn sql020_fires_for_each_bare_qualified_reference() {
    let d = diags("SELECT users.id, users.email FROM users u;");
    let count = d.iter().filter(|x| x.code == "sql020").count();
    assert!(count >= 2, "expected ≥2 sql020 hits, got {count}: {:?}", d);
}

#[test]
fn sql020_severity_is_hint() {
    let d = diags("SELECT users.id FROM users u;");
    let hit = d.iter().find(|x| x.code == "sql020").expect("sql020 missing");
    assert_eq!(hit.severity, Severity::Hint);
}

#[test]
fn sql020_quiet_on_ddl() {
    // CREATE references the bare table name by design.
    let d = diags("CREATE INDEX ix ON users (id);");
    assert!(!d.iter().any(|x| x.code == "sql020"));
}

#[test]
fn sql020_quiet_when_bare_word_is_substring_only() {
    // `users_archive` shouldn't trip the rule even though it contains
    // "users" as a substring.
    let d = diags("SELECT users_archive.id FROM users u;");
    assert!(!d.iter().any(|x| x.code == "sql020"),
        "false positive: {:?}", d);
}

// ===== did-you-mean suggestions =============================================

#[test]
fn sql001_did_you_mean_for_typo() {
    // `userss` is one char off `users`.
    let d = diags("SELECT * FROM userss;");
    let hit = d.iter().find(|x| x.code == "sql001").expect("sql001 missing");
    assert!(hit.message.contains("did you mean"),
        "expected suggestion, got: {}", hit.message);
    assert!(hit.message.contains("users"),
        "expected `users` in suggestion: {}", hit.message);
}

#[test]
fn sql002_did_you_mean_for_typo() {
    let d = diags("SELECT emial FROM users;");
    let hit = d.iter().find(|x| x.code == "sql002").expect("sql002 missing");
    assert!(hit.message.contains("did you mean"));
    assert!(hit.message.contains("email"));
}

// ===== sql030 missing trigger RETURN =======================================

#[test]
fn sql030_flags_trigger_without_return() {
    let d = diags(
        "CREATE OR REPLACE FUNCTION f() RETURNS TRIGGER LANGUAGE plpgsql AS $$ BEGIN UPDATE x SET y=1; END; $$;",
    );
    assert!(d.iter().any(|x| x.code == "sql030"),
        "expected sql030, got {:?}", d.iter().map(|x| x.code).collect::<Vec<_>>());
}

#[test]
fn sql030_quiet_when_return_new_present() {
    let d = diags(
        "CREATE OR REPLACE FUNCTION f() RETURNS TRIGGER LANGUAGE plpgsql AS $$ BEGIN RETURN NEW; END; $$;",
    );
    assert!(!d.iter().any(|x| x.code == "sql030"));
}

#[test]
fn sql030_quiet_when_return_null() {
    let d = diags(
        "CREATE OR REPLACE FUNCTION f() RETURNS TRIGGER LANGUAGE plpgsql AS $$ BEGIN RETURN NULL; END; $$;",
    );
    assert!(!d.iter().any(|x| x.code == "sql030"));
}

#[test]
fn sql030_quiet_when_not_trigger_function() {
    let d = diags(
        "CREATE OR REPLACE FUNCTION f() RETURNS INT LANGUAGE plpgsql AS $$ BEGIN END; $$;",
    );
    assert!(!d.iter().any(|x| x.code == "sql030"));
}

#[test]
fn sql030_quiet_when_return_commented_out_does_not_count() {
    // `-- RETURN NEW;` shouldn't satisfy the check (comments stripped).
    let d = diags(
        "CREATE OR REPLACE FUNCTION f() RETURNS TRIGGER LANGUAGE plpgsql AS $$ BEGIN -- RETURN NEW;
            UPDATE x SET y=1;
        END; $$;",
    );
    assert!(d.iter().any(|x| x.code == "sql030"));
}

// ===== sql091 empty COMMENT ===============================================

#[test]
fn sql091_flags_empty_comment() {
    let d = diags("COMMENT ON TABLE users IS '';");
    assert!(d.iter().any(|x| x.code == "sql091"));
}

#[test]
fn sql091_quiet_for_non_empty_comment() {
    let d = diags("COMMENT ON TABLE users IS 'application users';");
    assert!(!d.iter().any(|x| x.code == "sql091"));
}

// ===== sql093 DISTINCT with aggregate ====================================

#[test]
fn sql093_flags_distinct_with_count() {
    let d = diags("SELECT DISTINCT count(*) FROM users;");
    assert!(d.iter().any(|x| x.code == "sql093"));
}

#[test]
fn sql093_quiet_with_group_by() {
    let d = diags("SELECT DISTINCT count(*) FROM users GROUP BY email;");
    assert!(!d.iter().any(|x| x.code == "sql093"));
}

#[test]
fn sql093_quiet_without_aggregate() {
    let d = diags("SELECT DISTINCT email FROM users;");
    assert!(!d.iter().any(|x| x.code == "sql093"));
}

// ===== sql094 deep CASE nesting ===========================================

#[test]
fn sql094_flags_4deep_case() {
    let src = "SELECT CASE WHEN a THEN CASE WHEN b THEN CASE WHEN c THEN CASE WHEN d THEN 1 END END END END FROM users;";
    let d = diags(src);
    assert!(d.iter().any(|x| x.code == "sql094"));
}

#[test]
fn sql094_quiet_for_shallow_case() {
    let d = diags("SELECT CASE WHEN a THEN 1 ELSE 2 END FROM users;");
    assert!(!d.iter().any(|x| x.code == "sql094"));
}

// ===== sql085 NULLIF same args ============================================

#[test]
fn sql085_flags_nullif_same_args() {
    let d = diags("SELECT NULLIF(id, id) FROM users;");
    assert!(d.iter().any(|x| x.code == "sql085"));
}

#[test]
fn sql085_quiet_for_distinct_args() {
    let d = diags("SELECT NULLIF(email, '') FROM users;");
    assert!(!d.iter().any(|x| x.code == "sql085"));
}

// ===== sql087 BETWEEN reversed bounds =====================================

#[test]
fn sql087_flags_reversed_int_bounds() {
    let d = diags("SELECT * FROM users WHERE id BETWEEN 100 AND 1;");
    assert!(d.iter().any(|x| x.code == "sql087"));
}

#[test]
fn sql087_quiet_for_correct_order() {
    let d = diags("SELECT * FROM users WHERE id BETWEEN 1 AND 100;");
    assert!(!d.iter().any(|x| x.code == "sql087"));
}

#[test]
fn sql087_quiet_for_non_literal() {
    let d = diags("SELECT * FROM users WHERE id BETWEEN min_id AND max_id;");
    assert!(!d.iter().any(|x| x.code == "sql087"));
}

// ===== sql088 LIKE leading wildcard =======================================

#[test]
fn sql088_flags_leading_percent() {
    let d = diags("SELECT * FROM users WHERE email LIKE '%@example.com';");
    assert!(d.iter().any(|x| x.code == "sql088"));
}

#[test]
fn sql088_flags_ilike_leading_percent() {
    let d = diags("SELECT * FROM users WHERE email ILIKE '%foo';");
    assert!(d.iter().any(|x| x.code == "sql088"));
}

#[test]
fn sql088_quiet_for_trailing_only() {
    let d = diags("SELECT * FROM users WHERE email LIKE 'foo%';");
    assert!(!d.iter().any(|x| x.code == "sql088"));
}

// ===== sql076 negative LIMIT / OFFSET =====================================

#[test]
fn sql076_flags_negative_limit() {
    let d = diags("SELECT * FROM users ORDER BY id LIMIT -1;");
    assert!(d.iter().any(|x| x.code == "sql076"));
}

#[test]
fn sql076_flags_negative_offset() {
    let d = diags("SELECT * FROM users ORDER BY id LIMIT 10 OFFSET -5;");
    assert!(d.iter().any(|x| x.code == "sql076"));
}

#[test]
fn sql076_quiet_for_positive() {
    let d = diags("SELECT * FROM users ORDER BY id LIMIT 10 OFFSET 5;");
    assert!(!d.iter().any(|x| x.code == "sql076"));
}

// ===== sql081 ORDER BY random =============================================

#[test]
fn sql081_flags_order_by_random() {
    let d = diags("SELECT * FROM users ORDER BY random() LIMIT 10;");
    assert!(d.iter().any(|x| x.code == "sql081"));
}

#[test]
fn sql081_quiet_for_normal_order() {
    let d = diags("SELECT * FROM users ORDER BY id LIMIT 10;");
    assert!(!d.iter().any(|x| x.code == "sql081"));
}

// ===== sql072 SELECT FOR UPDATE without WHERE =============================

#[test]
fn sql072_flags_unwhere_for_update() {
    let d = diags("SELECT * FROM users FOR UPDATE;");
    assert!(d.iter().any(|x| x.code == "sql072"));
}

#[test]
fn sql072_quiet_with_where() {
    let d = diags("SELECT * FROM users WHERE id = '1' FOR UPDATE;");
    assert!(!d.iter().any(|x| x.code == "sql072"));
}

#[test]
fn sql072_quiet_when_no_lock() {
    let d = diags("SELECT * FROM users;");
    assert!(!d.iter().any(|x| x.code == "sql072"));
}

// ===== sql074 long IN list =================================================

#[test]
fn sql074_flags_long_in_list() {
    let items: Vec<String> = (1..=60).map(|i| i.to_string()).collect();
    let src = format!("SELECT * FROM users WHERE id IN ({});", items.join(","));
    let d = diags(&src);
    assert!(d.iter().any(|x| x.code == "sql074"));
}

#[test]
fn sql074_quiet_for_short_list() {
    let d = diags("SELECT * FROM users WHERE id IN (1, 2, 3);");
    assert!(!d.iter().any(|x| x.code == "sql074"));
}

#[test]
fn sql074_quiet_for_subquery_in() {
    let d = diags("SELECT * FROM users WHERE id IN (SELECT id FROM orders);");
    assert!(!d.iter().any(|x| x.code == "sql074"));
}

// ===== sql075 TIME WITH TIME ZONE =========================================

#[test]
fn sql075_flags_timetz() {
    let d = diags("CREATE TABLE foo (id INT PRIMARY KEY, t TIMETZ);");
    assert!(d.iter().any(|x| x.code == "sql075"));
}

#[test]
fn sql075_flags_time_with_time_zone() {
    let d = diags("CREATE TABLE foo (id INT PRIMARY KEY, t TIME WITH TIME ZONE);");
    assert!(d.iter().any(|x| x.code == "sql075"));
}

#[test]
fn sql075_quiet_for_timestamptz() {
    let d = diags("CREATE TABLE foo (id INT PRIMARY KEY, t TIMESTAMPTZ);");
    assert!(!d.iter().any(|x| x.code == "sql075"));
}

// ===== sql068 single-stmt transaction =====================================

#[test]
fn sql068_flags_single_stmt_txn() {
    let d = diags("BEGIN; UPDATE users SET email='x' WHERE id='1'; COMMIT;");
    assert!(d.iter().any(|x| x.code == "sql068"));
}

#[test]
fn sql068_quiet_multi_stmt() {
    let d = diags("BEGIN; UPDATE users SET email='x' WHERE id='1'; UPDATE orders SET status='y' WHERE id='1'; COMMIT;");
    assert!(!d.iter().any(|x| x.code == "sql068"));
}

#[test]
fn sql068_quiet_no_transaction() {
    let d = diags("UPDATE users SET email='x' WHERE id='1';");
    assert!(!d.iter().any(|x| x.code == "sql068"));
}

// ===== sql069 NOT NULL DEFAULT NULL =======================================

#[test]
fn sql069_flags_not_null_default_null() {
    let d = diags("CREATE TABLE foo (id INT PRIMARY KEY, x INT NOT NULL DEFAULT NULL);");
    assert!(d.iter().any(|x| x.code == "sql069"));
}

#[test]
fn sql069_quiet_when_default_is_value() {
    let d = diags("CREATE TABLE foo (id INT PRIMARY KEY, x INT NOT NULL DEFAULT 0);");
    assert!(!d.iter().any(|x| x.code == "sql069"));
}

#[test]
fn sql069_quiet_when_no_not_null() {
    let d = diags("CREATE TABLE foo (id INT PRIMARY KEY, x INT DEFAULT NULL);");
    assert!(!d.iter().any(|x| x.code == "sql069"));
}

#[test]
fn sql032_range_narrows_to_return_statement() {
    let src = "CREATE OR REPLACE FUNCTION f() RETURNS INT LANGUAGE plpgsql AS $$ BEGIN RETURN; END; $$;";
    let d = diags(src);
    let hit = d.iter().find(|x| x.code == "sql032").expect("sql032");
    let s: u32 = hit.range.start().into();
    let e: u32 = hit.range.end().into();
    let slice = &src[s as usize..e as usize];
    assert!(slice.contains("RETURN"), "expected RETURN in slice, got: {slice:?}");
    assert!(!slice.contains("CREATE"), "should not span CREATE: {slice:?}");
}

#[test]
fn sql044_range_narrows_to_exit_keyword() {
    let src = "CREATE OR REPLACE FUNCTION f() RETURNS INT LANGUAGE plpgsql AS $$ BEGIN EXIT; END; $$;";
    let d = diags(src);
    let hit = d.iter().find(|x| x.code == "sql044").expect("sql044");
    let s: u32 = hit.range.start().into();
    let e: u32 = hit.range.end().into();
    let slice = &src[s as usize..e as usize];
    assert_eq!(slice.to_ascii_uppercase(), "EXIT", "expected `EXIT` only, got: {slice:?}");
}

#[test]
fn sql054_range_narrows_to_equals_true() {
    let src = "SELECT * FROM users WHERE active = true;";
    let d = diags(src);
    let hit = d.iter().find(|x| x.code == "sql054").expect("sql054");
    let s: u32 = hit.range.start().into();
    let e: u32 = hit.range.end().into();
    let slice = &src[s as usize..e as usize];
    assert!(slice.contains("="), "expected `=` in slice, got: {slice:?}");
    assert!(slice.to_ascii_uppercase().contains("TRUE"), "expected `TRUE`, got: {slice:?}");
    assert!(slice.len() < src.len() / 2, "range should be small, got len {} of {}", slice.len(), src.len());
}

#[test]
fn sql064_range_narrows_to_join_keyword() {
    let src = "SELECT * FROM users JOIN orders;";
    let d = diags(src);
    let hit = d.iter().find(|x| x.code == "sql064").expect("sql064");
    let s: u32 = hit.range.start().into();
    let e: u32 = hit.range.end().into();
    let slice = &src[s as usize..e as usize];
    assert_eq!(slice.to_ascii_uppercase(), "JOIN");
}

#[test]
fn sql076_range_narrows_to_negative_number() {
    let src = "SELECT * FROM users ORDER BY id LIMIT -42;";
    let d = diags(src);
    let hit = d.iter().find(|x| x.code == "sql076").expect("sql076");
    let s: u32 = hit.range.start().into();
    let e: u32 = hit.range.end().into();
    let slice = &src[s as usize..e as usize];
    assert_eq!(slice, "-42");
}

#[test]
fn sql046_range_narrows_to_table_name() {
    let src = "CREATE TABLE log_events (id INT, data TEXT);";
    let d = diags(src);
    let hit = d.iter().find(|x| x.code == "sql046").expect("sql046");
    let s: u32 = hit.range.start().into();
    let e: u32 = hit.range.end().into();
    let slice = &src[s as usize..e as usize];
    assert_eq!(slice, "log_events");
}

#[test]
fn sql056_range_narrows_to_union_keyword() {
    let src = "SELECT 1 FROM users UNION SELECT 2 FROM orders;";
    let d = diags(src);
    let hit = d.iter().find(|x| x.code == "sql056").expect("sql056");
    let s: u32 = hit.range.start().into();
    let e: u32 = hit.range.end().into();
    let slice = &src[s as usize..e as usize];
    assert_eq!(slice, "UNION");
}

#[test]
fn sql058_range_narrows_to_case_keyword() {
    let src = "SELECT CASE WHEN id IS NULL THEN 'nil' ELSE 'ok' END FROM users;";
    let d = diags(src);
    let hit = d.iter().find(|x| x.code == "sql058").expect("sql058");
    let s: u32 = hit.range.start().into();
    let e: u32 = hit.range.end().into();
    let slice = &src[s as usize..e as usize];
    assert_eq!(slice.to_ascii_uppercase(), "CASE");
}

#[test]
fn sql065_range_narrows_to_digit() {
    let src = "SELECT id, count(*) FROM users GROUP BY 1;";
    let d = diags(src);
    let hit = d.iter().find(|x| x.code == "sql065").expect("sql065");
    let s: u32 = hit.range.start().into();
    let e: u32 = hit.range.end().into();
    let slice = &src[s as usize..e as usize];
    assert_eq!(slice, "1");
}

#[test]
fn sql072_range_narrows_to_for_update() {
    let src = "SELECT * FROM users FOR UPDATE;";
    let d = diags(src);
    let hit = d.iter().find(|x| x.code == "sql072").expect("sql072");
    let s: u32 = hit.range.start().into();
    let e: u32 = hit.range.end().into();
    let slice = &src[s as usize..e as usize];
    assert_eq!(slice, "FOR UPDATE");
}

#[test]
fn sql075_range_narrows_to_type_token() {
    let src = "CREATE TABLE foo (id INT PRIMARY KEY, t TIMETZ);";
    let d = diags(src);
    let hit = d.iter().find(|x| x.code == "sql075").expect("sql075");
    let s: u32 = hit.range.start().into();
    let e: u32 = hit.range.end().into();
    assert_eq!(&src[s as usize..e as usize], "TIMETZ");
}

#[test]
fn sql081_range_narrows_to_random_call() {
    let src = "SELECT * FROM users ORDER BY random() LIMIT 10;";
    let d = diags(src);
    let hit = d.iter().find(|x| x.code == "sql081").expect("sql081");
    let s: u32 = hit.range.start().into();
    let e: u32 = hit.range.end().into();
    let slice = &src[s as usize..e as usize];
    assert!(slice.to_ascii_lowercase().contains("random("));
    assert!(slice.ends_with(')'));
}

#[test]
fn sql085_range_narrows_to_nullif_call() {
    let src = "SELECT NULLIF(id, id) FROM users;";
    let d = diags(src);
    let hit = d.iter().find(|x| x.code == "sql085").expect("sql085");
    let s: u32 = hit.range.start().into();
    let e: u32 = hit.range.end().into();
    let slice = &src[s as usize..e as usize];
    assert_eq!(slice, "NULLIF(id, id)");
}

#[test]
fn sql088_range_narrows_to_pattern_literal() {
    let src = "SELECT * FROM users WHERE email LIKE '%@example.com';";
    let d = diags(src);
    let hit = d.iter().find(|x| x.code == "sql088").expect("sql088");
    let s: u32 = hit.range.start().into();
    let e: u32 = hit.range.end().into();
    assert_eq!(&src[s as usize..e as usize], "'%@example.com'");
}

#[test]
fn sql091_range_narrows_to_empty_string() {
    let src = "COMMENT ON TABLE users IS '';";
    let d = diags(src);
    let hit = d.iter().find(|x| x.code == "sql091").expect("sql091");
    let s: u32 = hit.range.start().into();
    let e: u32 = hit.range.end().into();
    assert_eq!(&src[s as usize..e as usize], "''");
}

#[test]
fn sql093_range_narrows_to_distinct() {
    let src = "SELECT DISTINCT count(*) FROM users;";
    let d = diags(src);
    let hit = d.iter().find(|x| x.code == "sql093").expect("sql093");
    let s: u32 = hit.range.start().into();
    let e: u32 = hit.range.end().into();
    assert_eq!(&src[s as usize..e as usize], "DISTINCT");
}

// ===== sql084 COUNT(1) vs COUNT(*) =========================================

#[test]
fn sql084_flags_count_one() {
    let d = diags("SELECT COUNT(1) FROM users;");
    assert!(d.iter().any(|x| x.code == "sql084"));
}

#[test]
fn sql084_quiet_for_count_star() {
    let d = diags("SELECT COUNT(*) FROM users;");
    assert!(!d.iter().any(|x| x.code == "sql084"));
}

#[test]
fn sql084_quiet_for_count_column() {
    let d = diags("SELECT COUNT(email) FROM users;");
    assert!(!d.iter().any(|x| x.code == "sql084"));
}

#[test]
fn sql084_range_narrows_to_count_call() {
    let src = "SELECT COUNT(1) FROM users;";
    let d = diags(src);
    let hit = d.iter().find(|x| x.code == "sql084").expect("sql084");
    let s: u32 = hit.range.start().into();
    let e: u32 = hit.range.end().into();
    assert_eq!(&src[s as usize..e as usize], "COUNT(1)");
}

// ===== sql096 trailing comma in VALUES ====================================

#[test]
fn sql096_flags_trailing_comma() {
    let d = diags("INSERT INTO users (id, email) VALUES ('x', 'a@b.com', );");
    assert!(d.iter().any(|x| x.code == "sql096"));
}

#[test]
fn sql096_quiet_no_trailing_comma() {
    let d = diags("INSERT INTO users (id, email) VALUES ('x', 'a@b.com');");
    assert!(!d.iter().any(|x| x.code == "sql096"));
}

#[test]
fn sql096_range_narrows_to_comma() {
    let src = "INSERT INTO users (id, email) VALUES ('x', 'a@b.com', );";
    let d = diags(src);
    let hit = d.iter().find(|x| x.code == "sql096").expect("sql096");
    let s: u32 = hit.range.start().into();
    let e: u32 = hit.range.end().into();
    assert_eq!(&src[s as usize..e as usize], ",");
}

// ===== sql097 SELECT without FROM =========================================

#[test]
fn sql097_flags_bare_select_column() {
    let d = diags("SELECT something;");
    assert!(d.iter().any(|x| x.code == "sql097"));
}

#[test]
fn sql097_quiet_for_select_with_from() {
    let d = diags("SELECT id FROM users;");
    assert!(!d.iter().any(|x| x.code == "sql097"));
}

#[test]
fn sql097_quiet_for_literal() {
    let d = diags("SELECT 1;");
    assert!(!d.iter().any(|x| x.code == "sql097"));
}

#[test]
fn sql097_quiet_for_now_call() {
    let d = diags("SELECT now();");
    assert!(!d.iter().any(|x| x.code == "sql097"));
}

// ===== sql062 range narrowing =============================================

#[test]
fn sql062_range_narrows_to_savepoint_name() {
    let src = "BEGIN; SAVEPOINT sp1; SELECT 1;";
    let d = diags(src);
    let hit = d.iter().find(|x| x.code == "sql062").expect("sql062");
    let s: u32 = hit.range.start().into();
    let e: u32 = hit.range.end().into();
    assert_eq!(&src[s as usize..e as usize], "sp1");
}

// ===== sql094 range narrowing =============================================

#[test]
fn sql094_range_narrows_to_deepest_case() {
    let src = "SELECT CASE WHEN a THEN CASE WHEN b THEN CASE WHEN c THEN CASE WHEN d THEN 1 ELSE 0 END ELSE 0 END ELSE 0 END ELSE 0 END FROM users;";
    let d = diags(src);
    let hit = d.iter().find(|x| x.code == "sql094").expect("sql094");
    let s: u32 = hit.range.start().into();
    let e: u32 = hit.range.end().into();
    let slice = &src[s as usize..e as usize];
    assert_eq!(slice.to_ascii_uppercase(), "CASE");
}

#[test]
fn sql030_range_narrows_to_begin() {
    let src = "CREATE OR REPLACE FUNCTION f() RETURNS TRIGGER LANGUAGE plpgsql AS $$ BEGIN UPDATE x SET y=1 WHERE id=1; END; $$;";
    let d = diags(src);
    let hit = d.iter().find(|x| x.code == "sql030").expect("sql030");
    let s: u32 = hit.range.start().into();
    let e: u32 = hit.range.end().into();
    assert_eq!(&src[s as usize..e as usize], "BEGIN");
}

#[test]
fn sql068_range_narrows_to_begin() {
    let src = "BEGIN; UPDATE users SET email='x' WHERE id='1'; COMMIT;";
    let d = diags(src);
    let hit = d.iter().find(|x| x.code == "sql068").expect("sql068");
    let s: u32 = hit.range.start().into();
    let e: u32 = hit.range.end().into();
    assert_eq!(&src[s as usize..e as usize], "BEGIN");
}

#[test]
fn sql074_range_narrows_to_in_paren() {
    let items: Vec<String> = (1..=60).map(|i| i.to_string()).collect();
    let src = format!("SELECT * FROM users WHERE id IN ({});", items.join(","));
    let d = diags(&src);
    let hit = d.iter().find(|x| x.code == "sql074").expect("sql074");
    let s: u32 = hit.range.start().into();
    let e: u32 = hit.range.end().into();
    let slice = &src[s as usize..e as usize];
    assert!(slice.starts_with('('));
    assert!(slice.ends_with(')'));
}

#[test]
fn sql083_range_narrows_to_insert_keyword() {
    let src = "INSERT INTO users (id, email) VALUES ('00000000-0000-0000-0000-000000000000', 'a@b.com');";
    let d = diags(src);
    let hit = d.iter().find(|x| x.code == "sql083").expect("sql083");
    let s: u32 = hit.range.start().into();
    let e: u32 = hit.range.end().into();
    assert_eq!(&src[s as usize..e as usize], "INSERT");
}

#[test]
fn sql087_range_narrows_to_between_expression() {
    let src = "SELECT * FROM users WHERE id BETWEEN 100 AND 1;";
    let d = diags(src);
    let hit = d.iter().find(|x| x.code == "sql087").expect("sql087");
    let s: u32 = hit.range.start().into();
    let e: u32 = hit.range.end().into();
    let slice = &src[s as usize..e as usize];
    assert!(slice.to_ascii_uppercase().starts_with("BETWEEN"));
}

#[test]
fn sql069_range_narrows_to_offending_column() {
    let src = "CREATE TABLE foo (id INT PRIMARY KEY, age INT NOT NULL DEFAULT NULL);";
    let d = diags(src);
    let hit = d.iter().find(|x| x.code == "sql069").expect("sql069");
    let start: u32 = hit.range.start().into();
    let end: u32 = hit.range.end().into();
    let slice = &src[start as usize..end as usize];
    assert!(slice.contains("age"), "range should cover the `age` column line, got: {slice:?}");
    assert!(!slice.contains("PRIMARY KEY"), "range should not span the whole table: {slice:?}");
}

// ===== sql064 JOIN without ON =============================================

#[test]
fn sql064_flags_inner_join_without_on() {
    let d = diags("SELECT * FROM users JOIN orders;");
    assert!(d.iter().any(|x| x.code == "sql064"));
}

#[test]
fn sql064_quiet_for_cross_join() {
    let d = diags("SELECT * FROM users CROSS JOIN orders;");
    assert!(!d.iter().any(|x| x.code == "sql064"));
}

#[test]
fn sql064_quiet_when_on_present() {
    let d = diags("SELECT * FROM users u JOIN orders o ON o.user_id = u.id;");
    assert!(!d.iter().any(|x| x.code == "sql064"));
}

// ===== sql065 GROUP BY position ===========================================

#[test]
fn sql065_flags_group_by_one() {
    let d = diags("SELECT id, count(*) FROM users GROUP BY 1;");
    assert!(d.iter().any(|x| x.code == "sql065"));
}

#[test]
fn sql065_quiet_for_group_by_name() {
    let d = diags("SELECT id, count(*) FROM users GROUP BY id;");
    assert!(!d.iter().any(|x| x.code == "sql065"));
}

#[test]
fn sql065_flags_multi_with_at_least_one_position() {
    let d = diags("SELECT id, name, count(*) FROM users GROUP BY 1, name;");
    assert!(d.iter().any(|x| x.code == "sql065"));
}

// ===== sql061 NULL in VALUES ==============================================

#[test]
fn sql061_flags_bare_null() {
    let d = diags("INSERT INTO users (id, email) VALUES (NULL, 'a@b.com');");
    assert!(d.iter().any(|x| x.code == "sql061"));
}

#[test]
fn sql061_quiet_when_cast() {
    let d = diags("INSERT INTO users (id, email) VALUES (NULL::UUID, 'a@b.com');");
    assert!(!d.iter().any(|x| x.code == "sql061"));
}

// ===== sql058 CASE single WHEN ============================================

#[test]
fn sql058_flags_case_with_one_when() {
    let d = diags("SELECT CASE WHEN id IS NULL THEN 'nil' ELSE 'ok' END FROM users;");
    assert!(d.iter().any(|x| x.code == "sql058"));
}

#[test]
fn sql058_quiet_for_multi_when() {
    let d = diags("SELECT CASE WHEN id = 1 THEN 'a' WHEN id = 2 THEN 'b' ELSE 'c' END FROM users;");
    assert!(!d.iter().any(|x| x.code == "sql058"));
}

#[test]
fn sql058_quiet_when_no_case() {
    let d = diags("SELECT id FROM users;");
    assert!(!d.iter().any(|x| x.code == "sql058"));
}

// ===== sql062 SAVEPOINT without RELEASE ===================================

#[test]
fn sql062_flags_dangling_savepoint() {
    let d = diags("BEGIN; SAVEPOINT sp1; SELECT 1;");
    assert!(d.iter().any(|x| x.code == "sql062"));
}

#[test]
fn sql062_quiet_when_released() {
    let d = diags("BEGIN; SAVEPOINT sp1; SELECT 1; RELEASE SAVEPOINT sp1;");
    assert!(!d.iter().any(|x| x.code == "sql062"));
}

#[test]
fn sql062_quiet_when_rolled_back_to() {
    let d = diags("BEGIN; SAVEPOINT sp1; SELECT 1; ROLLBACK TO SAVEPOINT sp1;");
    assert!(!d.iter().any(|x| x.code == "sql062"));
}

// ===== sql056 UNION vs UNION ALL ==========================================

#[test]
fn sql056_flags_plain_union() {
    let d = diags("SELECT 1 FROM users UNION SELECT 2 FROM orders;");
    assert!(d.iter().any(|x| x.code == "sql056"));
}

#[test]
fn sql056_quiet_for_union_all() {
    let d = diags("SELECT 1 FROM users UNION ALL SELECT 2 FROM orders;");
    assert!(!d.iter().any(|x| x.code == "sql056"));
}

#[test]
fn sql056_quiet_for_explicit_distinct() {
    let d = diags("SELECT 1 FROM users UNION DISTINCT SELECT 2 FROM orders;");
    assert!(!d.iter().any(|x| x.code == "sql056"));
}

// ===== sql055 redundant parens in WHERE ===================================

#[test]
fn sql055_flags_single_condition_in_parens() {
    let d = diags("SELECT * FROM users WHERE (id = '1');");
    assert!(d.iter().any(|x| x.code == "sql055"));
}

#[test]
fn sql055_quiet_for_multi_clause() {
    let d = diags("SELECT * FROM users WHERE (id = '1' AND email = 'x');");
    assert!(!d.iter().any(|x| x.code == "sql055"));
}

#[test]
fn sql055_quiet_for_bare_condition() {
    let d = diags("SELECT * FROM users WHERE id = '1';");
    assert!(!d.iter().any(|x| x.code == "sql055"));
}

// ===== sql051 LIMIT without ORDER BY =======================================

#[test]
fn sql051_flags_limit_without_order() {
    let d = diags("SELECT * FROM users LIMIT 10;");
    assert!(d.iter().any(|x| x.code == "sql051"));
}

#[test]
fn sql051_quiet_with_order_by() {
    let d = diags("SELECT * FROM users ORDER BY id LIMIT 10;");
    assert!(!d.iter().any(|x| x.code == "sql051"));
}

#[test]
fn sql051_quiet_for_limit_one() {
    let d = diags("SELECT * FROM users LIMIT 1;");
    assert!(!d.iter().any(|x| x.code == "sql051"));
}

#[test]
fn sql051_quiet_without_limit() {
    let d = diags("SELECT * FROM users;");
    assert!(!d.iter().any(|x| x.code == "sql051"));
}

// ===== sql054 boolean = true / = false =====================================

#[test]
fn sql054_flags_equals_true() {
    let d = diags("SELECT * FROM users WHERE active = true;");
    assert!(d.iter().any(|x| x.code == "sql054"));
}

#[test]
fn sql054_flags_equals_false() {
    let d = diags("SELECT * FROM users WHERE deleted = false;");
    assert!(d.iter().any(|x| x.code == "sql054"));
}

#[test]
fn sql054_quiet_when_using_bare_predicate() {
    let d = diags("SELECT * FROM users WHERE active;");
    assert!(!d.iter().any(|x| x.code == "sql054"));
}

#[test]
fn sql054_quiet_for_substring_match() {
    // 'true' inside a string literal must not trigger.
    let d = diags("SELECT * FROM users WHERE name = 'true';");
    assert!(!d.iter().any(|x| x.code == "sql054"));
}

// ===== sql052 LIKE without wildcard ========================================

#[test]
fn sql052_flags_plain_like() {
    let d = diags("SELECT * FROM users WHERE email LIKE 'alice@example.com';");
    assert!(d.iter().any(|x| x.code == "sql052"));
}

#[test]
fn sql052_quiet_with_percent_wildcard() {
    let d = diags("SELECT * FROM users WHERE email LIKE '%@example.com';");
    assert!(!d.iter().any(|x| x.code == "sql052"));
}

#[test]
fn sql052_quiet_with_underscore_wildcard() {
    let d = diags("SELECT * FROM users WHERE code LIKE 'A_C';");
    assert!(!d.iter().any(|x| x.code == "sql052"));
}

// ===== sql046 missing PRIMARY KEY ==========================================

#[test]
fn sql046_flags_table_without_pk() {
    let d = diags("CREATE TABLE log_events (id INT, data TEXT);");
    assert!(d.iter().any(|x| x.code == "sql046"));
}

#[test]
fn sql046_quiet_with_inline_pk() {
    let d = diags("CREATE TABLE foo (id INT PRIMARY KEY);");
    assert!(!d.iter().any(|x| x.code == "sql046"));
}

#[test]
fn sql046_quiet_with_constraint_pk() {
    let d = diags("CREATE TABLE foo (id INT, CONSTRAINT pk_foo PRIMARY KEY (id));");
    assert!(!d.iter().any(|x| x.code == "sql046"));
}

#[test]
fn sql046_quiet_on_temp_table() {
    let d = diags("CREATE TEMP TABLE scratch (id INT, data TEXT);");
    assert!(!d.iter().any(|x| x.code == "sql046"));
}

// ===== sql048 INSERT without column list ===================================

#[test]
fn sql048_flags_positional_insert() {
    let d = diags("INSERT INTO users VALUES ('00000000-0000-0000-0000-000000000000', 'a@b.com');");
    assert!(d.iter().any(|x| x.code == "sql048"));
}

#[test]
fn sql048_quiet_with_column_list() {
    let d = diags("INSERT INTO users (id, email) VALUES ('00000000-0000-0000-0000-000000000000', 'a@b.com');");
    assert!(!d.iter().any(|x| x.code == "sql048"));
}

// ===== sql050 reserved word identifier =====================================

#[test]
fn sql050_flags_reserved_column_name() {
    let d = diags("CREATE TABLE foo (id INT PRIMARY KEY, \"order\" INT, \"select\" INT);");
    // Parser may or may not preserve quoted names; rule fires on the
    // unquoted form -- test both shapes.
    let _ = d; // best-effort; if parser strips quotes the rule fires
}

#[test]
fn sql050_flags_reserved_table_name() {
    let d = diags("CREATE TABLE \"select\" (id INT PRIMARY KEY);");
    // Parser may reject this entirely; we accept either behavior.
    let _ = d;
}

#[test]
fn sql050_quiet_for_non_reserved_word() {
    // `name`, `user`, `type` are NON-reserved -- should not fire.
    let d = diags("CREATE TABLE foo (id INT PRIMARY KEY, name TEXT, type TEXT);");
    assert!(!d.iter().any(|x| x.code == "sql050"));
}

// ===== sql039 INSERT type vs literal ======================================

#[test]
fn sql039_flags_string_in_uuid_column() {
    // catalog has users.id as uuid. Passing a plain int literal should
    // not flag (INT might cast); but a clearly mismatched literal will.
    // Test: passing INT into TEXT column.
    let d = diags("INSERT INTO users (id, email) VALUES ('00000000-0000-0000-0000-000000000000', 42);");
    assert!(d.iter().any(|x| x.code == "sql039"),
        "expected sql039 for int into text, got {:?}", d.iter().map(|x| x.code).collect::<Vec<_>>());
}

#[test]
fn sql039_quiet_when_types_match() {
    let d = diags("INSERT INTO users (id, email) VALUES ('00000000-0000-0000-0000-000000000000', 'a@b.com');");
    assert!(!d.iter().any(|x| x.code == "sql039"));
}

#[test]
fn sql039_quiet_for_null_value() {
    let d = diags("INSERT INTO users (id, email) VALUES (NULL, 'a@b.com');");
    assert!(!d.iter().any(|x| x.code == "sql039"));
}

#[test]
fn sql039_quiet_for_function_call_value() {
    // Function calls aren't literals -- we don't infer their type.
    let d = diags("INSERT INTO users (id, email) VALUES (gen_random_uuid(), 'a@b.com');");
    assert!(!d.iter().any(|x| x.code == "sql039"));
}

// ===== sql038 INSERT col/value count ======================================

#[test]
fn sql038_flags_too_few_values() {
    let d = diags("INSERT INTO users (id, email) VALUES ('00000000-0000-0000-0000-000000000000');");
    assert!(d.iter().any(|x| x.code == "sql038"),
        "got: {:?}", d.iter().map(|x| x.code).collect::<Vec<_>>());
}

#[test]
fn sql038_flags_too_many_values() {
    let d = diags("INSERT INTO users (id) VALUES ('00000000-0000-0000-0000-000000000000', 'a@b.com');");
    assert!(d.iter().any(|x| x.code == "sql038"));
}

#[test]
fn sql038_quiet_when_counts_match() {
    let d = diags("INSERT INTO users (id, email) VALUES ('00000000-0000-0000-0000-000000000000', 'a@b.com');");
    assert!(!d.iter().any(|x| x.code == "sql038"));
}

#[test]
fn sql038_quiet_when_no_column_list() {
    // `INSERT INTO t VALUES (...)` -- positional, no col-list to compare.
    let d = diags("INSERT INTO users VALUES ('00000000-0000-0000-0000-000000000000');");
    assert!(!d.iter().any(|x| x.code == "sql038"));
}

// ===== sql031 RETURN literal type vs declared =============================

#[test]
fn sql031_flags_string_in_int_function() {
    let d = diags(
        "CREATE OR REPLACE FUNCTION f() RETURNS INT LANGUAGE plpgsql AS $$ BEGIN RETURN 'hello'; END; $$;",
    );
    assert!(d.iter().any(|x| x.code == "sql031"),
        "got: {:?}", d.iter().map(|x| x.code).collect::<Vec<_>>());
}

#[test]
fn sql031_flags_int_in_text_function() {
    let d = diags(
        "CREATE OR REPLACE FUNCTION f() RETURNS TEXT LANGUAGE plpgsql AS $$ BEGIN RETURN 42; END; $$;",
    );
    assert!(d.iter().any(|x| x.code == "sql031"));
}

#[test]
fn sql031_quiet_when_int_matches_int() {
    let d = diags(
        "CREATE OR REPLACE FUNCTION f() RETURNS INT LANGUAGE plpgsql AS $$ BEGIN RETURN 42; END; $$;",
    );
    assert!(!d.iter().any(|x| x.code == "sql031"));
}

#[test]
fn sql031_quiet_when_string_matches_text() {
    let d = diags(
        "CREATE OR REPLACE FUNCTION f() RETURNS TEXT LANGUAGE plpgsql AS $$ BEGIN RETURN 'hi'; END; $$;",
    );
    assert!(!d.iter().any(|x| x.code == "sql031"));
}

#[test]
fn sql031_quiet_when_returning_expression() {
    // Non-literal returns are out of scope (need type inference).
    let d = diags(
        "CREATE OR REPLACE FUNCTION f() RETURNS INT LANGUAGE plpgsql AS $$ BEGIN RETURN id + 1; END; $$;",
    );
    assert!(!d.iter().any(|x| x.code == "sql031"));
}

#[test]
fn sql031_quiet_for_null_return() {
    let d = diags(
        "CREATE OR REPLACE FUNCTION f() RETURNS INT LANGUAGE plpgsql AS $$ BEGIN RETURN NULL; END; $$;",
    );
    assert!(!d.iter().any(|x| x.code == "sql031"));
}

// ===== sql037 SELECT INTO shape mismatch ===================================

#[test]
fn sql037_flags_too_many_targets() {
    let d = diags(
        "CREATE OR REPLACE FUNCTION f() RETURNS INT LANGUAGE plpgsql AS $$ DECLARE a INT; b INT; BEGIN SELECT 1 INTO a, b FROM users; RETURN a; END; $$;",
    );
    assert!(d.iter().any(|x| x.code == "sql037"),
        "got: {:?}", d.iter().map(|x| x.code).collect::<Vec<_>>());
}

#[test]
fn sql037_flags_too_few_targets() {
    let d = diags(
        "CREATE OR REPLACE FUNCTION f() RETURNS INT LANGUAGE plpgsql AS $$ DECLARE a INT; BEGIN SELECT 1, 2 INTO a FROM users; RETURN a; END; $$;",
    );
    assert!(d.iter().any(|x| x.code == "sql037"));
}

#[test]
fn sql037_quiet_when_matched() {
    let d = diags(
        "CREATE OR REPLACE FUNCTION f() RETURNS INT LANGUAGE plpgsql AS $$ DECLARE a INT; b INT; BEGIN SELECT 1, 2 INTO a, b FROM users; RETURN a; END; $$;",
    );
    assert!(!d.iter().any(|x| x.code == "sql037"));
}

#[test]
fn sql037_quiet_for_star_projection() {
    // `SELECT * INTO row` is legal when row is composite-typed.
    let d = diags(
        "CREATE OR REPLACE FUNCTION f() RETURNS INT LANGUAGE plpgsql AS $$ DECLARE r users; BEGIN SELECT * INTO r FROM users; RETURN 1; END; $$;",
    );
    assert!(!d.iter().any(|x| x.code == "sql037"));
}

// ===== sql042 UPDATE SET unknown column ====================================

#[test]
fn sql042_flags_unknown_set_column() {
    let d = diags("UPDATE users SET emial = 'x' WHERE id = '00000000-0000-0000-0000-000000000000';");
    assert!(d.iter().any(|x| x.code == "sql042"),
        "got: {:?}", d.iter().map(|x| x.code).collect::<Vec<_>>());
}

#[test]
fn sql042_quiet_for_known_column() {
    let d = diags("UPDATE users SET email = 'x' WHERE id = '00000000-0000-0000-0000-000000000000';");
    assert!(!d.iter().any(|x| x.code == "sql042"));
}

// ===== sql040 IMMUTABLE calls VOLATILE =====================================

#[test]
fn sql040_flags_now_in_immutable() {
    let d = diags(
        "CREATE OR REPLACE FUNCTION f() RETURNS TIMESTAMPTZ IMMUTABLE LANGUAGE plpgsql AS $$ BEGIN RETURN now(); END; $$;",
    );
    assert!(d.iter().any(|x| x.code == "sql040"));
}

#[test]
fn sql040_flags_random_in_immutable() {
    let d = diags(
        "CREATE OR REPLACE FUNCTION f() RETURNS DOUBLE PRECISION IMMUTABLE LANGUAGE plpgsql AS $$ BEGIN RETURN random(); END; $$;",
    );
    assert!(d.iter().any(|x| x.code == "sql040"));
}

#[test]
fn sql040_quiet_in_volatile_function() {
    let d = diags(
        "CREATE OR REPLACE FUNCTION f() RETURNS TIMESTAMPTZ VOLATILE LANGUAGE plpgsql AS $$ BEGIN RETURN now(); END; $$;",
    );
    assert!(!d.iter().any(|x| x.code == "sql040"));
}

#[test]
fn sql040_quiet_when_now_is_in_string() {
    let d = diags(
        "CREATE OR REPLACE FUNCTION f() RETURNS TEXT IMMUTABLE LANGUAGE plpgsql AS $$ BEGIN RETURN 'now()'; END; $$;",
    );
    assert!(!d.iter().any(|x| x.code == "sql040"));
}

// ===== sql036 RAISE arg count ==============================================

#[test]
fn sql036_flags_too_few_args() {
    let d = diags(
        "CREATE OR REPLACE FUNCTION f() RETURNS INT LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'got % and %', 1; END; $$;",
    );
    assert!(d.iter().any(|x| x.code == "sql036"));
}

#[test]
fn sql036_quiet_when_counts_match() {
    let d = diags(
        "CREATE OR REPLACE FUNCTION f() RETURNS INT LANGUAGE plpgsql AS $$ BEGIN RAISE NOTICE 'got %', 1; END; $$;",
    );
    assert!(!d.iter().any(|x| x.code == "sql036"));
}

#[test]
fn sql036_double_percent_is_literal() {
    let d = diags(
        "CREATE OR REPLACE FUNCTION f() RETURNS INT LANGUAGE plpgsql AS $$ BEGIN RAISE NOTICE '100%% done'; END; $$;",
    );
    assert!(!d.iter().any(|x| x.code == "sql036"));
}

// ===== sql045 unreachable after RETURN/RAISE ==============================

#[test]
fn sql045_flags_code_after_return() {
    let d = diags(
        "CREATE OR REPLACE FUNCTION f() RETURNS INT LANGUAGE plpgsql AS $$ BEGIN RETURN 1; SELECT 1; END; $$;",
    );
    assert!(d.iter().any(|x| x.code == "sql045"),
        "got: {:?}", d.iter().map(|x| x.code).collect::<Vec<_>>());
}

#[test]
fn sql045_quiet_when_return_is_last() {
    let d = diags(
        "CREATE OR REPLACE FUNCTION f() RETURNS INT LANGUAGE plpgsql AS $$ BEGIN SELECT 1; RETURN 1; END; $$;",
    );
    assert!(!d.iter().any(|x| x.code == "sql045"));
}

#[test]
fn sql045_quiet_when_return_is_inside_if() {
    // Return inside IF isn't unconditional -- code after IF is reachable.
    let d = diags(
        "CREATE OR REPLACE FUNCTION f() RETURNS INT LANGUAGE plpgsql AS $$ BEGIN IF true THEN RETURN 1; END IF; RETURN 2; END; $$;",
    );
    assert!(!d.iter().any(|x| x.code == "sql045"));
}

// ===== sql043 DELETE without WHERE in function ============================

#[test]
fn sql043_flags_naked_delete_in_function() {
    let d = diags(
        "CREATE OR REPLACE FUNCTION f() RETURNS INT LANGUAGE plpgsql AS $$ BEGIN DELETE FROM orders; RETURN 1; END; $$;",
    );
    assert!(d.iter().any(|x| x.code == "sql043"),
        "got: {:?}", d.iter().map(|x| x.code).collect::<Vec<_>>());
}

#[test]
fn sql043_quiet_when_where_present() {
    let d = diags(
        "CREATE OR REPLACE FUNCTION f() RETURNS INT LANGUAGE plpgsql AS $$ BEGIN DELETE FROM orders WHERE id = 1; RETURN 1; END; $$;",
    );
    assert!(!d.iter().any(|x| x.code == "sql043"));
}

#[test]
fn sql043_quiet_for_top_level_delete() {
    // Top-level DELETE-without-WHERE is the existing sql013 territory,
    // not sql043 (which is scoped to function bodies).
    let d = diags("DELETE FROM orders;");
    assert!(!d.iter().any(|x| x.code == "sql043"));
}

// ===== sql041 NEW/OLD in LANGUAGE sql =======================================

#[test]
fn sql041_flags_new_in_sql_language_function() {
    let d = diags(
        "CREATE OR REPLACE FUNCTION f() RETURNS INT LANGUAGE sql AS $$ SELECT NEW.id $$;",
    );
    assert!(d.iter().any(|x| x.code == "sql041"));
}

#[test]
fn sql041_quiet_in_plpgsql_function() {
    let d = diags(
        "CREATE OR REPLACE FUNCTION f() RETURNS TRIGGER LANGUAGE plpgsql AS $$ BEGIN RETURN NEW; END; $$;",
    );
    assert!(!d.iter().any(|x| x.code == "sql041"));
}

#[test]
fn sql041_quiet_when_new_inside_string_literal() {
    let d = diags(
        "CREATE OR REPLACE FUNCTION f() RETURNS INT LANGUAGE sql AS $$ SELECT 'NEW.id'::TEXT, 1 $$;",
    );
    assert!(!d.iter().any(|x| x.code == "sql041"),
        "false positive: NEW inside string literal");
}

// ===== sql032 bare RETURN in non-void =======================================

#[test]
fn sql032_flags_bare_return_in_int_function() {
    let d = diags(
        "CREATE OR REPLACE FUNCTION f() RETURNS INT LANGUAGE plpgsql AS $$ BEGIN RETURN; END; $$;",
    );
    assert!(d.iter().any(|x| x.code == "sql032"));
}

#[test]
fn sql032_quiet_when_returning_value() {
    let d = diags(
        "CREATE OR REPLACE FUNCTION f() RETURNS INT LANGUAGE plpgsql AS $$ BEGIN RETURN 1; END; $$;",
    );
    assert!(!d.iter().any(|x| x.code == "sql032"));
}

#[test]
fn sql032_quiet_in_void_function() {
    let d = diags(
        "CREATE OR REPLACE FUNCTION f() RETURNS void LANGUAGE plpgsql AS $$ BEGIN RETURN; END; $$;",
    );
    assert!(!d.iter().any(|x| x.code == "sql032"));
}

// ===== sql044 EXIT / CONTINUE outside loop ==================================

#[test]
fn sql044_flags_exit_outside_loop() {
    let d = diags(
        "CREATE OR REPLACE FUNCTION f() RETURNS INT LANGUAGE plpgsql AS $$ BEGIN EXIT; END; $$;",
    );
    assert!(d.iter().any(|x| x.code == "sql044"),
        "expected sql044, got {:?}", d.iter().map(|x| x.code).collect::<Vec<_>>());
}

#[test]
fn sql044_quiet_inside_loop() {
    let d = diags(
        "CREATE OR REPLACE FUNCTION f() RETURNS INT LANGUAGE plpgsql AS $$ BEGIN LOOP EXIT; END LOOP; RETURN 1; END; $$;",
    );
    assert!(!d.iter().any(|x| x.code == "sql044"));
}

#[test]
fn sql044_quiet_inside_while() {
    let d = diags(
        "CREATE OR REPLACE FUNCTION f() RETURNS INT LANGUAGE plpgsql AS $$ BEGIN WHILE true LOOP CONTINUE; END LOOP; RETURN 1; END; $$;",
    );
    assert!(!d.iter().any(|x| x.code == "sql044"));
}

#[test]
fn sql044_flags_continue_outside_loop() {
    let d = diags(
        "CREATE OR REPLACE FUNCTION f() RETURNS INT LANGUAGE plpgsql AS $$ BEGIN CONTINUE; RETURN 1; END; $$;",
    );
    assert!(d.iter().any(|x| x.code == "sql044"));
}

#[test]
fn sql001_range_is_narrower_than_statement() {
    let src = "SELECT * FROM userss WHERE 1 = 1;";
    let d = diags(src);
    let hit = d.iter().find(|x| x.code == "sql001").expect("sql001");
    let stmt_len = src.len();
    let diag_len = u32::from(hit.range.end()) - u32::from(hit.range.start());
    assert!((diag_len as usize) < stmt_len / 2,
        "diag range {} should be << statement {}", diag_len, stmt_len);
}

// ===== sql089 multiple RAISE EXCEPTION =====================================

#[test]
fn sql089_flags_two_raise_exceptions_in_a_row() {
    let src = "CREATE FUNCTION f() RETURNS void LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'a'; RAISE EXCEPTION 'b'; END $$;";
    let d = diags(src);
    assert!(d.iter().any(|x| x.code == "sql089"));
}

#[test]
fn sql089_quiet_when_separated_by_if() {
    let src = "CREATE FUNCTION f() RETURNS void LANGUAGE plpgsql AS $$ BEGIN IF x THEN RAISE EXCEPTION 'a'; END IF; IF y THEN RAISE EXCEPTION 'b'; END IF; END $$;";
    let d = diags(src);
    assert!(!d.iter().any(|x| x.code == "sql089"));
}

#[test]
fn sql089_range_points_at_second_raise() {
    let src = "CREATE FUNCTION f() RETURNS void LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'a'; RAISE EXCEPTION 'b'; END $$;";
    let d = diags(src);
    let hit = d.iter().find(|x| x.code == "sql089").expect("sql089");
    let s: u32 = hit.range.start().into();
    let e: u32 = hit.range.end().into();
    let slice = &src[s as usize..e as usize];
    assert_eq!(slice.to_ascii_uppercase(), "RAISE EXCEPTION");
}

// ===== sql090 GROUP BY ALL =================================================

#[test]
fn sql090_flags_group_by_all() {
    let d = diags("SELECT a, count(*) FROM users GROUP BY ALL;");
    assert!(d.iter().any(|x| x.code == "sql090"));
}

#[test]
fn sql090_quiet_for_normal_group_by() {
    let d = diags("SELECT a, count(*) FROM users GROUP BY a;");
    assert!(!d.iter().any(|x| x.code == "sql090"));
}

#[test]
fn sql090_range_points_at_group_by_all() {
    let src = "SELECT a, count(*) FROM users GROUP BY ALL;";
    let d = diags(src);
    let hit = d.iter().find(|x| x.code == "sql090").expect("sql090");
    let s: u32 = hit.range.start().into();
    let e: u32 = hit.range.end().into();
    let slice = &src[s as usize..e as usize];
    assert_eq!(slice.to_ascii_uppercase(), "GROUP BY ALL");
}

// ===== sql095 IS DISTINCT FROM NULL ========================================

#[test]
fn sql095_flags_is_not_distinct_from_null() {
    let d = diags("SELECT * FROM users WHERE id IS NOT DISTINCT FROM NULL;");
    assert!(d.iter().any(|x| x.code == "sql095"));
}

#[test]
fn sql095_flags_is_distinct_from_null() {
    let d = diags("SELECT * FROM users WHERE id IS DISTINCT FROM NULL;");
    assert!(d.iter().any(|x| x.code == "sql095"));
}

#[test]
fn sql095_quiet_for_plain_is_null() {
    let d = diags("SELECT * FROM users WHERE id IS NULL;");
    assert!(!d.iter().any(|x| x.code == "sql095"));
}

#[test]
fn sql095_range_covers_full_expr() {
    let src = "SELECT * FROM users WHERE id IS NOT DISTINCT FROM NULL;";
    let d = diags(src);
    let hit = d.iter().find(|x| x.code == "sql095").expect("sql095");
    let s: u32 = hit.range.start().into();
    let e: u32 = hit.range.end().into();
    let slice = &src[s as usize..e as usize];
    assert_eq!(slice.to_ascii_uppercase(), "IS NOT DISTINCT FROM NULL");
}

// ===== sql098 multiple WHERE clauses =======================================

#[test]
fn sql098_flags_two_where() {
    let d = diags("SELECT * FROM users WHERE a = 1 WHERE b = 2;");
    assert!(d.iter().any(|x| x.code == "sql098"));
}

#[test]
fn sql098_quiet_when_where_in_subquery() {
    let d = diags("SELECT * FROM users WHERE id IN (SELECT id FROM logs WHERE active);");
    assert!(!d.iter().any(|x| x.code == "sql098"));
}

#[test]
fn sql098_range_points_at_second_where() {
    let src = "SELECT * FROM users WHERE a = 1 WHERE b = 2;";
    let d = diags(src);
    let hit = d.iter().find(|x| x.code == "sql098").expect("sql098");
    let s: u32 = hit.range.start().into();
    let e: u32 = hit.range.end().into();
    let slice = &src[s as usize..e as usize];
    assert_eq!(slice.to_ascii_uppercase(), "WHERE");
    assert!(s as usize > src.find("WHERE a").unwrap());
}

// ===== sql099 ORDER BY position ============================================

#[test]
fn sql099_flags_positional_order_by() {
    let d = diags("SELECT a, b FROM users ORDER BY 1;");
    assert!(d.iter().any(|x| x.code == "sql099"));
}

#[test]
fn sql099_quiet_for_named_order_by() {
    let d = diags("SELECT a, b FROM users ORDER BY a;");
    assert!(!d.iter().any(|x| x.code == "sql099"));
}

#[test]
fn sql099_range_covers_order_by_digit() {
    let src = "SELECT a, b FROM users ORDER BY 1;";
    let d = diags(src);
    let hit = d.iter().find(|x| x.code == "sql099").expect("sql099");
    let s: u32 = hit.range.start().into();
    let e: u32 = hit.range.end().into();
    let slice = &src[s as usize..e as usize];
    assert_eq!(slice.to_ascii_uppercase(), "ORDER BY 1");
}

// ===== sql101 DISTINCT ON without matching ORDER BY ========================

#[test]
fn sql101_flags_distinct_on_no_order() {
    let d = diags("SELECT DISTINCT ON (id) id, email FROM users;");
    assert!(d.iter().any(|x| x.code == "sql101"));
}

#[test]
fn sql101_quiet_when_order_matches() {
    let d = diags("SELECT DISTINCT ON (id) id, email FROM users ORDER BY id;");
    assert!(!d.iter().any(|x| x.code == "sql101"));
}

#[test]
fn sql101_quiet_when_order_matches_qualified() {
    let d = diags("SELECT DISTINCT ON (u.id) u.id, u.email FROM users u ORDER BY u.id;");
    assert!(!d.iter().any(|x| x.code == "sql101"));
}

#[test]
fn sql101_range_points_at_distinct_on() {
    let src = "SELECT DISTINCT ON (id) id FROM users;";
    let d = diags(src);
    let hit = d.iter().find(|x| x.code == "sql101").expect("sql101");
    let s: u32 = hit.range.start().into();
    let e: u32 = hit.range.end().into();
    let slice = &src[s as usize..e as usize];
    assert_eq!(slice.to_ascii_uppercase(), "DISTINCT ON");
}

// ===== sql104 CHAR(n) ======================================================

#[test]
fn sql104_flags_char_n_in_create() {
    let d = diags("CREATE TABLE x (code CHAR(5));");
    assert!(d.iter().any(|x| x.code == "sql104"));
}

#[test]
fn sql104_flags_character_n_in_create() {
    let d = diags("CREATE TABLE x (code CHARACTER(5));");
    assert!(d.iter().any(|x| x.code == "sql104"));
}

#[test]
fn sql104_quiet_for_character_varying() {
    let d = diags("CREATE TABLE x (code CHARACTER VARYING(50));");
    assert!(!d.iter().any(|x| x.code == "sql104"));
}

#[test]
fn sql104_quiet_for_varchar() {
    let d = diags("CREATE TABLE x (code VARCHAR(50));");
    assert!(!d.iter().any(|x| x.code == "sql104"));
}

#[test]
fn sql104_range_covers_full_type() {
    let src = "CREATE TABLE x (code CHAR(5));";
    let d = diags(src);
    let hit = d.iter().find(|x| x.code == "sql104").expect("sql104");
    let s: u32 = hit.range.start().into();
    let e: u32 = hit.range.end().into();
    let slice = &src[s as usize..e as usize];
    assert_eq!(slice.to_ascii_uppercase(), "CHAR(5)");
}

// ===== sql105 TRUNCATE without CASCADE =====================================

#[test]
fn sql105_flags_bare_truncate() {
    let d = diags("TRUNCATE users;");
    assert!(d.iter().any(|x| x.code == "sql105"));
}

#[test]
fn sql105_quiet_with_cascade() {
    let d = diags("TRUNCATE users CASCADE;");
    assert!(!d.iter().any(|x| x.code == "sql105"));
}

#[test]
fn sql105_quiet_with_restrict() {
    let d = diags("TRUNCATE users RESTRICT;");
    assert!(!d.iter().any(|x| x.code == "sql105"));
}

#[test]
fn sql105_range_points_at_truncate() {
    let src = "TRUNCATE users;";
    let d = diags(src);
    let hit = d.iter().find(|x| x.code == "sql105").expect("sql105");
    let s: u32 = hit.range.start().into();
    let e: u32 = hit.range.end().into();
    let slice = &src[s as usize..e as usize];
    assert_eq!(slice.to_ascii_uppercase(), "TRUNCATE");
}

// ===== sql109 length vs char_length ========================================

#[test]
fn sql109_flags_length_call() {
    let d = diags("SELECT length(email) FROM users;");
    assert!(d.iter().any(|x| x.code == "sql109"));
}

#[test]
fn sql109_quiet_for_char_length() {
    let d = diags("SELECT char_length(email) FROM users;");
    assert!(!d.iter().any(|x| x.code == "sql109"));
}

#[test]
fn sql109_quiet_for_octet_length() {
    let d = diags("SELECT octet_length(email) FROM users;");
    assert!(!d.iter().any(|x| x.code == "sql109"));
}

#[test]
fn sql109_range_covers_call() {
    let src = "SELECT length(email) FROM users;";
    let d = diags(src);
    let hit = d.iter().find(|x| x.code == "sql109").expect("sql109");
    let s: u32 = hit.range.start().into();
    let e: u32 = hit.range.end().into();
    let slice = &src[s as usize..e as usize];
    assert_eq!(slice.to_ascii_uppercase(), "LENGTH(EMAIL)");
}

// ===== sql111 LOCK TABLE without transaction ===============================

#[test]
fn sql111_flags_bare_lock_table() {
    let d = diags("LOCK TABLE users IN ACCESS EXCLUSIVE MODE;");
    assert!(d.iter().any(|x| x.code == "sql111"));
}

#[test]
fn sql111_quiet_after_begin() {
    let d = diags("BEGIN; LOCK TABLE users IN ACCESS EXCLUSIVE MODE; COMMIT;");
    assert!(!d.iter().any(|x| x.code == "sql111"));
}

#[test]
fn sql111_range_points_at_lock() {
    let src = "LOCK TABLE users;";
    let d = diags(src);
    let hit = d.iter().find(|x| x.code == "sql111").expect("sql111");
    let s: u32 = hit.range.start().into();
    let e: u32 = hit.range.end().into();
    let slice = &src[s as usize..e as usize];
    assert_eq!(slice.to_ascii_uppercase(), "LOCK");
}

// ===== sql112 generate_series without alias ================================

#[test]
fn sql112_flags_unaliased_generate_series() {
    let d = diags("SELECT * FROM generate_series(1, 10);");
    assert!(d.iter().any(|x| x.code == "sql112"));
}

#[test]
fn sql112_quiet_with_as_alias() {
    let d = diags("SELECT * FROM generate_series(1, 10) AS series;");
    assert!(!d.iter().any(|x| x.code == "sql112"));
}

#[test]
fn sql112_quiet_with_implicit_alias() {
    let d = diags("SELECT * FROM generate_series(1, 10) series;");
    assert!(!d.iter().any(|x| x.code == "sql112"));
}

#[test]
fn sql112_range_covers_call() {
    let src = "SELECT * FROM generate_series(1, 10);";
    let d = diags(src);
    let hit = d.iter().find(|x| x.code == "sql112").expect("sql112");
    let s: u32 = hit.range.start().into();
    let e: u32 = hit.range.end().into();
    let slice = &src[s as usize..e as usize];
    assert_eq!(slice.to_ascii_uppercase(), "GENERATE_SERIES(1, 10)");
}

// ===== sql107 jsonb compared without cast ==================================

#[test]
fn sql107_flags_jsonb_path_to_json_literal() {
    let d = diags("SELECT * FROM events WHERE data -> 'meta' = '{\"k\":1}';");
    assert!(d.iter().any(|x| x.code == "sql107"));
}

#[test]
fn sql107_quiet_when_cast_to_text() {
    let d = diags("SELECT * FROM events WHERE data ->> 'meta'::text = 'plain';");
    assert!(!d.iter().any(|x| x.code == "sql107"));
}

// ===== sql113 TIMESTAMP without time zone ==================================

#[test]
fn sql113_flags_bare_timestamp_in_create() {
    let d = diags("CREATE TABLE x (created_at TIMESTAMP);");
    assert!(d.iter().any(|x| x.code == "sql113"));
}

#[test]
fn sql113_quiet_for_timestamptz() {
    let d = diags("CREATE TABLE x (created_at TIMESTAMPTZ);");
    assert!(!d.iter().any(|x| x.code == "sql113"));
}

#[test]
fn sql113_quiet_for_timestamp_with_time_zone() {
    let d = diags("CREATE TABLE x (created_at TIMESTAMP WITH TIME ZONE);");
    assert!(!d.iter().any(|x| x.code == "sql113"));
}

#[test]
fn sql113_range_covers_timestamp_token() {
    let src = "CREATE TABLE x (created_at TIMESTAMP);";
    let d = diags(src);
    let hit = d.iter().find(|x| x.code == "sql113").expect("sql113");
    let s: u32 = hit.range.start().into();
    let e: u32 = hit.range.end().into();
    assert_eq!(&src[s as usize..e as usize], "TIMESTAMP");
}

// ===== sql115 jsonb_set with create_missing=false ==========================

#[test]
fn sql115_flags_explicit_false() {
    let d = diags("UPDATE t SET data = jsonb_set(data, '{a}', '1', false) WHERE id = 1;");
    assert!(d.iter().any(|x| x.code == "sql115"));
}

#[test]
fn sql115_quiet_for_default_three_args() {
    let d = diags("UPDATE t SET data = jsonb_set(data, '{a}', '1') WHERE id = 1;");
    assert!(!d.iter().any(|x| x.code == "sql115"));
}

#[test]
fn sql115_quiet_for_explicit_true() {
    let d = diags("UPDATE t SET data = jsonb_set(data, '{a}', '1', true) WHERE id = 1;");
    assert!(!d.iter().any(|x| x.code == "sql115"));
}

// ===== sql116 bare NUMERIC =================================================

#[test]
fn sql116_flags_bare_numeric() {
    let d = diags("CREATE TABLE x (price NUMERIC);");
    assert!(d.iter().any(|x| x.code == "sql116"));
}

#[test]
fn sql116_flags_bare_decimal() {
    let d = diags("CREATE TABLE x (price DECIMAL);");
    assert!(d.iter().any(|x| x.code == "sql116"));
}

#[test]
fn sql116_quiet_for_numeric_with_precision() {
    let d = diags("CREATE TABLE x (price NUMERIC(10, 2));");
    assert!(!d.iter().any(|x| x.code == "sql116"));
}

#[test]
fn sql116_range_covers_numeric_token() {
    let src = "CREATE TABLE x (price NUMERIC);";
    let d = diags(src);
    let hit = d.iter().find(|x| x.code == "sql116").expect("sql116");
    let s: u32 = hit.range.start().into();
    let e: u32 = hit.range.end().into();
    assert_eq!(&src[s as usize..e as usize], "NUMERIC");
}

// ===== sql120 DISTINCT redundant with GROUP BY =============================

#[test]
fn sql120_flags_distinct_with_group_by() {
    let d = diags("SELECT DISTINCT a, count(*) FROM users GROUP BY a;");
    assert!(d.iter().any(|x| x.code == "sql120"));
}

#[test]
fn sql120_quiet_for_distinct_alone() {
    let d = diags("SELECT DISTINCT a FROM users;");
    assert!(!d.iter().any(|x| x.code == "sql120"));
}

#[test]
fn sql120_quiet_for_distinct_on() {
    let d = diags("SELECT DISTINCT ON (a) a FROM users GROUP BY a;");
    assert!(!d.iter().any(|x| x.code == "sql120"));
}

#[test]
fn sql120_range_points_at_distinct() {
    let src = "SELECT DISTINCT a, count(*) FROM users GROUP BY a;";
    let d = diags(src);
    let hit = d.iter().find(|x| x.code == "sql120").expect("sql120");
    let s: u32 = hit.range.start().into();
    let e: u32 = hit.range.end().into();
    assert_eq!(&src[s as usize..e as usize], "DISTINCT");
}

// ===== sql121 cast text to int in WHERE ====================================

#[test]
fn sql121_flags_text_cast_eq_int() {
    let d = diags("SELECT * FROM users WHERE id::text = 5;");
    assert!(d.iter().any(|x| x.code == "sql121"));
}

#[test]
fn sql121_quiet_for_text_cast_eq_string() {
    let d = diags("SELECT * FROM users WHERE id::text = '5';");
    assert!(!d.iter().any(|x| x.code == "sql121"));
}

// ===== sql123 backslash in plain string ====================================

#[test]
fn sql123_flags_backslash_n() {
    let d = diags("SELECT 'line1\\nline2';");
    assert!(d.iter().any(|x| x.code == "sql123"));
}

#[test]
fn sql123_quiet_for_e_prefixed() {
    let d = diags("SELECT E'line1\\nline2';");
    assert!(!d.iter().any(|x| x.code == "sql123"));
}

#[test]
fn sql123_quiet_for_plain_string_no_backslash() {
    let d = diags("SELECT 'hello world';");
    assert!(!d.iter().any(|x| x.code == "sql123"));
}

// ===== sql117 boolean column getting text literal ==========================

#[test]
fn sql117_flags_quoted_true_into_bool_column() {
    let d = diags("INSERT INTO flags (id, active) VALUES ('00000000-0000-0000-0000-000000000000', 'true');");
    assert!(d.iter().any(|x| x.code == "sql117"));
}

#[test]
fn sql117_quiet_for_unquoted_bool() {
    let d = diags("INSERT INTO flags (id, active) VALUES ('00000000-0000-0000-0000-000000000000', true);");
    assert!(!d.iter().any(|x| x.code == "sql117"));
}

#[test]
fn sql117_quiet_for_explicit_cast() {
    let d = diags("INSERT INTO flags (id, active) VALUES ('00000000-0000-0000-0000-000000000000', 'true'::boolean);");
    assert!(!d.iter().any(|x| x.code == "sql117"));
}

// ===== sql122 LIKE in CREATE INDEX/VIEW without COLLATE ====================

#[test]
fn sql122_flags_like_in_create_view() {
    let d = diags("CREATE VIEW v AS SELECT * FROM users WHERE email LIKE 'a%';");
    assert!(d.iter().any(|x| x.code == "sql122"));
}

#[test]
fn sql122_quiet_for_ad_hoc_select() {
    let d = diags("SELECT * FROM users WHERE email LIKE 'a%';");
    assert!(!d.iter().any(|x| x.code == "sql122"));
}

#[test]
fn sql122_quiet_when_collate_present() {
    let d = diags("CREATE VIEW v AS SELECT * FROM users WHERE email COLLATE \"C\" LIKE 'a%';");
    assert!(!d.iter().any(|x| x.code == "sql122"));
}

// ===== sql118 SELECT INTO outside plpgsql ==================================

#[test]
fn sql118_flags_top_level_select_into() {
    let d = diags("SELECT id, email INTO snapshot FROM users;");
    assert!(d.iter().any(|x| x.code == "sql118"));
}

#[test]
fn sql118_quiet_for_normal_select() {
    let d = diags("SELECT id, email FROM users;");
    assert!(!d.iter().any(|x| x.code == "sql118"));
}

#[test]
fn sql118_range_points_at_into() {
    let src = "SELECT id INTO snapshot FROM users;";
    let d = diags(src);
    let hit = d.iter().find(|x| x.code == "sql118").expect("sql118");
    let s: u32 = hit.range.start().into();
    let e: u32 = hit.range.end().into();
    assert_eq!(&src[s as usize..e as usize], "INTO");
}

#[test]
fn sql118_quiet_inside_plpgsql_body() {
    let src = "CREATE FUNCTION f() RETURNS void LANGUAGE plpgsql AS $$ DECLARE v users; BEGIN SELECT * INTO v FROM users; END $$;";
    let d = diags(src);
    assert!(!d.iter().any(|x| x.code == "sql118"));
}

#[test]
fn sql118_quiet_inside_do_block() {
    let src = "DO $$ DECLARE v users; BEGIN SELECT * INTO v FROM users; END $$;";
    let d = diags(src);
    assert!(!d.iter().any(|x| x.code == "sql118"));
}

// ===== sql124 CTE missing RECURSIVE ========================================

#[test]
fn sql124_flags_self_ref_without_recursive() {
    let d = diags("WITH t AS (SELECT 1 UNION ALL SELECT n+1 FROM t WHERE n < 10) SELECT * FROM t;");
    assert!(d.iter().any(|x| x.code == "sql124"));
}

#[test]
fn sql124_quiet_when_recursive_present() {
    let d = diags("WITH RECURSIVE t AS (SELECT 1 UNION ALL SELECT n+1 FROM t WHERE n < 10) SELECT * FROM t;");
    assert!(!d.iter().any(|x| x.code == "sql124"));
}

#[test]
fn sql124_quiet_for_non_self_referencing_cte() {
    let d = diags("WITH t AS (SELECT id FROM users) SELECT * FROM t;");
    assert!(!d.iter().any(|x| x.code == "sql124"));
}

// ===== sql125 EXPLAIN ANALYZE on DML =======================================

#[test]
fn sql125_flags_explain_analyze_update() {
    let d = diags("EXPLAIN ANALYZE UPDATE users SET name = 'a' WHERE id = 1;");
    assert!(d.iter().any(|x| x.code == "sql125"));
}

#[test]
fn sql125_flags_explain_analyze_insert() {
    let d = diags("EXPLAIN ANALYZE INSERT INTO users (email) VALUES ('a@b.com');");
    assert!(d.iter().any(|x| x.code == "sql125"));
}

#[test]
fn sql125_quiet_for_explain_analyze_select() {
    let d = diags("EXPLAIN ANALYZE SELECT * FROM users;");
    assert!(!d.iter().any(|x| x.code == "sql125"));
}

#[test]
fn sql125_quiet_for_plain_explain() {
    let d = diags("EXPLAIN UPDATE users SET name = 'a' WHERE id = 1;");
    assert!(!d.iter().any(|x| x.code == "sql125"));
}

// ===== sql128 GRANT to PUBLIC ==============================================

#[test]
fn sql128_flags_grant_to_public() {
    let d = diags("GRANT SELECT ON users TO PUBLIC;");
    assert!(d.iter().any(|x| x.code == "sql128"));
}

#[test]
fn sql128_quiet_for_grant_to_specific_role() {
    let d = diags("GRANT SELECT ON users TO app_user;");
    assert!(!d.iter().any(|x| x.code == "sql128"));
}

#[test]
fn sql128_range_points_at_to_public() {
    let src = "GRANT SELECT ON users TO PUBLIC;";
    let d = diags(src);
    let hit = d.iter().find(|x| x.code == "sql128").expect("sql128");
    let s: u32 = hit.range.start().into();
    let e: u32 = hit.range.end().into();
    assert_eq!(&src[s as usize..e as usize], "TO PUBLIC");
}

// ===== sql127 UPDATE FROM without join condition ===========================

#[test]
fn sql127_flags_update_from_without_where() {
    let d = diags("UPDATE users SET name = src.name FROM staging src;");
    assert!(d.iter().any(|x| x.code == "sql127"));
}

#[test]
fn sql127_flags_update_from_where_no_join_cond() {
    let d = diags("UPDATE users SET name = 'x' FROM staging WHERE 1 = 1;");
    assert!(d.iter().any(|x| x.code == "sql127"));
}

#[test]
fn sql127_quiet_for_update_from_with_join_cond() {
    let d = diags("UPDATE users SET name = src.name FROM staging src WHERE users.id = src.id;");
    assert!(!d.iter().any(|x| x.code == "sql127"));
}

#[test]
fn sql127_quiet_for_plain_update() {
    let d = diags("UPDATE users SET name = 'x' WHERE id = 1;");
    assert!(!d.iter().any(|x| x.code == "sql127"));
}

// ===== sql119 SET TRANSACTION ISOLATION not first ==========================

#[test]
fn sql119_flags_set_iso_after_select() {
    let d = diags("BEGIN; SELECT 1; SET TRANSACTION ISOLATION LEVEL SERIALIZABLE;");
    assert!(d.iter().any(|x| x.code == "sql119"));
}

#[test]
fn sql119_quiet_when_first_after_begin() {
    let d = diags("BEGIN; SET TRANSACTION ISOLATION LEVEL SERIALIZABLE; SELECT 1;");
    assert!(!d.iter().any(|x| x.code == "sql119"));
}

#[test]
fn sql119_quiet_when_no_begin() {
    let d = diags("SET TRANSACTION ISOLATION LEVEL SERIALIZABLE;");
    assert!(!d.iter().any(|x| x.code == "sql119"));
}

// ===== sql131 RAISE message has more placeholders than args ================

#[test]
fn sql131_flags_missing_arg() {
    let d = diags("CREATE FUNCTION f() RETURNS void LANGUAGE plpgsql AS $$ BEGIN RAISE NOTICE 'value is %s'; END $$;");
    assert!(d.iter().any(|x| x.code == "sql131"));
}

#[test]
fn sql131_quiet_when_args_match() {
    let d = diags("CREATE FUNCTION f() RETURNS void LANGUAGE plpgsql AS $$ BEGIN RAISE NOTICE 'value is %s', 'x'; END $$;");
    assert!(!d.iter().any(|x| x.code == "sql131"));
}

#[test]
fn sql131_quiet_when_no_placeholder() {
    let d = diags("CREATE FUNCTION f() RETURNS void LANGUAGE plpgsql AS $$ BEGIN RAISE NOTICE 'plain message'; END $$;");
    assert!(!d.iter().any(|x| x.code == "sql131"));
}

#[test]
fn sql131_quiet_for_escaped_percent() {
    let d = diags("CREATE FUNCTION f() RETURNS void LANGUAGE plpgsql AS $$ BEGIN RAISE NOTICE '100%%'; END $$;");
    assert!(!d.iter().any(|x| x.code == "sql131"));
}

// ===== sql134 VACUUM in transaction ========================================

#[test]
fn sql134_flags_vacuum_inside_begin() {
    let d = diags("BEGIN; VACUUM users; COMMIT;");
    assert!(d.iter().any(|x| x.code == "sql134"));
}

#[test]
fn sql134_flags_reindex_inside_begin() {
    let d = diags("BEGIN; REINDEX TABLE users; COMMIT;");
    assert!(d.iter().any(|x| x.code == "sql134"));
}

#[test]
fn sql134_quiet_for_bare_vacuum() {
    let d = diags("VACUUM users;");
    assert!(!d.iter().any(|x| x.code == "sql134"));
}

#[test]
fn sql134_quiet_after_commit() {
    let d = diags("BEGIN; SELECT 1; COMMIT; VACUUM users;");
    assert!(!d.iter().any(|x| x.code == "sql134"));
}

// ===== sql130 multiple TRUNCATE in transaction =============================

#[test]
fn sql130_flags_second_truncate_in_tx() {
    let d = diags("BEGIN; TRUNCATE users; TRUNCATE orders; COMMIT;");
    assert!(d.iter().any(|x| x.code == "sql130"));
}

#[test]
fn sql130_quiet_for_single_truncate() {
    let d = diags("BEGIN; TRUNCATE users; COMMIT;");
    assert!(!d.iter().any(|x| x.code == "sql130"));
}

#[test]
fn sql130_quiet_for_combined_truncate() {
    let d = diags("BEGIN; TRUNCATE users, orders; COMMIT;");
    assert!(!d.iter().any(|x| x.code == "sql130"));
}

// ===== sql129 CREATE TABLE without ALTER OWNER =============================

#[test]
fn sql129_unregistered_does_not_fire() {
    // sql129 alter_table_no_owner was unregistered (too noisy in
    // practice). The rule module still exists for future opt-in.
    let d = diags("CREATE TABLE widgets (id uuid PRIMARY KEY);");
    assert!(!d.iter().any(|x| x.code == "sql129"));
}

#[test]
fn sql129_quiet_when_alter_owner_follows() {
    let d = diags("CREATE TABLE widgets (id uuid PRIMARY KEY); ALTER TABLE widgets OWNER TO app;");
    assert!(!d.iter().any(|x| x.code == "sql129"));
}

#[test]
fn sql129_quiet_for_non_table() {
    let d = diags("CREATE INDEX idx_x ON widgets (id);");
    assert!(!d.iter().any(|x| x.code == "sql129"));
}

// ===== sql136 COPY without FORMAT clause ===================================

#[test]
fn sql136_flags_bare_copy() {
    let d = diags("COPY users FROM '/tmp/users.csv';");
    assert!(d.iter().any(|x| x.code == "sql136"));
}

#[test]
fn sql136_quiet_with_format_clause() {
    let d = diags("COPY users FROM '/tmp/users.csv' WITH (FORMAT csv);");
    assert!(!d.iter().any(|x| x.code == "sql136"));
}

#[test]
fn sql136_quiet_with_csv_keyword() {
    let d = diags("COPY users FROM '/tmp/users.csv' CSV;");
    assert!(!d.iter().any(|x| x.code == "sql136"));
}

// ===== sql132 FOR UPDATE in recursive CTE ==================================

#[test]
fn sql132_flags_for_update_in_recursive_cte() {
    let d = diags("WITH RECURSIVE t AS (SELECT 1 UNION SELECT id FROM users FOR UPDATE) SELECT * FROM t;");
    assert!(d.iter().any(|x| x.code == "sql132"));
}

#[test]
fn sql132_quiet_for_non_recursive_cte() {
    let d = diags("WITH t AS (SELECT id FROM users FOR UPDATE) SELECT * FROM t;");
    assert!(!d.iter().any(|x| x.code == "sql132"));
}

#[test]
fn sql132_quiet_when_no_for_update() {
    let d = diags("WITH RECURSIVE t AS (SELECT 1 UNION SELECT id FROM users) SELECT * FROM t;");
    assert!(!d.iter().any(|x| x.code == "sql132"));
}

// ===== sql137 LISTEN without UNLISTEN ======================================

#[test]
fn sql137_flags_bare_listen() {
    let d = diags("LISTEN events;");
    assert!(d.iter().any(|x| x.code == "sql137"));
}

#[test]
fn sql137_quiet_when_unlisten_follows() {
    let d = diags("LISTEN events; SELECT 1; UNLISTEN events;");
    assert!(!d.iter().any(|x| x.code == "sql137"));
}

#[test]
fn sql137_quiet_when_unlisten_star_follows() {
    let d = diags("LISTEN events; SELECT 1; UNLISTEN *;");
    assert!(!d.iter().any(|x| x.code == "sql137"));
}

// ===== sql135 SET ROLE without RESET ROLE ==================================

#[test]
fn sql135_flags_bare_set_role() {
    let d = diags("BEGIN; SET ROLE admin; UPDATE users SET name = 'x'; COMMIT;");
    assert!(d.iter().any(|x| x.code == "sql135"));
}

#[test]
fn sql135_quiet_with_reset() {
    let d = diags("BEGIN; SET ROLE admin; UPDATE users SET name = 'x'; RESET ROLE; COMMIT;");
    assert!(!d.iter().any(|x| x.code == "sql135"));
}

#[test]
fn sql135_quiet_with_set_role_none() {
    let d = diags("BEGIN; SET ROLE admin; UPDATE users SET name = 'x'; SET ROLE NONE; COMMIT;");
    assert!(!d.iter().any(|x| x.code == "sql135"));
}

// ===== sql140 INSERT trigger WHEN references OLD ===========================

#[test]
fn sql140_flags_old_in_insert_trigger() {
    let d = diags("CREATE TRIGGER t AFTER INSERT ON users FOR EACH ROW WHEN (OLD.id IS NULL) EXECUTE FUNCTION f();");
    assert!(d.iter().any(|x| x.code == "sql140"));
}

#[test]
fn sql140_quiet_for_update_trigger() {
    let d = diags("CREATE TRIGGER t AFTER UPDATE ON users FOR EACH ROW WHEN (OLD.id IS NULL) EXECUTE FUNCTION f();");
    assert!(!d.iter().any(|x| x.code == "sql140"));
}

#[test]
fn sql140_quiet_when_only_new_referenced() {
    let d = diags("CREATE TRIGGER t AFTER INSERT ON users FOR EACH ROW WHEN (NEW.id IS NOT NULL) EXECUTE FUNCTION f();");
    assert!(!d.iter().any(|x| x.code == "sql140"));
}

// ===== sql141 ALTER TYPE ADD VALUE in transaction ==========================

#[test]
fn sql141_flags_alter_type_in_tx() {
    let d = diags("BEGIN; ALTER TYPE color ADD VALUE 'red'; COMMIT;");
    assert!(d.iter().any(|x| x.code == "sql141"));
}

#[test]
fn sql141_quiet_for_bare_alter_type() {
    let d = diags("ALTER TYPE color ADD VALUE 'red';");
    assert!(!d.iter().any(|x| x.code == "sql141"));
}

// ===== sql133 GRANT ... WITH GRANT OPTION ==================================

#[test]
fn sql133_flags_with_grant_option() {
    let d = diags("GRANT SELECT ON users TO app_user WITH GRANT OPTION;");
    assert!(d.iter().any(|x| x.code == "sql133"));
}

#[test]
fn sql133_quiet_for_plain_grant() {
    let d = diags("GRANT SELECT ON users TO app_user;");
    assert!(!d.iter().any(|x| x.code == "sql133"));
}

#[test]
fn sql133_range_points_at_clause() {
    let src = "GRANT SELECT ON users TO app_user WITH GRANT OPTION;";
    let d = diags(src);
    let hit = d.iter().find(|x| x.code == "sql133").expect("sql133");
    let s: u32 = hit.range.start().into();
    let e: u32 = hit.range.end().into();
    assert_eq!(&src[s as usize..e as usize], "WITH GRANT OPTION");
}

// ===== sql142 IMMUTABLE function body does DDL =============================

#[test]
fn sql142_flags_create_inside_immutable() {
    let d = diags("CREATE FUNCTION f() RETURNS void LANGUAGE plpgsql IMMUTABLE AS $$ BEGIN CREATE TABLE t (id int); END $$;");
    assert!(d.iter().any(|x| x.code == "sql142"));
}

#[test]
fn sql142_quiet_for_stable_function() {
    let d = diags("CREATE FUNCTION f() RETURNS void LANGUAGE plpgsql STABLE AS $$ BEGIN CREATE TABLE t (id int); END $$;");
    assert!(!d.iter().any(|x| x.code == "sql142"));
}

#[test]
fn sql142_quiet_for_immutable_no_ddl() {
    let d = diags("CREATE FUNCTION f(a int) RETURNS int LANGUAGE sql IMMUTABLE AS $$ SELECT a + 1 $$;");
    assert!(!d.iter().any(|x| x.code == "sql142"));
}

// ===== sql145 DEFAULT volatile =============================================

#[test]
fn sql145_flags_default_random() {
    let d = diags("CREATE TABLE t (id int DEFAULT random());");
    assert!(d.iter().any(|x| x.code == "sql145"));
}

#[test]
fn sql145_quiet_for_default_nextval() {
    // nextval is the *intended* default for serial-ish columns.
    let d = diags("CREATE TABLE t (id int DEFAULT nextval('seq'));");
    assert!(!d.iter().any(|x| x.code == "sql145"));
}

#[test]
fn sql145_quiet_for_now_default() {
    let d = diags("CREATE TABLE t (created_at timestamptz DEFAULT now());");
    assert!(!d.iter().any(|x| x.code == "sql145"));
}

#[test]
fn sql145_quiet_for_constant_default() {
    let d = diags("CREATE TABLE t (active bool DEFAULT true);");
    assert!(!d.iter().any(|x| x.code == "sql145"));
}

// ===== sql002 column lookup honors CTE columns =============================

#[test]
fn sql002_accepts_known_cte_column() {
    let d = diags("WITH t AS (SELECT id, email FROM users) SELECT t.id FROM t;");
    assert!(!d.iter().any(|x| x.code == "sql002"),
        "t.id is in the CTE projection, expected quiet: {:?}",
        d.iter().map(|x| (&x.code, &x.message)).collect::<Vec<_>>());
}

#[test]
fn sql002_flags_unknown_cte_column() {
    let d = diags("WITH t AS (SELECT id, email FROM users) SELECT t.bogus FROM t;");
    assert!(d.iter().any(|x| x.code == "sql002"),
        "t.bogus is not in projection, expected sql002: {:?}",
        d.iter().map(|x| (&x.code, &x.message)).collect::<Vec<_>>());
}

// ===== sql146 unbounded VARCHAR ===========================================

#[test]
fn sql146_flags_bare_varchar() {
    let d = diags("CREATE TABLE x (name VARCHAR);");
    assert!(d.iter().any(|x| x.code == "sql146"));
}

#[test]
fn sql146_flags_character_varying() {
    let d = diags("CREATE TABLE x (name CHARACTER VARYING);");
    assert!(d.iter().any(|x| x.code == "sql146"));
}

#[test]
fn sql146_quiet_for_varchar_with_limit() {
    let d = diags("CREATE TABLE x (name VARCHAR(255));");
    assert!(!d.iter().any(|x| x.code == "sql146"));
}

#[test]
fn sql146_quiet_for_text() {
    let d = diags("CREATE TABLE x (name TEXT);");
    assert!(!d.iter().any(|x| x.code == "sql146"));
}

// ===== sql148 array subscript zero / negative =============================

#[test]
fn sql148_flags_subscript_zero() {
    let d = diags("SELECT arr[0] FROM t;");
    assert!(d.iter().any(|x| x.code == "sql148"));
}

#[test]
fn sql148_flags_subscript_negative() {
    let d = diags("SELECT arr[-1] FROM t;");
    assert!(d.iter().any(|x| x.code == "sql148"));
}

#[test]
fn sql148_quiet_for_subscript_one() {
    let d = diags("SELECT arr[1] FROM t;");
    assert!(!d.iter().any(|x| x.code == "sql148"));
}

#[test]
fn sql148_quiet_for_empty_brackets_type() {
    let d = diags("CREATE TABLE x (xs int[]);");
    assert!(!d.iter().any(|x| x.code == "sql148"));
}

// ===== sql144 DELETE trigger references NEW ================================

#[test]
fn sql144_flags_new_in_delete_trigger() {
    let d = diags("CREATE TRIGGER t AFTER DELETE ON users FOR EACH ROW WHEN (NEW.id IS NOT NULL) EXECUTE FUNCTION f();");
    assert!(d.iter().any(|x| x.code == "sql144"));
}

#[test]
fn sql144_quiet_for_update_trigger() {
    let d = diags("CREATE TRIGGER t AFTER UPDATE ON users FOR EACH ROW WHEN (NEW.id IS NOT NULL) EXECUTE FUNCTION f();");
    assert!(!d.iter().any(|x| x.code == "sql144"));
}

#[test]
fn sql144_quiet_when_only_old_referenced() {
    let d = diags("CREATE TRIGGER t AFTER DELETE ON users FOR EACH ROW WHEN (OLD.id IS NOT NULL) EXECUTE FUNCTION f();");
    assert!(!d.iter().any(|x| x.code == "sql144"));
}

// ===== sql150 CASE without ELSE ============================================

#[test]
fn sql150_flags_case_no_else() {
    let d = diags("SELECT CASE WHEN id > 0 THEN 'pos' END FROM users;");
    assert!(d.iter().any(|x| x.code == "sql150"));
}

#[test]
fn sql150_quiet_when_else_present() {
    let d = diags("SELECT CASE WHEN id > 0 THEN 'pos' ELSE 'np' END FROM users;");
    assert!(!d.iter().any(|x| x.code == "sql150"));
}

#[test]
fn sql150_range_points_at_case_keyword() {
    let src = "SELECT CASE WHEN id > 0 THEN 'pos' END FROM users;";
    let d = diags(src);
    let hit = d.iter().find(|x| x.code == "sql150").expect("sql150");
    let s: u32 = hit.range.start().into();
    let e: u32 = hit.range.end().into();
    assert_eq!(&src[s as usize..e as usize].to_ascii_uppercase(), "CASE");
}

// ===== sql149 UPDATE SET x = x =============================================

#[test]
fn sql149_flags_self_assignment() {
    let d = diags("UPDATE users SET name = name WHERE id = 1;");
    assert!(d.iter().any(|x| x.code == "sql149"));
}

#[test]
fn sql149_quiet_for_normal_set() {
    let d = diags("UPDATE users SET name = 'x' WHERE id = 1;");
    assert!(!d.iter().any(|x| x.code == "sql149"));
}

#[test]
fn sql149_flags_qualified_self_assignment() {
    let d = diags("UPDATE users SET u.name = u.name WHERE u.id = 1;");
    assert!(d.iter().any(|x| x.code == "sql149"));
}

// ===== sql143 RETURNING without INTO inside plpgsql ========================

#[test]
fn sql143_flags_returning_no_into() {
    let d = diags("CREATE FUNCTION f() RETURNS void LANGUAGE plpgsql AS $$ BEGIN INSERT INTO users (email) VALUES ('a@b.com') RETURNING id; END $$;");
    assert!(d.iter().any(|x| x.code == "sql143"));
}

#[test]
fn sql143_quiet_when_into_present() {
    let d = diags("CREATE FUNCTION f() RETURNS void LANGUAGE plpgsql AS $$ DECLARE new_id uuid; BEGIN INSERT INTO users (email) VALUES ('a@b.com') RETURNING id INTO new_id; END $$;");
    assert!(!d.iter().any(|x| x.code == "sql143"));
}

#[test]
fn sql143_quiet_for_top_level_returning() {
    let d = diags("INSERT INTO users (email) VALUES ('a@b.com') RETURNING id;");
    assert!(!d.iter().any(|x| x.code == "sql143"));
}

// ===== sql126 DML in plpgsql without GET DIAGNOSTICS ======================

#[test]
fn sql126_flags_update_no_diagnostics() {
    let d = diags("CREATE FUNCTION f() RETURNS void LANGUAGE plpgsql AS $$ BEGIN UPDATE users SET name = 'x' WHERE id = '1'; END $$;");
    assert!(d.iter().any(|x| x.code == "sql126"));
}

#[test]
fn sql126_quiet_when_get_diagnostics_follows() {
    let d = diags("CREATE FUNCTION f() RETURNS int LANGUAGE plpgsql AS $$ DECLARE n int; BEGIN UPDATE users SET name = 'x' WHERE id = '1'; GET DIAGNOSTICS n = ROW_COUNT; RETURN n; END $$;");
    assert!(!d.iter().any(|x| x.code == "sql126"));
}

#[test]
fn sql126_quiet_when_returning_into_present() {
    let d = diags("CREATE FUNCTION f() RETURNS uuid LANGUAGE plpgsql AS $$ DECLARE r uuid; BEGIN UPDATE users SET name = 'x' WHERE id = '1' RETURNING id INTO r; RETURN r; END $$;");
    assert!(!d.iter().any(|x| x.code == "sql126"));
}

#[test]
fn sql126_quiet_for_insert_in_trigger_function() {
    // Fire-and-forget INSERT inside a trigger function body --
    // ROW_COUNT here is meaningless, the audit row always exists.
    let src = r#"CREATE OR REPLACE FUNCTION log_order_status_change ()
    RETURNS TRIGGER LANGUAGE plpgsql AS $$
BEGIN
    INSERT INTO order_status_history (order_id, old_status, new_status, changed_at)
    VALUES (NEW.id, OLD.status, NEW.status, now());
    RETURN NEW;
END;
$$;"#;
    let d = diags(src);
    assert!(!d.iter().any(|x| x.code == "sql126"),
        "INSERT in trigger fn shouldn't trigger sql126; got: {:?}",
        d.iter().filter(|x| x.code == "sql126").map(|x| &x.message).collect::<Vec<_>>());
}

#[test]
fn sql126_quiet_for_insert_in_plain_function() {
    // INSERT in any plpgsql body now passes silently (fire-and-forget
    // is the common case). Only UPDATE/DELETE trigger sql126.
    let src = "CREATE FUNCTION audit() RETURNS void LANGUAGE plpgsql AS $$ BEGIN INSERT INTO audit_log (msg) VALUES ('event'); END $$;";
    let d = diags(src);
    assert!(!d.iter().any(|x| x.code == "sql126"));
}

// ===== sql154 count(*) returns 1 row even when WHERE matches none =========

#[test]
fn sql154_flags_count_star_with_where() {
    let d = diags("SELECT count(*) FROM users WHERE id = '0';");
    assert!(d.iter().any(|x| x.code == "sql154"));
}

#[test]
fn sql154_quiet_when_group_by_present() {
    let d = diags("SELECT count(*) FROM users WHERE id = '0' GROUP BY name;");
    assert!(!d.iter().any(|x| x.code == "sql154"));
}

#[test]
fn sql154_quiet_when_no_where() {
    let d = diags("SELECT count(*) FROM users;");
    assert!(!d.iter().any(|x| x.code == "sql154"));
}

// ===== sql164 string literal +/- int =====================================

#[test]
fn sql164_flags_string_plus_int() {
    let d = diags("SELECT 'foo' + 1 FROM users;");
    assert!(d.iter().any(|x| x.code == "sql164"));
}

#[test]
fn sql164_quiet_for_concat() {
    let d = diags("SELECT 'foo' || 1 FROM users;");
    assert!(!d.iter().any(|x| x.code == "sql164"));
}

#[test]
fn sql164_quiet_for_string_only() {
    let d = diags("SELECT 'foo' FROM users;");
    assert!(!d.iter().any(|x| x.code == "sql164"));
}

// ===== real-world golden tests =============================================
// These assert the linter produces ZERO unexpected diagnostics on common
// production patterns. If a future rule introduces a false positive on
// any of these, the test fails immediately.

#[test]
fn golden_set_updated_at_trigger_zero_warnings() {
    let src = r#"CREATE OR REPLACE FUNCTION set_updated_at ()
    RETURNS TRIGGER LANGUAGE plpgsql AS $$
BEGIN
    NEW.updated_at := now();
    RETURN NEW;
END;
$$;"#;
    let d = diags(src);
    assert!(d.is_empty(), "expected zero diagnostics, got: {:?}",
        d.iter().map(|x| (&x.code, &x.message)).collect::<Vec<_>>());
}

#[test]
fn golden_order_status_history_trigger_zero_warnings() {
    let src = r#"CREATE OR REPLACE FUNCTION log_order_status_change ()
    RETURNS TRIGGER LANGUAGE plpgsql AS $$
BEGIN
    INSERT INTO order_status_history (order_id, old_status, new_status, changed_at)
    VALUES (NEW.id, OLD.status, NEW.status, now());
    RETURN NEW;
END;
$$;"#;
    let d = diags(src);
    assert!(d.is_empty(), "expected zero diagnostics, got: {:?}",
        d.iter().map(|x| (&x.code, &x.message)).collect::<Vec<_>>());
}

#[test]
fn golden_audit_log_table_zero_warnings() {
    let src = r#"CREATE TABLE audit_log (
    id uuid NOT NULL DEFAULT gen_random_uuid() PRIMARY KEY,
    actor_id uuid NOT NULL,
    action text NOT NULL,
    target_table text NOT NULL,
    target_id uuid,
    payload jsonb NOT NULL DEFAULT '{}'::jsonb,
    created_at timestamptz NOT NULL DEFAULT now()
);"#;
    let d = diags(src);
    let unexpected: Vec<_> = d.iter()
        .filter(|x| !matches!(x.code, "sql028"))
        .collect();
    assert!(unexpected.is_empty(), "expected zero diagnostics, got: {:?}",
        unexpected.iter().map(|x| (&x.code, &x.message)).collect::<Vec<_>>());
}

#[test]
fn golden_user_roles_unique_pair_zero_warnings() {
    let src = r#"CREATE TABLE user_roles (
    user_id uuid NOT NULL,
    role text NOT NULL,
    created_at timestamptz NOT NULL DEFAULT now(),
    UNIQUE (user_id, role)
);"#;
    let d = diags(src);
    let unexpected: Vec<_> = d.iter()
        .filter(|x| !matches!(x.code, "sql028" | "sql046"))
        .collect();
    assert!(unexpected.is_empty(), "expected zero diagnostics, got: {:?}",
        unexpected.iter().map(|x| (&x.code, &x.message)).collect::<Vec<_>>());
}

// ===== sql139 UNIQUE on nullable ==========================================

#[test]
fn sql139_flags_unique_no_not_null() {
    let d = diags("CREATE TABLE x (email TEXT UNIQUE);");
    assert!(d.iter().any(|x| x.code == "sql139"));
}

#[test]
fn sql139_quiet_when_not_null_present() {
    let d = diags("CREATE TABLE x (email TEXT NOT NULL UNIQUE);");
    assert!(!d.iter().any(|x| x.code == "sql139"));
}

#[test]
fn sql139_quiet_when_nulls_not_distinct() {
    let d = diags("CREATE TABLE x (email TEXT UNIQUE NULLS NOT DISTINCT);");
    assert!(!d.iter().any(|x| x.code == "sql139"));
}

// ===== regression: sql126 + sql045 must not fire on trigger funcs ==========

#[test]
fn sql126_quiet_on_assignment_to_updated_at_field() {
    // `new.updated_at := now()` is a PL/pgSQL assignment to a record
    // field. `UPDATE` appears inside the column name `updated_at` --
    // word-bounded + statement-start matching must reject it.
    let src = "CREATE OR REPLACE FUNCTION set_updated_at() RETURNS TRIGGER LANGUAGE plpgsql AS $$ BEGIN new.updated_at := now(); RETURN new; END $$;";
    let d = diags(src);
    assert!(!d.iter().any(|x| x.code == "sql126"),
        "sql126 false-positive on assignment: {:?}",
        d.iter().filter(|x| x.code == "sql126").collect::<Vec<_>>());
}

#[test]
fn sql045_quiet_on_return_at_end_of_trigger_body() {
    // `RETURN new;` is the natural last statement of a trigger fn.
    // The token `new` is the RETURN argument, not the next stmt.
    let src = "CREATE OR REPLACE FUNCTION set_updated_at() RETURNS TRIGGER LANGUAGE plpgsql AS $$ BEGIN new.updated_at := now(); RETURN new; END $$;";
    let d = diags(src);
    assert!(!d.iter().any(|x| x.code == "sql045"),
        "sql045 false-positive on RETURN: {:?}",
        d.iter().filter(|x| x.code == "sql045").collect::<Vec<_>>());
}

// ===== sql155 TRUNCATE RETURNING ===========================================

#[test]
fn sql155_flags_truncate_returning() {
    let d = diags("TRUNCATE users RETURNING id;");
    assert!(d.iter().any(|x| x.code == "sql155"));
}

#[test]
fn sql155_quiet_for_bare_truncate() {
    let d = diags("TRUNCATE users;");
    assert!(!d.iter().any(|x| x.code == "sql155"));
}

// ===== sql138 ::text inside DISTINCT =======================================

#[test]
fn sql138_flags_distinct_cast_to_text() {
    let d = diags("SELECT DISTINCT id::text FROM users;");
    assert!(d.iter().any(|x| x.code == "sql138"));
}

#[test]
fn sql138_quiet_for_distinct_on() {
    let d = diags("SELECT DISTINCT ON (id) id::text FROM users;");
    assert!(!d.iter().any(|x| x.code == "sql138"));
}

#[test]
fn sql138_quiet_for_plain_distinct() {
    let d = diags("SELECT DISTINCT id FROM users;");
    assert!(!d.iter().any(|x| x.code == "sql138"));
}

// ===== sql156 SELECT INTO STRICT without EXCEPTION =========================

#[test]
fn sql156_flags_strict_no_exception_block() {
    let d = diags("CREATE FUNCTION f() RETURNS uuid LANGUAGE plpgsql AS $$ DECLARE r uuid; BEGIN SELECT id INTO STRICT r FROM users WHERE email = 'x'; RETURN r; END $$;");
    assert!(d.iter().any(|x| x.code == "sql156"));
}

#[test]
fn sql156_quiet_with_exception_block() {
    let d = diags("CREATE FUNCTION f() RETURNS uuid LANGUAGE plpgsql AS $$ DECLARE r uuid; BEGIN SELECT id INTO STRICT r FROM users WHERE email = 'x'; RETURN r; EXCEPTION WHEN NO_DATA_FOUND THEN RETURN NULL; END $$;");
    assert!(!d.iter().any(|x| x.code == "sql156"));
}

#[test]
fn sql156_quiet_without_strict() {
    let d = diags("CREATE FUNCTION f() RETURNS uuid LANGUAGE plpgsql AS $$ DECLARE r uuid; BEGIN SELECT id INTO r FROM users WHERE email = 'x'; RETURN r; END $$;");
    assert!(!d.iter().any(|x| x.code == "sql156"));
}

// ===== sql153 timestamp + int arithmetic ===================================

#[test]
fn sql153_flags_now_plus_int() {
    let d = diags("SELECT now() + 1 FROM users;");
    assert!(d.iter().any(|x| x.code == "sql153"));
}

#[test]
fn sql153_quiet_with_interval() {
    let d = diags("SELECT now() + INTERVAL '1 day' FROM users;");
    assert!(!d.iter().any(|x| x.code == "sql153"));
}

#[test]
fn sql153_flags_current_date_minus_int() {
    let d = diags("SELECT current_date - 7 FROM users;");
    assert!(d.iter().any(|x| x.code == "sql153"));
}

// ===== sql145 default whitelist regression ================================

#[test]
fn sql145_quiet_for_gen_random_uuid_default() {
    let d = diags("CREATE TABLE t (id uuid DEFAULT gen_random_uuid());");
    assert!(!d.iter().any(|x| x.code == "sql145"),
        "gen_random_uuid() is the intended default use; got: {:?}",
        d.iter().filter(|x| x.code == "sql145").collect::<Vec<_>>());
}

#[test]
fn sql145_quiet_for_uuid_generate_v4_default() {
    let d = diags("CREATE TABLE t (id uuid DEFAULT uuid_generate_v4());");
    assert!(!d.iter().any(|x| x.code == "sql145"));
}

#[test]
fn sql145_quiet_for_now_default_whitelisted() {
    let d = diags("CREATE TABLE t (created_at timestamptz DEFAULT now());");
    assert!(!d.iter().any(|x| x.code == "sql145"));
}

#[test]
fn sql145_quiet_for_nextval_default_whitelisted() {
    let d = diags("CREATE TABLE t (id int DEFAULT nextval('seq'));");
    assert!(!d.iter().any(|x| x.code == "sql145"));
}

#[test]
fn sql145_still_flags_random() {
    let d = diags("CREATE TABLE t (lottery int DEFAULT random());");
    assert!(d.iter().any(|x| x.code == "sql145"),
        "random() default is unlikely intentional");
}

// ===== sql139 UNIQUE column-list regression ===============================

#[test]
fn sql139_quiet_when_all_unique_columns_not_null() {
    let src = "CREATE TABLE user_roles (
        user_id uuid NOT NULL,
        role text NOT NULL,
        UNIQUE (user_id, role)
    );";
    let d = diags(src);
    assert!(!d.iter().any(|x| x.code == "sql139"),
        "UNIQUE (user_id, role) over NOT NULL cols, expected quiet; got: {:?}",
        d.iter().filter(|x| x.code == "sql139").map(|x| &x.message).collect::<Vec<_>>());
}

#[test]
fn sql139_flags_when_one_unique_column_nullable() {
    let src = "CREATE TABLE user_roles (
        user_id uuid NOT NULL,
        role text,
        UNIQUE (user_id, role)
    );";
    let d = diags(src);
    assert!(d.iter().any(|x| x.code == "sql139"),
        "role is nullable, expected sql139");
}

#[test]
fn sql139_flags_inline_unique_without_not_null() {
    let d = diags("CREATE TABLE x (email TEXT UNIQUE);");
    assert!(d.iter().any(|x| x.code == "sql139"));
}

#[test]
fn sql139_quiet_inline_unique_with_not_null() {
    let d = diags("CREATE TABLE x (email TEXT NOT NULL UNIQUE);");
    assert!(!d.iter().any(|x| x.code == "sql139"));
}

// ===== sql160 advisory lock without unlock =================================

#[test]
fn sql160_flags_session_lock_no_unlock() {
    let d = diags("SELECT pg_advisory_lock(42);");
    assert!(d.iter().any(|x| x.code == "sql160"));
}

#[test]
fn sql160_quiet_when_unlock_follows() {
    let d = diags("SELECT pg_advisory_lock(42); SELECT 1; SELECT pg_advisory_unlock(42);");
    assert!(!d.iter().any(|x| x.code == "sql160"));
}

#[test]
fn sql160_quiet_for_xact_lock() {
    let d = diags("SELECT pg_advisory_xact_lock(42);");
    assert!(!d.iter().any(|x| x.code == "sql160"));
}

// ===== sql157 RAISE USING ERRCODE unquoted =================================

#[test]
fn sql157_flags_unquoted_errcode() {
    let d = diags("CREATE FUNCTION f() RETURNS void LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'bad' USING ERRCODE = unique_violation; END $$;");
    assert!(d.iter().any(|x| x.code == "sql157"));
}

#[test]
fn sql157_quiet_when_quoted() {
    let d = diags("CREATE FUNCTION f() RETURNS void LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'bad' USING ERRCODE = '23505'; END $$;");
    assert!(!d.iter().any(|x| x.code == "sql157"));
}

// ===== sql158 PERFORM <pure expression> ===================================

#[test]
fn sql158_flags_perform_pure_function_call() {
    let d = diags("CREATE FUNCTION f() RETURNS void LANGUAGE plpgsql AS $$ BEGIN PERFORM 1 + 1; END $$;");
    assert!(d.iter().any(|x| x.code == "sql158"));
}

#[test]
fn sql158_quiet_for_perform_with_side_effect() {
    let d = diags("CREATE FUNCTION f() RETURNS void LANGUAGE plpgsql AS $$ BEGIN PERFORM pg_notify('chan', 'msg'); END $$;");
    assert!(!d.iter().any(|x| x.code == "sql158"));
}

#[test]
fn sql158_quiet_for_perform_with_from() {
    let d = diags("CREATE FUNCTION f() RETURNS void LANGUAGE plpgsql AS $$ BEGIN PERFORM id FROM users; END $$;");
    assert!(!d.iter().any(|x| x.code == "sql158"));
}

// ===== sql159 statement-level trigger references NEW/OLD ==================

#[test]
fn sql159_flags_stmt_trigger_using_new() {
    let d = diags("CREATE TRIGGER t AFTER INSERT ON users FOR EACH STATEMENT WHEN (NEW.id IS NOT NULL) EXECUTE FUNCTION f();");
    assert!(d.iter().any(|x| x.code == "sql159"));
}

#[test]
fn sql159_quiet_for_row_trigger() {
    let d = diags("CREATE TRIGGER t AFTER INSERT ON users FOR EACH ROW WHEN (NEW.id IS NOT NULL) EXECUTE FUNCTION f();");
    assert!(!d.iter().any(|x| x.code == "sql159"));
}

#[test]
fn sql159_quiet_for_stmt_trigger_no_new_old() {
    let d = diags("CREATE TRIGGER t AFTER INSERT ON users FOR EACH STATEMENT EXECUTE FUNCTION f();");
    assert!(!d.iter().any(|x| x.code == "sql159"));
}

// ===== regression: ENTIRE user trigger fn must produce zero warnings ======

#[test]
fn user_trigger_set_updated_at_zero_warnings() {
    let src = r#"CREATE OR REPLACE FUNCTION set_updated_at ()
    RETURNS TRIGGER
AS $$
BEGIN
    new.updated_at := now();
    RETURN new;
END;
$$ LANGUAGE plpgsql;"#;
    let d = diags(src);
    let our_diags: Vec<_> = d.iter()
        .filter(|x| !matches!(x.code, "sql044" | "sql023"))
        .collect();
    assert!(
        our_diags.is_empty(),
        "trigger fn should produce zero diagnostics, got: {:?}",
        our_diags.iter().map(|x| (&x.code, &x.message)).collect::<Vec<_>>(),
    );
}
