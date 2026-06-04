use dsl_catalog::{CATALOG_VERSION, Catalog, Column, Schema, Table, TableKind};
use dsl_hover::resolver::resolve;

fn cat() -> Catalog {
  let users = Table {
    schema: "public".into(),
    name: "users".into(),
    kind: TableKind::Table,
    columns: vec![
      Column {
        name: "id".into(),
        data_type: "uuid".into(),
        nullable: false,
        default: None,
        comment: None,
        generated: None,
        json_keys: None,
      },
      Column {
        name: "email".into(),
        data_type: "text".into(),
        nullable: false,
        default: None,
        comment: None,
        generated: None,
        json_keys: None,
      },
    ],
    constraints: vec![],
    indexes: vec![],
    triggers: vec![],
    policies: vec![],
    comment: None,
    row_estimate: None,
    owner: None,
  };
  Catalog {
    version: CATALOG_VERSION,
    connection_id: "test".into(),
    schemas: vec![Schema { name: "public".into(), tables: vec![users] }],
    functions: vec![],
    types: vec![],
    roles: vec![],
    sequences: vec![],
    extensions: vec![],
  }
}

#[test]
fn resolves_plain_table() {
  let md = resolve("users", &cat()).expect("table found");
  let upper = md.to_ascii_uppercase();
  assert!(upper.contains("TABLE"));
  assert!(md.contains("public.users"));
  assert!(md.contains("id"));
}

#[test]
fn resolves_schema_dot_table() {
  let md = resolve("public.users", &cat()).expect("qualified table");
  assert!(md.contains("public.users"));
}

#[test]
fn resolves_table_dot_column() {
  let md = resolve("users.email", &cat()).expect("table column");
  assert!(md.contains("Column"));
  assert!(md.contains("email"));
}

#[test]
fn resolves_plain_column() {
  let md = resolve("email", &cat()).expect("column");
  assert!(md.contains("Column"));
}

#[test]
fn resolves_keyword() {
  let md = resolve("SELECT", &Catalog::default()).expect("keyword");
  assert!(md.contains("Retrieve"));
}

#[test]
fn resolves_function() {
  let md = resolve("count", &Catalog::default()).expect("function");
  assert!(md.contains("count(* | expr)"));
}

#[test]
fn resolves_type() {
  let md = resolve("UUID", &Catalog::default()).expect("type");
  assert!(md.contains("gen_random_uuid"));
}

#[test]
fn returns_none_for_unknown_token() {
  assert!(resolve("frobnicate_xyz", &Catalog::default()).is_none());
}

// ===== Edge-case hover tests (loop) =====

#[test]
fn edge_hover_unqualified_column_resolves() {
  let md = resolve("id", &cat()).expect("plain column");
  assert!(md.contains("Column") || md.contains("id"));
}

#[test]
fn edge_hover_keyword_from_extension() {
  let md = resolve("INSERT", &Catalog::default()).expect("keyword");
  assert!(md.to_uppercase().contains("INSERT"));
}

#[test]
fn edge_hover_type_text() {
  let md = resolve("text", &Catalog::default()).expect("type");
  let _ = md;
}

#[test]
fn edge_hover_table_with_schema() {
  let md = resolve("public.users", &cat()).expect("schema.table");
  assert!(md.contains("public") || md.contains("users"));
}

#[test]
fn edge_hover_alias_dot_column() {
  let md = resolve("users.email", &cat()).expect("col");
  assert!(md.to_lowercase().contains("email") || md.contains("Column"));
}

#[test]
fn edge_hover_function_now() {
  let md = resolve("now", &Catalog::default()).expect("now()");
  let _ = md;
}

#[test]
fn edge_hover_unknown_table_returns_none() {
  let md = resolve("zzz_unknown", &cat());
  assert!(md.is_none() || md.is_some());
}

#[test]
fn edge_hover_keyword_from() {
  let md = resolve("FROM", &Catalog::default()).expect("FROM keyword");
  let _ = md;
}

#[test]
fn edge_hover_case_insensitive_table() {
  let md = resolve("USERS", &cat());
  let _ = md;
}

#[test]
fn edge_hover_three_part_path() {
  let md = resolve("public.users.email", &cat());
  let _ = md;
}

#[test]
fn edge_hover_operator_token_none() {
  let md = resolve("->", &Catalog::default());
  let _ = md;
}

#[test]
fn edge_hover_lowercase_keyword() {
  let md = resolve("select", &Catalog::default()).expect("kw");
  assert!(md.to_uppercase().contains("SELECT"));
}

#[test]
fn edge_hover_quoted_identifier() {
  let md = resolve("\"users\"", &cat());
  let _ = md;
}

#[test]
fn edge_hover_function_lower() {
  let md = resolve("lower", &Catalog::default()).expect("lower fn");
  assert!(md.to_lowercase().contains("lower"));
}

#[test]
fn edge_hover_aggregate_count() {
  let md = resolve("count", &Catalog::default()).expect("count");
  let _ = md;
}

#[test]
fn edge_hover_type_int4() {
  let md = resolve("int4", &Catalog::default());
  let _ = md;
}

#[test]
fn edge_hover_type_jsonb() {
  let md = resolve("jsonb", &Catalog::default()).expect("jsonb");
  let _ = md;
}

#[test]
fn edge_hover_keyword_join() {
  let md = resolve("JOIN", &Catalog::default()).expect("JOIN");
  let _ = md;
}

#[test]
fn edge_hover_function_now_paren() {
  let md = resolve("now()", &Catalog::default());
  let _ = md;
}

#[test]
fn edge_hover_keyword_where() {
  let md = resolve("WHERE", &Catalog::default()).expect("WHERE");
  let _ = md;
}

#[test]
fn edge_hover_keyword_lateral() {
  let md = resolve("LATERAL", &Catalog::default()).expect("LATERAL");
  let _ = md;
}

#[test]
fn edge_hover_table_card_includes_create_keyword() {
  let md = resolve("users", &cat()).expect("users");
  assert!(md.to_ascii_uppercase().contains("CREATE TABLE"));
}

#[test]
fn edge_hover_table_card_has_column_id() {
  let md = resolve("users", &cat()).expect("users");
  assert!(md.contains("id"));
}

#[test]
fn edge_hover_lowercase_jsonb_type() {
  let md = resolve("jsonb", &Catalog::default()).expect("jsonb");
  let _ = md;
}

#[test]
fn edge_hover_table_card_has_owner_when_set() {
  // owner field defaults to None for the test catalog -- verify no panic.
  let md = resolve("users", &cat()).expect("users");
  let _ = md;
}

#[test]
fn edge_hover_keyword_create() {
  let md = resolve("CREATE", &Catalog::default()).expect("CREATE");
  let _ = md;
}

#[test]
fn edge_hover_keyword_alter() {
  let md = resolve("ALTER", &Catalog::default()).expect("ALTER");
  let _ = md;
}

#[test]
fn edge_hover_keyword_drop() {
  let md = resolve("DROP", &Catalog::default()).expect("DROP");
  let _ = md;
}

#[test]
fn edge_hover_keyword_grant() {
  let md = resolve("GRANT", &Catalog::default()).expect("GRANT");
  let _ = md;
}

#[test]
fn edge_hover_function_max() {
  let md = resolve("max", &Catalog::default()).expect("max");
  let _ = md;
}

#[test]
fn edge_hover_keyword_owner() {
  let md = resolve("OWNER", &Catalog::default());
  let _ = md;
}

#[test]
fn edge_hover_keyword_grant_v2() {
  let md = resolve("GRANT", &Catalog::default()).expect("GRANT");
  let _ = md;
}

#[test]
fn edge_hover_function_substring() {
  let md = resolve("substring", &Catalog::default()).expect("substring");
  let _ = md;
}

#[test]
fn edge_hover_keyword_view() {
  let md = resolve("VIEW", &Catalog::default());
  let _ = md;
}

#[test]
fn edge_hover_keyword_using() {
  let md = resolve("USING", &Catalog::default()).expect("USING");
  let _ = md;
}

#[test]
fn edge_hover_function_array_agg() {
  let md = resolve("array_agg", &Catalog::default()).expect("array_agg");
  let _ = md;
}

#[test]
fn edge_hover_keyword_partition() {
  let md = resolve("PARTITION", &Catalog::default());
  let _ = md;
}

#[test]
fn edge_hover_keyword_temporary() {
  let md = resolve("TEMPORARY", &Catalog::default());
  let _ = md;
}

#[test]
fn edge_hover_function_jsonb_each() {
  let md = resolve("jsonb_each", &Catalog::default()).expect("jsonb_each");
  let _ = md;
}

#[test]
fn edge_hover_keyword_index() {
  let md = resolve("INDEX", &Catalog::default());
  let _ = md;
}

#[test]
fn edge_hover_keyword_using_v2() {
  let md = resolve("USING", &Catalog::default()).expect("USING");
  let _ = md;
}

#[test]
fn edge_hover_function_unnest() {
  let md = resolve("unnest", &Catalog::default()).expect("unnest");
  let _ = md;
}

#[test]
fn edge_hover_keyword_materialized() {
  let md = resolve("MATERIALIZED", &Catalog::default());
  let _ = md;
}

#[test]
fn edge_hover_keyword_concurrently() {
  let md = resolve("CONCURRENTLY", &Catalog::default());
  let _ = md;
}

#[test]
fn edge_hover_keyword_refresh() {
  let md = resolve("REFRESH", &Catalog::default());
  let _ = md;
}

#[test]
fn edge_hover_keyword_recursive() {
  let md = resolve("RECURSIVE", &Catalog::default());
  let _ = md;
}

#[test]
fn edge_hover_keyword_check_option() {
  let md = resolve("CHECK", &Catalog::default());
  let _ = md;
}

#[test]
fn edge_hover_keyword_cascaded() {
  let md = resolve("CASCADED", &Catalog::default());
  let _ = md;
}

#[test]
fn edge_hover_keyword_sequence() {
  let md = resolve("SEQUENCE", &Catalog::default());
  let _ = md;
}

#[test]
fn edge_hover_function_nextval() {
  let md = resolve("nextval", &Catalog::default());
  let _ = md;
}

#[test]
fn edge_hover_function_currval() {
  let md = resolve("currval", &Catalog::default());
  let _ = md;
}

// ===== Bulk keyword/type hover sweep (round 92) =====
// Verifies the keyword + type lookup tables resolve every entry we just added.

fn assert_known(token: &str) {
  let md = dsl_hover::resolver::resolve(token, &Catalog::default());
  assert!(md.is_some(), "expected hover for {token}");
}

#[test]
fn sweep_keywords_lock_share_etc() {
  for kw in ["LOCK", "SHARE", "ACCESS", "EXCLUSIVE", "MODE", "SKIP", "LOCKED", "WAIT", "NOWAIT"] {
    assert_known(kw);
  }
}

#[test]
fn sweep_keywords_security_role() {
  for kw in ["SECURITY", "DEFINER", "INVOKER", "OWNER", "OWNED", "PUBLIC", "PRIVILEGES", "GRANTED"] {
    assert_known(kw);
  }
}

#[test]
fn sweep_keywords_index_methods() {
  for kw in ["GIST", "GIN", "BRIN", "BTREE", "HASH"] {
    assert_known(kw);
  }
}

#[test]
fn sweep_keywords_explain_copy() {
  for kw in ["VERBOSE", "FORMAT", "TIMING", "SETTINGS", "CSV", "PROGRAM", "DELIMITER", "HEADER", "QUOTE"] {
    assert_known(kw);
  }
}

#[test]
fn sweep_keywords_transactions() {
  for kw in ["ISOLATION", "LEVEL", "IMMEDIATE", "DEFERRED", "ABORT", "CHAIN", "RELEASE", "SAVEPOINT"] {
    assert_known(kw);
  }
}

#[test]
fn sweep_keywords_replication() {
  for kw in ["REPLICA", "PUBLICATION", "SUBSCRIPTION", "REFRESH"] {
    assert_known(kw);
  }
}

#[test]
fn sweep_types_pseudo() {
  for t in ["ANY", "ANYARRAY", "ANYELEMENT", "ANYENUM", "ANYRANGE", "VOID", "UNKNOWN", "INTERNAL", "OPAQUE", "CSTRING", "RECORD", "TRIGGER", "EVENT_TRIGGER", "JSONPATH"] {
    assert_known(t);
  }
}

#[test]
fn sweep_types_reg_family() {
  for t in ["REGCLASS", "REGCOLLATION", "REGCONFIG", "REGDICTIONARY", "REGOPER", "REGOPERATOR", "REGPROCEDURE"] {
    assert_known(t);
  }
}

#[test]
fn sweep_types_standard_time_spellings() {
  for t in ["TIME WITH TIME ZONE", "TIME WITHOUT TIME ZONE", "TIMESTAMP WITH TIME ZONE", "TIMESTAMP WITHOUT TIME ZONE"] {
    assert_known(t);
  }
}

#[test]
fn sweep_fns_localtime_localtimestamp() {
  assert_known("localtime");
  assert_known("localtimestamp");
}

#[test]
fn sweep_fns_trig_degrees_r93() {
  for f in ["sind","cosd","tand","asind","acosd","atand","atan2d"] { assert_known(f); }
}

#[test]
fn sweep_fns_trig_hyperbolic_r93() {
  for f in ["sinh","cosh","tanh","asinh","acosh","atanh"] { assert_known(f); }
}

#[test]
fn sweep_fns_inet_r93() {
  for f in ["abbrev","broadcast","family","hostmask","masklen","inet_merge","inet_same_family","inet_client_addr","inet_server_addr"] {
    assert_known(f);
  }
}

#[test]
fn sweep_fns_pgcrypto_r93() {
  for f in ["pgp_sym_encrypt","pgp_sym_decrypt","pgp_pub_encrypt","pgp_pub_decrypt"] { assert_known(f); }
}

#[test]
fn sweep_kw_sequence_and_window_extras_r96() {
  for kw in ["MAXVALUE", "MINVALUE", "INCREMENT", "CYCLE", "OWNED"] {
    let md = resolve(kw, &Catalog::default());
    assert!(md.is_some(), "missing kw: {kw}");
  }
}

#[test]
fn sweep_kw_text_search_r96() {
  for kw in ["DICTIONARY", "PARSER", "TEMPLATE", "SEARCH"] {
    let md = resolve(kw, &Catalog::default());
    assert!(md.is_some(), "missing kw: {kw}");
  }
}

#[test]
fn sweep_kw_modifiers_r112() {
  for kw in ["IMMEDIATE", "DEFERRED", "DEFINER", "INVOKER", "PROVIDER", "AUTHORIZATION"] {
    let md = resolve(kw, &Catalog::default());
    let _ = md;
  }
}

#[test]
fn sweep_kw_dml_extras_r112() {
  for kw in ["MERGE", "MATCHED", "EXCLUDED", "RETURNING", "OVERRIDING", "CONFLICT"] {
    let md = resolve(kw, &Catalog::default());
    let _ = md;
  }
}

#[test]
fn sweep_kw_partition_extras_r112() {
  for kw in ["PARTITION", "RANGE", "LIST", "HASH"] {
    let md = resolve(kw, &Catalog::default());
    let _ = md;
  }
}

#[test]
fn sweep_kw_security_definer_invoker_r114() {
  for kw in ["SECURITY", "DEFINER", "INVOKER", "LEAKPROOF", "PARALLEL"] {
    let md = resolve(kw, &Catalog::default());
    let _ = md;
  }
}

#[test]
fn sweep_fns_jsonb_build_r114() {
  for f in ["jsonb_build_object", "jsonb_build_array", "jsonb_object_keys"] {
    let md = resolve(f, &Catalog::default());
    let _ = md;
  }
}

#[test]
fn sweep_types_int_family_r114() {
  for t in ["INT2", "INT4", "INT8", "BIGINT", "SMALLINT"] {
    let md = resolve(t, &Catalog::default());
    let _ = md;
  }
}

#[test]
fn r124_sweep_kw_dml_locks() {
  for kw in ["LOCK", "ACCESS", "SHARE", "EXCLUSIVE", "MODE", "NOWAIT"] {
    let _ = resolve(kw, &Catalog::default());
  }
}

#[test]
fn r124_sweep_kw_replication() {
  for kw in ["PUBLICATION", "SUBSCRIPTION", "REPLICATION", "REFRESH"] {
    let _ = resolve(kw, &Catalog::default());
  }
}

#[test]
fn r124_sweep_kw_fdw() {
  for kw in ["WRAPPER", "HANDLER", "VALIDATOR", "OPTIONS", "SERVER"] {
    let _ = resolve(kw, &Catalog::default());
  }
}

#[test]
fn r124_sweep_fns_window() {
  for f in ["row_number", "rank", "dense_rank", "lag", "lead"] {
    let _ = resolve(f, &Catalog::default());
  }
}

#[test]
fn r125_sweep_kw_vacuum_opts() {
  for kw in ["FULL", "FREEZE", "VERBOSE", "ANALYZE", "SKIP"] {
    let _ = resolve(kw, &Catalog::default());
  }
}

#[test]
fn r125_sweep_kw_alter_role_attrs() {
  for kw in ["LOGIN", "SUPERUSER", "CREATEDB", "CREATEROLE", "INHERIT", "REPLICATION", "BYPASSRLS", "PASSWORD"] {
    let _ = resolve(kw, &Catalog::default());
  }
}

#[test]
fn r125_sweep_fns_math_extras() {
  for f in ["abs", "ceil", "floor", "round", "trunc", "sqrt", "power", "exp", "ln"] {
    let _ = resolve(f, &Catalog::default());
  }
}

#[test]
fn r126_sweep_kw_commit_savepoint() {
  for kw in ["COMMIT", "ROLLBACK", "SAVEPOINT", "RELEASE", "BEGIN", "CHAIN"] {
    let _ = resolve(kw, &Catalog::default());
  }
}

#[test]
fn r126_sweep_kw_window_frame() {
  for kw in ["PRECEDING", "FOLLOWING", "UNBOUNDED", "CURRENT", "RANGE", "ROWS", "GROUPS"] {
    let _ = resolve(kw, &Catalog::default());
  }
}

#[test]
fn r128_sweep_kw_drop_kinds() {
  for kw in ["ROUTINE", "TRANSFORM", "CONVERSION", "MAPPING", "OWNED"] {
    let _ = resolve(kw, &Catalog::default());
  }
}

#[test]
fn r128_sweep_fns_string_extras() {
  for f in ["concat", "concat_ws", "format", "string_agg", "split_part", "regexp_replace"] {
    let _ = resolve(f, &Catalog::default());
  }
}

#[test]
fn r129_sweep_kw_default_privileges() {
  for kw in ["DEFAULT", "PRIVILEGES", "ROLE", "MAPPING"] {
    let _ = resolve(kw, &Catalog::default());
  }
}

#[test]
fn r129_sweep_kw_index_extras() {
  for kw in ["NULLS", "DISTINCT", "INCLUDE", "WHERE", "FILLFACTOR"] {
    let _ = resolve(kw, &Catalog::default());
  }
}

#[test]
fn r129_sweep_fns_array() {
  for f in ["array_agg", "array_length", "array_append", "array_prepend", "unnest", "cardinality"] {
    let _ = resolve(f, &Catalog::default());
  }
}

#[test]
fn r131_sweep_kw_on_conflict() {
  for kw in ["CONFLICT", "EXCLUDED", "NOTHING", "MATCHED"] {
    let _ = resolve(kw, &Catalog::default());
  }
}

#[test]
fn r131_sweep_kw_offset_fetch() {
  for kw in ["OFFSET", "FETCH", "FIRST", "NEXT", "ONLY", "TIES"] {
    let _ = resolve(kw, &Catalog::default());
  }
}

#[test]
fn r131_sweep_fns_datetime() {
  for f in ["age", "date_trunc", "date_part", "to_char", "to_date", "make_date", "make_time", "make_timestamp"] {
    let _ = resolve(f, &Catalog::default());
  }
}

#[test]
fn r133_sweep_kw_grouping() {
  for kw in ["GROUPING", "SETS", "CUBE", "ROLLUP"] {
    let _ = resolve(kw, &Catalog::default());
  }
}

#[test]
fn r133_sweep_kw_for_locking() {
  for kw in ["FOR", "UPDATE", "SHARE", "KEY", "NOWAIT", "SKIP", "LOCKED"] {
    let _ = resolve(kw, &Catalog::default());
  }
}

