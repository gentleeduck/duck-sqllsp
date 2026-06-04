//! ct_align idempotency + clause-break tests.
//!
//! Property: running the formatter twice on the same input must yield
//! the same output. If a pass injects line breaks, a second pass
//! shouldn't stack more breaks on top of the already-broken text.

use dsl_format::{CreateTableStyle, rewrite};

fn rw(src: &str) -> String {
  rewrite(src, &CreateTableStyle::default())
}

#[test]
fn idempotent_create_table() {
  let src = "\
CREATE TABLE IF NOT EXISTS users (
    id UUID PRIMARY KEY,
    email VARCHAR(255) NOT NULL,
    CONSTRAINT uq_email UNIQUE (email)
);
";
  let once = rw(src);
  let twice = rw(&once);
  assert_eq!(once, twice, "ct_align not idempotent for CREATE TABLE");
}

#[test]
fn idempotent_create_trigger() {
  let src = "CREATE OR REPLACE TRIGGER trg_x BEFORE UPDATE ON users FOR EACH ROW EXECUTE FUNCTION fn();";
  let once = rw(src);
  let twice = rw(&once);
  assert_eq!(once, twice);
}

#[test]
fn idempotent_create_function() {
  let src = "CREATE OR REPLACE FUNCTION foo() RETURNS INT LANGUAGE plpgsql AS $$ BEGIN RETURN 1; END; $$;";
  let once = rw(src);
  let twice = rw(&once);
  assert_eq!(once, twice);
}

#[test]
fn idempotent_create_index() {
  let src = "CREATE INDEX ix ON users USING btree (id) INCLUDE (email) WHERE id IS NOT NULL;";
  let once = rw(src);
  let twice = rw(&once);
  assert_eq!(once, twice);
}

#[test]
fn idempotent_foreign_key_clauses() {
  let src = "\
CREATE TABLE t (
    id UUID,
    CONSTRAINT fk_x FOREIGN KEY (id) REFERENCES other (id) ON DELETE CASCADE ON UPDATE CASCADE
);
";
  let once = rw(src);
  let twice = rw(&once);
  assert_eq!(once, twice);
}

#[test]
fn trigger_clauses_break_onto_indented_lines() {
  let src = "CREATE OR REPLACE TRIGGER trg_x BEFORE UPDATE ON users FOR EACH ROW EXECUTE FUNCTION fn();";
  let out = rw(src);
  // Each header keyword should sit on its own line, indented except
  // EXECUTE (which lands at column 0).
  assert!(out.contains("\n    BEFORE "), "BEFORE not broken: {out}");
  assert!(out.contains("\n    ON "), "ON not broken: {out}");
  assert!(out.contains("\n    FOR EACH ROW"), "FOR EACH ROW not broken: {out}");
  assert!(out.contains("\nEXECUTE FUNCTION "), "EXECUTE FUNCTION not broken: {out}");
}

#[test]
fn function_clauses_break_onto_indented_lines() {
  let src = "CREATE OR REPLACE FUNCTION foo() RETURNS TRIGGER STABLE LANGUAGE plpgsql AS $$ BEGIN END; $$;";
  let out = rw(src);
  assert!(out.contains("\n    RETURNS "), "RETURNS not broken: {out}");
  assert!(out.contains("\n    LANGUAGE "), "LANGUAGE not broken: {out}");
  assert!(out.contains("\nAS "), "AS not broken: {out}");
}

#[test]
fn no_break_inside_unrelated_contexts() {
  // `ON` is a keyword inside many contexts; the formatter should only
  // break it inside CREATE TRIGGER / CREATE INDEX / CREATE TABLE FK.
  let src = "SELECT 1 FROM users u JOIN orders o ON u.id = o.user_id;";
  let out = rw(src);
  assert!(!out.contains("\n    ON "), "spurious break in SELECT: {out}");
}

// FK / CHECK / REFERENCES clause breaking was disabled per user feedback:
// the dangling closing-paren on its own line looked wrong. Constraints
// now stay on a single line. Idempotence still required.
#[test]
fn fk_clauses_stay_inline() {
  let src = "CREATE TABLE t (id UUID, CONSTRAINT fk_x FOREIGN KEY (id) REFERENCES other (id) ON DELETE CASCADE);";
  let out = rw(src);
  assert!(out.contains("REFERENCES other(id) ON DELETE CASCADE") || out.contains("REFERENCES other (id) ON DELETE CASCADE"), "FK clauses must stay on one line; got: {out}");
}

#[test]
fn fk_on_update_and_on_delete_stay_inline() {
  let src = "CREATE TABLE t (id UUID, CONSTRAINT fk_x FOREIGN KEY (id) REFERENCES other (id) ON UPDATE CASCADE ON DELETE SET NULL);";
  let out = rw(src);
  assert!(!out.contains("\n        ON UPDATE "), "ON UPDATE should stay inline: {out}");
}

#[test]
fn fk_with_match_full_stays_inline() {
  let src = "CREATE TABLE t (id UUID, CONSTRAINT fk_x FOREIGN KEY (id) REFERENCES other (id) MATCH FULL ON DELETE CASCADE);";
  let out = rw(src);
  assert!(!out.contains("\n        MATCH FULL"), "MATCH FULL should stay inline: {out}");
}

#[test]
fn fk_deferrable_stays_inline() {
  let src = "CREATE TABLE t (id UUID, CONSTRAINT fk_x FOREIGN KEY (id) REFERENCES other (id) DEFERRABLE INITIALLY DEFERRED);";
  let out = rw(src);
  assert!(!out.contains("\n        DEFERRABLE"), "DEFERRABLE should stay inline: {out}");
}

// ===== round-trip / token preservation tests ==============================

fn extract_tokens(s: &str) -> Vec<String> {
  let mut out = Vec::new();
  let bytes = s.as_bytes();
  let n = bytes.len();
  let mut i = 0;
  while i < n {
    let c = bytes[i];
    if c.is_ascii_alphanumeric() || c == b'_' {
      let start = i;
      while i < n && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_') {
        i += 1;
      }
      out.push(s[start..i].to_ascii_uppercase());
    } else {
      i += 1;
    }
  }
  out
}

#[test]
fn round_trip_preserves_all_identifiers_create_table() {
  let src = "\
CREATE TABLE users (
    id UUID PRIMARY KEY,
    email VARCHAR(255) NOT NULL,
    created_at TIMESTAMPTZ DEFAULT now(),
    CONSTRAINT uq_email UNIQUE (email)
);
";
  let before = extract_tokens(src);
  let after = extract_tokens(&rw(src));
  assert_eq!(before, after, "tokens diverged after format");
}

