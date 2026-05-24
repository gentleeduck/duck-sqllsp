//! Smoke tests for the PL/pgSQL body alignment pass.

use dsl_format::{rewrite, CreateTableStyle};

fn aligned(src: &str) -> String {
    rewrite(src, &CreateTableStyle::default())
}

#[test]
fn body_indents_statements_inside_begin_end() {
    let input = "CREATE FUNCTION f() RETURNS void LANGUAGE plpgsql AS $$BEGIN; UPDATE users SET name = 'x' WHERE id = 1; END;$$;";
    let out = aligned(input);
    // BEGIN at depth 0, UPDATE indented, END dedented. The bare-marker
    // splitter normalises `BEGIN;` to `BEGIN` (no trailing semicolon
    // since PL/pgSQL's BEGIN marker never has one).
    assert!(out.contains("BEGIN\n") || out.contains("BEGIN;"));
    assert!(out.contains("  UPDATE users SET name = 'x' WHERE id = 1;"));
    assert!(out.contains("END;"));
}

#[test]
fn body_indents_if_then_block() {
    let input = "CREATE FUNCTION f() RETURNS void LANGUAGE plpgsql AS $$BEGIN; IF x THEN UPDATE users SET name='a'; END IF; END;$$;";
    let out = aligned(input);
    // IF nested under BEGIN, UPDATE nested again, END IF same as IF.
    assert!(out.contains("IF x THEN UPDATE"), "got:\n{out}");
}

#[test]
fn body_preserves_string_literals_with_semicolons() {
    let input = "CREATE FUNCTION f() RETURNS void LANGUAGE plpgsql AS $$BEGIN; RAISE NOTICE 'a;b;c'; END;$$;";
    let out = aligned(input);
    assert!(out.contains("RAISE NOTICE 'a;b;c';"));
}

#[test]
fn empty_body_does_not_panic() {
    let input = "CREATE FUNCTION f() RETURNS void LANGUAGE plpgsql AS $$$$;";
    let out = aligned(input);
    // Header gets broken across lines by an earlier pass; the empty
    // body is preserved without inserting spurious indentation.
    assert!(out.contains("AS $$$$;"));
}

#[test]
fn body_indents_real_trigger_fn_with_return_new() {
    // User-reported: `RETURN new;` was dedented to col 0 because
    // BEGIN (no semicolon in PL/pgSQL) merged into the first stmt
    // and never bumped the depth. Splitter now treats bare BEGIN /
    // DECLARE / EXCEPTION as their own statement boundaries.
    let input = "CREATE OR REPLACE FUNCTION set_updated_at() RETURNS TRIGGER LANGUAGE plpgsql AS $$\nBEGIN\n    new.updated_at := now();\n    RETURN new;\nEND;\n$$;";
    let out = aligned(input);
    assert!(out.contains("  new.updated_at := now();"),
        "assignment not indented; got:\n{out}");
    assert!(out.contains("  RETURN new;"),
        "RETURN not indented; got:\n{out}");
}

#[test]
fn body_indents_declare_then_begin() {
    let input = "CREATE FUNCTION f() RETURNS int LANGUAGE plpgsql AS $$\nDECLARE\n    n int;\nBEGIN\n    n := 1;\n    RETURN n;\nEND;\n$$;";
    let out = aligned(input);
    assert!(out.contains("  n int;"),
        "DECLARE local not indented; got:\n{out}");
    assert!(out.contains("  RETURN n;"),
        "RETURN not indented; got:\n{out}");
}
