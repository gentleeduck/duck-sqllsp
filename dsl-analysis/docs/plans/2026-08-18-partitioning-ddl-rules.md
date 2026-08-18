# Partitioning DDL rules (batch 1) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task (this session runs inline, not subagent-driven, per standing project preference against unsolicited subagent dispatch). Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add 6 new dsl-analysis lint rules (sql757-sql762) covering declarative table partitioning DDL mistakes, batch 1 of the SQL rule expansion plan.

**Architecture:** Each rule is a self-contained per-statement text/token scanner implementing the existing `LintRule` trait, exactly matching all 654 existing rules -- no new shared infrastructure. Rules reuse `clause_scan::{find_clause, split_top_level, parse_simple_ident}` and `crate::{stmt_body_upper, range_at}` rather than re-deriving byte-scanning primitives.

**Tech Stack:** Rust, `dsl-analysis` crate, existing `LintRule` trait, `text_size::TextRange`.

**Spec:** `dsl-analysis/docs/specs/2026-08-18-sql-rule-expansion-design.md` (batch 1 section; note sql760 changed during planning -- see Deviation note below).

## Global Constraints

- Every rule's `check()` must not panic on empty/malformed input (the engine already wraps rule execution in `catch_unwind`, but a rule that panics on every input for a whole statement loses coverage from every other rule on that statement -- write defensively, prefer early `return`/`?`/`else return` over `unwrap`).
- Severity per the design doc: sql757 Error, sql758 Error, sql759 Error, sql760 Warning (revised, see below), sql761 Error, sql762 Error.
- Commit message: `feat(analysis): flag partitioning DDL mistakes` (matches existing history style), one commit for the whole batch.
- `cargo test --workspace --release` and `cargo clippy --workspace --all-features --release -- -D warnings` must both be clean before commit.

## Deviation from spec: sql760

The spec's original sql760 (`attach_partition_no_for_values`) was verified during planning to be **unreachable dead code**: `ALTER TABLE t ATTACH PARTITION p;` with no `FOR VALUES`/`DEFAULT` is a hard pg_query grammar rejection (confirmed empirically -- `duck-sqllsp lint` on that exact input produces `sql000: syntax error at end of input`, so the statement never becomes a `Statement` for `LintRule::check` to see). Its replacement, `FOR VALUES IN (1, 2, 2, 3)` duplicate-literal detection, was also tried and found to be **already covered** by the existing `sql306 in_list_duplicates` rule (confirmed empirically -- it fires on partition bound `IN` lists too, since its scan is generic to any `IN (...)`).

Final sql760: **`partition_by_duplicate_column`** -- `PARTITION BY RANGE/LIST/HASH (a, a)`, the same column listed twice in the partition key. Verified empirically to parse cleanly with zero collision against any existing rule.

## Task 1: sql757 partition_by_no_key_in_pk

**Files:**
- Create: `dsl-analysis/src/rules/partition_by_no_key_in_pk.rs`
- Modify: `dsl-analysis/src/rules/mod.rs` (add `pub mod partition_by_no_key_in_pk;` and `Box::new(partition_by_no_key_in_pk::Rule),`)
- Test: `dsl-analysis/tests/rules_partitioning.rs` (new file, shared by all 6 tasks in this batch)

**Interfaces:**
- Consumes: `crate::stmt_body_upper` (start, body, upper-body), `crate::range_at`, `crate::clause_scan::{split_top_level, parse_simple_ident}`, `crate::{Diagnostic, LintRule, Severity}`.
- Produces: `partition_by_no_key_in_pk::Rule` registered under code `sql757`.

- [ ] **Step 1: Write the failing test**

Create `dsl-analysis/tests/rules_partitioning.rs` with this header (shared across all 6 tasks -- only write this file once, in Task 1; later tasks append to it) and the first test pair:

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
fn sql757_pk_missing_partition_column() {
  let d = diags(
    "CREATE TABLE t757a (id int, created_at date, PRIMARY KEY (id)) PARTITION BY RANGE (created_at);",
  );
  assert!(d.iter().any(|x| x.code == "sql757" && x.severity == Severity::Error));
}