#[test]
fn round_trip_preserves_all_identifiers_trigger() {
  let src = "CREATE OR REPLACE TRIGGER trg_x BEFORE UPDATE ON users FOR EACH ROW EXECUTE FUNCTION set_updated_at();";
  let before = extract_tokens(src);
  let after = extract_tokens(&rw(src));
  assert_eq!(before, after, "tokens diverged after format");
}

#[test]
fn round_trip_preserves_all_identifiers_function() {
  let src =
    "CREATE OR REPLACE FUNCTION foo(x INT) RETURNS INT STABLE LANGUAGE plpgsql AS $$ BEGIN RETURN x + 1; END; $$;";
  let before = extract_tokens(src);
  let after = extract_tokens(&rw(src));
  assert_eq!(before, after);
}

#[test]
fn round_trip_preserves_all_identifiers_index() {
  let src = "CREATE INDEX ix_users ON users USING btree (email) INCLUDE (id) WHERE deleted_at IS NULL;";
  let before = extract_tokens(src);
  let after = extract_tokens(&rw(src));
  assert_eq!(before, after);
}

#[test]
fn empty_input_is_idempotent() {
  assert_eq!(rw(""), rw(&rw("")));
}

#[test]
fn whitespace_only_input_is_idempotent() {
  assert_eq!(rw("   \n  \n"), rw(&rw("   \n  \n")));
}

#[test]
fn r2_172_long_select_list_idempotent() {
  let style = dsl_format::FormatterStyle::default();
  let ct_style = dsl_format::CreateTableStyle::default();
  let mut src = "SELECT ".to_string();
  for i in 0..200 {
    if i > 0 { src.push_str(", "); }
    src.push_str(&format!("col{i}"));
  }
  src.push_str(" FROM users;");
  let once = dsl_format::format(&src, &style, &ct_style);
  let twice = dsl_format::format(&once, &style, &ct_style);
  assert_eq!(once, twice, "long select list not idempotent");
}

#[test]
fn r2_172_deeply_nested_subquery_idempotent() {
  let style = dsl_format::FormatterStyle::default();
  let ct_style = dsl_format::CreateTableStyle::default();
  let src = "SELECT * FROM (SELECT * FROM (SELECT * FROM (SELECT * FROM users) a) b) c WHERE c.id = 1;";
  let once = dsl_format::format(src, &style, &ct_style);
  let twice = dsl_format::format(&once, &style, &ct_style);
  assert_eq!(once, twice, "nested subquery not idempotent");
}

#[test]
fn r2_172_mixed_case_idempotent() {
  let style = dsl_format::FormatterStyle::default();
  let ct_style = dsl_format::CreateTableStyle::default();
  let src = "select ID, Email FROM Users where ID = 1;";
  let once = dsl_format::format(src, &style, &ct_style);
  let twice = dsl_format::format(&once, &style, &ct_style);
  assert_eq!(once, twice, "mixed case not idempotent");
}

#[test]
fn r2_172_cte_chain_idempotent() {
  let style = dsl_format::FormatterStyle::default();
  let ct_style = dsl_format::CreateTableStyle::default();
  let src = "WITH a AS (SELECT 1), b AS (SELECT * FROM a), c AS (SELECT * FROM b) SELECT * FROM c;";
  let once = dsl_format::format(src, &style, &ct_style);
  let twice = dsl_format::format(&once, &style, &ct_style);
  assert_eq!(once, twice, "CTE chain not idempotent");
}

#[test]
fn r2_172_union_chain_idempotent() {
  let style = dsl_format::FormatterStyle::default();
  let ct_style = dsl_format::CreateTableStyle::default();
  let src = "SELECT id FROM users UNION ALL SELECT id FROM orders UNION ALL SELECT id FROM events;";
  let once = dsl_format::format(src, &style, &ct_style);
  let twice = dsl_format::format(&once, &style, &ct_style);
  assert_eq!(once, twice, "union chain not idempotent");
}

#[test]
fn r2_172_create_table_idempotent() {
  let style = dsl_format::FormatterStyle::default();
  let ct_style = dsl_format::CreateTableStyle::default();
  let src = "CREATE TABLE users (id INT PRIMARY KEY, name TEXT NOT NULL, created_at TIMESTAMPTZ DEFAULT now());";
  let once = dsl_format::format(src, &style, &ct_style);
  let twice = dsl_format::format(&once, &style, &ct_style);
  assert_eq!(once, twice, "CREATE TABLE not idempotent");
}

#[test]
fn r2_172_multi_stmt_idempotent() {
  let style = dsl_format::FormatterStyle::default();
  let ct_style = dsl_format::CreateTableStyle::default();
  let src = "BEGIN; SELECT 1; UPDATE users SET id = 2 WHERE id = 1; COMMIT;";
  let once = dsl_format::format(src, &style, &ct_style);
  let twice = dsl_format::format(&once, &style, &ct_style);
  assert_eq!(once, twice, "multi-stmt not idempotent");
}

#[test]
fn r2_172_window_function_idempotent() {
  let style = dsl_format::FormatterStyle::default();
  let ct_style = dsl_format::CreateTableStyle::default();
  let src = "SELECT id, COUNT(*) OVER (PARTITION BY name ORDER BY id ROWS BETWEEN UNBOUNDED PRECEDING AND CURRENT ROW) FROM users;";
  let once = dsl_format::format(src, &style, &ct_style);
  let twice = dsl_format::format(&once, &style, &ct_style);
  assert_eq!(once, twice, "window function not idempotent");
}

#[test]
fn r2_172_grouping_sets_idempotent() {
  let style = dsl_format::FormatterStyle::default();
  let ct_style = dsl_format::CreateTableStyle::default();
  let src = "SELECT a, b, count(*) FROM t GROUP BY GROUPING SETS ((a, b), (a), ());";
  let once = dsl_format::format(src, &style, &ct_style);
  let twice = dsl_format::format(&once, &style, &ct_style);
  assert_eq!(once, twice, "grouping sets not idempotent");
}

#[test]
fn r2_172_case_when_idempotent() {
  let style = dsl_format::FormatterStyle::default();
  let ct_style = dsl_format::CreateTableStyle::default();
  let src = "SELECT CASE WHEN id > 0 THEN 'pos' WHEN id < 0 THEN 'neg' ELSE 'zero' END FROM users;";
  let once = dsl_format::format(src, &style, &ct_style);
  let twice = dsl_format::format(&once, &style, &ct_style);
  assert_eq!(once, twice, "CASE WHEN not idempotent");
}