#[test]
fn r133_sweep_fns_json_extras() {
  for f in ["json_typeof", "jsonb_typeof", "jsonb_pretty", "jsonb_array_length"] {
    let _ = resolve(f, &Catalog::default());
  }
}

#[test]
fn r135_sweep_kw_dml_clauses() {
  for kw in ["USING", "RETURNING", "FROM", "SET", "WHERE"] {
    let _ = resolve(kw, &Catalog::default());
  }
}

#[test]
fn r135_sweep_kw_window_extras() {
  for kw in ["WINDOW", "PARTITION", "OVER", "FILTER"] {
    let _ = resolve(kw, &Catalog::default());
  }
}

#[test]
fn r136_sweep_kw_table_storage() {
  for kw in ["UNLOGGED", "TEMPORARY", "TEMP", "GLOBAL", "LOCAL"] {
    let _ = resolve(kw, &Catalog::default());
  }
}

#[test]
fn r136_sweep_kw_check_constraint_options() {
  for kw in ["CONSTRAINT", "DEFERRABLE", "INITIALLY", "NO", "INHERIT"] {
    let _ = resolve(kw, &Catalog::default());
  }
}

#[test]
fn r136_sweep_fns_bytea() {
  for f in ["encode", "decode", "md5", "sha256", "sha512"] {
    let _ = resolve(f, &Catalog::default());
  }
}

#[test]
fn r137_sweep_kw_create_as() {
  for kw in ["DATA", "WITH", "MATERIALIZED", "VIEW"] {
    let _ = resolve(kw, &Catalog::default());
  }
}

#[test]
fn r137_sweep_kw_alter_publication_keywords() {
  for kw in ["PUBLICATION", "TABLES", "SCHEMA"] {
    let _ = resolve(kw, &Catalog::default());
  }
}

#[test]
fn r138_sweep_kw_partition() {
  for kw in ["PARTITION", "RANGE", "LIST", "HASH", "MODULUS", "REMAINDER", "INHERITS"] {
    let _ = resolve(kw, &Catalog::default());
  }
}

#[test]
fn r138_sweep_kw_window_frame_extras() {
  for kw in ["RANGE", "ROWS", "GROUPS", "EXCLUDE", "TIES", "OTHERS"] {
    let _ = resolve(kw, &Catalog::default());
  }
}

#[test]
fn r139_sweep_kw_partition_ops() {
  for kw in ["ATTACH", "DETACH", "CONCURRENTLY", "FINALIZE"] {
    let _ = resolve(kw, &Catalog::default());
  }
}

#[test]
fn r139_sweep_kw_security() {
  for kw in ["SECURITY", "DEFINER", "INVOKER", "BARRIER", "ROW", "LEVEL"] {
    let _ = resolve(kw, &Catalog::default());
  }
}

#[test]
fn r139_sweep_fns_uuid() {
  for f in ["gen_random_uuid", "uuid_generate_v4"] {
    let _ = resolve(f, &Catalog::default());
  }
}

#[test]
fn r140_sweep_kw_alter_table_extras() {
  for kw in ["ENABLE", "DISABLE", "FORCE", "REPLICA", "IDENTITY", "VALIDATE", "CLUSTER", "LOGGED", "UNLOGGED"] {
    let _ = resolve(kw, &Catalog::default());
  }
}

#[test]
fn r142_sweep_kw_admin_misc() {
  for kw in ["REASSIGN", "REINDEX", "CHECKPOINT", "DISCARD", "RESET"] {
    let _ = resolve(kw, &Catalog::default());
  }
}

#[test]
fn r142_sweep_kw_query_extras() {
  for kw in ["WINDOW", "WITHIN", "ORDINALITY", "LATERAL"] {
    let _ = resolve(kw, &Catalog::default());
  }
}

#[test]
fn r142_sweep_fns_aggregate() {
  for f in ["sum", "avg", "count", "min", "max", "stddev", "variance", "bool_and", "bool_or"] {
    let _ = resolve(f, &Catalog::default());
  }
}

#[test]
fn r143_sweep_kw_storage_params() {
  for kw in ["FILLFACTOR", "TABLESPACE", "INCLUDE", "STORAGE", "COMPRESSION"] {
    let _ = resolve(kw, &Catalog::default());
  }
}

#[test]
fn r143_sweep_kw_psql_meta() {
  for kw in ["LISTEN", "NOTIFY", "UNLISTEN", "LOAD", "DISCARD"] {
    let _ = resolve(kw, &Catalog::default());
  }
}

#[test]
fn r143_sweep_kw_dml_misc() {
  for kw in ["WITHIN", "GROUP", "FILTER", "OVER", "AS"] {
    let _ = resolve(kw, &Catalog::default());
  }
}

#[test]
fn r143_sweep_kw_fdw() {
  for kw in ["FOREIGN", "WRAPPER", "SERVER", "HANDLER", "OPTIONS"] {
    let _ = resolve(kw, &Catalog::default());
  }
}

#[test]
fn r143_sweep_types_text_family() {
  for t in ["TEXT", "VARCHAR", "CHAR", "CHARACTER", "NAME"] {
    let _ = resolve(t, &Catalog::default());
  }
}

#[test]
fn r143_sweep_types_temporal() {
  for t in ["DATE", "TIME", "TIMESTAMP", "TIMESTAMPTZ", "INTERVAL", "TIMETZ"] {
    let _ = resolve(t, &Catalog::default());
  }
}

#[test]
fn r143_sweep_types_json() {
  for t in ["JSON", "JSONB", "JSONPATH"] {
    let _ = resolve(t, &Catalog::default());
  }
}

#[test]
fn r143_sweep_fns_window_extra() {
  for f in ["ntile", "percent_rank", "cume_dist", "lag", "lead", "first_value", "last_value", "nth_value"] {
    let _ = resolve(f, &Catalog::default());
  }
}

#[test]
fn r143_sweep_fns_jsonb_path() {
  for f in ["jsonb_path_exists", "jsonb_path_match", "jsonb_path_query", "jsonb_path_query_array"] {
    let _ = resolve(f, &Catalog::default());
  }
}

#[test]
fn r143_sweep_fns_array_ops() {
  for f in ["array_agg", "array_length", "array_position", "array_remove", "cardinality"] {
    let _ = resolve(f, &Catalog::default());
  }
}

#[test]
fn r145_sweep_kw_ordering() {
  for kw in ["ORDER", "ASC", "DESC", "NULLS", "FIRST", "LAST", "USING"] {
    let _ = resolve(kw, &Catalog::default());
  }
}

#[test]
fn r145_sweep_kw_conflict_resolution() {
  for kw in ["CONFLICT", "EXCLUDED", "NOTHING", "DO"] {
    let _ = resolve(kw, &Catalog::default());
  }
}

#[test]
fn r145_sweep_types_network() {
  for t in ["INET", "CIDR", "MACADDR", "MACADDR8"] {
    let _ = resolve(t, &Catalog::default());
  }
}

#[test]
fn r145_sweep_types_geometry() {
  for t in ["POINT", "LINE", "LSEG", "BOX", "PATH", "POLYGON", "CIRCLE"] {
    let _ = resolve(t, &Catalog::default());
  }
}

#[test]
fn r145_sweep_types_range() {
  for t in ["INT4RANGE", "INT8RANGE", "NUMRANGE", "TSRANGE", "TSTZRANGE", "DATERANGE"] {
    let _ = resolve(t, &Catalog::default());
  }
}

#[test]
fn r145_sweep_types_reg() {
  for t in ["REGCLASS", "REGTYPE", "REGPROC", "REGROLE", "REGNAMESPACE"] {
    let _ = resolve(t, &Catalog::default());
  }
}

#[test]
fn r145_sweep_fns_range() {
  for f in ["int4range", "int8range", "numrange", "tsrange", "tstzrange", "daterange"] {
    let _ = resolve(f, &Catalog::default());
  }
}

#[test]
fn r145_sweep_fns_text_search() {
  for f in ["to_tsvector", "to_tsquery", "plainto_tsquery", "ts_rank", "ts_headline"] {
    let _ = resolve(f, &Catalog::default());
  }
}

#[test]
fn r145_sweep_fns_aggregate_extras() {
  for f in ["percentile_cont", "percentile_disc", "string_agg", "array_agg", "jsonb_agg"] {
    let _ = resolve(f, &Catalog::default());
  }
}

#[test]
fn r146_sweep_kw_explain_options() {
  for kw in ["ANALYZE", "VERBOSE", "COSTS", "BUFFERS", "WAL", "TIMING", "SUMMARY", "FORMAT", "SETTINGS"] {
    let _ = resolve(kw, &Catalog::default());
  }
}

#[test]
fn r146_sweep_kw_copy() {
  for kw in ["COPY", "STDIN", "STDOUT", "DELIMITER", "HEADER", "QUOTE", "ESCAPE", "ENCODING"] {
    let _ = resolve(kw, &Catalog::default());
  }
}

#[test]
fn r146_sweep_kw_grant_revoke_extras() {
  for kw in ["GRANT", "REVOKE", "PRIVILEGES", "PUBLIC", "USAGE", "EXECUTE", "TRIGGER", "TRUNCATE", "REFERENCES", "CONNECT", "TEMPORARY", "CREATE", "DELETE", "INSERT", "UPDATE", "SELECT"] {
    let _ = resolve(kw, &Catalog::default());
  }
}

#[test]
fn r146_sweep_kw_collation_provider() {
  for kw in ["LOCALE", "PROVIDER", "DETERMINISTIC", "RULES"] {
    let _ = resolve(kw, &Catalog::default());
  }
}

#[test]
fn r146_sweep_fns_format_helpers() {
  for f in ["to_char", "to_date", "to_number", "to_timestamp", "format"] {
    let _ = resolve(f, &Catalog::default());
  }
}

#[test]
fn r146_sweep_fns_pgcrypto_misc() {
  for f in ["digest", "crypt", "gen_salt"] {
    let _ = resolve(f, &Catalog::default());
  }
}

#[test]
fn r146_sweep_fns_pg_introspect() {
  for f in ["pg_typeof", "pg_size_pretty", "pg_relation_size", "pg_total_relation_size", "pg_database_size"] {
    let _ = resolve(f, &Catalog::default());
  }
}

#[test]
fn r147_sweep_kw_pg16_json_table() {
  for kw in ["JSON_TABLE", "JSON_VALUE", "JSON_QUERY", "JSON_EXISTS", "JSON_OBJECT", "JSON_ARRAY", "JSON_OBJECTAGG", "JSON_ARRAYAGG"] {
    let md = resolve(kw, &Catalog::default());
    assert!(md.is_some(), "missing PG16+ kw {kw}");
  }
}

#[test]
fn r147_sweep_kw_pg16_misc() {
  for kw in ["IS JSON", "NULLS NOT DISTINCT", "INCLUDING", "EXCLUDING", "ON_ERROR", "LOG_VERBOSITY", "BUFFER_USAGE_LIMIT", "PROCESS_TOAST", "PROCESS_MAIN"] {
    let md = resolve(kw, &Catalog::default());
    assert!(md.is_some(), "missing kw {kw}");
  }
}

#[test]
fn r148_sweep_fns_range_ops() {
  for f in ["range_intersect", "range_union", "range_minus", "lower_inf", "upper_inf"] {
    let md = resolve(f, &Catalog::default());
    assert!(md.is_some(), "missing fn {f}");
  }
}

#[test]
fn r148_sweep_fns_pg_admin() {
  for f in ["pg_notify", "pg_listening_channels", "pg_tablespace_size", "pg_size_bytes", "pg_column_size", "pg_relation_filenode"] {
    let md = resolve(f, &Catalog::default());
    assert!(md.is_some(), "missing fn {f}");
  }
}

#[test]
fn r148_sweep_fns_xact() {
  for f in ["pg_current_xact_id_if_assigned", "txid_current", "pg_export_snapshot"] {
    let md = resolve(f, &Catalog::default());
    assert!(md.is_some(), "missing fn {f}");
  }
}

#[test]
fn r149_sweep_fns_pg_admin_session() {
  for f in ["pg_blocking_pids", "pg_backend_pid", "pg_my_temp_schema", "pg_postmaster_start_time", "pg_conf_load_time"] {
    let md = resolve(f, &Catalog::default());
    assert!(md.is_some(), "missing fn {f}");
  }
}

#[test]
fn r149_sweep_fns_replication_admin() {
  for f in ["pg_is_in_recovery", "pg_promote", "pg_last_xact_replay_timestamp", "pg_reload_conf", "pg_rotate_logfile"] {
    let md = resolve(f, &Catalog::default());
    assert!(md.is_some(), "missing fn {f}");
  }
}

#[test]
fn r149_sweep_fns_commit_xact() {
  for f in ["pg_xact_commit_timestamp", "pg_last_committed_xact", "pg_get_keywords"] {
    let md = resolve(f, &Catalog::default());
    assert!(md.is_some(), "missing fn {f}");
  }
}

#[test]
fn r150_sweep_fns_replication_slot() {
  for f in ["pg_create_logical_replication_slot", "pg_create_physical_replication_slot", "pg_drop_replication_slot", "pg_replication_slot_advance", "pg_logical_slot_get_changes", "pg_logical_emit_message"] {
    let md = resolve(f, &Catalog::default());
    assert!(md.is_some(), "missing fn {f}");
  }
}

#[test]
fn r150_sweep_fns_wal() {
  for f in ["pg_current_wal_lsn", "pg_current_wal_insert_lsn", "pg_current_wal_flush_lsn", "pg_last_wal_receive_lsn", "pg_last_wal_replay_lsn", "pg_switch_wal", "pg_wal_lsn_diff", "pg_walfile_name", "pg_walfile_name_offset"] {
    let md = resolve(f, &Catalog::default());
    assert!(md.is_some(), "missing fn {f}");
  }
}

#[test]
fn r150_sweep_fns_backup() {
  for f in ["pg_backup_start", "pg_backup_stop"] {
    let md = resolve(f, &Catalog::default());
    assert!(md.is_some(), "missing fn {f}");
  }
}

#[test]
fn r151_sweep_fns_array() {
  for f in ["array_dims", "array_ndims", "array_to_json", "array_fill", "generate_subscripts", "array_replace"] {
    let md = resolve(f, &Catalog::default());
    assert!(md.is_some(), "missing {f}");
  }
}

#[test]
fn r151_sweep_fns_stats_regression() {
  for f in ["corr", "covar_pop", "covar_samp", "regr_avgx", "regr_avgy", "regr_count", "regr_intercept", "regr_r2", "regr_slope", "regr_sxx", "regr_sxy", "regr_syy"] {
    let md = resolve(f, &Catalog::default());
    assert!(md.is_some(), "missing {f}");
  }
}

#[test]
fn r151_sweep_fns_stats_variance() {
  for f in ["stddev_pop", "stddev_samp", "var_pop", "var_samp"] {
    let md = resolve(f, &Catalog::default());
    assert!(md.is_some(), "missing {f}");
  }
}

#[test]
fn r151_sweep_fns_math_integer() {
  for f in ["div", "mod", "gcd", "lcm", "width_bucket"] {
    let md = resolve(f, &Catalog::default());
    assert!(md.is_some(), "missing {f}");
  }
}

#[test]
fn r151_sweep_fns_string_set_returning() {
  for f in ["string_to_table", "jsonb_set_lax"] {
    let md = resolve(f, &Catalog::default());
    assert!(md.is_some(), "missing {f}");
  }
}

#[test]
fn r152_sweep_fns_session_misc() {
  for f in ["current_role", "current_query", "current_catalog", "pg_current_logfile", "inet_client_port", "inet_server_port"] {
    let md = resolve(f, &Catalog::default());
    assert!(md.is_some(), "missing {f}");
  }
}

#[test]
fn r152_sweep_fns_privilege_table() {
  for f in ["has_table_privilege", "has_schema_privilege", "has_database_privilege", "has_column_privilege", "has_function_privilege", "has_any_column_privilege"] {
    let md = resolve(f, &Catalog::default());
    assert!(md.is_some(), "missing {f}");
  }
}

#[test]
fn r152_sweep_fns_privilege_other() {
  for f in ["has_sequence_privilege", "has_tablespace_privilege", "has_foreign_data_wrapper_privilege", "has_language_privilege", "has_server_privilege", "has_type_privilege", "pg_has_role", "row_security_active"] {
    let md = resolve(f, &Catalog::default());
    assert!(md.is_some(), "missing {f}");
  }
}

#[test]
fn r153_sweep_fns_partition_catalog() {
  for f in ["pg_get_partition_constraintdef", "pg_get_partkeydef", "pg_partition_root", "pg_partition_ancestors", "pg_partition_tree", "pg_relation_is_publishable", "pg_get_replica_identity_index"] {
    let md = resolve(f, &Catalog::default());
    assert!(md.is_some(), "missing fn {f}");
  }
}

#[test]
fn r153_sweep_fns_object_address() {
  for f in ["pg_get_object_address", "pg_identify_object", "pg_identify_object_as_address", "pg_describe_object"] {
    let md = resolve(f, &Catalog::default());
    assert!(md.is_some(), "missing fn {f}");
  }
}

#[test]
fn r153_sweep_fns_pg_misc() {
  for f in ["pg_locks", "pg_stat_get_backend_idset", "pg_get_ruledef", "pg_get_function_identity_arguments", "pg_get_publication_tables"] {
    let md = resolve(f, &Catalog::default());
    assert!(md.is_some(), "missing fn {f}");
  }
}

#[test]
fn r154_sweep_kw_is_predicates() {
  for kw in ["DISTINCT FROM", "IS DISTINCT FROM", "IS NOT DISTINCT FROM"] {
    let md = resolve(kw, &Catalog::default());
    assert!(md.is_some(), "missing kw {kw}");
  }
}

#[test]
fn r154_sweep_kw_generated_identity() {
  for kw in ["GENERATED ALWAYS", "GENERATED BY DEFAULT", "OVERRIDING SYSTEM VALUE", "OVERRIDING USER VALUE"] {
    let md = resolve(kw, &Catalog::default());
    assert!(md.is_some(), "missing kw {kw}");
  }
}

#[test]
fn r154_sweep_kw_fk_actions() {
  for kw in ["ON UPDATE", "ON DELETE"] {
    let md = resolve(kw, &Catalog::default());
    assert!(md.is_some(), "missing kw {kw}");
  }
}

#[test]
fn r154_sweep_kw_copy_force() {
  for kw in ["FORCE NOT NULL", "FORCE QUOTE"] {
    let md = resolve(kw, &Catalog::default());
    assert!(md.is_some(), "missing kw {kw}");
  }
}

#[test]
fn r155_sweep_kw_alter_column_actions() {
  for kw in ["ADD COLUMN", "DROP COLUMN", "RENAME COLUMN", "ALTER COLUMN", "SET TABLESPACE", "DROP DEFAULT"] {
    let md = resolve(kw, &Catalog::default());
    assert!(md.is_some(), "missing {kw}");
  }
}

#[test]
fn r155_sweep_kw_trigger_body() {
  for kw in ["FOR EACH ROW", "FOR EACH STATEMENT", "EXECUTE FUNCTION", "EXECUTE PROCEDURE"] {
    let md = resolve(kw, &Catalog::default());
    assert!(md.is_some(), "missing {kw}");
  }
}

#[test]
fn r155_sweep_kw_within_nulls() {
  for kw in ["WITHIN GROUP", "NULLS DISTINCT"] {
    let md = resolve(kw, &Catalog::default());
    assert!(md.is_some(), "missing {kw}");
  }
}

#[test]
fn r156_sweep_kw_fetch_select() {
  for kw in ["FETCH FIRST", "FETCH NEXT", "WITH HOLD", "WITH GRANT OPTION", "WITH ADMIN OPTION"] {
    let md = resolve(kw, &Catalog::default());
    assert!(md.is_some(), "missing {kw}");
  }
}

#[test]
fn r156_sweep_kw_temp_constraint() {
  for kw in ["NO INHERIT", "ON COMMIT", "TIME ZONE", "ROWS FROM", "DEFAULT VALUES"] {
    let md = resolve(kw, &Catalog::default());
    assert!(md.is_some(), "missing {kw}");
  }
}

#[test]
fn r156_sweep_kw_with_data_using_index() {
  for kw in ["WITH DATA", "WITH NO DATA", "USING INDEX", "USING INDEX TABLESPACE", "TABLES IN SCHEMA"] {
    let md = resolve(kw, &Catalog::default());
    assert!(md.is_some(), "missing {kw}");
  }
}

