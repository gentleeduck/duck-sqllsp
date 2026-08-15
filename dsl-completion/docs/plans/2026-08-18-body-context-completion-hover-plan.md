# Body-context completion/hover consistency Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make completion and hover behave the same way inside every embedded SQL "body" construct (RLS policy expressions, CHECK constraints, GENERATED ALWAYS AS, PL/pgSQL bodies) as they already do in a plain top-level statement, and eliminate hover's per-call full-buffer re-parse.

**Architecture:** Five independent tasks, no task depends on another's output. Tasks 1-3 touch `dsl-completion` (two are new detectors reusing existing item-building helpers, one fixes an existing but incomplete phase-classification branch). Task 4 is a one-line `dsl-hover` fix. Task 5 threads a pre-parsed `ParsedFile` through `dsl-hover`'s reachable call graph, mirroring the `complete()`/`complete_with_derived()` split from the prior completion-engine-redesign project.

**Tech Stack:** Rust, the existing `dsl-completion`/`dsl-hover`/`dsl-server` crates in this workspace. No new dependencies.

**Spec:** `dsl-completion/docs/specs/2026-08-18-body-context-completion-hover-design.md`

## Global Constraints

- Do not use subagents (the Agent tool) -- work directly (user's standing global instruction).
- Never add a `Co-Authored-By` trailer to commits (user's standing global instruction).
- `cargo test --workspace --release` must show 0 failed and `cargo clippy --workspace --all-features --release -- -D warnings` must be clean after every task, before committing.
- One commit per task, following this session's established `feat(completion): ...` / `fix(hover): ...` message style -- no placeholder messages.
- Every new completion/hover case must be hand-verified against the real engine (a throwaway probe test, deleted before committing) before writing the permanent regression test for it -- this is the discipline that already caught, during this plan's own investigation, that the spec's original "extend the anchor-point mechanism" approach for Task 1/2 would not actually have worked (the anchored body has no leading clause keyword for the state machine to recognize), so those two tasks use direct item-building detectors instead. Trust that discipline over assumptions while implementing, including this plan's own code -- if a hand-probe shows something different than expected, stop and reconcile before writing the test.

---

### Task 1: Column-level CHECK / GENERATED ALWAYS AS in CREATE TABLE

**Files:**
- Modify: `dsl-completion/src/create_table.rs:353-372` (the `classify_entry` function's final match arm)
- Test: `dsl-completion/tests/engine.rs`

**Interfaces:**
- Consumes: `Phase::CtlCheckExpr { table: Option<String> }` (already exists, already correctly handled by `route_phase` in `dsl-completion/src/engine.rs` around line 1239 -- do not touch that handler, it already does the right thing: catalog column lookup via `sources::columns_of_table`, falling back to `crate::source_tables::buffer_column_names(source, t)` when the table isn't in the catalog yet, then `push_all_functions` + `sources::expression_keywords`).
- Produces: nothing new for later tasks (this task is self-contained).

**Root cause (verified by reading the code, not assumed):** `classify_entry`'s final match arm (on `committed_tokens.len()`) only distinguishes "0 tokens" (fresh, `CtlBodyStart`), "1 token" (`CtlExpectType`), and "2+ tokens" (`CtlExpectColumnConstraint`, unconditionally). Once past a column's name and type, ANY further typing -- including being deep inside an already-opened `CHECK (` or `GENERATED ALWAYS AS (` expression -- returns the same generic `CtlExpectColumnConstraint`, which offers constraint *keywords* (`NOT NULL`, `PRIMARY KEY`, ...) instead of the expression completion `CtlCheckExpr` already provides correctly for the separate, comma-delimited *table-level* `CHECK (...)` entry (that one is handled correctly a few lines earlier in the same function, at line 331-336 -- `column_check`/`generated_column` never reach that branch because they have no comma before them; they're part of the same entry as the column's name and type).

- [ ] **Step 1: Write the failing tests**

```rust
// In dsl-completion/tests/engine.rs, near the other CREATE TABLE tests.
#[test]
fn column_level_check_expression_offers_columns() {
  // `id int CHECK (id > <cursor>` -- column-level CHECK (no comma
  // before it, unlike the already-working table-level case), mid
  // expression. Should offer this table's columns, not column-def
  // constraint keywords.
  let cat = catalog_with_users_and_orders();
  let src = "CREATE TABLE users2 (id int, org_id int CHECK (org_id > ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(!labels.contains(&"NOT NULL"), "should not offer column-def keywords mid-expression; got {labels:?}");
}

#[test]
fn generated_always_as_offers_sibling_columns() {
  // GENERATED ALWAYS AS (<expr>) STORED -- the whole point is
  // referencing sibling columns; currently offers nothing useful.
  let cat = catalog_with_users_and_orders();
  let src = "CREATE TABLE t (id int, org_id int, full_name text GENERATED ALWAYS AS (";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"id") && labels.contains(&"org_id"), "expected sibling columns; got {labels:?}");
  assert!(!labels.contains(&"NOT NULL"), "should not offer column-def keywords; got {labels:?}");
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p dsl-completion --release column_level_check_expression_offers_columns generated_always_as_offers_sibling_columns -- --nocapture`
Expected: both FAIL (the first because `NOT NULL` is present when it shouldn't be; the second because `id`/`org_id` are absent).

- [ ] **Step 3: Hand-probe before implementing**

Write a throwaway `dsl-completion/tests/_scratch_probe_check.rs` using the `complete_at`-style harness (see any recent probe in this session's history for the exact shape: build a `Catalog`, `parse()`, `resolve_with_source()`, call `complete()`, print labels) against both source strings above, plus the *already-working* table-level fresh-slot case (`CREATE TABLE t (id int, org_id int, CHECK (`) to confirm it's still correct after the fix. Delete the probe file before committing Step 6.

- [ ] **Step 4: Implement the minimal fix**

In `dsl-completion/src/create_table.rs`, replace the final match arm of `classify_entry` (currently):

```rust
    _ => {
      let second = committed_tokens[1].as_str();
      if is_complete_type_token(second) || committed_tokens.len() > 2 {
        Phase::CtlExpectColumnConstraint
      } else {
        Phase::CtlExpectType
      }
    },
```

with:

```rust
    _ => {
      // Column-level CHECK (...) / GENERATED ALWAYS AS (...): same
      // "arbitrary expression over this table's columns" need the
      // table-level CHECK entry above already gets right, but a
      // column-level one (no comma before it -- it's a modifier
      // directly after the column's type, not its own list entry)
      // never reached that check. Whatever was typed after CHECK(/
      // GENERATED...AS( just fell into the generic column-constraint
      // keyword phase below regardless of being mid-expression.
      if inside_paren(committed)
        && (upper.contains("CHECK(")
          || upper.contains("CHECK (")
          || (upper.contains("GENERATED") && (upper.contains("AS(") || upper.contains("AS ("))))
      {
        return Phase::CtlCheckExpr { table: enclosing.map(str::to_string) };
      }
      let second = committed_tokens[1].as_str();
      if is_complete_type_token(second) || committed_tokens.len() > 2 {
        Phase::CtlExpectColumnConstraint
      } else {
        Phase::CtlExpectType
      }
    },
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test -p dsl-completion --release column_level_check_expression_offers_columns generated_always_as_offers_sibling_columns -- --nocapture`
Expected: both PASS. Then run the full suite: `cargo test -p dsl-completion --release` -- expect the same pass count as before this task plus 2, 0 failed (confirms no regression to the table-level CHECK case or anything else routed through `classify_entry`). Then `cargo clippy -p dsl-completion --all-features --release -- -D warnings` -- expect clean.

- [ ] **Step 6: Delete the scratch probe and commit**

```bash
rm dsl-completion/tests/_scratch_probe_check.rs
git add dsl-completion/src/create_table.rs dsl-completion/tests/engine.rs
git commit -m "$(cat <<'EOF'
fix(completion): fix column-level CHECK/GENERATED expression completion

classify_entry's final match arm only distinguished token counts (0/1/2+),
so any typing past a column's name+type -- including deep inside an
already-opened CHECK( or GENERATED ALWAYS AS( -- fell into the generic
CtlExpectColumnConstraint (constraint *keywords*: NOT NULL, PRIMARY
KEY, ...) instead of the CtlCheckExpr phase that already correctly
handles the separate, comma-delimited table-level CHECK entry.

Fixed by checking for an unclosed CHECK(/GENERATED...AS( before falling
through to the generic column-constraint classification. Reuses
CtlCheckExpr's existing, already-correct handler (catalog column
lookup falling back to buffer_column_names, then functions +
expression keywords) -- no changes to route_phase needed.

2 new tests in dsl-completion/tests/engine.rs.

Tests: cargo test -p dsl-completion --release green. cargo clippy -p
dsl-completion --all-features --release -- -D warnings clean.
EOF
)"
```

---

### Task 2: RLS policy expression completion

**Files:**
- Modify: `dsl-completion/src/detectors.rs` (add `policy_target_table` and `policy_expr_items`)
- Modify: `dsl-completion/src/engine.rs` (add `detect_policy_expr` wrapper, register in `POST_PHASE_DETECTORS`)
- Test: `dsl-completion/tests/engine.rs`

**Interfaces:**
- Consumes: `sources::columns_of_table(cat: &Catalog, schema: Option<&str>, name: &str, out: &mut Vec<Item>)`, `crate::source_tables::buffer_column_names(source: &str, table: &str) -> Vec<String>`, `push_all_functions(cat: &Catalog, out: &mut Vec<Item>)`, `sources::expression_keywords(out: &mut Vec<Item>)` -- all already exist and are already used together this exact way by `Phase::CtlCheckExpr`'s handler in `engine.rs` (~line 1239).
- Produces: `pub(crate) fn policy_target_table(source: &str) -> Option<String>` and `pub(crate) fn policy_expr_items(source: &str, offset: TextSize, cat: &Catalog) -> Option<Vec<Item>>` in `detectors.rs`, consumed by the new `detect_policy_expr` wrapper in `engine.rs`.

**Root cause (verified):** `CREATE POLICY` has no AST representation (`dsl_parse::StatementKind` only models `Select`/`Insert`/`Update`/`Delete`/`CreateTable`/`AlterTable`/`DropTable`; everything else -- including `CREATE POLICY` -- is `Unknown`), and it isn't routed through `create_index::detect`/`create_table::detect` either. `phase::detect`'s general tokenizer walk doesn't recognize the statement shape at all and falls back to something dumping ~2300 items dominated by irrelevant statement-start keywords -- confirmed via hand-probe during this plan's investigation, not assumed.

Note this diverges from the design spec's Section 1, which sketched extending `phase.rs`'s `subquery_body_start`/`create_view_body_start` anchor-point pattern for this case. That pattern only works when the anchored body starts with a keyword the tokenizer recognizes (`SELECT`/`INSERT`/.../`VALUES`) -- a `USING (org_id = ...)` body is a bare expression with no such keyword, so re-anchoring there would not reliably land on a WHERE-equivalent phase. This task instead adds a direct detector (matching the established pattern this session already used for `FILTER (`, `WITH ORDINALITY`, etc.) that builds the item list itself, using the exact same building blocks `CtlCheckExpr`'s handler already relies on -- verified correct via hand-probe before finalizing, per this plan's Global Constraints.

- [ ] **Step 1: Write the failing test**

```rust
// In dsl-completion/tests/engine.rs.
#[test]
fn rls_policy_using_expr_offers_target_table_columns() {
  let cat = catalog_with_users_and_orders();
  let src = "CREATE POLICY p ON users USING (";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"id"), "expected users' columns; got {} items, sample {:?}", labels.len(), &labels[..labels.len().min(10)]);
  assert!(labels.len() < 100, "menu should be scoped to this table + expression keywords, not the ~2300-item generic dump; got {}", labels.len());
}

#[test]
fn rls_policy_with_check_expr_offers_target_table_columns() {
  let cat = catalog_with_users_and_orders();
  let src = "CREATE POLICY p ON users WITH CHECK (";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"id"), "expected users' columns; got {labels:?}");
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p dsl-completion --release rls_policy_using_expr_offers_target_table_columns rls_policy_with_check_expr_offers_target_table_columns -- --nocapture`
Expected: both FAIL (menu size >= 100, dominated by irrelevant keywords per this plan's own investigation probe).

- [ ] **Step 3: Hand-probe before implementing**

Throwaway probe (delete before Step 7) covering: `USING (⏵`, `WITH CHECK (⏵`, a policy on a table not in the catalog at all (should get `None`/fall through gracefully, not panic), and a sanity check that a plain `WHERE` clause elsewhere in the same probe run is completely unaffected.

- [ ] **Step 4: Implement `policy_target_table` in `detectors.rs`**

Add near `trigger_target_table` (which this mirrors closely):

```rust
/// Find the target table of the `CREATE POLICY <name> ON <table>`
/// statement enclosing the cursor -- mirrors `trigger_target_table`'s
/// approach for `CREATE TRIGGER ... ON <table>`.
pub(crate) fn policy_target_table(source: &str) -> Option<String> {
  let upper = source.to_uppercase();
  let idx = upper.rfind("CREATE POLICY")?;
  let rest_upper = &upper[idx..];
  let on_idx = rest_upper.find(" ON ")?;
  let after = &source[idx + on_idx + 4..];
  let tok = after
    .trim_start()
    .split(|c: char| c.is_whitespace() || c == '(' || c == ';' || c == ',')
    .find(|s| !s.is_empty())?;
  let bare = tok.rsplit('.').next().unwrap_or(tok);
  Some(bare.to_string())
}
```

- [ ] **Step 5: Implement `policy_expr_items` in `detectors.rs`**

Add directly below `policy_target_table`:

```rust
/// `CREATE POLICY ... USING (⏵` or `... WITH CHECK (⏵` -- the policy's
/// boolean expression is scoped to the target table's columns, same
/// completion need `Phase::CtlCheckExpr`'s handler already serves for
/// CHECK constraints (see `engine.rs`'s `route_phase`, ~line 1239).
/// CREATE POLICY has no AST representation and isn't routed through
/// `create_table::detect`, so this is its own detector rather than
/// reusing either existing mechanism -- but it calls the exact same
/// column-lookup building blocks `CtlCheckExpr` does.
pub(crate) fn policy_expr_items(source: &str, offset: TextSize, cat: &Catalog) -> Option<Vec<Item>> {
  let (_, upper) = stmt_slice_upper(source, offset);
  if !upper.contains("POLICY") {
    return None;
  }
  let mut open_at = None;
  for kw in ["USING (", "WITH CHECK ("] {
    if let Some(rel) = upper.rfind(kw) {
      let candidate = rel + kw.len();
      let mut depth = 1i32;
      for b in upper[candidate..].bytes() {
        match b {
          b'(' => depth += 1,
          b')' => depth -= 1,
          _ => {},
        }
      }
      if depth >= 1 && open_at.map_or(true, |o: usize| candidate > o) {
        open_at = Some(candidate);
      }
    }
  }
  open_at?;
  let mut out = Vec::new();
  if let Some(t) = policy_target_table(source) {
    sources::columns_of_table(cat, None, &t, &mut out);
    if out.is_empty() {
      for name in crate::source_tables::buffer_column_names(source, &t) {
        out.push(crate::item::Item {
          label: name.clone(),
          kind: crate::item::ItemKind::Column,
          detail: Some(format!("column of `{t}` (buffer)")),
          description: None,
          documentation_md: None,
          insert_text: name,
          is_snippet: false,
          sort_priority: 0,
        });
      }
    }
  }
  push_all_functions(cat, &mut out);
  sources::expression_keywords(&mut out);
  Some(out)
}
```

- [ ] **Step 6: Wire into `POST_PHASE_DETECTORS` in `engine.rs`**

Add the wrapper near the other JSON_TABLE/FILTER-style detectors (e.g. right after `detect_filter_clause`):

```rust
/// `CREATE POLICY ... USING (⏵` / `... WITH CHECK (⏵` -- must beat
/// the generic phase's ~2300-item fallback dump the same way the
/// other slot-keyword shortcuts beat the generic menu for their
/// narrower contexts.
fn detect_policy_expr(
  source: &str,
  offset: TextSize,
  _file: &ParsedFile,
  _scopes: &[Scope],
  cat: &Catalog,
) -> Option<Vec<Item>> {
  policy_expr_items(source, offset, cat)
}
```

Add `detect_policy_expr` to the `POST_PHASE_DETECTORS` array (any position is safe -- it can't overlap with any other entry's trigger condition; add it right after `detect_filter_clause` for readability, grouping it with the other expression-context detectors).

- [ ] **Step 7: Run tests, delete the probe, commit**

`cargo test -p dsl-completion --release` -- expect prior count + 2, 0 failed. `cargo clippy -p dsl-completion --all-features --release -- -D warnings` -- clean. `rm dsl-completion/tests/_scratch_probe_policy.rs` (or whatever the Step 3 probe was named). Commit as `feat(completion): add RLS policy expression completion`, body explaining the root cause and the spec-vs-plan mechanism divergence noted in this task's Root Cause section above.

---

### Task 3: PL/pgSQL body FROM/WHERE targeted sub-detection

**Files:**
- Modify: `dsl-completion/src/detectors.rs` (add `plpgsql_inner_stmt_span` and `plpgsql_body_from_or_where_items`)
- Modify: `dsl-completion/src/engine.rs` (call the new function from `Phase::PlpgsqlBody`'s handler)
- Test: `dsl-completion/tests/engine.rs`

**Interfaces:**
- Consumes: `sources::tables(cat: &Catalog, out: &mut Vec<Item>)`, `push_scope_columns_or_all`, `push_aliases`, `push_all_functions`, `sources::expression_keywords` -- all already exist, all already used together by `Phase::WhereClause`'s handler.
- Produces: nothing new for later tasks.

**Root cause (verified):** `Phase::PlpgsqlBody`'s handler in `engine.rs` (~line 1283) is an unconditional kitchen sink -- PL/pgSQL locals, PL/pgSQL keywords, all aliases, all functions, `NEW`/`OLD`, all tables, all columns -- with no awareness of what SQL clause is currently being typed inside the body. Confirmed via hand-probe: `SELECT * FROM <cursor>` inside a `$$ ... $$` body offers `DECLARE`/`BEGIN`/`END`/`IF`/`LOOP` mixed in at a position where only table names make sense.

- [ ] **Step 1: Write the failing tests**

```rust
// In dsl-completion/tests/engine.rs.
#[test]
fn plpgsql_body_from_slot_offers_tables_not_kitchen_sink() {
  let cat = catalog_with_users_and_orders();
  let src = "CREATE FUNCTION f() RETURNS trigger AS $$ BEGIN SELECT id FROM ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"users"), "expected `users`; got {labels:?}");
  assert!(!labels.contains(&"BEGIN"), "FROM slot should not re-offer BEGIN; got {labels:?}");
}

#[test]
fn plpgsql_body_where_slot_offers_columns_not_kitchen_sink() {
  let cat = catalog_with_users_and_orders();
  let src = "CREATE FUNCTION f() RETURNS trigger AS $$ BEGIN SELECT id FROM users WHERE ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"id"), "expected `id`; got {labels:?}");
  assert!(!labels.contains(&"BEGIN"), "WHERE slot should not re-offer BEGIN; got {labels:?}");
}

#[test]
fn plpgsql_body_other_position_keeps_kitchen_sink() {
  // Not a FROM/WHERE slot -- the broad fallback (PL/pgSQL keywords
  // included) must still work, unchanged, for everything else.
  let cat = catalog_with_users_and_orders();
  let src = "CREATE FUNCTION f() RETURNS trigger AS $$ BEGIN ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"DECLARE") || labels.contains(&"IF"), "expected PL/pgSQL keywords still offered here; got {labels:?}");
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p dsl-completion --release plpgsql_body_from_slot_offers_tables_not_kitchen_sink plpgsql_body_where_slot_offers_columns_not_kitchen_sink -- --nocapture`
Expected: both FAIL (`BEGIN` present in both, matching this plan's own investigation probe). The third test (`plpgsql_body_other_position_keeps_kitchen_sink`) should already PASS before any change -- run it too to confirm the baseline.

- [ ] **Step 3: Hand-probe before implementing**

Throwaway probe covering both slots plus: a `WHERE` slot after `UPDATE`/`DELETE` (not just `SELECT`), a second inner statement after a prior `;` inside the same body (confirms the span-finding boundary logic doesn't leak across inner statement boundaries), and the `plpgsql_body_other_position_keeps_kitchen_sink` case. Delete before Step 7.

- [ ] **Step 4: Implement `plpgsql_inner_stmt_span` and `plpgsql_body_from_or_where_items` in `detectors.rs`**

```rust
/// Text since the nearest PL/pgSQL statement/block boundary before
/// `pos` -- the last `;`, or the last occurrence of a block keyword
/// (BEGIN/THEN/ELSE/LOOP/DECLARE), whichever is closer to `pos`.
/// Scoped to `source[..pos]` only.
fn plpgsql_inner_stmt_span(source: &str, pos: usize) -> &str {
  let before = &source[..pos];
  let upper = before.to_ascii_uppercase();
  let mut boundary = upper.rfind(';').map(|p| p + 1).unwrap_or(0);
  for kw in ["BEGIN", "THEN", "ELSE", "LOOP", "DECLARE"] {
    if let Some(p) = upper.rfind(kw) {
      let after = p + kw.len();
      if after > boundary {
        boundary = after;
      }
    }
  }
  &source[boundary..pos]
}

/// `Phase::PlpgsqlBody` sub-detection: the current inner PL/pgSQL
/// statement is itself a SELECT/UPDATE/DELETE at its FROM or WHERE
/// slot, so offer the same targeted menu the top-level phase would --
/// tables at FROM/JOIN, columns + expression keywords at WHERE/AND/OR
/// -- instead of the kitchen-sink fallback. Returns `None` (falls
/// through to the kitchen sink) for every other position -- this only
/// narrows the two clearest, highest-value slots, it doesn't attempt
/// full clause-by-clause parity with top-level SQL.
pub(crate) fn plpgsql_body_from_or_where_items(
  source: &str,
  offset: TextSize,
  file: &ParsedFile,
  scopes: &[Scope],
  cat: &Catalog,
) -> Option<Vec<Item>> {
  let pos: usize = u32::from(offset) as usize;
  let pos = pos.min(source.len());
  let span = plpgsql_inner_stmt_span(source, pos);
  let upper = span.to_ascii_uppercase();
  let has_dml_verb = ["SELECT", "UPDATE", "DELETE"].iter().any(|kw| upper.contains(kw));
  if !has_dml_verb {
    return None;
  }
  let trimmed = upper.trim_end();
  let mut words: Vec<&str> = trimmed.split_ascii_whitespace().collect();
  // Strip a trailing partial identifier the user is still typing.
  if pos > 0 && source.as_bytes().get(pos - 1).is_some_and(|b| b.is_ascii_alphanumeric() || *b == b'_') {
    words.pop();
  }
  let last = words.last().copied();
  let mut out = Vec::new();
  if matches!(last, Some("FROM") | Some("JOIN")) {
    sources::tables(cat, &mut out);
    return Some(out);
  }
  if matches!(last, Some("WHERE") | Some("AND") | Some("OR")) {
    push_scope_columns_or_all(file, scopes, source, cat, offset, &mut out);
    push_aliases(file, scopes, source, offset, &mut out);
    push_all_functions(cat, &mut out);
    sources::expression_keywords(&mut out);
    return Some(out);
  }
  None
}
```

- [ ] **Step 5: Call the new function from `Phase::PlpgsqlBody`'s handler in `engine.rs`**

Change (~line 1283):

```rust
    Phase::PlpgsqlBody => {
      // Function parameters and DECLARE'd locals first so they
      // sort above the broader keyword / function lists.
      let locals = crate::plpgsql_locals::extract(source, u32::from(offset) as usize);
      crate::plpgsql_locals::push_items(&locals, &mut out);
      // PL/pgSQL flow keywords + standard built-ins + NEW / OLD
      // identifiers + any FROM/JOIN aliases inside the body.
      sources::plpgsql_keywords(&mut out);
      push_aliases(file, scopes, source, offset, &mut out);
      push_all_functions(cat, &mut out);
      sources::new_old_aliases(&mut out);
      sources::tables(cat, &mut out);
      sources::columns(cat, &mut out);
    },
```

to:

```rust
    Phase::PlpgsqlBody => {
      if let Some(items) = plpgsql_body_from_or_where_items(source, offset, file, scopes, cat) {
        out = items;
      } else {
        // Function parameters and DECLARE'd locals first so they
        // sort above the broader keyword / function lists.
        let locals = crate::plpgsql_locals::extract(source, u32::from(offset) as usize);
        crate::plpgsql_locals::push_items(&locals, &mut out);
        // PL/pgSQL flow keywords + standard built-ins + NEW / OLD
        // identifiers + any FROM/JOIN aliases inside the body.
        sources::plpgsql_keywords(&mut out);
        push_aliases(file, scopes, source, offset, &mut out);
        push_all_functions(cat, &mut out);
        sources::new_old_aliases(&mut out);
        sources::tables(cat, &mut out);
        sources::columns(cat, &mut out);
      }
    },
```

- [ ] **Step 6: Run tests to verify they pass**

`cargo test -p dsl-completion --release` -- expect prior count + 3, 0 failed. `cargo clippy -p dsl-completion --all-features --release -- -D warnings` -- clean.

- [ ] **Step 7: Delete the probe and commit**

Commit as `feat(completion): add PL/pgSQL body FROM/WHERE sub-detection`, explaining the kitchen-sink root cause and the deliberate narrow scope (FROM/WHERE only, not full clause parity -- matches the design spec's Section 2 and the "targeted sub-detection" approach chosen during brainstorming over full recursive re-entry into the top-level tokenizer).

---

### Task 4: Hover CREATE POLICY role/column disambiguation

**Files:**
- Modify: `dsl-hover/src/lib.rs:1540` (the `near_role_slot` keyword list)
- Test: `dsl-hover/tests/role.rs` (existing file -- confirm it covers this shape of test already; add to it)

**Interfaces:** None -- self-contained one-line change.

**Root cause (verified):** `near_role_slot`'s trigger-keyword list includes the bare word `"POLICY"`, so any identifier within 60 characters of that word anywhere before it is treated as a role reference -- including identifiers inside `USING (...)`/`WITH CHECK (...)` expressions that have nothing to do with a role. `" TO "`, also in the same list, already correctly catches the real role slot (`... TO <role>`).

- [ ] **Step 1: Write the failing test**

```rust
// In dsl-hover/tests/role.rs.
#[test]
fn policy_using_expr_column_not_misidentified_as_role() {
  let c = cat(); // reuse this file's existing catalog helper -- see
                 // the top of dsl-hover/tests/role.rs for its exact
                 // shape and column set before assuming names.
  let src = "CREATE POLICY p ON users USING (org_id = 1);";
  let cur = src.find("org_id").unwrap() + 3;
  let result = hover_with(src, TextSize::from(cur as u32), &c, KeywordCase::Upper);
  let text = result.expect("hover result");
  assert!(!text.contains("_role_"), "org_id should not be identified as a role; got: {text}");
}
```

Read `dsl-hover/tests/role.rs` in full first to confirm its existing `cat()` helper's exact table/column names and its exact imports (`hover_with`, `KeywordCase`, `TextSize` -- match whatever it already imports rather than assuming) -- adjust the test above to use whatever column name that file's fixture actually defines if it differs from `org_id`.

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p dsl-hover --release policy_using_expr_column_not_misidentified_as_role -- --nocapture`
Expected: FAIL (`text.contains("_role_")` is true).

- [ ] **Step 3: Implement the fix**

In `dsl-hover/src/lib.rs`, in `near_role_slot`'s keyword list (line 1540):

```rust
  for kw in ["OWNER TO", "GRANT", "REVOKE", "SET ROLE", "RESET ROLE", "POLICY", " TO "] {
```

remove `"POLICY"`:

```rust
  for kw in ["OWNER TO", "GRANT", "REVOKE", "SET ROLE", "RESET ROLE", " TO "] {
```

- [ ] **Step 4: Run test to verify it passes, check for regressions**

Run: `cargo test -p dsl-hover --release policy_using_expr_column_not_misidentified_as_role -- --nocapture` -- expect PASS.

Then hand-verify (throwaway probe, not a permanent test -- this is a true role slot that must keep working) that `CREATE POLICY p ON users FOR ALL TO admin_role` still correctly identifies `admin_role` as a role when hovered -- this is caught by the `" TO "` entry, unaffected by removing `"POLICY"`, but verify it directly since `near_role_slot`'s existing test coverage may not include a POLICY-specific `TO` case.

Run the full suite: `cargo test -p dsl-hover --release` -- expect prior count + 1, 0 failed. `cargo clippy -p dsl-hover --all-features --release -- -D warnings` -- clean.

- [ ] **Step 5: Commit**

```bash
git add dsl-hover/src/lib.rs dsl-hover/tests/role.rs
git commit -m "$(cat <<'EOF'
fix(hover): stop misidentifying CREATE POLICY expression columns as roles

near_role_slot's trigger-keyword list included the bare word "POLICY",
so any identifier within 60 characters of that word anywhere before it
was treated as a role reference -- including identifiers inside
USING (...)/WITH CHECK (...) expressions that have nothing to do with
a role. " TO ", already in the same list, correctly catches the real
role slot (... TO <role>) on its own.

1 new test in dsl-hover/tests/role.rs.

Tests: cargo test -p dsl-hover --release green. cargo clippy -p
dsl-hover --all-features --release -- -D warnings clean.
EOF
)"
```

---

### Task 5: Eliminate hover's per-call full-buffer re-parse

**Files:**
- Modify: `dsl-hover/src/lib.rs` (7 functions: `hover_with` plus 6 helpers reachable from it that each independently re-parse today)
- Modify: `dsl-server/src/handlers/hover.rs`
- Test: `dsl-hover`'s existing test suite (regression only -- this task changes performance characteristics, not behavior, so no new test *assertions*; the perf improvement itself is verified via a benchmark, see Step 6)

**Interfaces:**
- Consumes: `Document::parsed()` (existing, in `dsl-server/src/documents.rs`) for its `.file: ParsedFile`.
- Produces: `pub fn hover_with_parsed(source: &str, offset: TextSize, file: &dsl_parse::ParsedFile, catalog: &Catalog, case: KeywordCase) -> Option<String>` -- the new public entry point `dsl-server`'s handler switches to. `hover_with`'s existing public signature and behavior are unchanged (thin wrapper: parses, delegates) so nothing else calls it differently.

**Root cause (verified by tracing hover_with's full reachable call graph, not just its own top-level parse call):** `hover_with` itself parses once (line 99, feeding `ddl::column_decl_at`). Depending on what kind of token is under the cursor, it can also reach `star_hover` (which calls `qualified_star_hover` or `unqualified_star_hover`, each independently parsing), `scope_column_lookup`, `alias_lookup` (which calls `resolve_alias_in`, which parses), `enclosing_table_column`, and `scoped_column_in_text` -- six more independent `dsl_parse::parse` call sites, each reachable from a single `hover_with()` call depending on cursor context. This is why the fix touches 7 functions, not 1 -- every one of these needs the pre-parsed file threaded through, or the redundant re-parse just moves to whichever of them the cursor position happens to reach.

- [ ] **Step 1: Confirm the current call graph is unchanged since this plan's investigation**

Run these and confirm the line numbers / function set still match before editing anything (this file may have shifted slightly; re-verify rather than trusting the numbers below blindly):

```bash
grep -n "dsl_parse::parse" dsl-hover/src/lib.rs
```

Expect 7 matches: one inside `hover_with` itself, and one each inside `qualified_star_hover`, `unqualified_star_hover`, `enclosing_table_column`, `scope_column_lookup`, `resolve_alias_in`, `scoped_column_in_text`.

- [ ] **Step 2: Change the 6 leaf helper functions' signatures**

For each of `qualified_star_hover`, `unqualified_star_hover`, `enclosing_table_column`, `scope_column_lookup`, `resolve_alias_in`, `scoped_column_in_text`: add a `file: &dsl_parse::ParsedFile` parameter (position it right after `offset: TextSize` in each signature, matching where `dsl-completion`'s equivalent functions place it, for consistency across the workspace), and replace that function's own `let parsed = dsl_parse::parse(source, dsl_parse::Dialect::Postgres);` (or `dsl_parse::parse(src, ...)` for `resolve_alias_in`, which uses `src` as its parameter name) with using `file` directly wherever `parsed` was used afterward (e.g. `let scopes = dsl_resolve::resolve(&file.statements);` instead of `dsl_resolve::resolve(&parsed.statements)` -- keep every other line identical, this is a pure substitution of the parse source, not a logic change).

Do not change what each function does with the parsed file otherwise -- this step is a mechanical rename plus deleting one line per function, not a logic change. `cargo build -p dsl-hover` after this step will show "cannot find value `parsed`" errors at every call site that hasn't been updated yet -- that's expected and is the compiler-driven verification for Step 3 (the same technique used for the 189-function detector extraction earlier in the completion-engine-redesign project): don't hand-trace every call site, let the compiler enumerate them.

- [ ] **Step 3: Fix every call site using the compiler's errors**

Run `cargo build -p dsl-hover 2>&1 | grep "error\[" -A 3` and fix each reported call site by threading `file` through: `star_hover` needs a new `file` parameter too (it calls `qualified_star_hover`/`unqualified_star_hover`, both now requiring it) and its own call site inside `hover_with` needs updating; `alias_lookup` needs the same treatment (it calls `resolve_alias_in`). Repeat `cargo build -p dsl-hover` until it succeeds with zero errors -- do not guess at remaining call sites, let each build attempt reveal the next one.

- [ ] **Step 4: Split `hover_with` into a thin wrapper plus `hover_with_parsed`**

Rename the current `hover_with` function to `hover_with_parsed`, add a `file: &dsl_parse::ParsedFile` parameter (same position convention as Step 2), delete its own internal `let parsed = dsl_parse::parse(source, dsl_parse::Dialect::Postgres);` line, and replace every use of `parsed` in its body with `file` (this includes the call at former-line-100, `ddl::column_decl_at(&parsed, source, offset)` -> `ddl::column_decl_at(file, source, offset)`, and every downstream call this task's Steps 2-3 updated to take `file` -- pass this same `file` through to all of them).

Add a new, small `hover_with` that preserves the exact existing public signature and behavior:

```rust
pub fn hover_with(source: &str, offset: TextSize, catalog: &Catalog, case: KeywordCase) -> Option<String> {
  let file = dsl_parse::parse(source, dsl_parse::Dialect::Postgres);
  hover_with_parsed(source, offset, &file, catalog, case)
}
```

- [ ] **Step 5: Verify no behavior change**

`cargo build -p dsl-hover` -- clean. `cargo test -p dsl-hover --release` -- expect the *exact same* test count and 0 failed as before this task (this is pure code motion: `hover_with`'s existing callers, including every existing test in this crate, are unaffected since its signature and behavior are unchanged). `cargo clippy -p dsl-hover --all-features --release -- -D warnings` -- clean.

- [ ] **Step 6: Wire `dsl-server`'s hover handler to the cached parse**

In `dsl-server/src/handlers/hover.rs`, change:

```rust
  let md = hover_with(&doc.text, offset, &cat, case)?;
```

to:

```rust
  let md = dsl_hover::hover_with_parsed(&doc.text, offset, &cache.file, &cat, case)?;
```

(`cache` is already bound earlier in this function from `doc.parsed()` -- confirm the exact existing variable name by reading the current file before editing; it was `cache` as of this plan's investigation, holding a `ParseCache` with a `.file: ParsedFile` field, matching `dsl-completion`'s equivalent `Document::parsed()`/`ParseCache` pattern from the prior project). Remove the now-unused `use dsl_hover::{KeywordCase, hover_with};` import's `hover_with` if it's no longer referenced elsewhere in the file (check with `grep -n "hover_with\b" dsl-server/src/handlers/hover.rs` after the edit).

- [ ] **Step 7: Benchmark before/after, matching the completion project's Phase D methodology**

Add a benchmark to `dsl-server/tests/handlers_unit.rs` (the same file that already has `r5_201_perf_derived_catalog_cache_avoids_redundant_rescans` from the prior project -- follow its exact `#[test] #[ignore]` + `state_with` + `std::time::Instant` shape) that opens a large document (10,000 statements, matching the prior project's `n = 10_000` convention) and times `hover::run` for a realistic hover position (e.g. a bare column reference in a `WHERE` clause), printing the result the same way the existing benchmark does. Run it:

```bash
cargo test --release -p dsl-server --test handlers_unit <new_test_name> -- --ignored --nocapture
```

Record the number. It does not need to hit any specific target (there is no stated hover perf target in the spec, unlike completion's < 5ms) -- the goal is a clear, honest before/after comparison in the commit message. Since this task's fix is unconditional (every hover call through `dsl-server` now uses the cached parse instead of re-parsing), the "before" number can be reconstructed from this same benchmark's git history if needed, but simplest is to temporarily stash this task's changes (`git stash`), run the same benchmark once for the "before" number, then `git stash pop` and record both numbers in the commit message.

- [ ] **Step 8: Full workspace verification and commit**

```bash
cargo build --workspace
cargo test --workspace --release
cargo clippy --workspace --all-features --release -- -D warnings
```

All clean, 0 failed, identical test counts to before this task (except the 1 new ignored benchmark). Commit as `perf(hover): eliminate per-call full-buffer re-parse`, with the before/after benchmark numbers from Step 7 in the body, and an explanation of why 7 functions needed changing (the call-graph finding from this task's Root Cause section) rather than just `hover_with` itself.

---

## Self-review notes (completed during plan writing, not a separate pass)

- **Spec coverage:** Section 1 (RLS/CHECK/GENERATED) -> Tasks 1-2 (split because investigation found CHECK/GENERATED's bug is in `create_table.rs`'s existing dispatch, a different mechanism than RLS's "no dispatch exists at all" problem -- the spec's single "expression-anchor extension" framing undersold this split, corrected here). Section 2 (PL/pgSQL) -> Task 3. Section 3 (hover POLICY) -> Task 4. Section 4 (hover perf) -> Task 5, scope corrected from "mirror `derived_catalog`, thin wrapper" to "thin wrapper *plus* six leaf functions" after tracing the actual call graph -- the spec's framing undersold this one too.
- **Placeholder scan:** none found -- every step has literal, complete code or an exact mechanical instruction (Task 5 Steps 2-3's "repeat for each function" is mechanical and compiler-verified, not vague, matching the precedent Task 5's own text cites).
- **Type consistency:** `Item`/`ItemKind` used consistently with their existing `dsl-completion` paths (`crate::item::Item`, `crate::item::ItemKind::X`) throughout Tasks 1-3, matching every other detector added this session. `hover_with_parsed`'s signature in Task 5 matches the parameter order/style `dsl-completion`'s `complete_with_derived` established (source, offset, then the pre-computed data, then catalog/case) for consistency across the workspace.
