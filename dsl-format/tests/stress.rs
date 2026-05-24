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
  // FK sub-clauses broken.
  assert!(out.contains("\n        REFERENCES "));
  assert!(out.contains("\n        ON DELETE "));
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