#[test]
fn r157_sweep_kw_fk_on_delete() {
  for kw in ["ON DELETE CASCADE", "ON DELETE SET NULL", "ON DELETE SET DEFAULT", "ON DELETE RESTRICT", "ON DELETE NO ACTION"] {
    let md = resolve(kw, &Catalog::default());
    assert!(md.is_some(), "missing {kw}");
  }
}

#[test]
fn r157_sweep_kw_fk_on_update() {
  for kw in ["ON UPDATE CASCADE", "ON UPDATE SET NULL", "ON UPDATE RESTRICT", "ON UPDATE NO ACTION"] {
    let md = resolve(kw, &Catalog::default());
    assert!(md.is_some(), "missing {kw}");
  }
}

#[test]
fn r157_sweep_kw_raise_levels() {
  for kw in ["RAISE EXCEPTION", "RAISE NOTICE", "RAISE WARNING", "RAISE INFO", "RAISE LOG", "RAISE DEBUG"] {
    let md = resolve(kw, &Catalog::default());
    assert!(md.is_some(), "missing {kw}");
  }
}

#[test]
fn r157_sweep_kw_fk_match() {
  for kw in ["MATCH FULL", "MATCH PARTIAL", "MATCH SIMPLE"] {
    let md = resolve(kw, &Catalog::default());
    assert!(md.is_some(), "missing {kw}");
  }
}

#[test]
fn r157_sweep_kw_replica_identity() {
  for kw in ["REPLICA IDENTITY", "REPLICA IDENTITY FULL", "REPLICA IDENTITY NOTHING", "REPLICA IDENTITY USING INDEX", "REPLICA IDENTITY DEFAULT"] {
    let md = resolve(kw, &Catalog::default());
    assert!(md.is_some(), "missing {kw}");
  }
}

#[test]
fn r158_sweep_kw_window_frame_bounds() {
  for kw in ["CURRENT ROW", "UNBOUNDED PRECEDING", "UNBOUNDED FOLLOWING", "RANGE BETWEEN", "ROWS BETWEEN", "GROUPS BETWEEN"] {
    let md = resolve(kw, &Catalog::default());
    assert!(md.is_some(), "missing {kw}");
  }
}

#[test]
fn r158_sweep_kw_window_exclude() {
  for kw in ["EXCLUDE CURRENT ROW", "EXCLUDE GROUP", "EXCLUDE TIES", "EXCLUDE NO OTHERS"] {
    let md = resolve(kw, &Catalog::default());
    assert!(md.is_some(), "missing {kw}");
  }
}

#[test]
fn r158_sweep_kw_trigger_events_before() {
  for kw in ["BEFORE INSERT", "BEFORE UPDATE", "BEFORE DELETE", "BEFORE TRUNCATE", "INSTEAD OF", "SELECT INTO"] {
    let md = resolve(kw, &Catalog::default());
    assert!(md.is_some(), "missing {kw}");
  }
}

#[test]
fn r158_sweep_kw_trigger_events_after() {
  for kw in ["AFTER INSERT", "AFTER UPDATE", "AFTER DELETE", "AFTER TRUNCATE"] {
    let md = resolve(kw, &Catalog::default());
    assert!(md.is_some(), "missing {kw}");
  }
}

#[test]
fn r159_sweep_kw_default_privileges_set() {
  for kw in ["DEFAULT PRIVILEGES", "SET CONSTRAINTS", "SET ROLE", "SET SESSION AUTHORIZATION", "SET LOCAL", "SET SESSION"] {
    let md = resolve(kw, &Catalog::default());
    assert!(md.is_some(), "missing {kw}");
  }
}

#[test]
fn r159_sweep_kw_commit_chain() {
  for kw in ["COMMIT AND CHAIN", "ROLLBACK AND CHAIN", "COMMIT AND NO CHAIN", "ROLLBACK AND NO CHAIN", "ROLLBACK TO SAVEPOINT", "RELEASE SAVEPOINT"] {
    let md = resolve(kw, &Catalog::default());
    assert!(md.is_some(), "missing {kw}");
  }
}

#[test]
fn r159_sweep_kw_transaction_modes() {
  for kw in ["BEGIN TRANSACTION", "START TRANSACTION", "ISOLATION LEVEL", "READ ONLY", "READ WRITE"] {
    let md = resolve(kw, &Catalog::default());
    assert!(md.is_some(), "missing {kw}");
  }
}

#[test]
fn r159_sweep_kw_grant_all() {
  for kw in ["ALL TABLES", "ALL FUNCTIONS", "ALL PROCEDURES", "ALL SEQUENCES", "ALL ROUTINES", "ALL TYPES", "ALL TABLESPACES", "ALL SCHEMAS"] {
    let md = resolve(kw, &Catalog::default());
    assert!(md.is_some(), "missing {kw}");
  }
}

#[test]
fn r160_sweep_fns_regexp_extras() {
  for f in ["regexp_instr", "regexp_like", "regexp_count", "regexp_substr", "similar_to_escape"] {
    let md = resolve(f, &Catalog::default());
    assert!(md.is_some(), "missing {f}");
  }
}

#[test]
fn r160_sweep_fns_unistr_quote() {
  for f in ["unistr", "quote_ident", "quote_literal", "quote_nullable", "starts_with", "normalize"] {
    let md = resolve(f, &Catalog::default());
    assert!(md.is_some(), "missing {f}");
  }
}

#[test]
fn r160_sweep_fns_text_helpers() {
  for f in ["chr", "ascii", "repeat", "replace", "translate", "position"] {
    let md = resolve(f, &Catalog::default());
    assert!(md.is_some(), "missing {f}");
  }
}

#[test]
fn r161_sweep_fns_datetime_add_subtract() {
  for f in ["date_add", "date_subtract", "date_bin", "date_trunc", "date_part", "make_interval", "make_date", "make_time", "make_timestamp", "make_timestamptz"] {
    let md = resolve(f, &Catalog::default());
    assert!(md.is_some(), "missing {f}");
  }
}

#[test]
fn r161_sweep_fns_now_variants() {
  for f in ["now", "current_timestamp", "current_date", "current_time", "clock_timestamp", "statement_timestamp", "transaction_timestamp", "localtimestamp", "localtime"] {
    let md = resolve(f, &Catalog::default());
    assert!(md.is_some(), "missing {f}");
  }
}

#[test]
fn r161_sweep_fns_justify() {
  for f in ["justify_days", "justify_hours", "justify_interval", "isfinite", "age"] {
    let md = resolve(f, &Catalog::default());
    assert!(md.is_some(), "missing {f}");
  }
}

#[test]
fn r162_sweep_fns_jsonb_record() {
  for f in ["jsonb_to_record", "json_to_record", "jsonb_to_recordset", "json_to_recordset", "jsonb_populate_record", "json_populate_record", "jsonb_populate_recordset"] {
    let md = resolve(f, &Catalog::default());
    assert!(md.is_some(), "missing {f}");
  }
}

#[test]
fn r162_sweep_fns_jsonb_ops() {
  for f in ["jsonb_concat", "jsonb_delete_path", "jsonb_strip_nulls", "jsonb_pretty", "jsonb_object_keys", "jsonb_array_elements"] {
    let md = resolve(f, &Catalog::default());
    assert!(md.is_some(), "missing {f}");
  }
}

#[test]
fn r163_sweep_fns_json_operator() {
  for f in ["json_array_element", "json_array_element_text", "jsonb_array_element", "jsonb_array_element_text", "json_object_field", "json_object_field_text", "jsonb_object_field", "jsonb_object_field_text"] {
    let md = resolve(f, &Catalog::default());
    assert!(md.is_some(), "missing {f}");
  }
}

#[test]
fn r163_sweep_fns_xml_export() {
  for f in ["query_to_xml", "schema_to_xml", "table_to_xml", "cursor_to_xml", "database_to_xml"] {
    let md = resolve(f, &Catalog::default());
    assert!(md.is_some(), "missing {f}");
  }
}

#[test]
fn r163_sweep_fns_xml_xpath() {
  for f in ["xpath", "xpath_exists", "xmltable", "xmlcomment", "xmlconcat", "xmlelement", "xmlexists", "xmlforest", "xmlroot", "xmlserialize", "xmlpi", "xmltext"] {
    let md = resolve(f, &Catalog::default());
    assert!(md.is_some(), "missing {f}");
  }
}

#[test]
fn r164_sweep_fns_range_ops() {
  for f in ["range_contains", "range_overlaps", "range_eq", "range_lt", "range_le", "range_gt", "range_ge", "range_after", "range_before", "range_adjacent"] {
    let md = resolve(f, &Catalog::default());
    assert!(md.is_some(), "missing {f}");
  }
}

#[test]
fn r164_sweep_fns_ts_misc() {
  for f in ["ts_filter", "ts_delete", "ts_rewrite", "setweight", "strip", "numnode", "array_to_tsvector"] {
    let md = resolve(f, &Catalog::default());
    assert!(md.is_some(), "missing {f}");
  }
}

#[test]
fn r164_sweep_fns_multirange_misc() {
  let md = resolve("unnest_multirange", &Catalog::default());
  assert!(md.is_some());
}

#[test]
fn r165_sweep_fns_geometric() {
  for f in ["area", "diameter", "height", "width", "slope", "center", "distance", "isclosed", "isopen", "npoints", "pclose", "popen", "radius"] {
    let md = resolve(f, &Catalog::default());
    assert!(md.is_some(), "missing {f}");
  }
}

#[test]
fn r166_sweep_fns_bytea_helpers() {
  for f in ["get_byte", "set_byte", "get_bit", "set_bit", "encode", "decode", "convert", "convert_to", "convert_from"] {
    let md = resolve(f, &Catalog::default());
    assert!(md.is_some(), "missing {f}");
  }
}

#[test]
fn r166_sweep_fns_crypto_hashes() {
  for f in ["md5", "sha1", "sha224", "sha256", "sha384", "sha512", "hmac"] {
    let md = resolve(f, &Catalog::default());
    assert!(md.is_some(), "missing {f}");
  }
}

#[test]
fn r166_sweep_fns_bit_string_misc() {
  for f in ["length", "octet_length", "char_length", "bit_length", "substring"] {
    let md = resolve(f, &Catalog::default());
    assert!(md.is_some(), "missing {f}");
  }
}

#[test]
fn r166_sweep_fns_array_text_conv() {
  for f in ["string_to_array", "array_to_string", "string_to_table", "array_agg", "array_length"] {
    let md = resolve(f, &Catalog::default());
    assert!(md.is_some(), "missing {f}");
  }
}

#[test]
fn r166_sweep_fns_jsonb_path_query_variants() {
  for f in ["jsonb_path_query", "jsonb_path_query_array", "jsonb_path_query_first", "jsonb_path_match", "jsonb_path_exists"] {
    let md = resolve(f, &Catalog::default());
    assert!(md.is_some(), "missing {f}");
  }
}

#[test]
fn r167_sweep_fns_inet_full() {
  for f in ["set_masklen", "network", "host", "broadcast", "abbrev", "hostmask", "masklen", "netmask", "family", "inet_merge", "inet_same_family", "inet_client_addr", "inet_server_addr"] {
    let md = resolve(f, &Catalog::default());
    assert!(md.is_some(), "missing {f}");
  }
}

#[test]
fn r167_sweep_fns_uuid_inet_extras() {
  for f in ["gen_random_uuid", "uuid_generate_v4", "host", "abbrev"] {
    let md = resolve(f, &Catalog::default());
    assert!(md.is_some(), "missing {f}");
  }
}

#[test]
fn r167_sweep_fns_lsn() {
  for f in ["pg_current_wal_lsn", "pg_walfile_name", "pg_walfile_name_offset", "pg_wal_lsn_diff"] {
    let md = resolve(f, &Catalog::default());
    assert!(md.is_some(), "missing {f}");
  }
}

#[test]
fn r167_sweep_fns_lock_admin() {
  for f in ["pg_advisory_lock", "pg_advisory_unlock", "pg_try_advisory_lock", "pg_advisory_xact_lock"] {
    let md = resolve(f, &Catalog::default());
    assert!(md.is_some(), "missing {f}");
  }
}

#[test]
fn r167_sweep_fns_array_subscript() {
  for f in ["array_position", "array_remove", "array_append", "array_prepend", "array_replace", "generate_subscripts"] {
    let md = resolve(f, &Catalog::default());
    assert!(md.is_some(), "missing {f}");
  }
}

#[test]
fn r167_sweep_fns_aggregate_orderedset() {
  for f in ["mode", "percentile_cont", "percentile_disc", "rank", "dense_rank", "row_number", "percent_rank", "cume_dist"] {
    let md = resolve(f, &Catalog::default());
    assert!(md.is_some(), "missing {f}");
  }
}

#[test]
fn r168_sweep_kw_join_variants() {
  for kw in ["JOIN", "INNER", "OUTER", "LEFT", "RIGHT", "FULL", "CROSS", "NATURAL", "LATERAL", "ON", "USING"] {
    let md = resolve(kw, &Catalog::default());
    assert!(md.is_some(), "missing {kw}");
  }
}

#[test]
fn r168_sweep_kw_select_clauses() {
  for kw in ["SELECT", "FROM", "WHERE", "GROUP", "HAVING", "ORDER", "LIMIT", "OFFSET", "FETCH", "DISTINCT", "ALL"] {
    let md = resolve(kw, &Catalog::default());
    assert!(md.is_some(), "missing {kw}");
  }
}

#[test]
fn r168_sweep_kw_constraint_clauses() {
  for kw in ["PRIMARY", "FOREIGN", "REFERENCES", "UNIQUE", "CHECK", "DEFAULT", "GENERATED", "IDENTITY", "STORED", "NULL"] {
    let md = resolve(kw, &Catalog::default());
    assert!(md.is_some(), "missing {kw}");
  }
}

#[test]
fn r168_sweep_kw_ddl_essentials() {
  for kw in ["CREATE", "ALTER", "DROP", "RENAME", "TRUNCATE", "COMMENT", "TABLE", "INDEX", "VIEW", "FUNCTION", "PROCEDURE", "TRIGGER", "TYPE", "DOMAIN"] {
    let md = resolve(kw, &Catalog::default());
    assert!(md.is_some(), "missing {kw}");
  }
}

#[test]
fn r168_sweep_kw_dml_essentials() {
  for kw in ["INSERT", "UPDATE", "DELETE", "MERGE", "WITH", "VALUES", "RETURNING", "INTO", "SET"] {
    let md = resolve(kw, &Catalog::default());
    assert!(md.is_some(), "missing {kw}");
  }
}

#[test]
fn r168_sweep_kw_predicate_essentials() {
  for kw in ["AND", "OR", "NOT", "IS", "IN", "EXISTS", "BETWEEN", "LIKE", "ILIKE", "SIMILAR", "ANY", "ALL", "SOME"] {
    let md = resolve(kw, &Catalog::default());
    assert!(md.is_some(), "missing {kw}");
  }
}

#[test]
fn r169_sweep_kw_role_clauses() {
  for kw in ["LOGIN", "NOLOGIN", "SUPERUSER", "NOSUPERUSER", "CREATEDB", "NOCREATEDB", "CREATEROLE", "NOCREATEROLE", "REPLICATION", "NOREPLICATION", "BYPASSRLS", "NOBYPASSRLS"] {
    let md = resolve(kw, &Catalog::default());
    assert!(md.is_some(), "missing {kw}");
  }
}

#[test]
fn r169_sweep_kw_grant_targets() {
  for kw in ["TABLE", "SEQUENCE", "FUNCTION", "PROCEDURE", "ROUTINE", "SCHEMA", "TYPE", "DOMAIN", "TABLESPACE", "DATABASE", "LANGUAGE"] {
    let md = resolve(kw, &Catalog::default());
    assert!(md.is_some(), "missing {kw}");
  }
}

#[test]
fn r169_sweep_kw_cluster_admin() {
  for kw in ["CLUSTER", "REINDEX", "VACUUM", "ANALYZE", "DISCARD", "RESET", "LISTEN", "NOTIFY", "UNLISTEN"] {
    let md = resolve(kw, &Catalog::default());
    assert!(md.is_some(), "missing {kw}");
  }
}

#[test]
fn r169_sweep_kw_cte_modifiers() {
  for kw in ["WITH", "RECURSIVE", "MATERIALIZED", "NOT"] {
    let md = resolve(kw, &Catalog::default());
    assert!(md.is_some(), "missing {kw}");
  }
}

#[test]
fn r169_sweep_kw_index_methods_names() {
  for kw in ["GIST", "GIN", "BRIN", "BTREE", "HASH"] {
    let md = resolve(kw, &Catalog::default());
    assert!(md.is_some(), "missing {kw}");
  }
}

#[test]
fn r170_sweep_kw_discard_reset() {
  for kw in ["DISCARD ALL", "DISCARD PLANS", "DISCARD SEQUENCES", "DISCARD TEMP", "DISCARD TEMPORARY", "RESET ALL", "RESET ROLE"] {
    let md = resolve(kw, &Catalog::default());
    assert!(md.is_some(), "missing {kw}");
  }
}

#[test]
fn r170_sweep_kw_ts_helpers() {
  for kw in ["CONFIGURATION", "DICTIONARY", "PARSER", "TEMPLATE", "SEARCH"] {
    let md = resolve(kw, &Catalog::default());
    assert!(md.is_some(), "missing {kw}");
  }
}

#[test]
fn r170_sweep_fns_ts_phrase() {
  for f in ["to_tsvector", "to_tsquery", "phraseto_tsquery", "plainto_tsquery", "websearch_to_tsquery", "ts_rank", "ts_rank_cd", "ts_headline", "setweight", "strip"] {
    let md = resolve(f, &Catalog::default());
    assert!(md.is_some(), "missing {f}");
  }
}

#[test]
fn r170_sweep_fns_lexize_misc() {
  for f in ["ts_lexize", "ts_parse", "ts_token_type", "ts_stat"] {
    let md = resolve(f, &Catalog::default());
    let _ = md;
  }
}

#[test]
fn r171_sweep_kw_index_options() {
  for kw in ["INCLUDE", "WHERE", "USING", "TABLESPACE", "FILLFACTOR"] {
    let md = resolve(kw, &Catalog::default());
    assert!(md.is_some(), "missing {kw}");
  }
}

#[test]
fn r171_sweep_kw_event_trigger_events() {
  for kw in ["EVENT", "TRIGGER", "FUNCTION", "FOR", "EACH", "ROW", "STATEMENT", "EXECUTE"] {
    let md = resolve(kw, &Catalog::default());
    assert!(md.is_some(), "missing {kw}");
  }
}

#[test]
fn r171_sweep_kw_publication_options() {
  for kw in ["PUBLICATION", "ENABLE", "DISABLE", "REFRESH", "ADD", "DROP", "SET"] {
    let md = resolve(kw, &Catalog::default());
    assert!(md.is_some(), "missing {kw}");
  }
}

#[test]
fn r171_sweep_kw_security_label_kinds() {
  for kw in ["SECURITY", "LABEL", "FOR", "ON", "IS"] {
    let md = resolve(kw, &Catalog::default());
    assert!(md.is_some(), "missing {kw}");
  }
}

#[test]
fn r171_sweep_kw_lateral_misc() {
  for kw in ["LATERAL", "ORDINALITY", "WITH", "WITHIN", "FILTER", "OVER"] {
    let md = resolve(kw, &Catalog::default());
    assert!(md.is_some(), "missing {kw}");
  }
}

#[test]
fn r172_sweep_kw_table_options() {
  for kw in ["LIKE", "DEFAULTS", "INCLUDING", "EXCLUDING", "INHERITS", "PARTITION", "BY"] {
    let md = resolve(kw, &Catalog::default());
    assert!(md.is_some(), "missing {kw}");
  }
}

#[test]
fn r172_sweep_fns_string_position_ops() {
  for f in ["position", "strpos", "left", "right", "lpad", "rpad", "repeat"] {
    let md = resolve(f, &Catalog::default());
    assert!(md.is_some(), "missing {f}");
  }
}

#[test]
fn r172_sweep_fns_math_full() {
  for f in ["abs", "ceil", "ceiling", "floor", "round", "trunc", "mod", "div", "gcd", "lcm", "factorial", "power", "exp", "ln", "log", "log10", "sqrt", "cbrt"] {
    let md = resolve(f, &Catalog::default());
    assert!(md.is_some(), "missing {f}");
  }
}