#[test]
fn sql757_quiet_when_pk_covers_partition_column() {
  let d = diags(
    "CREATE TABLE t757b (id int, created_at date, PRIMARY KEY (id, created_at)) PARTITION BY RANGE (created_at);",
  );
  assert!(!d.iter().any(|x| x.code == "sql757"));
}

#[test]
fn sql757_quiet_when_not_partitioned() {
  let d = diags("CREATE TABLE t757c (id int, PRIMARY KEY (id));");
  assert!(!d.iter().any(|x| x.code == "sql757"));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p dsl-analysis --test rules_partitioning sql757 2>&1`
Expected: compile error (`partition_by_no_key_in_pk` module doesn't exist yet) or all three `sql757_*` tests fail/panic once it compiles as a no-op.

- [ ] **Step 3: Write minimal implementation**

```rust
//! sql757: a partitioned table's PRIMARY KEY does not include every
//! partition key column. PostgreSQL requires every unique constraint
//! (PRIMARY KEY included) on a partitioned table to cover all of the
//! table's partitioning columns -- CREATE TABLE fails with "unique
//! constraint on partitioned table must include all partitioning
//! columns" (0A000) otherwise. Only handles simple column-name
//! partition keys and a single table-level PRIMARY KEY (...) clause;
//! expression partition keys and UNIQUE constraints are out of scope
//! to avoid false positives.

use crate::clause_scan::{find_clause, parse_simple_ident, split_top_level};
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql757"
  }
  fn default_severity(&self) -> Severity {
    Severity::Error
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, body, upper) = crate::stmt_body_upper(stmt, source);
    if !upper.trim_start().starts_with("CREATE TABLE") {
      return;
    }
    let Some(part_cols) = partition_key_columns(&upper, body) else { return };
    if part_cols.is_empty() {
      return;
    }
    let Some((pk_start, pk_end, pk_cols)) = table_primary_key(&upper, body) else { return };
    let missing: Vec<&str> = part_cols
      .iter()
      .map(String::as_str)
      .filter(|c| !pk_cols.iter().any(|p| p.eq_ignore_ascii_case(c)))
      .collect();
    if missing.is_empty() {
      return;
    }
    out.push(Diagnostic {
      code: "sql757",
      severity: Severity::Error,
      message: format!(
        "PRIMARY KEY does not include partition key column(s) {} -- PostgreSQL requires every unique constraint on a partitioned table to include all partitioning columns",
        missing.join(", ")
      ),
      range: crate::range_at(start + pk_start, start + pk_end),
    });
  }
}

/// Find `PARTITION BY {RANGE|LIST|HASH} (col[, col...])` and return the
/// column names. `None` if not partitioned, or if any entry is not a
/// bare column reference (an expression key -- out of scope here).
fn partition_key_columns(upper: &str, body: &str) -> Option<Vec<String>> {
  let ub = upper.as_bytes();
  let kw = find_clause(ub, b"PARTITION BY")?;
  let mut i = skip_ws(ub, kw + "PARTITION BY".len());
  for strategy in ["RANGE", "LIST", "HASH"] {
    if upper[i..].starts_with(strategy) {
      i += strategy.len();
      break;
    }
  }
  i = skip_ws(ub, i);
  if ub.get(i) != Some(&b'(') {
    return None;
  }
  let close = match_paren(ub, i)?;
  let list = &body[i + 1..close];
  let mut cols = Vec::new();
  for (entry, _) in split_top_level(list) {
    let (_, name) = parse_simple_ident(entry)?;
    cols.push(name);
  }
  Some(cols)
}

