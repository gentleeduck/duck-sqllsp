//! Format stress + smoke tests against large/synthetic inputs.

use dsl_format::{CreateTableStyle, rewrite};

fn rw(src: &str) -> String {
  rewrite(src, &CreateTableStyle::default())
}

#[test]
fn stress_large_create_table_with_many_columns() {
  let mut src = String::from("CREATE TABLE big (\n");
  for i in 0..200 {
    src.push_str(&format!("    col_{i:03} INT NOT NULL DEFAULT 0,\n"));
  }
  src.push_str("    CONSTRAINT pk_big PRIMARY KEY (col_000)\n);\n");
  let out = rw(&src);
  let twice = rw(&out);
  assert_eq!(out, twice, "200-column table not idempotent");
}

#[test]
fn stress_large_index_run_collapses() {
  let mut src = String::new();
  for i in 0..50 {
    src.push_str(&format!("CREATE INDEX ix_{i} ON tbl (col_{i});\n"));
  }
  let out = rw(&src);
  assert_eq!(rw(&out), out, "50-index batch not idempotent");
}

#[test]
fn stress_many_triggers_idempotent() {
  let mut src = String::new();
  for i in 0..30 {
    src.push_str(&format!("CREATE TRIGGER trg_{i} BEFORE UPDATE ON tbl FOR EACH ROW EXECUTE FUNCTION fn();\n"));
  }
  let out = rw(&src);
  assert_eq!(rw(&out), out);
}

#[test]
fn stress_many_functions_idempotent() {
  let mut src = String::new();
  for i in 0..20 {
    src.push_str(&format!(
      "CREATE OR REPLACE FUNCTION fn_{i}() RETURNS INT LANGUAGE plpgsql AS $$ BEGIN RETURN {i}; END; $$;\n"
    ));
  }
  let out = rw(&src);
  assert_eq!(rw(&out), out);
}

#[test]
fn stress_mixed_ddl_handles_all_clause_breaks() {
  let src = "\
CREATE TABLE a (id INT PRIMARY KEY);
CREATE TABLE b (id INT, CONSTRAINT fk_b FOREIGN KEY (id) REFERENCES a (id) ON DELETE CASCADE ON UPDATE CASCADE);
CREATE INDEX ix_a ON a USING btree (id) INCLUDE (id);
CREATE OR REPLACE FUNCTION f() RETURNS TRIGGER LANGUAGE plpgsql AS $$ BEGIN RETURN NEW; END; $$;
CREATE OR REPLACE TRIGGER trg_a BEFORE UPDATE ON a FOR EACH ROW WHEN (NEW.id IS NOT NULL) EXECUTE FUNCTION f();
";
  let out = rw(src);
  assert_eq!(rw(&out), out);
  // Trigger clauses broken.
  assert!(out.contains("\n    BEFORE "));
  assert!(out.contains("\nEXECUTE FUNCTION "));
  // FK clauses stay inline (changed from breaking) per user feedback.
  assert!(!out.contains("\n        REFERENCES "), "FK REFERENCES must stay inline: {out}");
}

#[test]
fn stress_input_with_no_ddl_passes_through_idempotently() {
  let src = "SELECT 1; SELECT 2; SELECT 3;\n";
  let out = rw(src);
  assert_eq!(rw(&out), out);
}

#[test]
fn property_no_trailing_whitespace_on_any_line() {
  let inputs = [
    "CREATE TABLE t (id INT PRIMARY KEY, email VARCHAR(255) NOT NULL);",
    "CREATE OR REPLACE TRIGGER trg BEFORE UPDATE ON t FOR EACH ROW EXECUTE FUNCTION fn();",
    "CREATE INDEX ix ON t USING btree (email) WHERE email IS NOT NULL;",
    "CREATE OR REPLACE FUNCTION f() RETURNS INT LANGUAGE plpgsql AS $$ BEGIN RETURN 1; END; $$;",
    "SELECT 1; SELECT 2;",
  ];
  for src in &inputs {
    let out = rw(src);
    for (i, line) in out.lines().enumerate() {
      assert!(!line.ends_with(' ') && !line.ends_with('\t'), "trailing whitespace on line {i} of `{src}`: {line:?}");
    }
  }
}