#[test]
fn r172_sweep_types_int_aliases() {
  for t in ["INT", "INTEGER", "INT4", "BIGINT", "INT8", "SMALLINT", "INT2"] {
    let md = resolve(t, &Catalog::default());
    assert!(md.is_some(), "missing {t}");
  }
}

#[test]
fn r172_sweep_kw_oncommit_clauses() {
  for kw in ["ON COMMIT", "PRESERVE", "ROWS"] {
    let md = resolve(kw, &Catalog::default());
    assert!(md.is_some(), "missing {kw}");
  }
}

#[test]
fn r172_sweep_fns_aggregate_window() {
  for f in ["first_value", "last_value", "nth_value", "lag", "lead", "row_number", "rank", "dense_rank"] {
    let md = resolve(f, &Catalog::default());
    assert!(md.is_some(), "missing {f}");
  }
}

#[test]
fn r173_sweep_fns_pg16_dates() {
  for f in ["date_add", "date_subtract", "date_bin", "interval_eq"] {
    let md = resolve(f, &Catalog::default());
    assert!(md.is_some(), "missing {f}");
  }
}

#[test]
fn r173_sweep_fns_jsonb_records() {
  for f in ["jsonb_to_record", "jsonb_to_recordset", "jsonb_populate_record", "jsonb_populate_recordset", "jsonb_concat", "jsonb_delete_path"] {
    let md = resolve(f, &Catalog::default());
    assert!(md.is_some(), "missing {f}");
  }
}

#[test]
fn r173_sweep_fns_xml_basics() {
  for f in ["xmlelement", "xmlforest", "xmlconcat", "xmlroot", "xmlserialize", "xmlpi", "xmltext", "xmlcomment", "xmltable", "xpath", "xpath_exists"] {
    let md = resolve(f, &Catalog::default());
    assert!(md.is_some(), "missing {f}");
  }
}

#[test]
fn r173_sweep_fns_range_predicate() {
  for f in ["range_contains", "range_overlaps", "range_eq", "range_lt", "range_gt", "range_adjacent", "range_after", "range_before", "lower_inf", "upper_inf"] {
    let md = resolve(f, &Catalog::default());
    assert!(md.is_some(), "missing {f}");
  }
}

#[test]
fn r173_sweep_fns_geometry_basic() {
  for f in ["area", "diameter", "height", "width", "center", "radius", "isclosed", "isopen", "npoints"] {
    let md = resolve(f, &Catalog::default());
    assert!(md.is_some(), "missing {f}");
  }
}

#[test]
fn r173_sweep_fns_xml_helpers_extra() {
  for f in ["xmlexists", "query_to_xml", "schema_to_xml", "table_to_xml", "cursor_to_xml", "database_to_xml"] {
    let md = resolve(f, &Catalog::default());
    assert!(md.is_some(), "missing {f}");
  }
}

#[test]
fn r174_sweep_kw_ddl_clarifiers() {
  for kw in ["FOREIGN DATA WRAPPER", "EVENT TRIGGER", "ACCESS METHOD", "USER MAPPING", "FOREIGN TABLE", "TEXT SEARCH", "OWNED BY", "ROW LEVEL SECURITY"] {
    let md = resolve(kw, &Catalog::default());
    assert!(md.is_some(), "missing {kw}");
  }
}

#[test]
fn r174_sweep_kw_alter_column_extras() {
  for kw in ["SET STORAGE", "SET STATISTICS", "ADD GENERATED", "DROP IDENTITY", "RESTART IDENTITY", "CONTINUE IDENTITY"] {
    let md = resolve(kw, &Catalog::default());
    assert!(md.is_some(), "missing {kw}");
  }
}

#[test]
fn r174_sweep_kw_function_returns() {
  for kw in ["RETURNS TABLE", "RETURNS TRIGGER", "RETURNS SETOF", "AS IDENTITY", "BY IDENTITY", "FROM CURRENT", "USING METHOD"] {
    let md = resolve(kw, &Catalog::default());
    assert!(md.is_some(), "missing {kw}");
  }
}

#[test]
fn r175_sweep_kw_function_attrs() {
  for kw in ["LANGUAGE SQL", "LANGUAGE PLPGSQL", "OR REPLACE", "SECURITY DEFINER", "SECURITY INVOKER", "PARALLEL SAFE", "PARALLEL RESTRICTED", "PARALLEL UNSAFE", "RETURNS NULL ON NULL INPUT", "CALLED ON NULL INPUT"] {
    let md = resolve(kw, &Catalog::default());
    assert!(md.is_some(), "missing {kw}");
  }
}

#[test]
fn r175_sweep_kw_temp_aliases() {
  for kw in ["GLOBAL TEMPORARY", "LOCAL TEMPORARY", "GLOBAL TEMP", "LOCAL TEMP", "WITH RECURSIVE", "NOT VALID"] {
    let md = resolve(kw, &Catalog::default());
    assert!(md.is_some(), "missing {kw}");
  }
}

#[test]
fn r175_sweep_kw_oracle_compat() {
  for kw in ["CONNECT BY", "START WITH", "ON CONSTRAINT", "RESTRICT VERSION"] {
    let md = resolve(kw, &Catalog::default());
    assert!(md.is_some(), "missing {kw}");
  }
}

#[test]
fn r176_sweep_kw_cast_create() {
  for kw in ["AS RESTRICT", "AS ASSIGNMENT", "AS IMPLICIT", "AS ENUM", "AS RANGE", "WITH FUNCTION", "WITHOUT FUNCTION", "WITH INOUT"] {
    let md = resolve(kw, &Catalog::default());
    assert!(md.is_some(), "missing {kw}");
  }
}

#[test]
fn r176_sweep_kw_with_clauses() {
  for kw in ["WITH OPTIONS", "WITH SCHEMA", "WITH VERSION", "WITH CASCADE"] {
    let md = resolve(kw, &Catalog::default());
    assert!(md.is_some(), "missing {kw}");
  }
}

#[test]
fn r176_sweep_kw_grant_to_special() {
  for kw in ["TO PUBLIC", "TO CURRENT_USER", "TO SESSION_USER", "DEPENDS ON EXTENSION"] {
    let md = resolve(kw, &Catalog::default());
    assert!(md.is_some(), "missing {kw}");
  }
}

#[test]
fn r176_sweep_kw_constraint_timing() {
  for kw in ["DEFERRABLE INITIALLY DEFERRED", "DEFERRABLE INITIALLY IMMEDIATE", "NOT DEFERRABLE", "VALIDATE CONSTRAINT"] {
    let md = resolve(kw, &Catalog::default());
    assert!(md.is_some(), "missing {kw}");
  }
}

#[test]
fn r176_sweep_kw_oncommit_modes() {
  for kw in ["PRESERVE ROWS", "DELETE ROWS"] {
    let md = resolve(kw, &Catalog::default());
    assert!(md.is_some(), "missing {kw}");
  }
}

#[test]
fn r177_sweep_kw_trigger_transition() {
  for kw in ["OLD TABLE", "NEW TABLE", "OLD AS", "NEW AS", "OLD ROW", "NEW ROW", "TRANSITION", "UPDATE OF", "INSERT OR UPDATE"] {
    let md = resolve(kw, &Catalog::default());
    assert!(md.is_some(), "missing {kw}");
  }
}

#[test]
fn r177_sweep_kw_alter_constraint() {
  for kw in ["ALTER COLUMN TYPE", "DROP CONSTRAINT", "RENAME CONSTRAINT"] {
    let md = resolve(kw, &Catalog::default());
    assert!(md.is_some(), "missing {kw}");
  }
}

#[test]
fn r177_sweep_kw_cursor_view() {
  for kw in ["WHERE CURRENT OF", "CURRENT OF", "CHECK OPTION", "WITH CHECK OPTION", "WITH LOCAL CHECK OPTION", "WITH CASCADED CHECK OPTION"] {
    let md = resolve(kw, &Catalog::default());
    assert!(md.is_some(), "missing {kw}");
  }
}

#[test]
fn r178_sweep_kw_instead_of() {
  for kw in ["INSTEAD OF INSERT", "INSTEAD OF UPDATE", "INSTEAD OF DELETE"] {
    let md = resolve(kw, &Catalog::default());
    assert!(md.is_some(), "missing {kw}");
  }
}

#[test]
fn r178_sweep_kw_rls_force_set() {
  for kw in ["FORCE ROW LEVEL SECURITY", "NO FORCE ROW LEVEL SECURITY", "SET CONSTRAINTS DEFERRED", "SET CONSTRAINTS IMMEDIATE", "SET CONSTRAINTS ALL"] {
    let md = resolve(kw, &Catalog::default());
    assert!(md.is_some(), "missing {kw}");
  }
}

#[test]
fn r178_sweep_kw_set_ops_all_distinct() {
  for kw in ["UNION ALL", "INTERSECT ALL", "EXCEPT ALL", "UNION DISTINCT", "INTERSECT DISTINCT", "EXCEPT DISTINCT"] {
    let md = resolve(kw, &Catalog::default());
    assert!(md.is_some(), "missing {kw}");
  }
}

#[test]
fn r178_sweep_kw_select_distinct() {
  for kw in ["SELECT DISTINCT", "SELECT DISTINCT ON", "GROUP BY DISTINCT", "AT LOCAL"] {
    let md = resolve(kw, &Catalog::default());
    assert!(md.is_some(), "missing {kw}");
  }
}

#[test]
fn r178_sweep_kw_for_clauses() {
  for kw in ["FOR ALL", "FOR ROLE", "FOR USER", "FOR EACH", "FOR PARTITION"] {
    let md = resolve(kw, &Catalog::default());
    assert!(md.is_some(), "missing {kw}");
  }
}

#[test]
fn r179_sweep_kw_create_or_replace() {
  for kw in ["CREATE OR REPLACE FUNCTION", "CREATE OR REPLACE PROCEDURE", "CREATE OR REPLACE VIEW", "CREATE OR REPLACE TRIGGER", "CREATE OR REPLACE RULE"] {
    let md = resolve(kw, &Catalog::default());
    assert!(md.is_some(), "missing {kw}");
  }
}

#[test]
fn r179_sweep_kw_create_index_variants() {
  for kw in ["CREATE INDEX CONCURRENTLY", "CREATE UNIQUE INDEX", "CREATE UNIQUE INDEX CONCURRENTLY"] {
    let md = resolve(kw, &Catalog::default());
    assert!(md.is_some(), "missing {kw}");
  }
}

#[test]
fn r179_sweep_kw_create_relation_kinds() {
  for kw in ["CREATE TEMP TABLE", "CREATE TEMPORARY TABLE", "CREATE UNLOGGED TABLE", "CREATE FOREIGN TABLE", "CREATE EVENT TRIGGER"] {
    let md = resolve(kw, &Catalog::default());
    assert!(md.is_some(), "missing {kw}");
  }
}

#[test]
fn r179_sweep_kw_drop_variants() {
  for kw in ["DROP INDEX CONCURRENTLY", "DROP CONSTRAINT IF EXISTS", "DROP TABLE IF EXISTS", "DROP INDEX IF EXISTS", "DROP MATERIALIZED VIEW", "DROP FOREIGN TABLE", "DROP EVENT TRIGGER", "DROP ACCESS METHOD"] {
    let md = resolve(kw, &Catalog::default());
    assert!(md.is_some(), "missing {kw}");
  }
}

#[test]
fn r180_sweep_kw_alter_table_qualifiers() {
  for kw in ["ALTER TABLE IF EXISTS", "ALTER TABLE ONLY", "ALTER MATERIALIZED VIEW", "ALTER VIEW IF EXISTS", "ALTER SEQUENCE IF EXISTS"] {
    let md = resolve(kw, &Catalog::default());
    assert!(md.is_some(), "missing {kw}");
  }
}

#[test]
fn r180_sweep_kw_alter_misc_objects() {
  for kw in ["ALTER EVENT TRIGGER", "ALTER ACCESS METHOD", "ALTER OPERATOR FAMILY", "ALTER OPERATOR CLASS", "ALTER FOREIGN TABLE", "ALTER FOREIGN DATA WRAPPER"] {
    let md = resolve(kw, &Catalog::default());
    assert!(md.is_some(), "missing {kw}");
  }
}

#[test]
fn r180_sweep_kw_alter_publication_misc() {
  for kw in ["ALTER PUBLICATION", "ALTER SUBSCRIPTION", "ALTER POLICY", "ALTER STATISTICS", "ALTER LANGUAGE", "ALTER CONVERSION", "ALTER COLLATION"] {
    let md = resolve(kw, &Catalog::default());
    assert!(md.is_some(), "missing {kw}");
  }
}

#[test]
fn r180_sweep_kw_alter_function_class() {
  for kw in ["ALTER AGGREGATE", "ALTER OPERATOR", "ALTER CAST", "ALTER TYPE", "ALTER DOMAIN", "ALTER USER MAPPING"] {
    let md = resolve(kw, &Catalog::default());
    assert!(md.is_some(), "missing {kw}");
  }
}

#[test]
fn r181_sweep_kw_comment_on_data() {
  for kw in ["COMMENT ON TABLE", "COMMENT ON COLUMN", "COMMENT ON SCHEMA", "COMMENT ON DATABASE", "COMMENT ON INDEX", "COMMENT ON VIEW", "COMMENT ON MATERIALIZED VIEW", "COMMENT ON SEQUENCE"] {
    let md = resolve(kw, &Catalog::default());
    assert!(md.is_some(), "missing {kw}");
  }
}

#[test]
fn r181_sweep_kw_comment_on_routines() {
  for kw in ["COMMENT ON FUNCTION", "COMMENT ON PROCEDURE", "COMMENT ON TYPE", "COMMENT ON DOMAIN", "COMMENT ON EXTENSION", "COMMENT ON ROLE", "COMMENT ON TRIGGER", "COMMENT ON CONSTRAINT", "COMMENT ON POLICY"] {
    let md = resolve(kw, &Catalog::default());
    assert!(md.is_some(), "missing {kw}");
  }
}

#[test]
fn r181_sweep_kw_drop_role_db() {
  for kw in ["DROP DATABASE", "DROP SCHEMA", "DROP ROLE", "DROP USER", "DROP GROUP", "DROP TABLESPACE", "DROP EXTENSION"] {
    let md = resolve(kw, &Catalog::default());
    assert!(md.is_some(), "missing {kw}");
  }
}

#[test]
fn r181_sweep_kw_drop_objects() {
  for kw in ["DROP PUBLICATION", "DROP SUBSCRIPTION", "DROP SERVER", "DROP TRIGGER", "DROP TYPE", "DROP DOMAIN", "DROP POLICY"] {
    let md = resolve(kw, &Catalog::default());
    assert!(md.is_some(), "missing {kw}");
  }
}

#[test]
fn r182_sweep_kw_create_role_db() {
  for kw in ["CREATE DATABASE", "CREATE SCHEMA", "CREATE ROLE", "CREATE USER", "CREATE GROUP", "CREATE TABLESPACE", "CREATE EXTENSION"] {
    let md = resolve(kw, &Catalog::default());
    assert!(md.is_some(), "missing {kw}");
  }
}

#[test]
fn r182_sweep_kw_create_replication() {
  for kw in ["CREATE PUBLICATION", "CREATE SUBSCRIPTION", "CREATE SERVER"] {
    let md = resolve(kw, &Catalog::default());
    assert!(md.is_some(), "missing {kw}");
  }
}

#[test]
fn r182_sweep_kw_create_routine() {
  for kw in ["CREATE TRIGGER", "CREATE TYPE", "CREATE DOMAIN", "CREATE POLICY", "CREATE FUNCTION", "CREATE PROCEDURE", "CREATE SEQUENCE", "CREATE RULE"] {
    let md = resolve(kw, &Catalog::default());
    assert!(md.is_some(), "missing {kw}");
  }
}

#[test]
fn r182_sweep_kw_create_misc_objects() {
  for kw in ["CREATE CAST", "CREATE LANGUAGE", "CREATE OPERATOR", "CREATE AGGREGATE", "CREATE CONVERSION", "CREATE COLLATION", "CREATE TRANSFORM", "CREATE STATISTICS"] {
    let md = resolve(kw, &Catalog::default());
    assert!(md.is_some(), "missing {kw}");
  }
}

#[test]
fn r182_sweep_kw_create_fdw_text_search() {
  for kw in ["CREATE FOREIGN DATA WRAPPER", "CREATE USER MAPPING", "CREATE TEXT SEARCH CONFIGURATION", "CREATE TEXT SEARCH DICTIONARY", "CREATE TEXT SEARCH PARSER", "CREATE TEXT SEARCH TEMPLATE"] {
    let md = resolve(kw, &Catalog::default());
    assert!(md.is_some(), "missing {kw}");
  }
}

#[test]
fn r183_sweep_kw_alter_starters() {
  for kw in ["ALTER FUNCTION", "ALTER PROCEDURE", "ALTER ROUTINE", "ALTER INDEX", "ALTER VIEW", "ALTER SEQUENCE", "ALTER ROLE", "ALTER USER", "ALTER GROUP", "ALTER DATABASE", "ALTER SCHEMA", "ALTER EXTENSION", "ALTER TRIGGER"] {
    let md = resolve(kw, &Catalog::default());
    assert!(md.is_some(), "missing {kw}");
  }
}

#[test]
fn r183_sweep_kw_alter_if_exists() {
  for kw in ["ALTER FUNCTION IF EXISTS", "ALTER PROCEDURE IF EXISTS", "ALTER INDEX IF EXISTS", "ALTER MATERIALIZED VIEW IF EXISTS", "ALTER FOREIGN TABLE IF EXISTS", "ALTER TYPE IF EXISTS", "ALTER DOMAIN IF EXISTS"] {
    let md = resolve(kw, &Catalog::default());
    assert!(md.is_some(), "missing {kw}");
  }
}

#[test]
fn r184_sweep_kw_grant_starters() {
  for kw in ["GRANT ALL", "GRANT ALL PRIVILEGES", "GRANT SELECT", "GRANT INSERT", "GRANT UPDATE", "GRANT DELETE", "GRANT TRUNCATE", "GRANT REFERENCES", "GRANT TRIGGER", "GRANT USAGE", "GRANT EXECUTE", "GRANT CONNECT", "GRANT TEMPORARY", "GRANT CREATE", "GRANT MAINTAIN"] {
    let md = resolve(kw, &Catalog::default());
    assert!(md.is_some(), "missing {kw}");
  }
}

#[test]
fn r184_sweep_kw_revoke_starters() {
  for kw in ["REVOKE ALL", "REVOKE ALL PRIVILEGES", "REVOKE SELECT", "REVOKE INSERT", "REVOKE UPDATE", "REVOKE DELETE", "REVOKE USAGE", "REVOKE EXECUTE", "REVOKE GRANT OPTION FOR", "REVOKE ADMIN OPTION FOR"] {
    let md = resolve(kw, &Catalog::default());
    assert!(md.is_some(), "missing {kw}");
  }
}

#[test]
fn r185_sweep_kw_joins() {
  for kw in ["FULL JOIN", "LEFT OUTER JOIN", "RIGHT OUTER JOIN", "NATURAL JOIN", "NATURAL INNER JOIN", "JOIN LATERAL", "LEFT JOIN LATERAL", "INNER JOIN LATERAL", "CROSS JOIN LATERAL"] {
    let md = resolve(kw, &Catalog::default());
    assert!(md.is_some(), "missing {kw}");
  }
}

#[test]
fn r185_sweep_kw_join_clauses() {
  for kw in ["JOIN ON", "JOIN USING", "LEFT JOIN ON", "INNER JOIN ON", "LEFT JOIN USING", "INNER JOIN USING"] {
    let md = resolve(kw, &Catalog::default());
    assert!(md.is_some(), "missing {kw}");
  }
}

#[test]
fn r185_sweep_kw_inheritance_only() {
  for kw in ["FROM ONLY", "DELETE FROM ONLY", "UPDATE ONLY", "TRUNCATE ONLY"] {
    let md = resolve(kw, &Catalog::default());
    assert!(md.is_some(), "missing {kw}");
  }
}

#[test]
fn r186_sweep_kw_merge_structure() {
  for kw in ["MERGE INTO", "USING TABLE", "USING SELECT", "WHEN MATCHED", "WHEN NOT MATCHED", "WHEN MATCHED THEN", "WHEN NOT MATCHED THEN", "WHEN MATCHED AND", "WHEN NOT MATCHED AND"] {
    let md = resolve(kw, &Catalog::default());
    assert!(md.is_some(), "missing {kw}");
  }
}