/// Find a top-level `PRIMARY KEY ( col[, col...] )` table constraint
/// inside the CREATE TABLE column-def parens. Returns the constraint's
/// own byte span (for the diagnostic range) and its column names.
fn table_primary_key(upper: &str, body: &str) -> Option<(usize, usize, Vec<String>)> {
  let ub = upper.as_bytes();
  let open = ub.iter().position(|&b| b == b'(')?;
  let close_all = match_paren(ub, open)?;
  let list = &body[open + 1..close_all];
  let list_up = &upper[open + 1..close_all];
  for (entry, off) in split_top_level(list_up) {
    let trimmed = entry.trim_start();
    let lead_ws = entry.len() - trimmed.len();
    if !trimmed.starts_with("PRIMARY KEY") {
      continue;
    }
    let rest = &trimmed["PRIMARY KEY".len()..];
    let paren_rel = rest.find('(')?;
    let abs_open = open + 1 + off + lead_ws + "PRIMARY KEY".len() + paren_rel;
    let abs_close = match_paren(ub, abs_open)?;
    let cols_src = &body[abs_open + 1..abs_close];
    let mut cols = Vec::new();
    for (c, _) in split_top_level(cols_src) {
      if let Some((_, name)) = parse_simple_ident(c) {
        cols.push(name);
      }
    }
    return Some((open + 1 + off, abs_close + 1, cols));
  }
  None
}

fn match_paren(ub: &[u8], open: usize) -> Option<usize> {
  let mut depth = 0i32;
  let mut i = open;
  while i < ub.len() {
    match ub[i] {
      b'(' => depth += 1,
      b')' => {
        depth -= 1;
        if depth == 0 {
          return Some(i);
        }
      },
      b'\'' => {
        i += 1;
        while i < ub.len() && ub[i] != b'\'' {
          i += 1;
        }
      },
      _ => {},
    }
    i += 1;
  }
  None
}

fn skip_ws(ub: &[u8], mut i: usize) -> usize {
  while i < ub.len() && ub[i].is_ascii_whitespace() {
    i += 1;
  }
  i
}
```

Register in `dsl-analysis/src/rules/mod.rs`: add `pub mod partition_by_no_key_in_pk;` near the top with the other `pub mod` lines, and `Box::new(partition_by_no_key_in_pk::Rule),` as a new line inside the `all()` push list (append at the end, existing order is insertion-order not alphabetical).

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p dsl-analysis --test rules_partitioning sql757 2>&1`
Expected: PASS (3 tests: `sql757_pk_missing_partition_column`, `sql757_quiet_when_pk_covers_partition_column`, `sql757_quiet_when_not_partitioned`)

- [ ] **Step 5: Commit**

Do not commit yet -- this batch commits once at the end of Task 6 (Step 5 there covers the whole batch in one commit, matching the "one commit per batch" convention). Skip committing after each task; just move to Task 2.

## Task 2: sql758 partition_range_bound_reversed

**Files:**
- Create: `dsl-analysis/src/rules/partition_range_bound_reversed.rs`
- Modify: `dsl-analysis/src/rules/mod.rs`
- Test: append to `dsl-analysis/tests/rules_partitioning.rs`

**Interfaces:**
- Consumes: same as Task 1, plus none new.
- Produces: `partition_range_bound_reversed::Rule` registered under code `sql758`.

- [ ] **Step 1: Write the failing test**

Append to `dsl-analysis/tests/rules_partitioning.rs`:

```rust
#[test]
fn sql758_reversed_bound() {
  let d = diags("ALTER TABLE t758 ATTACH PARTITION t758_bad FOR VALUES FROM (100) TO (1);");
  assert!(d.iter().any(|x| x.code == "sql758" && x.severity == Severity::Error));
}

#[test]
fn sql758_quiet_when_ascending() {
  let d = diags("ALTER TABLE t758 ATTACH PARTITION t758_good FOR VALUES FROM (1) TO (100);");
  assert!(!d.iter().any(|x| x.code == "sql758"));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p dsl-analysis --test rules_partitioning sql758 2>&1`