#[test]
fn r3_101_merge_idempotent() {
  let style = dsl_format::FormatterStyle::default();
  let ct_style = dsl_format::CreateTableStyle::default();
  let src = "MERGE INTO users u USING orders o ON u.id = o.user_id WHEN MATCHED THEN UPDATE SET active = true WHEN NOT MATCHED THEN INSERT (id, email) VALUES (o.user_id, 'x');";
  let once = dsl_format::format(src, &style, &ct_style);
  let twice = dsl_format::format(&once, &style, &ct_style);
  assert_eq!(once, twice, "MERGE not idempotent");
}

#[test]
fn r3_102_update_from_idempotent() {
  let style = dsl_format::FormatterStyle::default();
  let ct_style = dsl_format::CreateTableStyle::default();
  let src = "UPDATE users SET x = o.y FROM orders o WHERE o.user_id = users.id;";
  let once = dsl_format::format(src, &style, &ct_style);
  let twice = dsl_format::format(&once, &style, &ct_style);
  assert_eq!(once, twice, "UPDATE FROM not idempotent");
}

#[test]
fn r3_103_delete_using_idempotent() {
  let style = dsl_format::FormatterStyle::default();
  let ct_style = dsl_format::CreateTableStyle::default();
  let src = "DELETE FROM users USING orders o WHERE o.user_id = users.id;";
  let once = dsl_format::format(src, &style, &ct_style);
  let twice = dsl_format::format(&once, &style, &ct_style);
  assert_eq!(once, twice, "DELETE USING not idempotent");
}

#[test]
fn r3_104_window_clause_idempotent() {
  let style = dsl_format::FormatterStyle::default();
  let ct_style = dsl_format::CreateTableStyle::default();
  let src = "SELECT row_number() OVER w FROM users WINDOW w AS (PARTITION BY id ORDER BY ts);";
  let once = dsl_format::format(src, &style, &ct_style);
  let twice = dsl_format::format(&once, &style, &ct_style);
  assert_eq!(once, twice, "WINDOW clause not idempotent");
}

#[test]
fn r3_105_create_index_concurrently_idempotent() {
  let style = dsl_format::FormatterStyle::default();
  let ct_style = dsl_format::CreateTableStyle::default();
  let src = "CREATE INDEX CONCURRENTLY idx_users_email ON users (email) WHERE active = true;";
  let once = dsl_format::format(src, &style, &ct_style);
  let twice = dsl_format::format(&once, &style, &ct_style);
  assert_eq!(once, twice, "CREATE INDEX CONCURRENTLY not idempotent");
}

#[test]
fn r3_191_alter_partition_idempotent() {
  let style = dsl_format::FormatterStyle::default();
  let ct_style = dsl_format::CreateTableStyle::default();
  let src = "ALTER TABLE t ATTACH PARTITION t_2024 FOR VALUES FROM ('2024-01-01') TO ('2025-01-01');";
  let once = dsl_format::format(src, &style, &ct_style);
  let twice = dsl_format::format(&once, &style, &ct_style);
  assert_eq!(once, twice, "ATTACH PARTITION not idempotent");
}

#[test]
fn r3_192_generate_series_lateral_idempotent() {
  let style = dsl_format::FormatterStyle::default();
  let ct_style = dsl_format::CreateTableStyle::default();
  let src = "SELECT * FROM users u, LATERAL generate_series(1, u.cnt) AS gs;";
  let once = dsl_format::format(src, &style, &ct_style);
  let twice = dsl_format::format(&once, &style, &ct_style);
  assert_eq!(once, twice, "LATERAL not idempotent");
}

#[test]
fn r3_193_create_publication_idempotent() {
  let style = dsl_format::FormatterStyle::default();
  let ct_style = dsl_format::CreateTableStyle::default();
  let src = "CREATE PUBLICATION p FOR ALL TABLES WITH (publish = 'insert,update');";
  let once = dsl_format::format(src, &style, &ct_style);
  let twice = dsl_format::format(&once, &style, &ct_style);
  assert_eq!(once, twice, "PUBLICATION not idempotent");
}

#[test]
fn r3_194_create_event_trigger_idempotent() {
  let style = dsl_format::FormatterStyle::default();
  let ct_style = dsl_format::CreateTableStyle::default();
  let src = "CREATE EVENT TRIGGER et ON ddl_command_start EXECUTE FUNCTION log_ddl();";
  let once = dsl_format::format(src, &style, &ct_style);
  let twice = dsl_format::format(&once, &style, &ct_style);
  assert_eq!(once, twice, "EVENT TRIGGER not idempotent");
}

#[test]
fn r3_195_create_statistics_idempotent() {
  let style = dsl_format::FormatterStyle::default();
  let ct_style = dsl_format::CreateTableStyle::default();
  let src = "CREATE STATISTICS s (dependencies, ndistinct) ON a, b FROM t;";
  let once = dsl_format::format(src, &style, &ct_style);
  let twice = dsl_format::format(&once, &style, &ct_style);
  assert_eq!(once, twice, "STATISTICS not idempotent");
}

#[test]
fn r3_296_copy_csv_options_idempotent() {
  let style = dsl_format::FormatterStyle::default();
  let ct_style = dsl_format::CreateTableStyle::default();
  let src = "COPY users TO STDOUT WITH (FORMAT csv, DELIMITER ',', HEADER true, QUOTE '\"', NULL '');";
  let once = dsl_format::format(src, &style, &ct_style);
  let twice = dsl_format::format(&once, &style, &ct_style);
  assert_eq!(once, twice, "COPY CSV not idempotent");
}

#[test]
fn r3_297_insert_on_conflict_multi_col_idempotent() {
  let style = dsl_format::FormatterStyle::default();
  let ct_style = dsl_format::CreateTableStyle::default();
  let src = "INSERT INTO t (a, b, c) VALUES (1, 2, 3) ON CONFLICT (a, b) DO UPDATE SET c = excluded.c;";
  let once = dsl_format::format(src, &style, &ct_style);
  let twice = dsl_format::format(&once, &style, &ct_style);
  assert_eq!(once, twice, "ON CONFLICT multi-col not idempotent");
}

#[test]
fn r3_298_declare_cursor_idempotent() {
  let style = dsl_format::FormatterStyle::default();
  let ct_style = dsl_format::CreateTableStyle::default();
  let src = "DECLARE c CURSOR WITH HOLD FOR SELECT * FROM users;";
  let once = dsl_format::format(src, &style, &ct_style);
  let twice = dsl_format::format(&once, &style, &ct_style);
  assert_eq!(once, twice, "DECLARE CURSOR not idempotent");
}