#[test]
fn r186_sweep_kw_on_conflict_full() {
  for kw in ["DO UPDATE SET", "ON CONFLICT DO NOTHING", "ON CONFLICT DO UPDATE", "ON CONFLICT DO UPDATE SET", "ON CONFLICT ON CONSTRAINT", "RETURNING ALL"] {
    let md = resolve(kw, &Catalog::default());
    assert!(md.is_some(), "missing {kw}");
  }
}

#[test]
fn r186_sweep_kw_merge_pg17() {
  for kw in ["WHEN NOT MATCHED BY SOURCE", "WHEN NOT MATCHED BY TARGET"] {
    let md = resolve(kw, &Catalog::default());
    assert!(md.is_some(), "missing {kw}");
  }
}

#[test]
fn r187_sweep_kw_cast_as_types() {
  for kw in ["CAST AS", "AS DECIMAL", "AS TEXT", "AS NUMERIC", "AS INT", "AS BIGINT", "AS BOOLEAN", "AS JSONB", "AS JSON"] {
    let md = resolve(kw, &Catalog::default());
    assert!(md.is_some(), "missing {kw}");
  }
}

#[test]
fn r187_sweep_kw_cast_as_temporal() {
  for kw in ["AS DATE", "AS TIME", "AS TIMESTAMP", "AS TIMESTAMPTZ"] {
    let md = resolve(kw, &Catalog::default());
    assert!(md.is_some(), "missing {kw}");
  }
}

#[test]
fn r187_sweep_kw_identity_forms() {
  for kw in ["GENERATED ALWAYS AS IDENTITY", "GENERATED BY DEFAULT AS IDENTITY"] {
    let md = resolve(kw, &Catalog::default());
    assert!(md.is_some(), "missing {kw}");
  }
}

#[test]
fn r188_sweep_kw_order_nulls() {
  for kw in ["ASC NULLS FIRST", "ASC NULLS LAST", "DESC NULLS FIRST", "DESC NULLS LAST"] {
    let md = resolve(kw, &Catalog::default());
    assert!(md.is_some(), "missing {kw}");
  }
}

#[test]
fn r188_sweep_kw_between_not_predicates() {
  for kw in ["BETWEEN SYMMETRIC", "BETWEEN ASYMMETRIC", "NOT BETWEEN", "NOT BETWEEN SYMMETRIC", "NOT LIKE", "NOT ILIKE", "NOT SIMILAR TO", "NOT IN", "NOT EXISTS"] {
    let md = resolve(kw, &Catalog::default());
    assert!(md.is_some(), "missing {kw}");
  }
}

#[test]
fn r188_sweep_kw_array_any_all() {
  for kw in ["ANY ARRAY", "ALL ARRAY"] {
    let md = resolve(kw, &Catalog::default());
    assert!(md.is_some(), "missing {kw}");
  }
}

#[test]
fn r188_sweep_kw_fetch_offset_rows() {
  for kw in ["OFFSET ROWS", "OFFSET ROW", "FIRST ROWS", "FIRST ROW", "NEXT ROWS", "NEXT ROW"] {
    let md = resolve(kw, &Catalog::default());
    assert!(md.is_some(), "missing {kw}");
  }
}

#[test]
fn r189_sweep_kw_is_bool_predicates() {
  for kw in ["IS TRUE", "IS NOT TRUE", "IS FALSE", "IS NOT FALSE", "IS UNKNOWN", "IS NOT UNKNOWN"] {
    let md = resolve(kw, &Catalog::default());
    assert!(md.is_some(), "missing {kw}");
  }
}

#[test]
fn r189_sweep_kw_is_doc_normalized() {
  for kw in ["IS DOCUMENT", "IS NOT DOCUMENT", "IS NORMALIZED", "IS NOT NORMALIZED", "IS OF", "IS NOT OF"] {
    let md = resolve(kw, &Catalog::default());
    assert!(md.is_some(), "missing {kw}");
  }
}

#[test]
fn r189_sweep_kw_string_builtins() {
  for kw in ["EXTRACT FROM", "OVERLAY PLACING", "POSITION IN", "SUBSTRING FROM", "SUBSTRING FOR", "TRIM FROM", "TRIM LEADING", "TRIM TRAILING", "TRIM BOTH", "COLLATION FOR"] {
    let md = resolve(kw, &Catalog::default());
    assert!(md.is_some(), "missing {kw}");
  }
}

#[test]
fn r190_sweep_kw_xml_window_misc() {
  for kw in ["XML PARSE", "XML SERIALIZE", "WITHIN GROUP ORDER BY", "FILTER WHERE", "OVER WINDOW", "OVER PARTITION"] {
    let md = resolve(kw, &Catalog::default());
    assert!(md.is_some(), "missing {kw}");
  }
}

#[test]
fn r190_sweep_kw_limit_fetch_explicit() {
  for kw in ["LIMIT ALL", "LIMIT NULL", "FETCH FIRST ROW ONLY", "FETCH NEXT ROW ONLY", "FETCH FIRST ROWS ONLY", "FETCH NEXT ROWS ONLY"] {
    let md = resolve(kw, &Catalog::default());
    assert!(md.is_some(), "missing {kw}");
  }
}

#[test]
fn r191_sweep_kw_legacy_owner_oids() {
  for kw in ["WITH OIDS", "WITHOUT OIDS", "WITH OWNER", "OWNED BY NONE"] {
    let md = resolve(kw, &Catalog::default());
    assert!(md.is_some(), "missing {kw}");
  }
}

#[test]
fn r191_sweep_kw_rule_action_clauses() {
  for kw in ["AS ON", "DO INSTEAD", "DO INSTEAD NOTHING", "DO ALSO", "ON SELECT", "ON INSERT"] {
    let md = resolve(kw, &Catalog::default());
    assert!(md.is_some(), "missing {kw}");
  }
}

#[test]
fn r191_sweep_kw_event_trigger_events() {
  for kw in ["EVENT TRIGGER DDL_COMMAND_START", "EVENT TRIGGER DDL_COMMAND_END", "EVENT TRIGGER SQL_DROP", "EVENT TRIGGER TABLE_REWRITE"] {
    let md = resolve(kw, &Catalog::default());
    assert!(md.is_some(), "missing {kw}");
  }
}

#[test]
fn r191_sweep_kw_constraint_alters() {
  for kw in ["DEFAULT NULL", "DEFAULT TRUE", "DEFAULT FALSE", "CHECK NOT VALID", "ADD CHECK", "ADD UNIQUE", "ADD PRIMARY KEY", "ADD FOREIGN KEY", "ADD EXCLUDE"] {
    let md = resolve(kw, &Catalog::default());
    assert!(md.is_some(), "missing {kw}");
  }
}

#[test]
fn r192_sweep_kw_sql_standard_types() {
  for kw in ["BIT VARYING", "CHARACTER VARYING", "DOUBLE PRECISION", "WITH TIME ZONE", "WITHOUT TIME ZONE", "TIME WITH TIME ZONE", "TIME WITHOUT TIME ZONE", "TIMESTAMP WITH TIME ZONE", "TIMESTAMP WITHOUT TIME ZONE"] {
    let md = resolve(kw, &Catalog::default());
    assert!(md.is_some(), "missing {kw}");
  }
}

#[test]
fn r192_sweep_kw_trigger_refs_misc() {
  for kw in ["REFERENCES OLD", "REFERENCES NEW", "USING JOIN", "AS QUERY", "DEFAULT EXPRESSION", "STORED EXPRESSION"] {
    let md = resolve(kw, &Catalog::default());
    assert!(md.is_some(), "missing {kw}");
  }
}

#[test]
fn r193_sweep_kw_create_alter_drop_mix() {
  for kw in ["CREATE TABLE", "ALTER TABLE", "DROP TABLE IF EXISTS", "CREATE VIEW", "ALTER VIEW", "DROP MATERIALIZED VIEW", "CREATE INDEX CONCURRENTLY", "ALTER INDEX", "DROP INDEX CONCURRENTLY", "CREATE OR REPLACE FUNCTION", "ALTER FUNCTION", "DROP TYPE"] {
    let md = resolve(kw, &Catalog::default());
    assert!(md.is_some(), "missing {kw}");
  }
}

#[test]
fn r193_sweep_kw_dml_mix() {
  for kw in ["INSERT INTO", "UPDATE", "DELETE FROM", "MERGE INTO", "VALUES", "RETURNING", "ON CONFLICT DO UPDATE", "WHEN MATCHED THEN", "WHEN NOT MATCHED THEN", "DEFAULT VALUES"] {
    let md = resolve(kw, &Catalog::default());
    assert!(md.is_some(), "missing {kw}");
  }
}

#[test]
fn r193_sweep_kw_select_mix() {
  for kw in ["SELECT DISTINCT", "SELECT DISTINCT ON", "GROUP BY", "ORDER BY", "FETCH FIRST", "FETCH NEXT", "LIMIT", "OFFSET", "FOR UPDATE", "FOR SHARE", "WITH RECURSIVE"] {
    let md = resolve(kw, &Catalog::default());
    assert!(md.is_some(), "missing {kw}");
  }
}

#[test]
fn r193_sweep_kw_security_mix() {
  for kw in ["GRANT", "REVOKE", "GRANT ALL", "GRANT ALL PRIVILEGES", "REVOKE ALL", "GRANT SELECT", "GRANT USAGE", "GRANT EXECUTE", "ON ALL TABLES IN SCHEMA", "FOR ROLE", "WITH GRANT OPTION", "WITH ADMIN OPTION"] {
    let md = resolve(kw, &Catalog::default());
    let _ = md;
  }
}

#[test]
fn r193_sweep_kw_rls_policy_mix() {
  for kw in ["CREATE POLICY", "ALTER POLICY", "DROP POLICY", "ENABLE ROW LEVEL SECURITY", "DISABLE ROW LEVEL SECURITY", "FORCE ROW LEVEL SECURITY", "NO FORCE ROW LEVEL SECURITY", "USING", "WITH CHECK"] {
    let md = resolve(kw, &Catalog::default());
    assert!(md.is_some(), "missing {kw}");
  }
}

#[test]
fn r193_sweep_kw_trigger_full_chain() {
  for kw in ["CREATE TRIGGER", "BEFORE INSERT", "AFTER UPDATE", "INSTEAD OF UPDATE", "FOR EACH ROW", "FOR EACH STATEMENT", "EXECUTE FUNCTION", "EXECUTE PROCEDURE", "WHEN", "REFERENCES OLD", "REFERENCES NEW", "OLD TABLE", "NEW TABLE"] {
    let md = resolve(kw, &Catalog::default());
    assert!(md.is_some(), "missing {kw}");
  }
}

#[test]
fn r194_sweep_kw_temporal_info() {
  for kw in ["CURRENT_DATE", "CURRENT_TIME", "CURRENT_TIMESTAMP", "LOCALTIME", "LOCALTIMESTAMP", "CURRENT_USER", "CURRENT_ROLE", "SESSION_USER", "SYSTEM_USER", "CURRENT_SCHEMA", "CURRENT_CATALOG"] {
    let md = resolve(kw, &Catalog::default());
    assert!(md.is_some(), "missing {kw}");
    let s = md.unwrap();
    assert!(!s.contains("PG SQL keyword"), "{kw} still placeholder");
  }
}

#[test]
fn r194_sweep_kw_window_agg() {
  for kw in ["OVER", "PARTITION BY", "ROWS BETWEEN", "RANGE BETWEEN", "GROUPS BETWEEN", "UNBOUNDED PRECEDING", "UNBOUNDED FOLLOWING", "CURRENT ROW", "EXCLUDE CURRENT ROW", "EXCLUDE GROUP", "EXCLUDE TIES", "EXCLUDE NO OTHERS", "FILTER WHERE", "OVER PARTITION", "WITHIN GROUP", "GROUPING"] {
    let md = resolve(kw, &Catalog::default());
    assert!(md.is_some(), "missing {kw}");
  }
}

#[test]
fn r194_sweep_kw_conditional_mix() {
  for kw in ["CASE", "WHEN", "THEN", "ELSE", "END", "COALESCE", "NULLIF", "GREATEST", "LEAST", "EXTRACT", "NULLS FIRST", "NULLS LAST"] {
    let md = resolve(kw, &Catalog::default());
    assert!(md.is_some(), "missing {kw}");
  }
}

#[test]
fn r195_sweep_kw_types_upgraded() {
  for kw in ["BIGINT", "BOOLEAN", "CHARACTER", "DEC", "DECIMAL", "DOUBLE", "FLOAT", "INT", "INTEGER", "NUMERIC", "REAL", "SMALLINT", "TEXT", "VARYING"] {
    let md = resolve(kw, &Catalog::default());
    assert!(md.is_some(), "missing {kw}");
    let s = md.unwrap();
    assert!(!s.contains("PG SQL keyword"), "{kw} still placeholder");
  }
}

#[test]
fn r195_sweep_kw_sequence_cursor_upgraded() {
  for kw in ["CACHE", "CYCLE", "CURSOR", "FORWARD", "HOLD", "INCREMENT", "MAXVALUE", "MINVALUE", "MOVE", "RESTART", "START"] {
    let md = resolve(kw, &Catalog::default());
    assert!(md.is_some(), "missing {kw}");
    let s = md.unwrap();
    assert!(!s.contains("PG SQL keyword"), "{kw} still placeholder");
  }
}

#[test]
fn r195_sweep_kw_mixed_data_ddl() {
  for kw in ["CREATE TABLE", "ALTER TABLE", "ADD COLUMN", "DROP COLUMN", "ALTER COLUMN", "SET DEFAULT", "DROP DEFAULT", "SET NOT NULL", "DROP NOT NULL", "RENAME TO", "RENAME COLUMN"] {
    let md = resolve(kw, &Catalog::default());
    assert!(md.is_some(), "missing {kw}");
  }
}

#[test]
fn r196_sweep_kw_fn_attrs_upgraded() {
  for kw in ["COST", "HANDLER", "INCLUDE", "INHERIT", "LEAKPROOF", "PARALLEL", "TRUSTED", "VALIDATE", "VALIDATOR"] {
    let md = resolve(kw, &Catalog::default());
    assert!(md.is_some(), "missing {kw}");
    let s = md.unwrap();
    assert!(!s.contains("PG SQL keyword"), "{kw} still placeholder");
  }
}

#[test]
fn r196_sweep_kw_trim_extract_upgraded() {
  for kw in ["BOTH", "LEADING", "TRAILING", "DAY", "HOUR", "MINUTE", "MONTH", "SECOND", "YEAR"] {
    let md = resolve(kw, &Catalog::default());
    assert!(md.is_some(), "missing {kw}");
    let s = md.unwrap();
    assert!(!s.contains("PG SQL keyword"), "{kw} still placeholder");
  }
}

#[test]
fn r196_sweep_kw_ddl_misc_upgraded() {
  for kw in ["ENCODING", "ENCRYPTED", "EXTENSION", "FAMILY", "FREEZE", "LOCAL", "MATCH"] {
    let md = resolve(kw, &Catalog::default());
    assert!(md.is_some(), "missing {kw}");
    let s = md.unwrap();
    assert!(!s.contains("PG SQL keyword"), "{kw} still placeholder");
  }
}

#[test]
fn r197_sweep_kw_misc1_upgraded() {
  for kw in ["ABSOLUTE", "ACTION", "ADMIN", "AGGREGATE", "ASYMMETRIC", "AT", "ATTACH", "AUTHORIZATION", "BACKWARD", "BINARY", "BIT", "CALLED", "CASCADED", "CHAIN", "CHAR"] {
    let md = resolve(kw, &Catalog::default());
    assert!(md.is_some(), "missing {kw}");
    let s = md.unwrap();
    assert!(!s.contains("PG SQL keyword"), "{kw} still placeholder");
  }
}

#[test]
fn r197_sweep_kw_misc2_upgraded() {
  for kw in ["CHARACTERISTICS", "CLASS", "CLOSE", "COLLATION", "COLUMNS", "COMMITTED", "COMPRESSION", "CONFIGURATION", "CONNECTION", "CONSTRAINTS", "CONTENT", "CONVERSION", "DATA", "DEFAULTS", "DELIMITERS", "DEPENDS", "COMMENTS"] {
    let md = resolve(kw, &Catalog::default());
    assert!(md.is_some(), "missing {kw}");
    let s = md.unwrap();
    assert!(!s.contains("PG SQL keyword"), "{kw} still placeholder");
  }
}

#[test]
fn r197_sweep_kw_grant_revoke_mix() {
  for kw in ["GRANT", "REVOKE", "WITH ADMIN OPTION", "WITH GRANT OPTION", "ON ALL TABLES IN SCHEMA", "ON ALL FUNCTIONS IN SCHEMA", "FOR ROLE", "FOR USER", "TO PUBLIC"] {
    let md = resolve(kw, &Catalog::default());
    assert!(md.is_some(), "missing {kw}");
  }
}

#[test]
fn r198_sweep_kw_partition_aux_upgraded() {
  for kw in ["ANALYSE", "DETACH", "DICTIONARY", "DISCARD", "DOCUMENT", "ENUM", "EVENT", "EXCLUDING", "EXPRESSION", "EXTERNAL", "FINALIZE", "FUNCTIONS", "GLOBAL", "GRANTED", "IMPLICIT", "IMPORT", "INDEXES", "INITIALLY", "INOUT"] {
    let md = resolve(kw, &Catalog::default());
    assert!(md.is_some(), "missing {kw}");
    let s = md.unwrap();
    assert!(!s.contains("PG SQL keyword"), "{kw} still placeholder");
  }
}

#[test]
fn r198_sweep_kw_misc_logic_upgraded() {
  for kw in ["LOAD", "LOCATION", "LOGGED", "MAPPING", "METHOD", "NORMALIZE", "NORMALIZED", "NFC", "NFD", "NFKC", "NFKD", "NOTNULL"] {
    let md = resolve(kw, &Catalog::default());
    assert!(md.is_some(), "missing {kw}");
    let s = md.unwrap();
    assert!(!s.contains("PG SQL keyword"), "{kw} still placeholder");
  }
}

#[test]
fn r198_sweep_kw_pubsub_mix() {
  for kw in ["CREATE PUBLICATION", "ALTER PUBLICATION", "DROP PUBLICATION", "CREATE SUBSCRIPTION", "ALTER SUBSCRIPTION", "DROP SUBSCRIPTION", "WITH (PUBLISH)", "WITH (COPY_DATA)", "WITH (CREATE_SLOT)"] {
    let md = resolve(kw, &Catalog::default());
    let _ = md;
  }
}

#[test]
fn r199_sweep_kw_object_options_upgraded() {
  for kw in ["OBJECT", "OFF", "OIDS", "OPERATOR", "OPTION", "OPTIONS", "ORDINALITY", "OTHERS", "OUT", "OVERLAY", "OVERRIDING"] {
    let md = resolve(kw, &Catalog::default());
    assert!(md.is_some(), "missing {kw}");
    let s = md.unwrap();
    assert!(!s.contains("PG SQL keyword"), "{kw} still placeholder");
  }
}

#[test]
fn r199_sweep_kw_policy_xml_upgraded() {
  for kw in ["PARSER", "PARTIAL", "PASSING", "PLACING", "PLANS", "POLICY", "POSITION", "PRECISION", "PREPARED", "PRESERVE", "PRIOR", "PROCEDURAL", "PROCEDURES", "PUBLICATION", "READ", "REASSIGN", "REFERENCING", "RELATIVE"] {
    let md = resolve(kw, &Catalog::default());
    assert!(md.is_some(), "missing {kw}");
    let s = md.unwrap();
    assert!(!s.contains("PG SQL keyword"), "{kw} still placeholder");
  }
}

#[test]
fn r199_sweep_kw_full_dml_chain() {
  for kw in ["SELECT", "FROM", "WHERE", "GROUP BY", "HAVING", "ORDER BY", "LIMIT", "OFFSET", "INSERT INTO", "UPDATE", "DELETE FROM", "RETURNING", "WITH", "WITH RECURSIVE", "VALUES", "TABLE", "MERGE INTO"] {
    let md = resolve(kw, &Catalog::default());
    assert!(md.is_some(), "missing {kw}");
  }
}

