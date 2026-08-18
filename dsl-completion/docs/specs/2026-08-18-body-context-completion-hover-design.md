# Body-context completion/hover consistency

**Branch:** `feat/body-context-completion` (based on `feat/completion-engine-redesign`,
not yet merged to master -- inherits its perf/coverage/registry work).

## Motivation

Completion and hover should work the same way inside every kind of
"embedded body" construct in SQL -- RLS policy expressions, `CHECK`
constraints, `GENERATED ALWAYS AS` expressions, PL/pgSQL function/
trigger bodies -- as they do in a plain top-level statement. They
currently don't, consistently. Confirmed empirically (hand-probed
against the real engines before writing this spec, not assumed):

**Completion, confirmed broken:**
- `CREATE POLICY ... USING (⏵)` / `... WITH CHECK (⏵)` -- the menu
  balloons to ~2300 items dominated by irrelevant top-level statement
  keywords instead of a clean column/expression menu.
- `GENERATED ALWAYS AS (⏵)` -- offers column-*definition* keywords
  (`NOT NULL`, `PRIMARY KEY`...) instead of the sibling-column
  references that are the entire point of a generated column.
- Mid-expression `CHECK (col > ⏵)` -- same wrong keyword set once
  past the opening paren (the fresh-slot case, `CHECK (⏵)`, already
  works correctly).
- Inside PL/pgSQL bodies (`$$ ... $$`) -- `Phase::PlpgsqlBody` is a
  deliberate kitchen-sink fallback (locals + PL/pgSQL keywords + all
  tables + all columns + all functions, unconditionally), not aware
  of the SQL statement currently being typed inside the body. Right
  columns often show up, buried in a confusing mix with `DECLARE`/
  `BEGIN`/`END`/`IF`/`LOOP` re-suggested at positions where they don't
  apply (e.g. offering `BEGIN` again at a `FROM ⏵` slot).

**Completion, confirmed already correct (not touched by this work):**
fresh-slot `CHECK (⏵)`, `DEFAULT` expressions, `NEW.`/`OLD.`
trigger-body resolution when a `CREATE TRIGGER` links the function to
a table.

**Hover, confirmed broken:** exactly one case --
`CREATE POLICY ... USING (org_id ⏵)` hovering a bare column reference
shows a role card ("`org_id` -- _role_ -- catalog not loaded, cannot
verify") instead of the column's info. Root cause:
`near_role_slot` in `dsl-hover/src/lib.rs` triggers on the bare
keyword `"POLICY"` appearing anywhere in the 60 characters before the
cursor, not specifically the `TO <role>` slot -- a `USING (⏵)`
expression close enough to the word `POLICY` false-triggers it.

**Hover, confirmed already correct (not touched by this work):**
`CHECK`, `GENERATED ALWAYS AS`, trigger `NEW.`/`OLD.`, and bare
column references inside PL/pgSQL bodies all already resolve
correctly. Hover doesn't share completion's phase-awareness problem
because it's token-lookup based (what *is* this token) rather than
next-clause-prediction based (what comes *next*) -- it doesn't need to
know what clause it's in to answer "what is this identifier."

**Separately bundled into this same effort (lower risk, already fully
diagnosed during the prior project, explicitly deferred there):**
`dsl_hover::hover_with` re-parses the whole buffer via `dsl_parse::parse`
on *every* call, ignoring `dsl-server`'s `Document::parse_cache`
entirely -- measured ~270ms/call at 10k statements during the
completion-engine-redesign project's Phase D benchmarking, worse than
everything else measured in that project combined.

**Explicitly out of scope for this spec** (per user-confirmed
sequencing): further open-ended "harden completion, add missing
things" auditing, and any `dsl-resolve` hardening. Those are real but
unscoped -- a future effort, not bundled here.

## Design

### 1. Completion: expression-anchor extension (RLS / CHECK / GENERATED)

`dsl-completion/src/phase.rs` already has a proven pattern for
exactly this problem: `subquery_body_start` and `create_view_body_start`
detect specific statement prefixes and re-anchor the tokenizer at the
construct's body, so the *existing* state machine treats it as a
fresh statement (e.g. `CREATE VIEW ... AS ⏵` gets walked as if it
were a bare `SELECT`). This is why fresh-slot `CHECK (⏵)` and
`DEFAULT` already work -- they're either already covered or don't
need column references at all.

Add new anchor functions, called from the same place
`subquery_body_start`/`create_view_body_start` already are in
`phase::detect`:
- `policy_expr_body_start` -- detects `CREATE POLICY ... USING (` or
  `... WITH CHECK (` and anchors at the paren, re-running the walker
  as if the content were a `WHERE (⏵`.
- `check_constraint_body_start` -- detects `CHECK (` (both
  column-level, inside a column def, and table-level, as its own
  list entry) and anchors the same way. Must not fire on the
  *fresh* slot (`CHECK (⏵)` with nothing typed) -- that already
  works via a different path and must keep working unchanged.
- `generated_column_body_start` -- detects `GENERATED ALWAYS AS (`
  and anchors the same way.

Each of these re-uses the *existing* WHERE-clause phase entirely --
no new item-building logic, just new entry points into logic that's
already correct. Scope columns come from the enclosing `CREATE TABLE`
(for `CHECK`/`GENERATED`) or the policy's `ON <table>` (for RLS) via
the same table-resolution helpers `detect_dot_context` and friends
already use.

### 2. Completion: PL/pgSQL body targeted sub-detection

`Phase::PlpgsqlBody`'s handler (in `engine.rs`'s `route_phase` match)
gets new checks *before* its existing kitchen-sink fallback, matching
the shape of existing slot-detectors like `filter_clause_next_keyword`:
- Text since the last `;` (or `BEGIN`) inside the body ends with
  `FROM `/`JOIN ` -> offer tables (same as the top-level FROM phase).