#[test]
fn r3_299_create_role_idempotent() {
  let style = dsl_format::FormatterStyle::default();
  let ct_style = dsl_format::CreateTableStyle::default();
  let src = "CREATE ROLE r WITH LOGIN PASSWORD 'x' VALID UNTIL '2026-01-01';";
  let once = dsl_format::format(src, &style, &ct_style);
  let twice = dsl_format::format(&once, &style, &ct_style);
  assert_eq!(once, twice, "CREATE ROLE not idempotent");
}

#[test]
fn r3_300_grant_chain_idempotent() {
  let style = dsl_format::FormatterStyle::default();
  let ct_style = dsl_format::CreateTableStyle::default();
  let src = "GRANT SELECT, INSERT, UPDATE ON ALL TABLES IN SCHEMA public TO app_role;";
  let once = dsl_format::format(src, &style, &ct_style);
  let twice = dsl_format::format(&once, &style, &ct_style);
  assert_eq!(once, twice, "GRANT not idempotent");
}

#[test]
fn r4_126_format_create_aggregate_idempotent() {
  let style = dsl_format::FormatterStyle::default();
  let ct_style = dsl_format::CreateTableStyle::default();
  let src = "CREATE AGGREGATE my_sum (int) (SFUNC = int4pl, STYPE = int, INITCOND = '0');";
  let once = dsl_format::format(src, &style, &ct_style);
  let twice = dsl_format::format(&once, &style, &ct_style);
  assert_eq!(once, twice);
}

#[test]
fn r4_127_format_create_operator_idempotent() {
  let style = dsl_format::FormatterStyle::default();
  let ct_style = dsl_format::CreateTableStyle::default();
  let src = "CREATE OPERATOR <-> (LEFTARG = point, RIGHTARG = point, FUNCTION = distance);";
  let once = dsl_format::format(src, &style, &ct_style);
  let twice = dsl_format::format(&once, &style, &ct_style);
  assert_eq!(once, twice);
}

#[test]
fn r4_128_format_create_index_predicate_idempotent() {
  let style = dsl_format::FormatterStyle::default();
  let ct_style = dsl_format::CreateTableStyle::default();
  let src = "CREATE INDEX idx ON t (a) WHERE a > 0 AND b IS NOT NULL;";
  let once = dsl_format::format(src, &style, &ct_style);
  let twice = dsl_format::format(&once, &style, &ct_style);
  assert_eq!(once, twice);
}

#[test]
fn r4_129_format_xml_table_idempotent() {
  let style = dsl_format::FormatterStyle::default();
  let ct_style = dsl_format::CreateTableStyle::default();
  let src = "SELECT t.* FROM xmltable('/r' PASSING d COLUMNS a int, b text PATH 'b') t;";
  let once = dsl_format::format(src, &style, &ct_style);
  let twice = dsl_format::format(&once, &style, &ct_style);
  assert_eq!(once, twice);
}

#[test]
fn r4_130_format_json_table_idempotent() {
  let style = dsl_format::FormatterStyle::default();
  let ct_style = dsl_format::CreateTableStyle::default();
  let src = "SELECT * FROM json_table(data, '$' COLUMNS (id int PATH '$.id')) t;";
  let once = dsl_format::format(src, &style, &ct_style);
  let twice = dsl_format::format(&once, &style, &ct_style);
  assert_eq!(once, twice);
}

#[test]
fn r4_plpgsql_long_if_reflow_idempotent() {
  let style = dsl_format::FormatterStyle::default();
  let ct_style = dsl_format::CreateTableStyle::default();
  let src = "CREATE OR REPLACE FUNCTION check_status() RETURNS TRIGGER AS $$\nBEGIN\n  IF NOT ((OLD.s = 'p' AND NEW.s = 'q') OR (OLD.s = 'q' AND NEW.s = 'r') OR (OLD.s = 'q' AND NEW.s = 's') OR (OLD.s = 'q' AND NEW.s = 't')) THEN\n    RAISE EXCEPTION 'bad';\n  END IF;\n  RETURN NEW;\nEND;\n$$ LANGUAGE plpgsql;";
  let once = dsl_format::format(src, &style, &ct_style);
  let twice = dsl_format::format(&once, &style, &ct_style);
  assert_eq!(once, twice, "plpgsql long-line reflow not idempotent");
}

#[test]
fn r4_plpgsql_short_lines_unchanged() {
  // Body where every line fits the width budget should pass through
  // verbatim (no false-positive reflow).
  let style = dsl_format::FormatterStyle::default();
  let ct_style = dsl_format::CreateTableStyle::default();
  let src = "CREATE OR REPLACE FUNCTION simple() RETURNS void AS $$\nBEGIN\n  RAISE NOTICE 'hi';\n  RETURN;\nEND;\n$$ LANGUAGE plpgsql;";
  let out = dsl_format::format(src, &style, &ct_style);
  assert!(out.contains("RAISE NOTICE 'hi';"));
  assert!(out.contains("END;"));
}

#[test]
fn r4_528_format_partition_idempotent() {
  // Was a known gap (sql-formatter shredded `FOR VALUES FROM (..) TO
  // (..)`); now stashed as opaque sentinel before sql-formatter runs
  // and align::rewrite_tables skips PARTITION OF too.
  let s = dsl_format::FormatterStyle::default();
  let c = dsl_format::CreateTableStyle::default();
  for src in [
    "CREATE TABLE t_2024 PARTITION OF events FOR VALUES FROM ('2024-01-01') TO ('2025-01-01');",
    "CREATE TABLE t_list PARTITION OF events FOR VALUES IN ('a', 'b', 'c');",
    "CREATE TABLE t_hash PARTITION OF events FOR VALUES WITH (MODULUS 4, REMAINDER 0);",
    "CREATE TABLE t_def PARTITION OF events DEFAULT;",
  ] {
    let o = dsl_format::format(src, &s, &c);
    let o2 = dsl_format::format(&o, &s, &c);
    assert_eq!(o, o2, "PARTITION OF not idempotent: {src}");
    assert!(o.contains("PARTITION OF"), "PARTITION OF lost: {o}");
  }
}

#[test]
fn r4_529_format_create_view_with_check_idempotent() {
  let s = dsl_format::FormatterStyle::default();
  let c = dsl_format::CreateTableStyle::default();
  let src = "CREATE VIEW v AS SELECT id, email FROM users WHERE active WITH LOCAL CHECK OPTION;";
  let o = dsl_format::format(src, &s, &c);
  let o2 = dsl_format::format(&o, &s, &c);
  assert_eq!(o, o2);
}

#[test]
fn r4_530_format_create_subscription_idempotent() {
  let s = dsl_format::FormatterStyle::default();
  let c = dsl_format::CreateTableStyle::default();
  let src = "CREATE SUBSCRIPTION sub CONNECTION 'host=h' PUBLICATION p WITH (slot_name = 's', copy_data = true);";
  let o = dsl_format::format(src, &s, &c);
  let o2 = dsl_format::format(&o, &s, &c);
  assert_eq!(o, o2);
}