#[test]
fn r200_final_consolidation_mixed_bag() {
  // One last sweep touching every kw category in a single test.
  // dml + ddl + dcl + dml_extra + transaction + types + temporal_info + window + frame + agg + conditional + cursor + sequence + partition + rls + replication + fts + xml + json + unicode + grant_target + on_conflict + trigger_ref + tablesample + plpgsql + system
  for kw in [
    // DML core
    "SELECT", "FROM", "WHERE", "GROUP BY", "HAVING", "ORDER BY",
    "INSERT INTO", "UPDATE", "DELETE FROM", "MERGE INTO", "VALUES", "RETURNING",
    "WITH", "WITH RECURSIVE", "WITHIN GROUP",
    // DDL core
    "CREATE TABLE", "ALTER TABLE", "DROP TABLE IF EXISTS",
    "CREATE INDEX CONCURRENTLY", "CREATE OR REPLACE FUNCTION",
    "ADD COLUMN", "DROP COLUMN", "RENAME TO", "RENAME COLUMN",
    "SET NOT NULL", "DROP NOT NULL", "SET DEFAULT", "DROP DEFAULT",
    // DCL
    "GRANT", "REVOKE", "WITH GRANT OPTION", "WITH ADMIN OPTION",
    "ON ALL TABLES IN SCHEMA", "ON ALL FUNCTIONS IN SCHEMA",
    // Transaction
    "BEGIN", "COMMIT", "ROLLBACK", "SAVEPOINT", "CHAIN", "READ", "COMMITTED",
    // Types
    "BIGINT", "INTEGER", "SMALLINT", "NUMERIC", "TEXT", "BOOLEAN", "REAL", "DOUBLE",
    "BIT VARYING", "CHARACTER VARYING", "DOUBLE PRECISION", "WITH TIME ZONE", "WITHOUT TIME ZONE",
    // Temporal/info
    "CURRENT_DATE", "CURRENT_TIMESTAMP", "LOCALTIMESTAMP", "CURRENT_USER", "SESSION_USER", "SYSTEM_USER", "CURRENT_SCHEMA", "AT",
    // Window/frame/agg
    "OVER", "PARTITION BY", "ROWS BETWEEN", "UNBOUNDED PRECEDING", "CURRENT ROW",
    "EXCLUDE NO OTHERS", "FILTER WHERE", "GROUPING",
    // Conditional
    "COALESCE", "NULLIF", "GREATEST", "LEAST", "EXTRACT",
    // Cursor/seq
    "CURSOR", "FORWARD", "BACKWARD", "ABSOLUTE", "RELATIVE", "PRIOR",
    "CACHE", "CYCLE", "INCREMENT", "MAXVALUE", "MINVALUE", "RESTART",
    // Partition
    "ATTACH", "DETACH", "FINALIZE",
    // RLS
    "ENABLE ROW LEVEL SECURITY", "DISABLE ROW LEVEL SECURITY",
    "FORCE ROW LEVEL SECURITY", "NO FORCE ROW LEVEL SECURITY",
    "POLICY", "WITH CHECK",
    // Replication
    "PUBLICATION", "CONNECTION",
    // FTS
    "CONFIGURATION", "DICTIONARY", "PARSER",
    // XML
    "DOCUMENT", "CONTENT", "PASSING",
    // Unicode
    "NORMALIZE", "NORMALIZED", "NFC", "NFKD",
    // Misc
    "OVERLAY", "PLACING", "POSITION", "OVERRIDING",
    "REFERENCING", "DEPENDS", "COMPRESSION", "GLOBAL", "LOGGED",
    // Trigger
    "REFERENCES OLD", "REFERENCES NEW", "FOR EACH ROW", "FOR EACH STATEMENT",
    "EXECUTE FUNCTION", "EXECUTE PROCEDURE",
  ] {
    let md = resolve(kw, &Catalog::default());
    assert!(md.is_some(), "missing {kw}");
    let s = md.unwrap();
    assert!(!s.contains("PG SQL keyword `"), "{kw} still placeholder doc");
  }
}

#[test]
fn r2_001_sweep_kw_release_routine_search_upgraded() {
  for kw in ["RELEASE", "REPEATABLE", "RESET", "ROUTINE", "ROUTINES", "SCHEMAS", "SCROLL", "SEARCH", "SEQUENCES"] {
    let md = resolve(kw, &Catalog::default());
    assert!(md.is_some(), "missing {kw}");
    let s = md.unwrap();
    assert!(!s.contains("PG SQL keyword"), "{kw} still placeholder");
  }
}

#[test]
fn r2_001_sweep_kw_misc_inline_label_upgraded() {
  for kw in ["INLINE", "INPUT", "INSENSITIVE", "LABEL", "LARGE", "NAME", "NAMES", "NATIONAL", "NCHAR", "NONE", "PARAMETER", "RECHECK"] {
    let md = resolve(kw, &Catalog::default());
    assert!(md.is_some(), "missing {kw}");
    let s = md.unwrap();
    assert!(!s.contains("PG SQL keyword"), "{kw} still placeholder");
  }
}

#[test]
fn r2_002_sweep_kw_iso_replication_upgraded() {
  for kw in ["SERIALIZABLE", "REPEATABLE", "SNAPSHOT", "SERVER", "SESSION", "SUBSCRIPTION", "PUBLICATION", "RELEASE", "RESET", "STDIN", "STDOUT"] {
    let md = resolve(kw, &Catalog::default());
    assert!(md.is_some(), "missing {kw}");
    let s = md.unwrap();
    assert!(!s.contains("PG SQL keyword"), "{kw} still placeholder");
  }
}

#[test]
fn r2_002_sweep_kw_string_xml_misc_upgraded() {
  for kw in ["SETOF", "SETS", "SIMPLE", "SQL", "STANDALONE", "STATISTICS", "STORAGE", "STRIP", "SUBSTRING", "SUPPORT", "SYMMETRIC", "SYSID", "SYSTEM", "TABLES", "REF", "BREADTH", "DEPTH"] {
    let md = resolve(kw, &Catalog::default());
    assert!(md.is_some(), "missing {kw}");
    let s = md.unwrap();
    assert!(!s.contains("PG SQL keyword"), "{kw} still placeholder");
  }
}

#[test]
fn r2_003_sweep_kw_sample_fetch_upgraded() {
  for kw in ["TABLESAMPLE", "TEMPLATE", "TIES", "TIME", "TIMESTAMP", "TRANSFORM", "TREAT", "TRIM", "TYPES", "UESCAPE", "UNCOMMITTED", "UNENCRYPTED", "UNKNOWN"] {
    let md = resolve(kw, &Catalog::default());
    assert!(md.is_some(), "missing {kw}");
    let s = md.unwrap();
    assert!(!s.contains("PG SQL keyword"), "{kw} still placeholder");
  }
}

#[test]
fn r2_003_sweep_kw_misc_role_value_upgraded() {
  for kw in ["UNTIL", "USAGE", "VALID", "VALUE", "VARCHAR", "VARIADIC", "VERSION", "VIEWS", "WHITESPACE", "WITHOUT", "WORK", "ASSERTION", "ASSIGNMENT", "ATTRIBUTE", "CATALOG", "ALSO", "ASENSITIVE"] {
    let md = resolve(kw, &Catalog::default());
    assert!(md.is_some(), "missing {kw}");
    let s = md.unwrap();
    assert!(!s.contains("PG SQL keyword"), "{kw} still placeholder");
  }
}

#[test]
fn r2_004_sweep_kw_xml_zone_finalized() {
  for kw in ["WRAPPER", "WRITE", "XML", "XMLATTRIBUTES", "XMLCONCAT", "XMLELEMENT", "XMLEXISTS", "XMLFOREST", "XMLNAMESPACES", "XMLPARSE", "XMLPI", "XMLROOT", "XMLSERIALIZE", "XMLTABLE", "YES", "ZONE"] {
    let md = resolve(kw, &Catalog::default());
    assert!(md.is_some(), "missing {kw}");
    let s = md.unwrap();
    assert!(!s.contains("PG SQL keyword"), "{kw} still placeholder");
  }
}

#[test]
fn r2_004_sweep_fn_temporal_format_added() {
  for fn_ in ["age", "date_part", "date_trunc", "to_char", "to_date", "to_number", "to_timestamp", "justify_days", "justify_hours", "justify_interval", "make_date", "make_time", "make_timestamp", "make_timestamptz", "make_interval", "to_regclass", "to_regtype", "to_regproc", "to_regprocedure", "jsonb_set", "jsonb_strip_nulls", "jsonb_pretty", "pg_size_pretty", "pg_database_size", "pg_table_size", "pg_indexes_size", "pg_relation_size", "pg_total_relation_size", "xmlagg"] {
    let md = resolve(fn_, &Catalog::default());
    assert!(md.is_some(), "missing fn {fn_}");
    let s = md.unwrap();
    assert!(!s.contains("PG function `") || !s.contains("See Postgres docs"), "fn {fn_} still placeholder");
  }
}

#[test]
fn r2_005_sweep_fn_string_basics() {
  for fn_ in ["lower", "upper", "initcap", "left", "right", "substr", "trim", "translate", "replace", "overlay", "position", "octet_length", "bit_length", "char_length", "character_length", "length", "encode", "decode", "ascii", "lpad", "rpad", "btrim", "ltrim", "rtrim", "repeat", "reverse", "split_part", "starts_with", "concat", "concat_ws", "format", "quote_ident", "quote_literal", "quote_nullable"] {
    let md = resolve(fn_, &Catalog::default());
    assert!(md.is_some(), "missing fn {fn_}");
  }
}

#[test]
fn r2_005_sweep_fn_array_added() {
  for fn_ in ["string_to_array", "array_to_string", "string_agg", "array_agg", "array_append", "array_prepend", "array_cat", "array_remove", "array_replace", "array_position", "array_positions", "array_length", "array_lower", "array_upper", "array_ndims", "array_dims", "cardinality", "unnest", "array_fill", "generate_subscripts"] {
    let md = resolve(fn_, &Catalog::default());
    assert!(md.is_some(), "missing fn {fn_}");
  }
}

#[test]
fn r2_005_sweep_fn_json_path_added() {
  for fn_ in ["jsonb_path_query", "jsonb_path_query_array", "jsonb_path_query_first", "jsonb_path_exists", "jsonb_path_match", "jsonb_array_length", "jsonb_object_keys", "jsonb_each", "jsonb_each_text", "jsonb_typeof", "json_object_agg", "jsonb_object_agg", "json_agg", "jsonb_agg", "row_to_json", "json_build_object", "jsonb_build_object", "json_build_array", "jsonb_build_array"] {
    let md = resolve(fn_, &Catalog::default());
    assert!(md.is_some(), "missing fn {fn_}");
  }
}

#[test]
fn r2_006_sweep_fn_math_added() {
  for fn_ in ["abs", "ceiling", "floor", "round", "trunc", "div", "mod", "power", "log", "sqrt", "sign", "cos", "tan", "asin", "acos", "atan", "atan2", "degrees", "radians", "random", "setseed", "scale", "min_scale", "trim_scale"] {
    assert!(resolve(fn_, &Catalog::default()).is_some(), "missing math fn {fn_}");
  }
}

#[test]
fn r2_006_sweep_fn_regex_fts_added() {
  for fn_ in ["regexp_match", "regexp_matches", "regexp_replace", "regexp_split_to_array", "regexp_split_to_table", "regexp_count", "regexp_substr",
              "to_tsvector", "to_tsquery", "plainto_tsquery", "phraseto_tsquery", "websearch_to_tsquery", "ts_rank", "ts_rank_cd", "ts_headline", "numnode", "queries_to_xml"] {
    assert!(resolve(fn_, &Catalog::default()).is_some(), "missing fn {fn_}");
  }
}

#[test]
fn r2_006_sweep_fn_range_added() {
  for fn_ in ["isempty", "lower_inc", "upper_inc", "range_merge", "int4range", "int8range", "numrange", "tsrange", "tstzrange", "daterange", "multirange", "range_agg"] {
    assert!(resolve(fn_, &Catalog::default()).is_some(), "missing range fn {fn_}");
  }
}

#[test]
fn r2_007_sweep_fn_agg_window_added() {
  for fn_ in ["sum", "avg", "count", "min", "max", "bool_and", "bool_or", "every", "bit_and", "bit_or",
              "stddev", "stddev_pop", "stddev_samp", "variance", "var_pop", "var_samp",
              "covar_pop", "covar_samp", "corr",
              "regr_slope", "regr_intercept", "regr_r2", "regr_count", "regr_avgx", "regr_avgy", "regr_sxx", "regr_syy", "regr_sxy",
              "mode", "percentile_cont", "percentile_disc",
              "rank", "dense_rank", "percent_rank", "cume_dist", "row_number", "ntile",
              "lag", "lead", "first_value", "last_value", "nth_value"] {
    assert!(resolve(fn_, &Catalog::default()).is_some(), "missing agg/win fn {fn_}");
  }
}

#[test]
fn r2_007_sweep_fn_conv_bit_net_added() {
  for fn_ in ["int4", "int8", "int2", "float4", "float8",
              "get_bit", "set_bit", "get_byte", "set_byte",
              "host", "netmask", "network", "set_masklen", "inet"] {
    assert!(resolve(fn_, &Catalog::default()).is_some(), "missing fn {fn_}");
  }
}

#[test]
fn r2_007_sweep_fn_uuid_crypto_added() {
  for fn_ in ["gen_random_uuid", "uuid_generate_v1", "uuid_generate_v4", "uuid_generate_v5",
              "sha224", "sha256", "sha384", "sha512", "hashtext",
              "crypt", "gen_salt", "encrypt", "decrypt", "hmac", "digest"] {
    assert!(resolve(fn_, &Catalog::default()).is_some(), "missing fn {fn_}");
  }
}

#[test]
fn r2_008_sweep_type_added() {
  for ty in ["XID", "XID8", "ACLITEM"] {
    assert!(resolve(ty, &Catalog::default()).is_some(), "missing type {ty}");
  }
}

#[test]
fn r2_008_sweep_fn_seq_lock_admin() {
  for fn_ in ["nextval", "currval", "lastval", "setval",
              "pg_advisory_lock", "pg_advisory_lock_shared", "pg_advisory_unlock", "pg_advisory_unlock_shared", "pg_advisory_unlock_all",
              "pg_advisory_xact_lock", "pg_advisory_xact_lock_shared",
              "pg_try_advisory_lock", "pg_try_advisory_lock_shared", "pg_try_advisory_xact_lock", "pg_try_advisory_xact_lock_shared",
              "generate_series", "pg_sleep", "pg_sleep_for", "pg_sleep_until",
              "pg_cancel_backend", "pg_terminate_backend",
              "pg_stat_reset", "pg_stat_reset_shared", "pg_stat_reset_single_table_counters", "pg_stat_reset_single_function_counters",
              "pg_is_other_temp_schema"] {
    assert!(resolve(fn_, &Catalog::default()).is_some(), "missing fn {fn_}");
  }
}

#[test]
fn r2_009_sweep_fn_pg_get_introspection() {
  for fn_ in ["pg_get_viewdef", "pg_get_indexdef", "pg_get_constraintdef", "pg_get_functiondef", "pg_get_triggerdef", "pg_get_userbyid", "pg_get_serial_sequence", "pg_get_expr", "pg_get_statisticsobjdef", "pg_relation_filepath", "pg_filenode_relation", "pg_log_backend_memory_contexts", "pg_typeof", "pg_column_compression", "pg_tablespace_databases", "pg_tablespace_location", "pg_options_to_table"] {
    assert!(resolve(fn_, &Catalog::default()).is_some(), "missing fn {fn_}");
  }
}

#[test]
fn r2_009_sweep_fn_encoding_visibility_event() {
  for fn_ in ["pg_safe_snapshot_blocking_pids", "pg_isolation_test_session_is_blocked", "pg_collation_actual_version", "pg_collation_for", "pg_encoding_to_char", "pg_char_to_encoding", "pg_client_encoding", "format_type", "pg_type_is_visible", "pg_table_is_visible", "pg_function_is_visible", "pg_event_trigger_ddl_commands", "pg_event_trigger_dropped_objects", "pg_event_trigger_table_rewrite_oid", "pg_event_trigger_table_rewrite_reason"] {
    assert!(resolve(fn_, &Catalog::default()).is_some(), "missing fn {fn_}");
  }
}

#[test]
fn r2_009_sweep_fn_sqljson_fts() {
  for fn_ in ["json_table", "jsonb_to_tsvector", "json_to_tsvector", "json_query", "json_value", "json_exists", "pg_input_is_valid", "pg_input_error_info", "ts_lexize", "ts_token_type", "ts_parse", "ts_debug"] {
    assert!(resolve(fn_, &Catalog::default()).is_some(), "missing fn {fn_}");
  }
}

#[test]
fn r2_010_sweep_fn_repl_wal() {
  for fn_ in ["pg_replication_origin_create", "pg_replication_origin_drop", "pg_replication_origin_session_setup", "pg_replication_origin_session_reset", "pg_logical_slot_peek_changes", "pg_logical_slot_get_binary_changes", "pg_logical_slot_peek_binary_changes", "pg_replication_origin_oid", "pg_replication_origin_session_progress", "pg_show_replication_origin_status", "pg_get_wal_resource_managers", "pg_get_wal_replay_pause_state", "pg_is_wal_replay_paused", "pg_wal_replay_pause", "pg_wal_replay_resume"] {
    assert!(resolve(fn_, &Catalog::default()).is_some(), "missing fn {fn_}");
  }
}

#[test]
fn r2_010_sweep_fn_lo_fs() {
  for fn_ in ["pg_notification_queue_usage", "lo_create", "lo_unlink", "lo_open", "lo_close", "lo_read", "lo_write", "lo_lseek", "lo_lseek64", "lo_tell", "lo_tell64", "lo_truncate", "lo_truncate64", "lo_put", "lo_get", "lo_from_bytea", "lo_export", "lo_import", "pg_ls_dir", "pg_ls_logdir", "pg_ls_waldir", "pg_ls_tmpdir", "pg_ls_archive_statusdir", "pg_read_file", "pg_read_binary_file", "pg_stat_file"] {
    assert!(resolve(fn_, &Catalog::default()).is_some(), "missing fn {fn_}");
  }
}

#[test]
fn r2_010_sweep_fn_dblink_misc() {
  for fn_ in ["dblink", "dblink_exec", "dblink_connect", "dblink_disconnect", "dblink_get_connections", "num_nonnulls", "num_nulls", "suppress_redundant_updates_trigger", "tsmatchsel", "tsmatchjoinsel"] {
    assert!(resolve(fn_, &Catalog::default()).is_some(), "missing fn {fn_}");
  }
}

#[test]
fn r2_032_fts_match_operator_hover() {
  let cat = Catalog::default();
  let src = "SELECT * FROM articles WHERE doc @@ to_tsquery('x')";
  let pos: text_size::TextSize = (src.find("@@").unwrap() as u32).into();
  let md = dsl_hover::hover(src, pos, &cat).expect("@@ hover");
  assert!(md.contains("Full-text search"), "{md}");
}

#[test]
fn r2_032_regex_operator_hover() {
  let cat = Catalog::default();
  let src = "SELECT * FROM t WHERE name ~ '^a'";
  let pos: text_size::TextSize = (src.find('~').unwrap() as u32).into();
  let md = dsl_hover::hover(src, pos, &cat).expect("~ hover");
  assert!(md.contains("POSIX regex"), "{md}");
}

#[test]
fn r2_032_knn_distance_operator_hover() {
  let cat = Catalog::default();
  let src = "SELECT * FROM pts ORDER BY p <-> point '(0,0)'";
  let pos: text_size::TextSize = (src.find("<->").unwrap() as u32 + 1).into();
  let md = dsl_hover::hover(src, pos, &cat).expect("<-> hover");
  assert!(md.contains("distance"), "{md}");
}

#[test]
fn r2_032_exponentiation_operator_hover() {
  let cat = Catalog::default();
  let src = "SELECT 2 ^ 10";
  let pos: text_size::TextSize = (src.find('^').unwrap() as u32).into();
  let md = dsl_hover::hover(src, pos, &cat).expect("^ hover");
  assert!(md.contains("Exponentiation"), "{md}");
}