- Ends with `WHERE `/`AND `/`OR ` *and* the statement started with a
  recognized DML verb (`SELECT`/`UPDATE`/`DELETE`) since that
  boundary -> offer columns + expression keywords (same as the
  top-level WHERE phase).
- Anything else -> unchanged kitchen-sink fallback. This narrows
  precision for the cases above; it does not remove the safety net
  for everything else (`RAISE`, `PERFORM`, `EXECUTE format(...)`,
  assignment, control flow, ...).

Deliberately *not* full recursive re-entry into the top-level
tokenizer (the alternative approach discussed and declined during
brainstorming) -- narrower, lower risk, doesn't touch the core
dollar-quote routing path every completion call goes through.

### 3. Hover: CREATE POLICY role/column disambiguation

One-line fix: remove the bare `"POLICY"` entry from
`near_role_slot`'s trigger-keyword list in `dsl-hover/src/lib.rs`.
The `" TO "` entry in the same list already correctly catches the
real role slot (`... TO <role>`); `"POLICY"` alone was over-broad,
firing for any identifier within 60 characters of the word appearing
*anywhere* before it, including inside `USING`/`WITH CHECK`
expressions that have nothing to do with a role.

### 4. Hover: eliminate the re-parse-per-call cost

Mirror the `Document::derived_catalog` pattern the completion project
already established in `dsl-server/src/documents.rs`: thread the
already-parsed `ParsedFile` (from `Document::parsed()`'s existing
`ParseCache`) through `hover_with` instead of it calling
`dsl_parse::parse(source, ...)` internally on every invocation.

Same shape as the `complete()` / `complete_with_derived()` split from
the completion project: `hover_with`'s public signature and behavior
stay unchanged (so its own test suite and `dsl-cli` usage, if any,
are unaffected) by keeping it as a thin wrapper that parses internally
and delegates to a new `hover_with_parsed(source, offset, file, catalog, case)`
that accepts the pre-parsed file. `dsl-server/src/handlers/hover.rs`
switches to calling the new function with `doc.parsed().file`.

## Testing

Same discipline as every batch in the completion-engine-redesign
project: hand-probe each new case against the real engine (via a
throwaway test harness, deleted before committing) before writing
permanent tests -- this caught, during brainstorming for this very
spec, that several suspected gaps (trigger `NEW.` resolution, hover
inside PL/pgSQL bodies) were already correct and don't need touching.
One themed batch per commit: (1) RLS/CHECK/GENERATED anchors, (2)
PL/pgSQL body sub-detection, (3) hover POLICY fix, (4) hover perf fix.
`cargo test --workspace --release` 0 failed and `cargo clippy
--workspace --all-features --release -- -D warnings` clean after
every batch, matching every prior batch this session.

## Success criteria

- Every confirmed-broken case in the Motivation section above (RLS
  `USING`/`WITH CHECK`, `GENERATED ALWAYS AS`, mid-expression `CHECK`,
  PL/pgSQL body `FROM`/`WHERE` slots, and the hover `POLICY`
  misfire) produces the correct menu / card when hand-probed, with a
  permanent regression test for each.
- Every confirmed-already-correct case above still passes after this
  work lands (regression, not just new coverage).
- `hover_with`'s per-call cost stops including a full buffer re-parse
  when called through `dsl-server` (verified via the same kind of
  before/after benchmark the completion project used in its Phase D).
- Full workspace test suite and clippy clean after every batch.