#[test]
fn r2_171_format_empty_no_panic() {
  let style = dsl_format::FormatterStyle::default();
  let ct_style = dsl_format::CreateTableStyle::default();
  let _ = dsl_format::format("", &style, &ct_style);
}

#[test]
fn r2_171_format_whitespace_only_no_panic() {
  let style = dsl_format::FormatterStyle::default();
  let ct_style = dsl_format::CreateTableStyle::default();
  let _ = dsl_format::format("   \n\t\n", &style, &ct_style);
}

#[test]
fn r2_171_format_unbalanced_paren_no_panic() {
  let style = dsl_format::FormatterStyle::default();
  let ct_style = dsl_format::CreateTableStyle::default();
  let _ = dsl_format::format("SELECT (((((id FROM users", &style, &ct_style);
}

#[test]
fn r2_171_format_unterminated_string_no_panic() {
  let style = dsl_format::FormatterStyle::default();
  let ct_style = dsl_format::CreateTableStyle::default();
  let _ = dsl_format::format("SELECT 'unterm FROM users", &style, &ct_style);
}

#[test]
fn r2_171_format_dollar_quote_no_panic() {
  let style = dsl_format::FormatterStyle::default();
  let ct_style = dsl_format::CreateTableStyle::default();
  let _ = dsl_format::format("CREATE FUNCTION f() RETURNS int LANGUAGE plpgsql AS $$ BEGIN RETURN 1; END $$;", &style, &ct_style);
}

#[test]
fn r2_171_format_multibyte_chars_no_panic() {
  let style = dsl_format::FormatterStyle::default();
  let ct_style = dsl_format::CreateTableStyle::default();
  let _ = dsl_format::format("SELECT 'café', '日本語' FROM users;", &style, &ct_style);
}

#[test]
fn r2_171_format_long_input_no_panic() {
  let style = dsl_format::FormatterStyle::default();
  let ct_style = dsl_format::CreateTableStyle::default();
  let mut s = String::new();
  for i in 0..500 {
    s.push_str(&format!("SELECT id FROM users WHERE id = {i};\n"));
  }
  let _ = dsl_format::format(&s, &style, &ct_style);
}

#[test]
fn r2_171_align_rewrite_empty_no_panic() {
  let style = dsl_format::CreateTableStyle::default();
  let _ = dsl_format::align::rewrite("", &style);
}

#[test]
fn r2_171_align_rewrite_broken_create_no_panic() {
  let style = dsl_format::CreateTableStyle::default();
  let _ = dsl_format::align::rewrite("CREATE TABLE x (((((", &style);
}

#[test]
fn r3_066_format_adversarial_unclosed_string() {
  let style = dsl_format::FormatterStyle::default();
  let ct_style = dsl_format::CreateTableStyle::default();
  let _ = dsl_format::format("SELECT 'unclosed string FROM users;", &style, &ct_style);
}

#[test]
fn r3_067_format_adversarial_unclosed_dollar_quote() {
  let style = dsl_format::FormatterStyle::default();
  let ct_style = dsl_format::CreateTableStyle::default();
  let _ = dsl_format::format("CREATE FUNCTION f() RETURNS void AS $$ BEGIN", &style, &ct_style);
}

#[test]
fn r3_068_format_adversarial_only_punctuation() {
  let style = dsl_format::FormatterStyle::default();
  let ct_style = dsl_format::CreateTableStyle::default();
  let _ = dsl_format::format("(((,,,)))(;)((), :: => -> ->>", &style, &ct_style);
}

#[test]
fn r3_069_format_adversarial_repeated_keywords() {
  let style = dsl_format::FormatterStyle::default();
  let ct_style = dsl_format::CreateTableStyle::default();
  let s = "SELECT SELECT SELECT FROM FROM FROM WHERE WHERE WHERE";
  let _ = dsl_format::format(s, &style, &ct_style);
}

#[test]
fn r3_070_format_huge_select_list() {
  let style = dsl_format::FormatterStyle::default();
  let ct_style = dsl_format::CreateTableStyle::default();
  let mut s = String::from("SELECT ");
  for i in 0..200 {
    if i > 0 { s.push_str(", "); }
    s.push_str(&format!("col{i}"));
  }
  s.push_str(" FROM t;");
  let _ = dsl_format::format(&s, &style, &ct_style);
}