#[test]
fn r4_531_format_window_chain_idempotent() {
  let s = dsl_format::FormatterStyle::default();
  let c = dsl_format::CreateTableStyle::default();
  let src = "SELECT rank() OVER w, row_number() OVER w2 FROM events WINDOW w AS (PARTITION BY uid), w2 AS (ORDER BY ts);";
  let o = dsl_format::format(src, &s, &c);
  let o2 = dsl_format::format(&o, &s, &c);
  assert_eq!(o, o2);
}

#[test]
fn r4_532_format_lateral_subquery_idempotent() {
  let s = dsl_format::FormatterStyle::default();
  let c = dsl_format::CreateTableStyle::default();
  let src = "SELECT u.id, last.ts FROM users u, LATERAL (SELECT max(ts) AS ts FROM events WHERE uid = u.id) last;";
  let o = dsl_format::format(src, &s, &c);
  let o2 = dsl_format::format(&o, &s, &c);
  assert_eq!(o, o2);
}

#[test]
fn r4_551_create_index_using_opclass_idempotent() {
  let s = dsl_format::FormatterStyle::default();
  let c = dsl_format::CreateTableStyle::default();
  for src in [
    "CREATE INDEX idx ON t USING gin (data jsonb_path_ops);",
    "CREATE INDEX idx ON t USING brin (ts) WITH (pages_per_range = 64);",
    "CREATE UNIQUE INDEX idx ON t (email) WHERE active;",
  ] {
    let o = dsl_format::format(src, &s, &c);
    let o2 = dsl_format::format(&o, &s, &c);
    assert_eq!(o, o2);
  }
}

#[test]
fn r4_552_generated_as_expr_preserved() {
  let s = dsl_format::FormatterStyle::default();
  let c = dsl_format::CreateTableStyle::default();
  let src = "CREATE TABLE t (id int, total int GENERATED ALWAYS AS (a + b * 2) STORED);";
  let o = dsl_format::format(src, &s, &c);
  assert!(o.contains("GENERATED ALWAYS AS"));
  assert!(o.contains("STORED"));
}

#[test]
fn r4_553_security_definer_chain_variations_idempotent() {
  let s = dsl_format::FormatterStyle::default();
  let c = dsl_format::CreateTableStyle::default();
  for src in [
    "CREATE FUNCTION f() RETURNS int LANGUAGE sql STABLE SECURITY DEFINER SET search_path = pg_catalog AS $$ SELECT 1 $$;",
    "CREATE FUNCTION f() RETURNS int LANGUAGE sql IMMUTABLE STRICT SECURITY DEFINER SET search_path = pg_catalog, pg_temp AS $$ SELECT 1 $$;",
    "CREATE FUNCTION f() RETURNS int LANGUAGE sql LEAKPROOF PARALLEL SAFE SECURITY DEFINER SET search_path = pg_catalog AS $$ SELECT 1 $$;",
  ] {
    let o = dsl_format::format(src, &s, &c);
    let o2 = dsl_format::format(&o, &s, &c);
    assert_eq!(o, o2);
    assert!(o.contains("SET search_path"));
  }
}

#[test]
fn r4_596_create_type_enum_idempotent() {
  let s = dsl_format::FormatterStyle::default();
  let c = dsl_format::CreateTableStyle::default();
  for src in [
    "CREATE TYPE color AS ENUM ('red', 'green', 'blue');",
    "CREATE TYPE addr AS (street text, city text, zip varchar(10));",
    "CREATE TYPE r AS RANGE (SUBTYPE = numeric);",
    "CREATE DOMAIN email AS text CHECK (VALUE ~* '^[^@]+@[^@]+$');",
  ] {
    let o = dsl_format::format(src, &s, &c);
    let o2 = dsl_format::format(&o, &s, &c);
    assert_eq!(o, o2);
  }
}

#[test]
fn r4_597_create_operator_family_idempotent() {
  let s = dsl_format::FormatterStyle::default();
  let c = dsl_format::CreateTableStyle::default();
  let src = "CREATE OPERATOR FAMILY my_fam USING btree;";
  let o = dsl_format::format(src, &s, &c);
  let o2 = dsl_format::format(&o, &s, &c);
  assert_eq!(o, o2);
}

#[test]
fn r4_598_user_kta_ticket_system_sql_block_idempotent() {
  // Mimics the SQL inside the user's kta-ticket-system.ts sql\`...\` blocks.
  let s = dsl_format::FormatterStyle::default();
  let c = dsl_format::CreateTableStyle::default();
  let src = "CREATE SCHEMA IF NOT EXISTS app;\n\
CREATE OR REPLACE FUNCTION app.current_user_id() RETURNS UUID\n\
  LANGUAGE sql STABLE SECURITY DEFINER SET search_path = pg_catalog, pg_temp AS $$\n\
  SELECT NULLIF(current_setting('app.user_id', true), '')::UUID;\n\
$$;\n";
  let o = dsl_format::format(src, &s, &c);
  let o2 = dsl_format::format(&o, &s, &c);
  assert_eq!(o, o2);
  assert!(o.contains("SET search_path = pg_catalog, pg_temp"));
}

#[test]
fn r9_pilot_fmt_nonempty() {
  let out = rw("SELECT 1");
  assert!(!out.is_empty());
}

#[test]
fn r9_pilot_fmt_contains_select() {
  let out = rw("SELECT id FROM users");
  assert!(out.to_ascii_uppercase().contains("SELECT"));
}

#[test]
fn r9_pilot_fmt_contains_table_name() {
  let out = rw("SELECT id FROM users");
  assert!(out.contains("users"));
}

#[test]
fn r9_pilot_fmt_idempotent_select() {
  let out1 = rw("SELECT id FROM users");
  let out2 = rw(&out1);
  assert_eq!(out1, out2);
}

#[test]
fn r9_fmt_idem_0001() {
  let a = rw("SELECT 1");
  let b = rw(&a);
  assert_eq!(a, b);
}

#[test]
fn r9_fmt_idem_0002() {
  let a = rw("SELECT id FROM users");
  let b = rw(&a);
  assert_eq!(a, b);
}

#[test]
fn r9_fmt_idem_0003() {
  let a = rw("SELECT id, name FROM users");
  let b = rw(&a);
  assert_eq!(a, b);
}

#[test]
fn r9_fmt_idem_0004() {
  let a = rw("SELECT id FROM users WHERE id = 1");
  let b = rw(&a);
  assert_eq!(a, b);
}

