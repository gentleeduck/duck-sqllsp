# jsonpath/jsonb operator depth rules (batch 6) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: superpowers:executing-plans (inline, no subagents). Process/global constraints match batch 1's plan.

**Goal:** Add 4 new dsl-analysis lint rules (sql780-sql783) extending the existing jsonb rule family with jsonpath filter and jsonb-literal-operator depth.

**Spec:** `dsl-analysis/docs/specs/2026-08-18-sql-rule-expansion-design.md` (batch 6). **Design note (not a spec deviation, a scoping decision made during planning):** sql782 (`jsonb_minus_integer_on_object`) can only be checked statically against a jsonb *literal* -- a jsonb *column*'s runtime shape (object vs array vs scalar) isn't visible in the catalog's static type (`jsonb` doesn't distinguish them), so the rule is scoped to `'<json-literal>'::jsonb - <int>` exactly like sql781, not to arbitrary columns.

## Tasks

| Code | Rule | Severity | Detects |
| --- | --- | --- | --- |
| sql780 | `jsonb_path_exists_static_false` | Warning | `jsonb_path_exists(doc, '$.a ? (N1 == N2)')` where N1/N2 are literal numbers making the filter always false (`==` with different literals, or `!=` with equal literals). |
| sql781 | `jsonb_array_length_on_object_literal` | Error | `jsonb_array_length('{"a":1}'::jsonb)` -- literal is an object, guaranteed 22023. |
| sql782 | `jsonb_minus_integer_on_object` | Error | `'{"a":1}'::jsonb - 0` -- integer-index delete is array-only; literal is an object. |
| sql783 | `jsonb_build_object_null_key` | Error | `jsonb_build_object(NULL, 1, ...)` -- literal null key, PG rejects at runtime. |

Files: `dsl-analysis/src/rules/{jsonb_path_exists_static_false,jsonb_array_length_on_object_literal,jsonb_minus_integer_on_object,jsonb_build_object_null_key}.rs`, `mod.rs`, new `dsl-analysis/tests/rules_jsonb_depth.rs`.

All 8 probe snippets (4 bad + 4 good) verified to produce zero `sql000` errors and no unexpected diagnostics beyond expected sql001/sql002 unresolved-name noise from the probe file's placeholder tables.

Tests:

```rust
use dsl_analysis::{Severity, run};
use dsl_catalog::{CATALOG_VERSION, Catalog, Schema};
use dsl_parse::{Dialect, parse};
use dsl_resolve::resolve_with_source;

fn empty_cat() -> Catalog {
  Catalog {
    version: CATALOG_VERSION,
    connection_id: "test".into(),
    schemas: vec![Schema { name: "public".into(), tables: vec![] }],
    functions: vec![],
    types: vec![],
    roles: vec![],
    sequences: vec![],
    extensions: vec![],
  }
}

fn diags(src: &str) -> Vec<dsl_analysis::Diagnostic> {
  let file = parse(src, Dialect::Postgres);
  let scopes = resolve_with_source(&file.statements, src);
  run(src, &file, &scopes, &empty_cat())
}

#[test]
fn sql780_static_false_filter() {
  let d = diags("SELECT jsonb_path_exists(doc, '$.a ? (1 == 2)') FROM t780;");
  assert!(d.iter().any(|x| x.code == "sql780" && x.severity == Severity::Warning));
}

#[test]
fn sql780_quiet_on_dynamic_filter() {
  let d = diags("SELECT jsonb_path_exists(doc, '$.a ? (@ == 2)') FROM t780;");
  assert!(!d.iter().any(|x| x.code == "sql780"));
}

#[test]
fn sql781_array_length_on_object() {
  let d = diags(r#"SELECT jsonb_array_length('{"a":1}'::jsonb);"#);
  assert!(d.iter().any(|x| x.code == "sql781" && x.severity == Severity::Error));
}

#[test]
fn sql781_quiet_on_array_literal() {
  let d = diags("SELECT jsonb_array_length('[1,2,3]'::jsonb);");
  assert!(!d.iter().any(|x| x.code == "sql781"));
}

#[test]
fn sql782_minus_integer_on_object_literal() {
  let d = diags(r#"SELECT '{"a":1}'::jsonb - 0;"#);
  assert!(d.iter().any(|x| x.code == "sql782" && x.severity == Severity::Error));
}

#[test]
fn sql782_quiet_on_array_literal() {
  let d = diags("SELECT '[1,2,3]'::jsonb - 0;");
  assert!(!d.iter().any(|x| x.code == "sql782"));
}

#[test]
fn sql783_null_key() {
  let d = diags("SELECT jsonb_build_object(NULL, 1);");
  assert!(d.iter().any(|x| x.code == "sql783" && x.severity == Severity::Error));
}

#[test]
fn sql783_quiet_string_key() {
  let d = diags("SELECT jsonb_build_object('k', 1);");
  assert!(!d.iter().any(|x| x.code == "sql783"));
}
```

## Final step

```bash
cargo test --workspace --release
cargo clippy -p dsl-analysis --all-features --release -- -D warnings
git add dsl-analysis/src/rules/jsonb_path_exists_static_false.rs \
        dsl-analysis/src/rules/jsonb_array_length_on_object_literal.rs \
        dsl-analysis/src/rules/jsonb_minus_integer_on_object.rs \
        dsl-analysis/src/rules/jsonb_build_object_null_key.rs \
        dsl-analysis/src/rules/mod.rs \
        dsl-analysis/tests/rules_jsonb_depth.rs \
        dsl-analysis/docs/plans/2026-08-18-jsonpath-jsonb-depth-rules.md
git commit -m "feat(analysis): flag jsonpath and jsonb-literal operator misuse"
```
