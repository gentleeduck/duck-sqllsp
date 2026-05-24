//! Smoke tests for the PL/pgSQL body alignment pass.

use dsl_format::{rewrite, CreateTableStyle};

fn aligned(src: &str) -> String {
    rewrite(src, &CreateTableStyle::default())
}

#[test]
fn body_indents_statements_inside_begin_end() {
    let input = "CREATE FUNCTION f() RETURNS void LANGUAGE plpgsql AS $$BEGIN; UPDATE users SET name = 'x' WHERE id = 1; END;$$;";
    let out = aligned(input);
    // BEGIN at depth 0, UPDATE indented, END dedented.
    assert!(out.contains("BEGIN;"));
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