#[test]
fn r9_fmt_idem_0005() {
  let a = rw("SELECT count(*) FROM users");
  let b = rw(&a);
  assert_eq!(a, b);
}

#[test]
fn r9_fmt_idem_0006() {
  let a = rw("SELECT * FROM users ORDER BY id");
  let b = rw(&a);
  assert_eq!(a, b);
}

#[test]
fn r9_fmt_idem_0007() {
  let a = rw("SELECT * FROM users LIMIT 10");
  let b = rw(&a);
  assert_eq!(a, b);
}

#[test]
fn r9_fmt_idem_0008() {
  let a = rw("INSERT INTO users (id, name) VALUES (1, 'a')");
  let b = rw(&a);
  assert_eq!(a, b);
}

#[test]
fn r9_fmt_idem_0009() {
  let a = rw("UPDATE users SET name = 'x' WHERE id = 1");
  let b = rw(&a);
  assert_eq!(a, b);
}

#[test]
fn r9_fmt_idem_0010() {
  let a = rw("DELETE FROM users WHERE id = 1");
  let b = rw(&a);
  assert_eq!(a, b);
}

#[test]
fn r9_fmt_kw_0301() {
  let out = rw("SELECT 1");
  assert!(out.to_ascii_uppercase().contains("SELECT"));
}

#[test]
fn r9_fmt_kw_0303() {
  let out = rw("SELECT id, name FROM users");
  assert!(out.to_ascii_uppercase().contains("SELECT"));
}

#[test]
fn r9_fmt_kw_0304() {
  let out = rw("SELECT id FROM users WHERE id = 1");
  assert!(out.to_ascii_uppercase().contains("SELECT"));
}

#[test]
fn r9_fmt_kw_0305() {
  let out = rw("SELECT count(*) FROM users");
  assert!(out.to_ascii_uppercase().contains("SELECT"));
}

#[test]
fn r9_fmt_kw_0306() {
  let out = rw("SELECT * FROM users ORDER BY id");
  assert!(out.to_ascii_uppercase().contains("SELECT"));
}

#[test]
fn r9_fmt_kw_0307() {
  let out = rw("SELECT * FROM users LIMIT 10");
  assert!(out.to_ascii_uppercase().contains("SELECT"));
}

#[test]
fn r9_fmt_kw_0308() {
  let out = rw("INSERT INTO users (id, name) VALUES (1, 'a')");
  assert!(out.to_ascii_uppercase().contains("INSERT"));
}

#[test]
fn r9_fmt_kw_0309() {
  let out = rw("UPDATE users SET name = 'x' WHERE id = 1");
  assert!(out.to_ascii_uppercase().contains("UPDATE"));
}

#[test]
fn r9_fmt_kw_0310() {
  let out = rw("DELETE FROM users WHERE id = 1");
  assert!(out.to_ascii_uppercase().contains("DELETE"));
}

#[test]
fn r9_fmt_ne_0502() {
  let out = rw("SELECT id FROM users");
  assert!(!out.is_empty());
}

#[test]
fn r9_fmt_ne_0503() {
  let out = rw("SELECT id, name FROM users");
  assert!(!out.is_empty());
}

#[test]
fn r9_fmt_ne_0504() {
  let out = rw("SELECT id FROM users WHERE id = 1");
  assert!(!out.is_empty());
}

#[test]
fn r9_fmt_ne_0505() {
  let out = rw("SELECT count(*) FROM users");
  assert!(!out.is_empty());
}

#[test]
fn r9_fmt_ne_0506() {
  let out = rw("SELECT * FROM users ORDER BY id");
  assert!(!out.is_empty());
}

#[test]
fn r9_fmt_ne_0507() {
  let out = rw("SELECT * FROM users LIMIT 10");
  assert!(!out.is_empty());
}

#[test]
fn r9_fmt_ne_0508() {
  let out = rw("INSERT INTO users (id, name) VALUES (1, 'a')");
  assert!(!out.is_empty());
}

#[test]
fn r9_fmt_ne_0509() {
  let out = rw("UPDATE users SET name = 'x' WHERE id = 1");
  assert!(!out.is_empty());
}

#[test]
fn r9_fmt_ne_0510() {
  let out = rw("DELETE FROM users WHERE id = 1");
  assert!(!out.is_empty());
}


#[test]
fn r10_fmt_idem_0001() {
  let a = rw("-- f0\nSELECT id FROM users WHERE id = 0");
  let b = rw(&a);
  assert_eq!(a, b);
}

#[test]
fn r10_fmt_idem_0002() {
  let a = rw("-- f0\nSELECT email FROM users WHERE id = 0");
  let b = rw(&a);
  assert_eq!(a, b);
}

#[test]
fn r10_fmt_idem_0003() {
  let a = rw("-- f0\nUPDATE users SET name = 'x' WHERE id = 0");
  let b = rw(&a);
  assert_eq!(a, b);
}

#[test]
fn r10_fmt_idem_0004() {
  let a = rw("-- f0\nDELETE FROM users WHERE id = 0");
  let b = rw(&a);
  assert_eq!(a, b);
}

#[test]
fn r10_fmt_idem_0005() {
  let a = rw("-- f0\nINSERT INTO users (id, name) VALUES (0, 'a')");
  let b = rw(&a);
  assert_eq!(a, b);
}

#[test]
fn r10_fmt_idem_0006() {
  let a = rw("-- f1\nSELECT id FROM users WHERE id = 1");
  let b = rw(&a);
  assert_eq!(a, b);
}

#[test]
fn r10_fmt_idem_0007() {
  let a = rw("-- f1\nSELECT email FROM users WHERE id = 1");
  let b = rw(&a);
  assert_eq!(a, b);
}

#[test]
fn r10_fmt_idem_0008() {
  let a = rw("-- f1\nUPDATE users SET name = 'x' WHERE id = 1");
  let b = rw(&a);
  assert_eq!(a, b);
}

#[test]
fn r10_fmt_idem_0009() {
  let a = rw("-- f1\nDELETE FROM users WHERE id = 1");
  let b = rw(&a);
  assert_eq!(a, b);
}

#[test]
fn r10_fmt_idem_0010() {
  let a = rw("-- f1\nINSERT INTO users (id, name) VALUES (1, 'a')");
  let b = rw(&a);
  assert_eq!(a, b);
}

#[test]
fn r10_fmt_idem_0012() {
  let a = rw("-- f2\nSELECT email FROM users WHERE id = 2");
  let b = rw(&a);
  assert_eq!(a, b);
}

#[test]
fn r11_fmt_idem_0001() {
  let a = rw("-- f0\nSELECT id FROM users");
  let b = rw(&a);
  assert_eq!(a, b);
}