Expected: FAIL (module doesn't exist / rule not registered).

- [ ] **Step 3: Write minimal implementation**

```rust
//! sql758: `FOR VALUES FROM (x) TO (y)` where the lower partition bound
//! is not strictly less than the upper bound. PostgreSQL rejects an
//! empty partition range at CREATE/ALTER TABLE time ("empty range
//! bound specified for partition"). Only fires on a single-column
//! bound where both sides are literals of the same simple kind (both
//! numeric, or both single-quoted strings) -- multi-column and
//! unbounded (MINVALUE/MAXVALUE) bounds are left alone.

use crate::clause_scan::find_clause;
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql758"
  }
  fn default_severity(&self) -> Severity {
    Severity::Error
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, body, upper) = crate::stmt_body_upper(stmt, source);
    let ub = upper.as_bytes();
    let Some(fv) = find_clause(ub, b"FOR VALUES FROM") else { return };
    let mut i = skip_ws(ub, fv + "FOR VALUES FROM".len());
    if ub.get(i) != Some(&b'(') {
      return;
    }
    let Some(from_close) = match_paren(ub, i) else { return };
    let Some(from_arg) = single_arg(body, i, from_close) else { return };
    let mut j = skip_ws(ub, from_close + 1);
    if !ub[j..].starts_with(b"TO") || ub.get(j + 2).is_some_and(|c| c.is_ascii_alphanumeric() || *c == b'_') {
      return;
    }
    j = skip_ws(ub, j + 2);
    if ub.get(j) != Some(&b'(') {
      return;
    }
    let Some(to_close) = match_paren(ub, j) else { return };
    let Some(to_arg) = single_arg(body, j, to_close) else { return };
    let reversed = match (Literal::parse(from_arg), Literal::parse(to_arg)) {
      (Some(Literal::Num(x)), Some(Literal::Num(y))) => x >= y,
      (Some(Literal::Str(x)), Some(Literal::Str(y))) => x >= y,
      _ => false,
    };
    if reversed {
      out.push(Diagnostic {
        code: "sql758",
        severity: Severity::Error,
        message: "partition lower bound is not less than the upper bound -- PostgreSQL rejects an empty partition range".into(),
        range: crate::range_at(start + fv, start + to_close + 1),
      });
    }
  }
}

enum Literal<'a> {
  Num(f64),
  Str(&'a str),
}

impl<'a> Literal<'a> {
  fn parse(s: &'a str) -> Option<Literal<'a>> {
    let t = s.trim();
    if t.is_empty() {
      return None;
    }
    if let Ok(n) = t.parse::<f64>() {
      return Some(Literal::Num(n));
    }
    if t.len() >= 2 && t.starts_with('\'') && t.ends_with('\'') {
      return Some(Literal::Str(&t[1..t.len() - 1]));
    }
    None
  }
}

/// The single top-level argument between `open+1` and `close`, or
/// `None` if there's a top-level comma (multi-column bound -- skip).
fn single_arg(body: &str, open: usize, close: usize) -> Option<&str> {
  let inner = body.as_bytes();
  let mut depth = 0i32;
  let mut k = open + 1;
  while k < close {
    match inner[k] {
      b'(' | b'[' => depth += 1,
      b')' | b']' => depth -= 1,
      b'\'' => {
        k += 1;
        while k < close && inner[k] != b'\'' {
          k += 1;
        }
      },
      b',' if depth == 0 => return None,
      _ => {},
    }
    k += 1;
  }
  Some(&body[open + 1..close])
}

fn match_paren(ub: &[u8], open: usize) -> Option<usize> {
  let mut depth = 0i32;
  let mut i = open;
  while i < ub.len() {
    match ub[i] {
      b'(' => depth += 1,
      b')' => {
        depth -= 1;
        if depth == 0 {
          return Some(i);
        }
      },
      b'\'' => {
        i += 1;
        while i < ub.len() && ub[i] != b'\'' {
          i += 1;
        }
      },
      _ => {},
    }
    i += 1;
  }
  None
}

fn skip_ws(ub: &[u8], mut i: usize) -> usize {
  while i < ub.len() && ub[i].is_ascii_whitespace() {
    i += 1;
  }
  i
}
```

Register in `mod.rs` same as Task 1.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p dsl-analysis --test rules_partitioning sql758 2>&1`
Expected: PASS (2 tests).

- [ ] **Step 5: Commit** -- skip, see Task 1 Step 5 note.

## Task 3: sql759 partition_by_expression_volatile

**Files:**
- Create: `dsl-analysis/src/rules/partition_by_expression_volatile.rs`
- Modify: `dsl-analysis/src/rules/mod.rs`
- Test: append to `dsl-analysis/tests/rules_partitioning.rs`

**Interfaces:**
- Consumes: same as Task 1.
- Produces: `partition_by_expression_volatile::Rule` registered under code `sql759`.

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn sql759_volatile_partition_expr() {
  let d = diags("CREATE TABLE t759a (id int, created_at timestamptz) PARTITION BY RANGE (now());");
  assert!(d.iter().any(|x| x.code == "sql759" && x.severity == Severity::Error));
}

#[test]
fn sql759_quiet_on_bare_column() {
  let d = diags("CREATE TABLE t759b (id int, created_at timestamptz) PARTITION BY RANGE (created_at);");
  assert!(!d.iter().any(|x| x.code == "sql759"));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p dsl-analysis --test rules_partitioning sql759 2>&1` -- expect FAIL.

- [ ] **Step 3: Write minimal implementation**

```rust
//! sql759: `PARTITION BY RANGE/LIST/HASH (some_volatile_fn(col))` --
//! PostgreSQL requires partition key expressions to be immutable and
//! rejects non-immutable functions ("functions in partition key
//! expression must be marked IMMUTABLE"). Flags the common cases where
//! the expression obviously calls a well-known volatile/stable
//! builtin; anything else is left alone (no catalog volatility lookup
//! available for user-defined functions).

use crate::clause_scan::{find_clause, split_top_level};
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

const VOLATILE_FNS: &[&str] =
  &["NOW", "CLOCK_TIMESTAMP", "STATEMENT_TIMESTAMP", "TRANSACTION_TIMESTAMP", "RANDOM", "GEN_RANDOM_UUID", "NEXTVAL"];
const VOLATILE_KEYWORDS: &[&str] = &["CURRENT_TIMESTAMP", "LOCALTIMESTAMP"];

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql759"
  }
  fn default_severity(&self) -> Severity {
    Severity::Error
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, _body, upper) = crate::stmt_body_upper(stmt, source);
    let ub = upper.as_bytes();
    let Some(kw) = find_clause(ub, b"PARTITION BY") else { return };
    let mut i = skip_ws(ub, kw + "PARTITION BY".len());
    for strategy in ["RANGE", "LIST", "HASH"] {
      if upper[i..].starts_with(strategy) {
        i += strategy.len();
        break;
      }
    }
    i = skip_ws(ub, i);
    if ub.get(i) != Some(&b'(') {
      return;
    }
    let Some(close) = match_paren(ub, i) else { return };
    let list = &upper[i + 1..close];
    for (entry, off) in split_top_level(list) {
      let trimmed = entry.trim_start();
      let lead = entry.len() - trimmed.len();
      let hit = VOLATILE_FNS
        .iter()
        .find(|f| trimmed.starts_with(**f) && trimmed[f.len()..].trim_start().starts_with('('))
        .or_else(|| VOLATILE_KEYWORDS.iter().find(|f| trimmed.starts_with(**f)));
      if let Some(fname) = hit {
        let abs = i + 1 + off + lead;
        out.push(Diagnostic {
          code: "sql759",
          severity: Severity::Error,
          message: format!(
            "partition key expression calls {fname}, which is not IMMUTABLE -- PostgreSQL rejects non-immutable partition key expressions"
          ),
          range: crate::range_at(start + abs, start + abs + fname.len()),
        });
      }
    }
  }
}

fn match_paren(ub: &[u8], open: usize) -> Option<usize> {
  let mut depth = 0i32;
  let mut i = open;
  while i < ub.len() {
    match ub[i] {
      b'(' => depth += 1,
      b')' => {
        depth -= 1;
        if depth == 0 {
          return Some(i);
        }
      },
      b'\'' => {
        i += 1;
        while i < ub.len() && ub[i] != b'\'' {
          i += 1;
        }
      },
      _ => {},
    }
    i += 1;
  }
  None
}

fn skip_ws(ub: &[u8], mut i: usize) -> usize {
  while i < ub.len() && ub[i].is_ascii_whitespace() {
    i += 1;
  }
  i
}
```

Register in `mod.rs` same as Task 1.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p dsl-analysis --test rules_partitioning sql759 2>&1` -- expect PASS (2 tests).

- [ ] **Step 5: Commit** -- skip, see Task 1 Step 5 note.

## Task 4: sql760 partition_by_duplicate_column

**Files:**
- Create: `dsl-analysis/src/rules/partition_by_duplicate_column.rs`
- Modify: `dsl-analysis/src/rules/mod.rs`
- Test: append to `dsl-analysis/tests/rules_partitioning.rs`

**Interfaces:**
- Consumes: same as Task 1.
- Produces: `partition_by_duplicate_column::Rule` registered under code `sql760`.

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn sql760_duplicate_partition_column() {
  let d = diags("CREATE TABLE t760a (a int, b int) PARTITION BY RANGE (a, a);");
  assert!(d.iter().any(|x| x.code == "sql760" && x.severity == Severity::Warning));
}

#[test]
fn sql760_quiet_when_distinct() {
  let d = diags("CREATE TABLE t760b (a int, b int) PARTITION BY RANGE (a, b);");
  assert!(!d.iter().any(|x| x.code == "sql760"));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p dsl-analysis --test rules_partitioning sql760 2>&1` -- expect FAIL.

- [ ] **Step 3: Write minimal implementation**

```rust
//! sql760: `PARTITION BY RANGE/LIST/HASH (a, a)` -- the same column
//! listed twice in the partition key. Always a copy-paste mistake; a
//! repeated column contributes nothing to the partitioning strategy.

use crate::clause_scan::{find_clause, parse_simple_ident, split_top_level};
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql760"
  }
  fn default_severity(&self) -> Severity {
    Severity::Warning
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, body, upper) = crate::stmt_body_upper(stmt, source);
    let ub = upper.as_bytes();
    let Some(kw) = find_clause(ub, b"PARTITION BY") else { return };
    let mut i = skip_ws(ub, kw + "PARTITION BY".len());
    for strategy in ["RANGE", "LIST", "HASH"] {
      if upper[i..].starts_with(strategy) {
        i += strategy.len();
        break;
      }
    }
    i = skip_ws(ub, i);
    if ub.get(i) != Some(&b'(') {
      return;
    }
    let Some(close) = match_paren(ub, i) else { return };
    let list = &body[i + 1..close];
    let mut seen: Vec<String> = Vec::new();
    for (entry, off) in split_top_level(list) {
      let Some((_, name)) = parse_simple_ident(entry) else { continue };
      if seen.iter().any(|s| s.eq_ignore_ascii_case(&name)) {
        let lead = entry.len() - entry.trim_start().len();
        let abs = i + 1 + off + lead;
        out.push(Diagnostic {
          code: "sql760",
          severity: Severity::Warning,
          message: format!("column `{name}` appears more than once in the partition key"),
          range: crate::range_at(start + abs, start + abs + name.len()),
        });
      } else {
        seen.push(name);
      }
    }
  }
}

fn match_paren(ub: &[u8], open: usize) -> Option<usize> {
  let mut depth = 0i32;
  let mut i = open;
  while i < ub.len() {
    match ub[i] {
      b'(' => depth += 1,
      b')' => {
        depth -= 1;
        if depth == 0 {
          return Some(i);
        }
      },
      b'\'' => {
        i += 1;
        while i < ub.len() && ub[i] != b'\'' {
          i += 1;
        }
      },
      _ => {},
    }
    i += 1;
  }
  None
}

fn skip_ws(ub: &[u8], mut i: usize) -> usize {
  while i < ub.len() && ub[i].is_ascii_whitespace() {
    i += 1;
  }
  i
}
```

Register in `mod.rs` same as Task 1.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p dsl-analysis --test rules_partitioning sql760 2>&1` -- expect PASS (2 tests).

- [ ] **Step 5: Commit** -- skip, see Task 1 Step 5 note.

## Task 5: sql761 detach_partition_concurrently_in_tx

**Files:**
- Create: `dsl-analysis/src/rules/detach_partition_concurrently_in_tx.rs`
- Modify: `dsl-analysis/src/rules/mod.rs`
- Test: append to `dsl-analysis/tests/rules_partitioning.rs`

**Interfaces:**
- Consumes: `crate::stmt_body_upper`, `crate::range_at`. Detection pattern mirrors the existing `drop_index_concurrently_in_tx.rs` exactly (scan `source[..start]` for a prior `BEGIN`/`START TRANSACTION`).
- Produces: `detach_partition_concurrently_in_tx::Rule` registered under code `sql761`.

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn sql761_detach_concurrently_in_tx() {
  let d = diags("BEGIN;\nALTER TABLE t761 DETACH PARTITION t761_bad CONCURRENTLY;\nCOMMIT;");
  assert!(d.iter().any(|x| x.code == "sql761" && x.severity == Severity::Error));
}

#[test]
fn sql761_quiet_outside_tx() {
  let d = diags("ALTER TABLE t761 DETACH PARTITION t761_good CONCURRENTLY;");
  assert!(!d.iter().any(|x| x.code == "sql761"));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p dsl-analysis --test rules_partitioning sql761 2>&1` -- expect FAIL.

- [ ] **Step 3: Write minimal implementation**

```rust
//! sql761: `ALTER TABLE ... DETACH PARTITION ... CONCURRENTLY` inside
//! an explicit transaction. Like `DROP INDEX CONCURRENTLY` (sql331),
//! the CONCURRENTLY detach variant cannot run inside a BEGIN/COMMIT
//! block -- PG raises 25001 at runtime. Flags when the same buffer
//! mixes a CONCURRENTLY detach with an earlier BEGIN.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql761"
  }
  fn default_severity(&self) -> Severity {
    Severity::Error
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, _body, upper) = crate::stmt_body_upper(stmt, source);
    let Some(rel) = upper.find("DETACH PARTITION") else { return };
    if !upper[rel..].contains("CONCURRENTLY") {
      return;
    }
    let prefix_upper = source[..start].to_ascii_uppercase();
    if !prefix_upper.contains("BEGIN") && !prefix_upper.contains("START TRANSACTION") {
      return;
    }
    let abs_s = start + rel;
    out.push(Diagnostic {
      code: "sql761",
      severity: Severity::Error,
      message: "DETACH PARTITION ... CONCURRENTLY cannot run inside a transaction (25001) -- move it out of BEGIN/COMMIT".into(),
      range: crate::range_at(abs_s, abs_s + "DETACH PARTITION".len()),
    });
  }
}
```

Register in `mod.rs` same as Task 1.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p dsl-analysis --test rules_partitioning sql761 2>&1` -- expect PASS (2 tests).

- [ ] **Step 5: Commit** -- skip, see Task 1 Step 5 note.

## Task 6: sql762 hash_partition_modulus_remainder

**Files:**
- Create: `dsl-analysis/src/rules/hash_partition_modulus_remainder.rs`
- Modify: `dsl-analysis/src/rules/mod.rs`
- Test: append to `dsl-analysis/tests/rules_partitioning.rs`

**Interfaces:**
- Consumes: `crate::stmt_body_upper`, `crate::range_at`.
- Produces: `hash_partition_modulus_remainder::Rule` registered under code `sql762`.

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn sql762_remainder_not_less_than_modulus() {
  let d = diags("ALTER TABLE t762 ATTACH PARTITION t762_bad FOR VALUES WITH (MODULUS 4, REMAINDER 4);");
  assert!(d.iter().any(|x| x.code == "sql762" && x.severity == Severity::Error));
}

#[test]
fn sql762_quiet_when_remainder_less_than_modulus() {
  let d = diags("ALTER TABLE t762 ATTACH PARTITION t762_good FOR VALUES WITH (MODULUS 4, REMAINDER 3);");
  assert!(!d.iter().any(|x| x.code == "sql762"));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p dsl-analysis --test rules_partitioning sql762 2>&1` -- expect FAIL.

- [ ] **Step 3: Write minimal implementation**

```rust
//! sql762: `FOR VALUES WITH (MODULUS m, REMAINDER r)` where the
//! remainder is not less than the modulus. PostgreSQL requires the
//! remainder to be in [0, modulus) and raises an error ("remainder for
//! hash partition must be less than modulus") otherwise.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql762"
  }
  fn default_severity(&self) -> Severity {
    Severity::Error
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, _body, upper) = crate::stmt_body_upper(stmt, source);
    let Some(mod_rel) = upper.find("MODULUS") else { return };
    let Some(modulus) = read_int_after(&upper, mod_rel + "MODULUS".len()) else { return };
    let Some(rem_rel_offset) = upper[mod_rel..].find("REMAINDER") else { return };
    let rem_rel = mod_rel + rem_rel_offset;
    let Some(remainder) = read_int_after(&upper, rem_rel + "REMAINDER".len()) else { return };
    if remainder >= modulus {
      out.push(Diagnostic {
        code: "sql762",
        severity: Severity::Error,
        message: format!("REMAINDER ({remainder}) must be less than MODULUS ({modulus}) for a hash partition"),
        range: crate::range_at(start + rem_rel, start + rem_rel + "REMAINDER".len()),
      });
    }
  }
}

/// Skip whitespace after `from`, then parse a run of ASCII digits.
fn read_int_after(upper: &str, from: usize) -> Option<i64> {
  let ub = upper.as_bytes();
  let mut i = from;
  while i < ub.len() && ub[i].is_ascii_whitespace() {
    i += 1;
  }
  let digit_start = i;
  while i < ub.len() && ub[i].is_ascii_digit() {
    i += 1;
  }
  if i == digit_start {
    return None;
  }
  upper[digit_start..i].parse().ok()
}
```

Register in `mod.rs` same as Task 1.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p dsl-analysis --test rules_partitioning sql762 2>&1` -- expect PASS (2 tests).

- [ ] **Step 5: Commit the whole batch**

```bash
cargo test --workspace --release
cargo clippy --workspace --all-features --release -- -D warnings
git add dsl-analysis/src/rules/partition_by_no_key_in_pk.rs \
        dsl-analysis/src/rules/partition_range_bound_reversed.rs \
        dsl-analysis/src/rules/partition_by_expression_volatile.rs \
        dsl-analysis/src/rules/partition_by_duplicate_column.rs \
        dsl-analysis/src/rules/detach_partition_concurrently_in_tx.rs \
        dsl-analysis/src/rules/hash_partition_modulus_remainder.rs \
        dsl-analysis/src/rules/mod.rs \
        dsl-analysis/tests/rules_partitioning.rs
git commit -m "feat(analysis): flag partitioning DDL mistakes"
```