#[test]
fn r3_071_format_deeply_nested_cte() {
  let style = dsl_format::FormatterStyle::default();
  let ct_style = dsl_format::CreateTableStyle::default();
  let mut s = String::from("WITH ");
  for i in 0..30 {
    if i > 0 { s.push_str(", "); }
    s.push_str(&format!("c{i} AS (SELECT 1)"));
  }
  s.push_str(" SELECT 1;");
  let _ = dsl_format::format(&s, &style, &ct_style);
}

#[test]
fn r3_072_format_only_comments() {
  let style = dsl_format::FormatterStyle::default();
  let ct_style = dsl_format::CreateTableStyle::default();
  let _ = dsl_format::format("-- only comments\n/* block */\n-- more\n", &style, &ct_style);
}

#[test]
fn r3_073_format_alternating_case_keywords() {
  let style = dsl_format::FormatterStyle::default();
  let ct_style = dsl_format::CreateTableStyle::default();
  let _ = dsl_format::format("SeLeCt * fRoM uSeRs WhErE iD = 1;", &style, &ct_style);
}

#[test]
fn r3_074_format_tabs_and_crlf() {
  let style = dsl_format::FormatterStyle::default();
  let ct_style = dsl_format::CreateTableStyle::default();
  let _ = dsl_format::format("SELECT\t*\r\nFROM\tusers\r\nWHERE\tid\t=\t1;", &style, &ct_style);
}

#[test]
fn r3_075_format_mixed_dml_ddl() {
  let style = dsl_format::FormatterStyle::default();
  let ct_style = dsl_format::CreateTableStyle::default();
  let s = "BEGIN; CREATE TABLE t (a int); INSERT INTO t VALUES (1); SELECT * FROM t; DROP TABLE t; COMMIT;";
  let _ = dsl_format::format(s, &style, &ct_style);
}

#[test]
fn r3_376_format_xmltable_no_panic() {
  let style = dsl_format::FormatterStyle::default();
  let ct_style = dsl_format::CreateTableStyle::default();
  let _ = dsl_format::format(
    "SELECT * FROM xmltable('/r' PASSING d COLUMNS a int PATH 'a', b text);",
    &style, &ct_style);
}

#[test]
fn r3_377_format_json_table_no_panic() {
  let style = dsl_format::FormatterStyle::default();
  let ct_style = dsl_format::CreateTableStyle::default();
  let _ = dsl_format::format(
    "SELECT * FROM json_table(data, '$' COLUMNS (id int PATH '$.id', name text PATH '$.name')) t;",
    &style, &ct_style);
}

#[test]
fn r3_378_format_extreme_long_identifier_no_panic() {
  let style = dsl_format::FormatterStyle::default();
  let ct_style = dsl_format::CreateTableStyle::default();
  let long = "x".repeat(500);
  let _ = dsl_format::format(&format!("SELECT {long} FROM t;"), &style, &ct_style);
}

#[test]
fn r3_379_format_deeply_nested_jsonb_path_no_panic() {
  let style = dsl_format::FormatterStyle::default();
  let ct_style = dsl_format::CreateTableStyle::default();
  let mut s = String::from("SELECT data");
  for k in 0..30 {
    s.push_str(&format!("->'k{k}'"));
  }
  s.push_str(" FROM t;");
  let _ = dsl_format::format(&s, &style, &ct_style);
}

#[test]
fn r3_380_format_repeated_caps_no_panic() {
  let style = dsl_format::FormatterStyle::default();
  let ct_style = dsl_format::CreateTableStyle::default();
  let _ = dsl_format::format(
    "SELECT SELECT SELECT FROM FROM FROM WHERE WHERE WHERE GROUP GROUP HAVING HAVING ORDER ORDER LIMIT LIMIT;",
    &style, &ct_style);
}