#[test]
fn r11_fmt_idem_0002() {
  let a = rw("-- f0\nSELECT email FROM users");
  let b = rw(&a);
  assert_eq!(a, b);
}

#[test]
fn r11_fmt_idem_0003() {
  let a = rw("-- f0\nSELECT name FROM users");
  let b = rw(&a);
  assert_eq!(a, b);
}

#[test]
fn r11_fmt_idem_0004() {
  let a = rw("-- f0\nSELECT id, email FROM users");
  let b = rw(&a);
  assert_eq!(a, b);
}

#[test]
fn r11_fmt_idem_0005() {
  let a = rw("-- f0\nSELECT id, name FROM users");
  let b = rw(&a);
  assert_eq!(a, b);
}

#[test]
fn r11_fmt_idem_0006() {
  let a = rw("-- f0\nSELECT email, name FROM users");
  let b = rw(&a);
  assert_eq!(a, b);
}

#[test]
fn r11_fmt_idem_0007() {
  let a = rw("-- f0\nSELECT id, email, name FROM users");
  let b = rw(&a);
  assert_eq!(a, b);
}

#[test]
fn r11_fmt_idem_0008() {
  let a = rw("-- f0\nSELECT * FROM users");
  let b = rw(&a);
  assert_eq!(a, b);
}

#[test]
fn r11_fmt_idem_0010() {
  let a = rw("-- f0\nSELECT id FROM users WHERE email = 'a'");
  let b = rw(&a);
  assert_eq!(a, b);
}

#[test]
fn r11_fmt_idem_0011() {
  let a = rw("-- f0\nINSERT INTO users (id) VALUES (1)");
  let b = rw(&a);
  assert_eq!(a, b);
}

#[test]
fn r11_fmt_idem_0013() {
  let a = rw("-- f0\nUPDATE users SET name = 'x'");
  let b = rw(&a);
  assert_eq!(a, b);
}

#[test]
fn r11_fmt_idem_0015() {
  let a = rw("-- f0\nDELETE FROM users");
  let b = rw(&a);
  assert_eq!(a, b);
}

#[test]
fn r11_fmt_idem_0017() {
  let a = rw("-- f0\nSELECT count(*) FROM users");
  let b = rw(&a);
  assert_eq!(a, b);
}

#[test]
fn r11_fmt_idem_0018() {
  let a = rw("-- f0\nSELECT * FROM orders");
  let b = rw(&a);
  assert_eq!(a, b);
}

#[test]
fn r11_fmt_idem_0019() {
  let a = rw("-- f0\nSELECT id FROM orders");
  let b = rw(&a);
  assert_eq!(a, b);
}

#[test]
fn r11_fmt_idem_0020() {
  let a = rw("-- f0\nSELECT user_id FROM orders");
  let b = rw(&a);
  assert_eq!(a, b);
}

#[test]
fn r11_fmt_idem_0021() {
  let a = rw("-- f1\nSELECT id FROM users");
  let b = rw(&a);
  assert_eq!(a, b);
}

#[test]
fn r11_fmt_idem_0022() {
  let a = rw("-- f1\nSELECT email FROM users");
  let b = rw(&a);
  assert_eq!(a, b);
}

#[test]
fn r11_fmt_idem_0023() {
  let a = rw("-- f1\nSELECT name FROM users");
  let b = rw(&a);
  assert_eq!(a, b);
}

#[test]
fn r11_fmt_idem_0024() {
  let a = rw("-- f1\nSELECT id, email FROM users");
  let b = rw(&a);
  assert_eq!(a, b);
}

#[test]
fn r11_fmt_idem_0025() {
  let a = rw("-- f1\nSELECT id, name FROM users");
  let b = rw(&a);
  assert_eq!(a, b);
}

#[test]
fn r11_fmt_idem_0026() {
  let a = rw("-- f1\nSELECT email, name FROM users");
  let b = rw(&a);
  assert_eq!(a, b);
}

#[test]
fn r11_fmt_idem_0027() {
  let a = rw("-- f1\nSELECT id, email, name FROM users");
  let b = rw(&a);
  assert_eq!(a, b);
}

#[test]
fn r11_fmt_idem_0028() {
  let a = rw("-- f1\nSELECT * FROM users");
  let b = rw(&a);
  assert_eq!(a, b);
}

#[test]
fn r11_fmt_idem_0030() {
  let a = rw("-- f1\nSELECT id FROM users WHERE email = 'a'");
  let b = rw(&a);
  assert_eq!(a, b);
}

#[test]
fn r11_fmt_idem_0031() {
  let a = rw("-- f1\nINSERT INTO users (id) VALUES (1)");
  let b = rw(&a);
  assert_eq!(a, b);
}

#[test]
fn r13_fmt_0003() {
  let a = rw("SELECT name FROM users WHERE id = 0");
  let b = rw(&a);
  assert_eq!(a, b);
}

#[test]
fn r13_fmt_0004() {
  let a = rw("SELECT id, email FROM users WHERE id = 0");
  let b = rw(&a);
  assert_eq!(a, b);
}

#[test]
fn r13_fmt_0005() {
  let a = rw("SELECT id, name FROM users WHERE id = 0");
  let b = rw(&a);
  assert_eq!(a, b);
}

#[test]
fn r13_fmt_0009() {
  let a = rw("SELECT * FROM orders WHERE id = 0");
  let b = rw(&a);
  assert_eq!(a, b);
}

#[test]
fn r13_fmt_0010() {
  let a = rw("SELECT id FROM orders WHERE user_id = 0");
  let b = rw(&a);
  assert_eq!(a, b);
}

#[test]
fn r13_fmt_0013() {
  let a = rw("SELECT name FROM users WHERE id = 1");
  let b = rw(&a);
  assert_eq!(a, b);
}

#[test]
fn r13_fmt_0014() {
  let a = rw("SELECT id, email FROM users WHERE id = 1");
  let b = rw(&a);
  assert_eq!(a, b);
}

#[test]
fn r13_fmt_0015() {
  let a = rw("SELECT id, name FROM users WHERE id = 1");
  let b = rw(&a);
  assert_eq!(a, b);
}

#[test]
fn r13_fmt_0019() {
  let a = rw("SELECT * FROM orders WHERE id = 1");
  let b = rw(&a);
  assert_eq!(a, b);
}

#[test]
fn r13_fmt_0020() {
  let a = rw("SELECT id FROM orders WHERE user_id = 1");
  let b = rw(&a);
  assert_eq!(a, b);
}

#[test]
fn r13_fmt_0023() {
  let a = rw("SELECT name FROM users WHERE id = 2");
  let b = rw(&a);
  assert_eq!(a, b);
}