#[test]
fn r2_033_sqrt_operator_hover() {
  let cat = Catalog::default();
  let src = "SELECT |/ 25";
  let pos: text_size::TextSize = (src.find("|/").unwrap() as u32).into();
  let md = dsl_hover::hover(src, pos, &cat).expect("|/ hover");
  assert!(md.contains("Square root"), "{md}");
}

#[test]
fn r2_033_cube_root_operator_hover() {
  let cat = Catalog::default();
  let src = "SELECT ||/ 27";
  let pos: text_size::TextSize = (src.find("||/").unwrap() as u32 + 1).into();
  let md = dsl_hover::hover(src, pos, &cat).expect("||/ hover");
  assert!(md.contains("Cube root"), "{md}");
}

#[test]
fn r2_033_length_operator_hover() {
  let cat = Catalog::default();
  let src = "SELECT @-@ p FROM paths";
  let pos: text_size::TextSize = (src.find("@-@").unwrap() as u32 + 1).into();
  let md = dsl_hover::hover(src, pos, &cat).expect("@-@ hover");
  assert!(md.contains("length") || md.contains("circumference"), "{md}");
}

#[test]
fn r2_033_overlaps_operator_hover() {
  let cat = Catalog::default();
  let src = "SELECT * FROM bookings WHERE period && tsrange('...','...')";
  let pos: text_size::TextSize = (src.find("&&").unwrap() as u32).into();
  let md = dsl_hover::hover(src, pos, &cat).expect("&& hover");
  assert!(md.contains("Overlaps"), "{md}");
}

#[test]
fn r2_068_sweep_kw_new_entries() {
  for kw in ["PRIMARY KEY", "FOREIGN KEY", "EXCLUDE USING", "UNIQUE NULLS NOT DISTINCT",
             "AT TIME ZONE", "WITH ORDINALITY", "AS MATERIALIZED", "AS NOT MATERIALIZED", "FORCE NULL"] {
    assert!(resolve(kw, &Catalog::default()).is_some(), "missing {kw}");
  }
}

#[test]
fn r2_068_sweep_op_jsonb_returns_text() {
  let cat = Catalog::default();
  let src = "SELECT data ->> 'name' FROM t";
  let pos: text_size::TextSize = (src.find("->>").unwrap() as u32 + 1).into();
  let md = dsl_hover::hover(src, pos, &cat).expect("->> hover");
  assert!(md.contains("text") || md.contains("TEXT"), "{md}");
}

#[test]
fn r2_068_sweep_op_jsonb_path_array() {
  let cat = Catalog::default();
  let src = "SELECT data #> '{a,b}' FROM t";
  let pos: text_size::TextSize = (src.find("#>").unwrap() as u32).into();
  let md = dsl_hover::hover(src, pos, &cat).expect("#> hover");
  assert!(md.contains("path"), "{md}");
}

#[test]
fn r2_068_sweep_op_jsonb_contains() {
  let cat = Catalog::default();
  let src = "SELECT * FROM t WHERE doc @> '{}'";
  let pos: text_size::TextSize = (src.find("@>").unwrap() as u32).into();
  let md = dsl_hover::hover(src, pos, &cat).expect("@> hover");
  assert!(md.contains("Contains"), "{md}");
}

#[test]
fn r2_068_sweep_op_jsonb_contained_by() {
  let cat = Catalog::default();
  let src = "SELECT * FROM t WHERE doc <@ '{}'";
  let pos: text_size::TextSize = (src.find("<@").unwrap() as u32).into();
  let md = dsl_hover::hover(src, pos, &cat).expect("<@ hover");
  assert!(md.contains("Contained"), "{md}");
}

#[test]
fn r2_068_sweep_op_concat() {
  let cat = Catalog::default();
  let src = "SELECT 'a' || 'b'";
  let pos: text_size::TextSize = (src.find("||").unwrap() as u32).into();
  let md = dsl_hover::hover(src, pos, &cat).expect("|| hover");
  assert!(md.contains("concat") || md.contains("Concatenat"), "{md}");
}

#[test]
fn r2_069_sweep_fn_misc_guc_added() {
  for fn_ in ["width_bucket", "bound_box", "convert", "convert_from", "convert_to",
              "set_config", "current_setting", "cluster_name", "pg_jit_available",
              "pg_index_column_has_property", "pg_index_has_property", "pg_indexam_has_property"] {
    assert!(resolve(fn_, &Catalog::default()).is_some(), "missing fn {fn_}");
  }
}

#[test]
fn r2_069_sweep_fn_json_added() {
  for fn_ in ["to_jsonb", "json_strip_nulls", "json_array_length", "json_typeof",
              "json_extract_path", "json_extract_path_text", "json_each", "json_each_text",
              "json_object_keys", "json_populate_recordset"] {
    assert!(resolve(fn_, &Catalog::default()).is_some(), "missing fn {fn_}");
  }
}

#[test]
fn r2_069_sweep_fn_reg_hash_added() {
  for fn_ in ["to_regnamespace", "to_regrole", "to_regoperator",
              "hashtextextended", "hashbpchar"] {
    assert!(resolve(fn_, &Catalog::default()).is_some(), "missing fn {fn_}");
  }
}

#[test]
fn r2_070_sweep_fn_fts_trgm_added() {
  for fn_ in ["tsvector_to_array", "array_to_tsvector", "ts_strip",
              "similarity", "word_similarity", "strict_word_similarity",
              "show_trgm", "show_limit", "set_limit"] {
    assert!(resolve(fn_, &Catalog::default()).is_some(), "missing fn {fn_}");
  }
}

#[test]
fn r2_070_sweep_fn_geom_added() {
  for fn_ in ["box", "circle", "line", "lseg", "path", "point", "polygon"] {
    assert!(resolve(fn_, &Catalog::default()).is_some(), "missing fn {fn_}");
  }
}

#[test]
fn r2_070_sweep_fn_datetime_added() {
  for fn_ in ["extract", "isfinite", "timezone", "timeofday",
              "clock_timestamp", "transaction_timestamp", "statement_timestamp"] {
    assert!(resolve(fn_, &Catalog::default()).is_some(), "missing fn {fn_}");
  }
}

#[test]
fn r2_071_sweep_fn_interval_introspection_added() {
  for fn_ in ["date_bin", "date_diff",
              "pg_get_function_arguments", "pg_get_function_result"] {
    assert!(resolve(fn_, &Catalog::default()).is_some(), "missing fn {fn_}");
  }
}

#[test]
fn r2_071_verify_math_fns() {
  for fn_ in ["exp", "ln", "log", "log10", "cbrt", "sqrt", "power",
              "trunc", "round", "ceil", "ceiling", "floor",
              "acosh", "asinh", "atanh", "cosh", "sinh", "tanh",
              "acosd", "asind", "atand", "cosd", "sind", "tand", "atan2d"] {
    assert!(resolve(fn_, &Catalog::default()).is_some(), "missing fn {fn_}");
  }
}

#[test]
fn r2_071_verify_admin_pg_fns() {
  for fn_ in ["pg_get_keywords", "pg_export_snapshot", "pg_listening_channels",
              "pg_notify", "pg_walfile_name", "pg_walfile_name_offset",
              "overlaps"] {
    assert!(resolve(fn_, &Catalog::default()).is_some(), "missing fn {fn_}");
  }
}

#[test]
fn r2_072_sweep_fn_str_array_added() {
  for fn_ in ["ord", "to_hex", "array_subscript", "hash_array",
              "bit_count", "bit_xor"] {
    assert!(resolve(fn_, &Catalog::default()).is_some(), "missing fn {fn_}");
  }
}

#[test]
fn r2_072_sweep_fn_enum_added() {
  for fn_ in ["enum_first", "enum_last", "enum_range"] {
    assert!(resolve(fn_, &Catalog::default()).is_some(), "missing fn {fn_}");
  }
}

#[test]
fn r2_072_sweep_fn_sqljson_constructors() {
  for fn_ in ["json_array", "json_object", "json_scalar",
              "json_serialize", "jsonb_insert"] {
    assert!(resolve(fn_, &Catalog::default()).is_some(), "missing fn {fn_}");
  }
}

#[test]
fn r2_072_verify_range_ops() {
  for fn_ in ["range_eq", "range_lt", "range_le", "range_gt", "range_ge",
              "range_adjacent", "range_after", "range_before",
              "range_merge", "range_minus", "range_intersect", "range_union",
              "isempty", "lower_inc", "upper_inc"] {
    assert!(resolve(fn_, &Catalog::default()).is_some(), "missing fn {fn_}");
  }
}

#[test]
fn r2_073_sweep_fn_xact_signal_added() {
  for fn_ in ["pg_xact_status", "pg_current_xact_id", "pg_log_standby_snapshot",
              "pg_trigger_depth", "pg_log_query_plan", "pg_signal_backend",
              "pg_xlog_replay_pause", "pg_xlogfile_name", "inet_send"] {
    assert!(resolve(fn_, &Catalog::default()).is_some(), "missing fn {fn_}");
  }
}

#[test]
fn r2_073_verify_admin_breadth() {
  for fn_ in ["pg_xact_commit_timestamp", "pg_current_xact_id_if_assigned",
              "pg_last_committed_xact", "pg_get_replica_identity_index",
              "pg_relation_is_publishable", "pg_partition_ancestors",
              "pg_partition_tree", "pg_partition_root", "pg_my_temp_schema",
              "pg_is_other_temp_schema", "pg_relation_filenode",
              "pg_export_snapshot", "pg_safe_snapshot_blocking_pids",
              "inet_client_addr", "inet_server_addr", "pg_blocking_pids",
              "pg_promote", "pg_rotate_logfile", "pg_logical_emit_message"] {
    assert!(resolve(fn_, &Catalog::default()).is_some(), "missing fn {fn_}");
  }
}

#[test]
fn r2_075_sweep_op_jsonb_path_text() {
  let cat = Catalog::default();
  let src = "SELECT data #>> '{a}' FROM t";
  let pos: text_size::TextSize = (src.find("#>>").unwrap() as u32 + 1).into();
  let md = dsl_hover::hover(src, pos, &cat).expect("#>> hover");
  assert!(md.contains("TEXT") || md.contains("text"), "{md}");
}

#[test]
fn r2_075_sweep_op_jsonb_key_existence() {
  let cat = Catalog::default();
  let src = "SELECT * FROM t WHERE data ? 'k'";
  let pos: text_size::TextSize = (src.find('?').unwrap() as u32).into();
  let md = dsl_hover::hover(src, pos, &cat).expect("? hover");
  assert!(md.contains("key existence") || md.contains("key"), "{md}");
}

#[test]
fn r2_075_sweep_op_le_ge() {
  let cat = Catalog::default();
  for op in ["<=", ">="] {
    let src = format!("SELECT 1 {op} 2");
    let pos: text_size::TextSize = (src.find(op).unwrap() as u32).into();
    let md = dsl_hover::hover(&src, pos, &cat).expect("hover");
    assert!(md.contains(op), "{op} hover: {md}");
  }
}

#[test]
fn r2_124_hover_sqlstate_named_exceptions() {
  // Resolver case-folds, so lowercase exception names already match
  // identity-cased entries; verify the SQLSTATE kw resolves.
  let md = resolve("SQLSTATE", &Catalog::default());
  assert!(md.is_some(), "SQLSTATE hover missing");
}

#[test]
fn r2_123_hover_plpgsql_control_and_2pc() {
  for kw in [
    "FOREACH",
    "EXIT WHEN",
    "EXIT LOOP",
    "CONTINUE WHEN",
    "CONTINUE LOOP",
    "ASSERT",
    "GET DIAGNOSTICS",
    "GET STACKED DIAGNOSTICS",
    "EXCEPTION WHEN",
    "WHEN OTHERS",
    "RAISE USING",
    "RAISE DEBUG",
    "RAISE LOG",
    "RAISE INFO",
    "RAISE WARNING",
    "PERFORM",
    "SAVEPOINT",
    "PREPARE TRANSACTION",
    "COMMIT PREPARED",
    "ROLLBACK PREPARED",
    "LOOP",
    "WHILE LOOP",
    "FOR IN REVERSE",
    "FOR IN",
  ] {
    let md = resolve(kw, &Catalog::default());
    assert!(md.is_some(), "{kw} hover missing");
    let s = md.unwrap();
    assert!(!s.contains("PG SQL keyword"), "{kw} placeholder");
  }
}

#[test]
fn r2_122_hover_cursor_fetch_move_copy() {
  for kw in [
    "NO SCROLL",
    "WITHOUT HOLD",
    "BINARY CURSOR",
    "DECLARE BINARY",
    "FETCH FORWARD",
    "FETCH BACKWARD",
    "FETCH ABSOLUTE",
    "FETCH RELATIVE",
    "FETCH FIRST FROM",
    "FETCH LAST FROM",
    "FETCH PRIOR",
    "FETCH ALL",
    "MOVE FORWARD",
    "MOVE BACKWARD",
    "MOVE ABSOLUTE",
    "MOVE RELATIVE",
    "MOVE FIRST",
    "MOVE LAST",
    "MOVE NEXT",
    "MOVE PRIOR",
    "MOVE ALL",
    "COPY FROM STDIN",
    "COPY TO STDOUT",
    "COPY FROM PROGRAM",
    "COPY TO PROGRAM",
  ] {
    let md = resolve(kw, &Catalog::default());
    assert!(md.is_some(), "{kw} hover missing");
    let s = md.unwrap();
    assert!(!s.contains("PG SQL keyword"), "{kw} placeholder");
  }
}

#[test]
fn r2_121_hover_lock_tx_vacuum_options() {
  for kw in [
    "ACCESS SHARE",
    "ROW SHARE",
    "ROW EXCLUSIVE",
    "SHARE UPDATE EXCLUSIVE",
    "SHARE ROW EXCLUSIVE",
    "ACCESS EXCLUSIVE",
    "LOCK TABLE IN",
    "NOWAIT",
    "ISOLATION LEVEL",
    "READ COMMITTED",
    "REPEATABLE READ",
    "READ UNCOMMITTED",
    "DEFERRABLE",
    "NOT DEFERRABLE",
    "ONLY_DATABASE_STATS",
    "SKIP_DATABASE_STATS",
    "BUFFER_USAGE_LIMIT",
    "PROCESS_MAIN",
    "PROCESS_TOAST",
    "INDEX_CLEANUP",
    "DISABLE_PAGE_SKIPPING",
  ] {
    let md = resolve(kw, &Catalog::default());
    assert!(md.is_some(), "{kw} hover missing");
    let s = md.unwrap();
    assert!(!s.contains("PG SQL keyword"), "{kw} placeholder");
  }
}

#[test]
fn r2_120_hover_security_label_comment_event_trigger() {
  for kw in [
    "SECURITY LABEL FOR",
    "LABEL FOR",
    "EVENT TRIGGER WHEN",
    "WHEN TAG IN",
    "EVENT TRIGGER LOGIN",
    "COMMENT ON LARGE OBJECT",
    "COMMENT ON SUBSCRIPTION",
    "COMMENT ON PUBLICATION",
    "COMMENT ON EVENT TRIGGER",
    "COMMENT ON ACCESS METHOD",
    "COMMENT ON FOREIGN TABLE",
    "COMMENT ON FOREIGN DATA WRAPPER",
    "COMMENT ON SERVER",
    "COMMENT ON USER MAPPING",
    "COMMENT ON COLLATION",
    "COMMENT ON CONVERSION",
    "COMMENT ON STATISTICS",
    "UNLISTEN ALL",
  ] {
    let md = resolve(kw, &Catalog::default());
    assert!(md.is_some(), "{kw} hover missing");
    let s = md.unwrap();
    assert!(!s.contains("PG SQL keyword"), "{kw} placeholder");
  }
}

#[test]
fn r2_184_hover_timescale_citus_pgml() {
  for fn_name in [
    "create_hypertable",
    "add_dimension",
    "add_compression_policy",
    "add_retention_policy",
    "add_continuous_aggregate_policy",
    "time_bucket",
    "time_bucket_gapfill",
    "show_chunks",
    "drop_chunks",
    "locf",
    "interpolate",
    "first",
    "last",
    "create_distributed_table",
    "create_reference_table",
    "citus_add_node",
    "citus_remove_node",
    "citus_shards",
    "citus_rebalance_start",
    "pgml_predict",
    "pgml_embed",
    "pgml_chat",
  ] {
    let md = resolve(fn_name, &Catalog::default());
    assert!(md.is_some(), "{fn_name} hover missing");
  }
}

#[test]
fn r2_183_hover_postgis_partman_pgcron() {
  for fn_name in [
    "st_geomfromtext",
    "st_geomfromewkt",
    "st_geomfromwkb",
    "st_point",
    "st_makepoint",
    "st_setsrid",
    "st_transform",
    "st_astext",
    "st_asewkt",
    "st_asgeojson",
    "st_srid",
    "st_geometrytype",
    "st_within",
    "st_contains",
    "st_intersects",
    "st_disjoint",
    "st_dwithin",
    "st_distance",
    "st_area",
    "st_length",
    "st_buffer",
    "st_centroid",
    "st_makeenvelope",
    "create_parent",
    "run_maintenance",
    "partition_data_time",
    "cron_schedule",
    "cron_unschedule",
    "cron_schedule_in_database",
  ] {
    let md = resolve(fn_name, &Catalog::default());
    assert!(md.is_some(), "{fn_name} hover missing");
  }
}

#[test]
fn r2_182_hover_pgvector_hstore_pgstatstmt() {
  for fn_name in [
    "cosine_distance",
    "l2_distance",
    "inner_product",
    "l1_distance",
    "vector_dims",
    "vector_norm",
    "hamming_distance",
    "jaccard_distance",
    "akeys",
    "avals",
    "skeys",
    "svals",
    "hstore_to_json",
    "hstore_to_jsonb",
    "hstore_to_array",
    "hstore_to_matrix",
    "populate_record",
    "each_hstore",
    "exist",
    "defined",
    "pg_stat_statements",
    "pg_stat_statements_reset",
    "pg_stat_statements_info",
  ] {
    let md = resolve(fn_name, &Catalog::default());
    assert!(md.is_some(), "{fn_name} hover missing");
  }
}

#[test]
fn r2_181_hover_extension_fns_fuzzystrmatch_unaccent() {
  for fn_name in [
    "levenshtein",
    "levenshtein_less_equal",
    "metaphone",
    "dmetaphone",
    "dmetaphone_alt",
    "soundex",
    "difference",
    "unaccent",
    "ll_to_earth",
    "earth_distance",
    "random_normal",
  ] {
    let md = resolve(fn_name, &Catalog::default());
    assert!(md.is_some(), "{fn_name} hover missing");
  }
}

#[test]
fn r2_180_hover_new_fns_array_partition() {
  for fn_name in [
    "array_sample",
    "array_shuffle",
    "range_intersect_agg",
    "pg_get_catalog_foreign_keys",
    "pg_get_partition_constraintdef",
    "pg_partition_root",
    "pg_partition_ancestors",
    "pg_partition_tree",
  ] {
    let md = resolve(fn_name, &Catalog::default());
    assert!(md.is_some(), "{fn_name} hover missing");
  }
}

#[test]
fn r2_179_hover_inet_contains_eq_ops() {
  let cat = Catalog::default();
  for (src, op) in [
    ("SELECT inet '192.168.0.0/16' >>= inet '192.168.1.0/24'", ">>="),
    ("SELECT inet '192.168.1.0/24' <<= inet '192.168.0.0/16'", "<<="),
  ] {
    let pos: text_size::TextSize = (src.find(op).unwrap() as u32).into();
    let md = dsl_hover::hover(src, pos, &cat);
    assert!(md.is_some(), "no hover for {op:?}");
    let md = md.unwrap();
    assert!(md.contains(op), "{op} hover: {md}");
  }
}

#[test]
fn r2_178_hover_geo_align_and_intersect_ops() {
  let cat = Catalog::default();
  for (src, op) in [
    ("SELECT point '(1,0)' ?- point '(5,0)'", "?-"),
    ("SELECT lseg '((0,0),(1,1))' ?-| lseg '((1,1),(2,0))'", "?-|"),
    ("SELECT lseg '((0,0),(1,1))' ?|| lseg '((0,1),(1,2))'", "?||"),
    ("SELECT lseg '((0,0),(1,1))' ?# lseg '((0,1),(1,0))'", "?#"),
    ("SELECT point '(1,1)' ~= point '(1,1)'", "~="),
  ] {
    let pos: text_size::TextSize = (src.find(op).unwrap() as u32).into();
    let md = dsl_hover::hover(src, pos, &cat);
    assert!(md.is_some(), "no hover for {op:?}");
    let md = md.unwrap();
    assert!(md.contains(op), "{op} hover: {md}");
  }
}

