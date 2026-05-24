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

#[test]
fn fk_clauses_break_inside_create_table() {
  let src = "\
CREATE TABLE t (
    id UUID,
    CONSTRAINT fk_x FOREIGN KEY (id) REFERENCES other (id) ON DELETE CASCADE
);
";
  let out = rw(src);
  // The constraint sub-clauses should sit on indented lines.
  assert!(out.contains("\n        REFERENCES "), "REFERENCES not broken: {out}");
  assert!(out.contains("\n        ON DELETE "), "ON DELETE not broken: {out}");
}

#[test]
fn fk_on_update_and_on_delete_both_break() {
  let src = "\
CREATE TABLE t (
    id UUID,
    CONSTRAINT fk_x FOREIGN KEY (id) REFERENCES other (id) ON UPDATE CASCADE ON DELETE SET NULL
);
";
  let out = rw(src);
  assert!(out.contains("\n        ON UPDATE "), "ON UPDATE not broken: {out}");
  assert!(out.contains("\n        ON DELETE "), "ON DELETE not broken: {out}");
  assert_eq!(rw(&out), out, "ON UPDATE/ON DELETE combo not idempotent");
}

#[test]
fn fk_with_match_full_breaks_match_clause() {
  let src = "\
CREATE TABLE t (
    id UUID,
    CONSTRAINT fk_x FOREIGN KEY (id) REFERENCES other (id) MATCH FULL ON DELETE CASCADE
);
";
  let out = rw(src);
  assert!(out.contains("\n        MATCH FULL"), "MATCH FULL not broken: {out}");
  assert_eq!(rw(&out), out);
}

#[test]
fn fk_deferrable_breaks() {
  let src = "\
CREATE TABLE t (
    id UUID,
    CONSTRAINT fk_x FOREIGN KEY (id) REFERENCES other (id) DEFERRABLE INITIALLY DEFERRED
);
";
  let out = rw(src);
  assert!(out.contains("\n        DEFERRABLE"), "DEFERRABLE not broken: {out}");
  assert!(out.contains("\n        INITIALLY DEFERRED"), "INITIALLY DEFERRED not broken: {out}");
  assert_eq!(rw(&out), out);
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