#[test]
fn r4_041_format_dollar_quote_with_body_no_panic() {
  let style = dsl_format::FormatterStyle::default();
  let ct_style = dsl_format::CreateTableStyle::default();
  let _ = dsl_format::format(
    "CREATE FUNCTION f() RETURNS int AS $body$ BEGIN RETURN 1; END $body$ LANGUAGE plpgsql;",
    &style, &ct_style);
}

#[test]
fn r4_042_format_mixed_case_keywords() {
  let style = dsl_format::FormatterStyle::default();
  let ct_style = dsl_format::CreateTableStyle::default();
  let _ = dsl_format::format(
    "create table T (Id INT, Name TEXT); INSERT INTO T VALUES (1, 'a');",
    &style, &ct_style);
}

#[test]
fn r4_043_format_window_with_frame() {
  let style = dsl_format::FormatterStyle::default();
  let ct_style = dsl_format::CreateTableStyle::default();
  let _ = dsl_format::format(
    "SELECT row_number() OVER (PARTITION BY a ORDER BY b ROWS BETWEEN UNBOUNDED PRECEDING AND CURRENT ROW) FROM t;",
    &style, &ct_style);
}

#[test]
fn r4_044_format_returning_with_aliases() {
  let style = dsl_format::FormatterStyle::default();
  let ct_style = dsl_format::CreateTableStyle::default();
  let _ = dsl_format::format(
    "INSERT INTO users (email) VALUES ('a') RETURNING id AS new_id, email AS new_email;",
    &style, &ct_style);
}

#[test]
fn r4_045_format_create_table_multiple_constraints() {
  let style = dsl_format::FormatterStyle::default();
  let ct_style = dsl_format::CreateTableStyle::default();
  let _ = dsl_format::format(
    "CREATE TABLE t (id int PRIMARY KEY, parent_id int REFERENCES t(id) ON DELETE CASCADE, CONSTRAINT uniq_parent UNIQUE (parent_id), CHECK (id > 0));",
    &style, &ct_style);
}

#[test]
fn r4_format_function_with_no_args_no_panic() {
  let style = dsl_format::FormatterStyle::default();
  let ct_style = dsl_format::CreateTableStyle::default();
  let _ = dsl_format::format(
    "CREATE FUNCTION f() RETURNS int LANGUAGE sql AS $$ SELECT 1 $$;",
    &style, &ct_style);
}

#[test]
fn r4_format_function_returning_table() {
  let style = dsl_format::FormatterStyle::default();
  let ct_style = dsl_format::CreateTableStyle::default();
  let _ = dsl_format::format(
    "CREATE FUNCTION f() RETURNS TABLE (a int, b text) LANGUAGE sql AS $$ SELECT 1, 'x' $$;",
    &style, &ct_style);
}

#[test]
fn r4_format_function_with_inout_args() {
  let style = dsl_format::FormatterStyle::default();
  let ct_style = dsl_format::CreateTableStyle::default();
  let _ = dsl_format::format(
    "CREATE PROCEDURE p(IN x int, OUT y int, INOUT z text) LANGUAGE plpgsql AS $$ BEGIN y := x; z := 'q'; END $$;",
    &style, &ct_style);
}

#[test]
fn r4_format_function_chained_sets_no_panic() {
  let style = dsl_format::FormatterStyle::default();
  let ct_style = dsl_format::CreateTableStyle::default();
  // Multiple SET clauses in function attributes.
  let _ = dsl_format::format(
    "CREATE FUNCTION f() RETURNS int LANGUAGE sql SET search_path = pg_catalog SET work_mem = '64MB' AS $$ SELECT 1 $$;",
    &style, &ct_style);
}

#[test]
fn r4_format_function_with_comment_in_body_no_panic() {
  let style = dsl_format::FormatterStyle::default();
  let ct_style = dsl_format::CreateTableStyle::default();
  let _ = dsl_format::format(
    "CREATE FUNCTION f() RETURNS int LANGUAGE sql AS $$ -- comment\n SELECT 1 $$;",
    &style, &ct_style);
}

#[test]
fn r5_286_format_4kb_long_select_no_panic() {
  let style = dsl_format::FormatterStyle::default();
  let ct_style = dsl_format::CreateTableStyle::default();
  let mut s = String::from("SELECT ");
  for i in 0..400 {
    if i > 0 { s.push_str(", "); }
    s.push_str(&format!("'col_{i}_value'"));
  }
  s.push_str(" FROM t;");
  let _ = dsl_format::format(&s, &style, &ct_style);
}