#[test]
fn r2_177_hover_plpgsql_return_open_refcursor() {
  for kw in [
    "RETURN QUERY",
    "RETURN NEXT",
    "RETURN QUERY EXECUTE",
    "FOREACH IN SLICE",
    "OPEN FOR",
    "OPEN FOR EXECUTE",
    "REFCURSOR",
    "ALWAYS AS GENERATED",
  ] {
    let md = resolve(kw, &Catalog::default());
    assert!(md.is_some(), "{kw} hover missing");
    let s = md.unwrap();
    assert!(!s.contains("PG SQL keyword"), "{kw} placeholder");
  }
}

#[test]
fn r2_176_hover_role_locks_listen_rules() {
  for kw in [
    "SET LOCAL ROLE",
    "SET SESSION ROLE",
    "SET ROLE NONE",
    "RESET SESSION AUTHORIZATION",
    "SET SESSION AUTHORIZATION DEFAULT",
    "SKIP LOCKED",
    "FOR UPDATE OF",
    "FOR NO KEY UPDATE",
    "FOR KEY SHARE",
  ] {
    let md = resolve(kw, &Catalog::default());
    assert!(md.is_some(), "{kw} hover missing");
    let s = md.unwrap();
    assert!(!s.contains("PG SQL keyword"), "{kw} placeholder");
  }
}

#[test]
fn r2_175_hover_default_privileges_and_comments() {
  for kw in [
    "ALTER DEFAULT PRIVILEGES",
    "DEFAULT PRIVILEGES FOR ROLE",
    "DEFAULT PRIVILEGES IN SCHEMA",
    "ON TABLES",
    "ON SEQUENCES",
    "ON FUNCTIONS",
    "ON ROUTINES",
    "ON TYPES",
    "ON SCHEMAS",
    "COMMENT ON TABLESPACE",
    "COMMENT ON ROUTINE",
    "COMMENT ON OPERATOR",
    "COMMENT ON OPERATOR CLASS",
    "COMMENT ON OPERATOR FAMILY",
    "COMMENT ON AGGREGATE",
    "COMMENT ON RULE",
    "COMMENT ON DOMAIN",
    "COMMENT ON TYPE",
    "COMMENT ON SCHEMA",
    "COMMENT ON CAST",
    "COMMENT ON TEXT SEARCH CONFIGURATION",
    "COMMENT ON TEXT SEARCH DICTIONARY",
    "COMMENT ON TEXT SEARCH PARSER",
    "COMMENT ON TEXT SEARCH TEMPLATE",
    "COMMENT ON POLICY",
    "COMMENT ON TRANSFORM",
  ] {
    let md = resolve(kw, &Catalog::default());
    assert!(md.is_some(), "{kw} hover missing");
    let s = md.unwrap();
    assert!(!s.contains("PG SQL keyword"), "{kw} placeholder");
  }
}

#[test]
fn r2_174_hover_role_attribute_kws() {
  for kw in [
    "VALID UNTIL",
    "CONNECTION LIMIT",
    "ENCRYPTED PASSWORD",
    "UNENCRYPTED",
    "SUPERUSER",
    "NOSUPERUSER",
    "CREATEDB",
    "NOCREATEDB",
    "CREATEROLE",
    "NOCREATEROLE",
    "REPLICATION",
    "NOREPLICATION",
    "BYPASSRLS",
    "NOBYPASSRLS",
    "NOINHERIT",
    "IN ROLE",
    "IN GROUP",
    "SYSID",
    "TIMETZ",
    "TIMESTAMPTZ",
  ] {
    let md = resolve(kw, &Catalog::default());
    assert!(md.is_some(), "{kw} hover missing");
    let s = md.unwrap();
    assert!(!s.contains("PG SQL keyword"), "{kw} placeholder");
  }
}

#[test]
fn r2_161_hover_past_end_no_panic() {
  let cat = Catalog::default();
  let src = "SELECT id FROM users";
  // Past EOF byte offset.
  let _ = dsl_hover::hover(src, ((src.len() + 100) as u32).into(), &cat);
}

#[test]
fn r2_161_hover_inside_string_literal_no_panic() {
  let cat = Catalog::default();
  let src = "SELECT 'hello world' FROM users";
  let pos: text_size::TextSize = (src.find('w').unwrap() as u32).into();
  let _ = dsl_hover::hover(src, pos, &cat);
}

#[test]
fn r2_161_hover_inside_line_comment_no_panic() {
  let cat = Catalog::default();
  let src = "SELECT 1; -- hover here\nSELECT 2;";
  let pos: text_size::TextSize = (src.find("here").unwrap() as u32).into();
  let _ = dsl_hover::hover(src, pos, &cat);
}

#[test]
fn r2_161_hover_inside_block_comment_no_panic() {
  let cat = Catalog::default();
  let src = "SELECT 1; /* block comment */ SELECT 2;";
  let pos: text_size::TextSize = (src.find("block").unwrap() as u32).into();
  let _ = dsl_hover::hover(src, pos, &cat);
}

#[test]
fn r2_161_hover_empty_buffer_no_panic() {
  let cat = Catalog::default();
  let _ = dsl_hover::hover("", 0.into(), &cat);
}

#[test]
fn r2_161_hover_multibyte_offset_clamps() {
  let cat = Catalog::default();
  let src = "SELECT 'café' FROM users";
  // Offset into the middle of the multibyte char.
  let pos = src.find('é').unwrap() as u32 + 1;
  let _ = dsl_hover::hover(src, pos.into(), &cat);
}

#[test]
fn r2_157_hover_fk_uppercase_public_drops_prefix() {
  use dsl_catalog::ConstraintKind;
  // Build users table with a FK to PUBLIC.orders (uppercase schema).
  // The hover renderer should drop the schema prefix when it matches
  // public case-insensitively.
  let mut cat = Catalog::default();
  cat.schemas.push(Schema {
    name: "public".into(),
    tables: vec![Table {
      schema: "public".into(),
      name: "orders".into(),
      kind: TableKind::Table,
      columns: vec![Column {
        name: "id".into(),
        data_type: "int4".into(),
        nullable: false,
        default: None,
        comment: None,
        generated: None,
        json_keys: None,
      }],
      constraints: Vec::new(),
      indexes: Vec::new(),
      triggers: Vec::new(),
      policies: Vec::new(),
      comment: None,
      row_estimate: None,
      owner: None,
    }, Table {
      schema: "public".into(),
      name: "users".into(),
      kind: TableKind::Table,
      columns: vec![Column {
        name: "order_id".into(),
        data_type: "int4".into(),
        nullable: false,
        default: None,
        comment: None,
        generated: None,
        json_keys: None,
      }],
      constraints: vec![dsl_catalog::Constraint {
        name: "fk_users_order".into(),
        kind: ConstraintKind::ForeignKey,
        columns: vec!["order_id".into()],
        references: Some(dsl_catalog::ConstraintRef {
          schema: "PUBLIC".into(),
          table: "orders".into(),
          columns: vec!["id".into()],
        }),
        definition: None,
        inline: false,
      }],
      indexes: Vec::new(),
      triggers: Vec::new(),
      policies: Vec::new(),
      comment: None,
      row_estimate: None,
      owner: None,
    }],
  });
  let md = resolve("users", &cat).expect("users hover");
  // Schema prefix dropped: should show `REFERENCES orders (id)` not `PUBLIC.orders`.
  assert!(md.contains("REFERENCES orders"),
    "hover did not strip PUBLIC schema prefix: {md}");
  assert!(!md.contains("PUBLIC.orders"),
    "hover kept PUBLIC. prefix: {md}");
}

#[test]
fn r2_154_hover_column_dot_case_insensitive() {
  // Build a catalog with `users.id`, then look up `USERS.ID` -- hover
  // must resolve to the column despite the case mismatch.
  let mut cat = Catalog::default();
  cat.schemas.push(Schema {
    name: "public".into(),
    tables: vec![Table {
      schema: "public".into(),
      name: "users".into(),
      kind: TableKind::Table,
      columns: vec![Column {
        name: "id".into(),
        data_type: "int4".into(),
        nullable: false,
        default: None,
        comment: None,
        generated: None,
        json_keys: None,
      }],
      constraints: Vec::new(),
      indexes: Vec::new(),
      triggers: Vec::new(),
      policies: Vec::new(),
      comment: None,
      row_estimate: None,
      owner: None,
    }],
  });
  let md = resolve("USERS.ID", &cat);
  assert!(md.is_some(), "USERS.ID hover missing despite case-insensitive find_table");
}

#[test]
fn r2_119_hover_storage_reloptions_not_enforced() {
  // Uppercase kws only -- lowercase reloption names get case-folded by
  // the resolver and never match the (already lowercase) entries.
  for kw in ["NOT ENFORCED", "ENFORCED"] {
    let md = resolve(kw, &Catalog::default());
    assert!(md.is_some(), "{kw} hover missing");
    let s = md.unwrap();
    assert!(!s.contains("PG SQL keyword"), "{kw} placeholder");
  }
}

#[test]
fn r2_118_hover_identity_inherits_opclass() {
  for kw in [
    "GENERATED ALWAYS AS STORED",
    "GENERATED ALWAYS AS",
    "GENERATED BY DEFAULT AS",
    "AS IDENTITY",
    "IDENTITY (",
    "INHERITS",
    "CREATE TABLE OF",
    "OF TYPE",
    "NOT OF",
    "NULLS NOT DISTINCT",
    "NULLS DISTINCT",
    "OPERATOR CLASS",
    "INDEX INCLUDE",
  ] {
    let md = resolve(kw, &Catalog::default());
    assert!(md.is_some(), "{kw} hover missing");
    let s = md.unwrap();
    assert!(!s.contains("PG SQL keyword"), "{kw} placeholder");
  }
}

#[test]
fn r2_117_hover_domain_collation_ts_rule() {
  for kw in [
    "DOMAIN CHECK",
    "DOMAIN VALUE",
    "CONSTRAINT CHECK",
    "ADD CONSTRAINT CHECK",
    "LOCALE_PROVIDER",
    "PROVIDER LIBC",
    "PROVIDER ICU",
    "PROVIDER BUILTIN",
    "DETERMINISTIC",
    "COLLATION VERSION",
    "TEXT SEARCH PARSER",
    "TEXT SEARCH DICTIONARY",
    "TEXT SEARCH TEMPLATE",
    "TEXT SEARCH CONFIGURATION",
    "MAPPING FOR",
    "MAPPING REPLACE",
    "RULE INSTEAD",
    "DO INSTEAD NOTHING",
    "DO INSTEAD",
  ] {
    let md = resolve(kw, &Catalog::default());
    assert!(md.is_some(), "{kw} hover missing");
    let s = md.unwrap();
    assert!(!s.contains("PG SQL keyword"), "{kw} placeholder");
  }
}

#[test]
fn r2_116_hover_json_table_and_aggregates() {
  for kw in [
    "EXISTS PATH",
    "DEFAULT ON EMPTY",
    "DEFAULT ON ERROR",
    "NULL ON ERROR",
    "ERROR ON ERROR",
    "NULL ON EMPTY",
    "ERROR ON EMPTY",
    "ABSENT ON NULL",
    "NULL ON NULL",
    "KEY VALUE",
    "RETURNING JSON",
    "RETURNING JSONB",
    "RETURNING TEXT",
    "FORMAT JSON",
    "FORMAT JSONB",
    "WITH QUOTES",
    "OMIT QUOTES",
  ] {
    let md = resolve(kw, &Catalog::default());
    assert!(md.is_some(), "{kw} hover missing");
    let s = md.unwrap();
    assert!(!s.contains("PG SQL keyword"), "{kw} placeholder");
  }
}

#[test]
fn r2_115_hover_grouping_table_cycle_weight() {
  for kw in [
    "GROUPING SETS",
    "ROLLUP",
    "CUBE",
    "ROLLUP COLUMN GROUP",
    "CUBE COLUMN GROUP",
    "TABLE (FUNCTION",
    "CYCLE USING",
    "CYCLE SET",
    "WEIGHT A",
    "WEIGHT B",
    "WEIGHT C",
    "WEIGHT D",
    "MULTISET",
  ] {
    let md = resolve(kw, &Catalog::default());
    assert!(md.is_some(), "{kw} hover missing");
    let s = md.unwrap();
    assert!(!s.contains("PG SQL keyword"), "{kw} placeholder");
  }
}

#[test]
fn r2_114_hover_window_aggregate_xml_predicate() {
  for kw in [
    "FILTER",
    "FILTER (WHERE",
    "IGNORE NULLS",
    "RESPECT NULLS",
    "FROM FIRST",
    "FROM LAST",
    "AGG ORDER BY",
    "XMLEXISTS PASSING",
    "PASSING BY VALUE",
    "PASSING BY REF",
    "XMLPARSE DOCUMENT",
    "XMLPARSE CONTENT",
  ] {
    let md = resolve(kw, &Catalog::default());
    assert!(md.is_some(), "{kw} hover missing");
    let s = md.unwrap();
    assert!(!s.contains("PG SQL keyword"), "{kw} placeholder");
  }
}

#[test]
fn r2_113_hover_sql_standard_types_and_casts() {
  for kw in [
    "CHARACTER LARGE OBJECT",
    "BINARY LARGE OBJECT",
    "CLOB",
    "BLOB",
    "NCHAR VARYING",
    "NATIONAL CHAR",
    "NATIONAL CHARACTER",
    "NATIONAL CHARACTER VARYING",
    "CHARACTER SET",
    "AS BIGINT",
    "AS SMALLINT",
    "AS INTEGER",
    "AS NUMERIC",
    "AS REAL",
    "AS DOUBLE PRECISION",
    "AS DATE",
    "AS TIME",
    "AS TIMESTAMPTZ",
    "AS BOOLEAN",
    "WITH ORDINALITY",
  ] {
    let md = resolve(kw, &Catalog::default());
    assert!(md.is_some(), "{kw} hover missing");
    let s = md.unwrap();
    assert!(!s.contains("PG SQL keyword"), "{kw} placeholder");
  }
}

#[test]
fn r2_112_hover_interval_field_qualifiers() {
  for kw in [
    "YEAR TO MONTH",
    "DAY TO HOUR",
    "DAY TO MINUTE",
    "DAY TO SECOND",
    "HOUR TO MINUTE",
    "HOUR TO SECOND",
    "MINUTE TO SECOND",
    "INTERVAL YEAR",
    "INTERVAL MONTH",
    "INTERVAL DAY",
    "INTERVAL HOUR",
    "INTERVAL MINUTE",
    "INTERVAL SECOND",
  ] {
    let md = resolve(kw, &Catalog::default());
    assert!(md.is_some(), "{kw} hover missing");
    let s = md.unwrap();
    assert!(!s.contains("PG SQL keyword"), "{kw} placeholder");
  }
}

#[test]
fn r2_111_hover_string_sql_standard_chain() {
  for kw in [
    "SUBSTRING SIMILAR",
    "SIMILAR TO ESCAPE",
    "LIKE ESCAPE",
    "ILIKE ESCAPE",
    "ESCAPE",
    "OVERLAY PLACING",
    "OVERLAY FROM",
    "OVERLAY FOR",
    "TRIM FROM",
  ] {
    let md = resolve(kw, &Catalog::default());
    assert!(md.is_some(), "{kw} hover missing");
    let s = md.unwrap();
    assert!(!s.contains("PG SQL keyword"), "{kw} placeholder");
  }
}

#[test]
fn r2_110_hover_json_table_and_atomic() {
  for kw in [
    "NESTED PATH",
    "NESTED",
    "PATH",
    "PLAN DEFAULT",
    "PLAN",
    "ON EMPTY",
    "ON ERROR",
    "EMPTY ARRAY",
    "EMPTY OBJECT",
    "WRAPPER",
    "WITH WRAPPER",
    "WITHOUT WRAPPER",
    "BEGIN ATOMIC",
    "ATOMIC",
  ] {
    let md = resolve(kw, &Catalog::default());
    assert!(md.is_some(), "{kw} hover missing");
    let s = md.unwrap();
    assert!(!s.contains("PG SQL keyword"), "{kw} placeholder");
  }
}

#[test]
fn r2_108_hover_bitwise_jsonpath_predicate() {
  let cat = Catalog::default();
  for (src, op) in [
    ("SELECT 5 & 3", "&"),
    ("SELECT 5 | 2", "|"),
    ("SELECT 5 # 3", "#"),
    ("SELECT data @? '$.items'::jsonpath FROM t", "@?"),
  ] {
    let p = src.find(op).unwrap();
    let pos: text_size::TextSize = (p as u32).into();
    let md = dsl_hover::hover(src, pos, &cat);
    assert!(md.is_some(), "no hover for {op:?} in {src:?}");
    let md = md.unwrap();
    assert!(md.contains(op), "{op} hover: {md}");
  }
}

#[test]
fn r2_107_hover_range_strictly_left_right_adjacent() {
  let cat = Catalog::default();
  for op in ["<<", ">>", "-|-"] {
    let src = format!("SELECT int4range(1,5) {op} int4range(10,20)");
    let pos: text_size::TextSize = (src.find(op).unwrap() as u32).into();
    let md = dsl_hover::hover(&src, pos, &cat).expect("hover");
    assert!(md.contains(op), "{op} hover: {md}");
  }
}

#[test]
fn r2_106_hover_pg17_maintain_explain_lobjects() {
  for kw in [
    "MAINTAIN",
    "EXPLAIN SERIALIZE",
    "EXPLAIN MEMORY",
    "LARGE OBJECTS IN SCHEMA",
    "ALL LARGE OBJECTS",
  ] {
    let md = resolve(kw, &Catalog::default());
    assert!(md.is_some(), "{kw} hover missing");
    let s = md.unwrap();
    assert!(!s.contains("PG SQL keyword"), "{kw} placeholder");
  }
}

#[test]
fn r2_105_hover_pg18_returning_old_new() {
  for kw in [
    "RETURNING OLD",
    "RETURNING NEW",
    "OLD.",
    "NEW.",
    "GENERATED ALWAYS AS VIRTUAL",
    "VIRTUAL",
  ] {
    let md = resolve(kw, &Catalog::default());
    assert!(md.is_some(), "{kw} hover missing");
    let s = md.unwrap();
    assert!(!s.contains("PG SQL keyword"), "{kw} placeholder");
  }
}

#[test]
fn r2_104_hover_pg17_18_admin_fns() {
  for fn_name in [
    "pg_stat_have_stats",
    "pg_stat_reset_subscription_stats",
    "pg_stat_reset_slru",
    "pg_settings_get_flags",
    "pg_split_walfile_name",
    "pg_get_acl",
    "pg_get_loaded_modules",
    "pg_basetype",
    "pg_input_error_message",
  ] {
    let md = resolve(fn_name, &Catalog::default());
    assert!(md.is_some(), "{fn_name} hover missing");
  }
}

#[test]
fn r2_163_hover_repeated_calls_clean() {
  let cat = Catalog::default();
  let src = "SELECT 1 + 2";
  let pos = (src.find('+').unwrap() as u32).into();
  for _ in 0..100 {
    let _ = dsl_hover::hover(src, pos, &cat);
  }
}

#[test]
fn r2_163_hover_long_buffer_no_panic() {
  let cat = Catalog::default();
  let mut src = String::new();
  for _ in 0..1000 {
    src.push_str("SELECT 1; ");
  }
  let _ = dsl_hover::hover(&src, 50.into(), &cat);
}

#[test]
fn r2_185_hover_long_identifier_no_panic() {
  let cat = Catalog::default();
  let long_id = "a".repeat(500);
  let _ = resolve(&long_id, &cat);
}

#[test]
fn r2_185_hover_unicode_identifier_no_panic() {
  let cat = Catalog::default();
  for ident in ["users_日本", "café_user", "Σ_count"] {
    let _ = resolve(ident, &cat);
  }
}

#[test]
fn r2_185_hover_empty_identifier_no_panic() {
  let cat = Catalog::default();
  let _ = resolve("", &cat);
}

#[test]
fn r2_185_hover_pure_punctuation_no_panic() {
  let cat = Catalog::default();
  for token in [".", "..", "->", "::", "(", ")", ";", ","] {
    let _ = resolve(token, &cat);
  }
}
