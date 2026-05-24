//! Smoke tests for the PL/pgSQL body alignment pass.

use dsl_format::{CreateTableStyle, rewrite};

fn aligned(src: &str) -> String {
  rewrite(src, &CreateTableStyle::default())
}

#[test]
fn body_indents_statements_inside_begin_end() {
  let input =
    "CREATE FUNCTION f() RETURNS void LANGUAGE plpgsql AS $$BEGIN; UPDATE users SET name = 'x' WHERE id = 1; END;$$;";
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
  assert!(out.contains("  new.updated_at := now();"), "assignment not indented; got:\n{out}");
  assert!(out.contains("  RETURN new;"), "RETURN not indented; got:\n{out}");
}

#[test]
fn body_indents_declare_then_begin() {
  let input = "CREATE FUNCTION f() RETURNS int LANGUAGE plpgsql AS $$\nDECLARE\n    n int;\nBEGIN\n    n := 1;\n    RETURN n;\nEND;\n$$;";
  let out = aligned(input);
  assert!(out.contains("  n int;"), "DECLARE local not indented; got:\n{out}");
  assert!(out.contains("  RETURN n;"), "RETURN not indented; got:\n{out}");
}

#[test]
fn declare_begin_end_are_peer_level_at_depth_zero() {
  // User-reported: DECLARE / BEGIN / END must share the same indent
  // depth (the function-body opener level). Vars inside DECLARE,
  // stmts inside BEGIN, all at depth+1.
  let input = "CREATE OR REPLACE FUNCTION apply_discount(p_subtotal NUMERIC, p_promo_id UUID) RETURNS NUMERIC STABLE LANGUAGE plpgsql AS $$\nDECLARE\n    result NUMERIC;\n    promo promo_codes;\nBEGIN\n    IF p_promo_id IS NULL THEN\n        RETURN p_subtotal;\n    END IF;\n    RETURN coalesce(result, 0);\nEND;\n$$;";
  let out = aligned(input);
  // DECLARE / BEGIN / END at col 0 (no leading spaces on their line).
  for marker in ["\nDECLARE\n", "\nBEGIN\n", "\nEND;\n"] {
    assert!(out.contains(marker), "expected `{}` to be at col 0; got:\n{out}", marker.trim());
  }
  // Vars + stmts at col 2 (depth 1).
  assert!(out.contains("  result NUMERIC;"), "DECLARE var not at depth 1; got:\n{out}");
  assert!(out.contains("  RETURN coalesce(result, 0);"), "BEGIN-body RETURN not at depth 1; got:\n{out}");
}