#[test]
fn r5_287_format_8kb_create_table_no_panic() {
  let style = dsl_format::FormatterStyle::default();
  let ct_style = dsl_format::CreateTableStyle::default();
  let mut s = String::from("CREATE TABLE t (");
  for i in 0..200 {
    if i > 0 { s.push_str(", "); }
    s.push_str(&format!("col_{i} text"));
  }
  s.push_str(");");
  let _ = dsl_format::format(&s, &style, &ct_style);
}

#[test]
fn r5_288_format_deeply_nested_with_clauses() {
  let style = dsl_format::FormatterStyle::default();
  let ct_style = dsl_format::CreateTableStyle::default();
  let mut s = String::from("WITH ");
  for i in 0..50 {
    if i > 0 { s.push_str(", "); }
    s.push_str(&format!("c{i} AS (SELECT 1)"));
  }
  s.push_str(" SELECT 1;");
  let _ = dsl_format::format(&s, &style, &ct_style);
}

#[test]
fn r5_289_format_lots_of_inserts_no_panic() {
  let style = dsl_format::FormatterStyle::default();
  let ct_style = dsl_format::CreateTableStyle::default();
  let mut s = String::new();
  for i in 0..50 {
    s.push_str(&format!("INSERT INTO t (id, name) VALUES ({i}, 'name_{i}');\n"));
  }
  let _ = dsl_format::format(&s, &style, &ct_style);
}

#[test]
fn r5_290_format_mixed_dml_ddl_alternating() {
  let style = dsl_format::FormatterStyle::default();
  let ct_style = dsl_format::CreateTableStyle::default();
  let mut s = String::new();
  for i in 0..30 {
    s.push_str(&format!("CREATE TABLE t_{i} (id int); INSERT INTO t_{i} VALUES (1); DROP TABLE t_{i};\n"));
  }
  let _ = dsl_format::format(&s, &style, &ct_style);
}

#[test]
fn r5_496_format_deeply_nested_case_expr() {
  let style = dsl_format::FormatterStyle::default();
  let ct_style = dsl_format::CreateTableStyle::default();
  let mut s = String::from("SELECT ");
  for i in 0..20 {
    s.push_str(&format!("CASE WHEN x{i} THEN {i} ELSE "));
  }
  s.push('0');
  for _ in 0..20 { s.push_str(" END"); }
  s.push_str(" FROM t;");
  let _ = dsl_format::format(&s, &style, &ct_style);
}

#[test]
fn r5_497_format_long_set_op_chain() {
  let style = dsl_format::FormatterStyle::default();
  let ct_style = dsl_format::CreateTableStyle::default();
  let mut s = String::from("SELECT 1");
  for i in 2..50 {
    s.push_str(&format!(" UNION ALL SELECT {i}"));
  }
  s.push(';');
  let _ = dsl_format::format(&s, &style, &ct_style);
}

#[test]
fn r5_498_format_quoted_identifiers_chain() {
  let style = dsl_format::FormatterStyle::default();
  let ct_style = dsl_format::CreateTableStyle::default();
  let src = r#"SELECT "Some Column", "Another Col"."Sub" FROM "Quoted Table" "ALIAS";"#;
  let _ = dsl_format::format(src, &style, &ct_style);
}

#[test]
fn r5_499_format_e_string_escape_chain() {
  let style = dsl_format::FormatterStyle::default();
  let ct_style = dsl_format::CreateTableStyle::default();
  let src = r"SELECT E'\\n\\t', E'\xFFA' FROM t;";
  let _ = dsl_format::format(src, &style, &ct_style);
}

#[test]
fn r5_500_format_u_amp_string_chain() {
  let style = dsl_format::FormatterStyle::default();
  let ct_style = dsl_format::CreateTableStyle::default();
  let src = r"SELECT U&'data \+000041\+000042' UESCAPE '\' FROM t;";
  let _ = dsl_format::format(src, &style, &ct_style);
}