#[test]
fn r13_fmt_0024() {
  let a = rw("SELECT id, email FROM users WHERE id = 2");
  let b = rw(&a);
  assert_eq!(a, b);
}

#[test]
fn r13_fmt_0025() {
  let a = rw("SELECT id, name FROM users WHERE id = 2");
  let b = rw(&a);
  assert_eq!(a, b);
}

#[test]
fn r13_fmt_0029() {
  let a = rw("SELECT * FROM orders WHERE id = 2");
  let b = rw(&a);
  assert_eq!(a, b);
}

#[test]
fn r13_fmt_0030() {
  let a = rw("SELECT id FROM orders WHERE user_id = 2");
  let b = rw(&a);
  assert_eq!(a, b);
}

#[test]
fn r14_fmt_0002() {
  let a = rw("SELECT email FROM users WHERE email = 'val0'");
  let b = rw(&a);
  assert_eq!(a, b);
}

#[test]
fn r14_fmt_0004() {
  let a = rw("DELETE FROM users WHERE id = 0 AND name = 'x0'");
  let b = rw(&a);
  assert_eq!(a, b);
}

#[test]
fn r14_fmt_0005() {
  let a = rw("INSERT INTO orders (id, user_id) VALUES (0, 0)");
  let b = rw(&a);
  assert_eq!(a, b);
}

#[test]
fn r14_fmt_0006() {
  let a = rw("SELECT id, email, name FROM users WHERE id = 0");
  let b = rw(&a);
  assert_eq!(a, b);
}

#[test]
fn r14_fmt_0007() {
  let a = rw("SELECT count(*) FROM users WHERE id > 0");
  let b = rw(&a);
  assert_eq!(a, b);
}

#[test]
fn r14_fmt_0008() {
  let a = rw("SELECT max(id) FROM orders WHERE user_id = 0");
  let b = rw(&a);
  assert_eq!(a, b);
}

#[test]
fn r14_fmt_0010() {
  let a = rw("SELECT email FROM users WHERE email = 'val1'");
  let b = rw(&a);
  assert_eq!(a, b);
}

#[test]
fn r14_fmt_0012() {
  let a = rw("DELETE FROM users WHERE id = 1 AND name = 'x1'");
  let b = rw(&a);
  assert_eq!(a, b);
}

#[test]
fn r14_fmt_0013() {
  let a = rw("INSERT INTO orders (id, user_id) VALUES (1, 1)");
  let b = rw(&a);
  assert_eq!(a, b);
}

#[test]
fn r14_fmt_0014() {
  let a = rw("SELECT id, email, name FROM users WHERE id = 1");
  let b = rw(&a);
  assert_eq!(a, b);
}

#[test]
fn r14_fmt_0015() {
  let a = rw("SELECT count(*) FROM users WHERE id > 1");
  let b = rw(&a);
  assert_eq!(a, b);
}

#[test]
fn r14_fmt_0016() {
  let a = rw("SELECT max(id) FROM orders WHERE user_id = 1");
  let b = rw(&a);
  assert_eq!(a, b);
}

#[test]
fn r14_fmt_0018() {
  let a = rw("SELECT email FROM users WHERE email = 'val2'");
  let b = rw(&a);
  assert_eq!(a, b);
}

#[test]
fn r14_fmt_0020() {
  let a = rw("DELETE FROM users WHERE id = 2 AND name = 'x2'");
  let b = rw(&a);
  assert_eq!(a, b);
}

#[test]
fn r14_fmt_0021() {
  let a = rw("INSERT INTO orders (id, user_id) VALUES (2, 2)");
  let b = rw(&a);
  assert_eq!(a, b);
}

#[test]
fn r14_fmt_0022() {
  let a = rw("SELECT id, email, name FROM users WHERE id = 2");
  let b = rw(&a);
  assert_eq!(a, b);
}

#[test]
fn r14_fmt_0023() {
  let a = rw("SELECT count(*) FROM users WHERE id > 2");
  let b = rw(&a);
  assert_eq!(a, b);
}

#[test]
fn r14_fmt_0024() {
  let a = rw("SELECT max(id) FROM orders WHERE user_id = 2");
  let b = rw(&a);
  assert_eq!(a, b);
}

#[test]
fn r16_fmt_ident_0001() {
  let out = rw("SELECT id FROM users WHERE id = 0");
  assert!(out.contains("users"));
}

#[test]
fn r16_fmt_ident_0002() {
  let out = rw("SELECT email FROM users WHERE id = 0");
  assert!(out.contains("email"));
}

#[test]
fn r16_fmt_ident_0003() {
  let out = rw("INSERT INTO orders (id, user_id) VALUES (0, 0)");
  assert!(out.contains("orders"));
}

#[test]
fn r16_fmt_ident_0004() {
  let out = rw("UPDATE users SET name = 'x0' WHERE id = 0");
  assert!(out.contains("name"));
}

#[test]
fn r16_fmt_ident_0005() {
  let out = rw("DELETE FROM orders WHERE id = 0");
  assert!(out.contains("orders"));
}

#[test]
fn r16_fmt_ident_0006() {
  let out = rw("SELECT id FROM users WHERE id = 1");
  assert!(out.contains("users"));
}

#[test]
fn r16_fmt_ident_0007() {
  let out = rw("SELECT email FROM users WHERE id = 1");
  assert!(out.contains("email"));
}

#[test]
fn r16_fmt_ident_0008() {
  let out = rw("INSERT INTO orders (id, user_id) VALUES (1, 1)");
  assert!(out.contains("orders"));
}

#[test]
fn r16_fmt_ident_0009() {
  let out = rw("UPDATE users SET name = 'x1' WHERE id = 1");
  assert!(out.contains("name"));
}

#[test]
fn r16_fmt_ident_0010() {
  let out = rw("DELETE FROM orders WHERE id = 1");
  assert!(out.contains("orders"));
}

#[test]
fn r16_fmt_ident_0011() {
  let out = rw("SELECT id FROM users WHERE id = 2");
  assert!(out.contains("users"));
}

#[test]
fn r16_fmt_ident_0012() {
  let out = rw("SELECT email FROM users WHERE id = 2");
  assert!(out.contains("email"));
}

#[test]
fn r16_fmt_ident_0013() {
  let out = rw("INSERT INTO orders (id, user_id) VALUES (2, 2)");
  assert!(out.contains("orders"));
}

#[test]
fn r16_fmt_ident_0014() {
  let out = rw("UPDATE users SET name = 'x2' WHERE id = 2");
  assert!(out.contains("name"));
}

#[test]
fn r16_fmt_ident_0015() {
  let out = rw("DELETE FROM orders WHERE id = 2");
  assert!(out.contains("orders"));
}

