# EXECUTE Dynamic-SQL Completion Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make SQL completion work inside a PL/pgSQL `EXECUTE '...'` (or `EXECUTE $tag$...$tag$`) dynamic-SQL string, for every phase/slot the top-level engine already supports, by recursively invoking the completion engine on the extracted string content.

**Architecture:** One new `PRE_PHASE_DETECTORS` entry (`detect_execute_dynamic_sql`) detects the cursor sitting inside an un-concatenated EXECUTE string, extracts and (for single-quoted strings) unescapes its content, and calls `dsl-completion`'s own public `complete()` recursively on that content as an independent tiny buffer. Results are returned as-is: `Item` carries no source range, so the LSP client's own cursor-relative replace logic positions the insertion correctly with no remapping needed.

**Tech Stack:** Rust, `dsl-completion` crate (byte-offset text scanning + the existing `Detector`/`PRE_PHASE_DETECTORS` registry from the completion-engine-redesign project).

**Spec:** `dsl-completion/docs/specs/2026-08-18-execute-dynamic-sql-completion-design.md`

## Global Constraints

- In scope: single-quoted `EXECUTE '...'` and dollar-quoted `EXECUTE $tag$...$tag$`, un-concatenated only.
- Out of scope (must NOT be implemented): string concatenation (`'...' || expr`), function-wrapped dynamic SQL (`EXECUTE format(...)`), hover inside EXECUTE strings, bind-parameter-aware completion, diagnostics against the extracted text.
- The inner parse is hardcoded to `Dialect::Postgres` (EXECUTE/dollar-quoting are Postgres-specific syntax; matches existing precedent in `dsl-hover`'s `alias_lookup`).
- Every prior batch this session ends with `cargo test --workspace --release` and `cargo clippy --workspace --all-features --release -- -D warnings` both clean before committing — this plan follows the same bar.

---

### Task 1: EXECUTE dynamic-SQL completion detector

**Files:**
- Modify: `dsl-completion/src/detectors.rs` (insert new code after `cursor_in_inert_span`, which currently ends at line 8548 — confirm the exact current line with `grep -n "^pub(crate) fn cursor_in_inert_span" dsl-completion/src/detectors.rs` before editing, since earlier commits on this branch may have shifted it)
- Modify: `dsl-completion/src/engine.rs` (add one entry to the `PRE_PHASE_DETECTORS` const, currently at line 527)
- Test: `dsl-completion/tests/engine.rs` (uses the existing `catalog_with_users_and_orders()` and `complete_at(src, cursor, cat)` helpers already defined near the top of that file — `catalog_with_users_and_orders()` provides `users(id, email, name)` and `orders(id, user_id)`)

**Interfaces:**
- Consumes: `dsl-completion`'s existing public `complete(source: &str, file: &ParsedFile, scopes: &[Scope], catalog: &Catalog, offset: TextSize) -> Vec<Item>` (in `engine.rs`); `dsl_parse::parse(source: &str, dialect: Dialect) -> ParsedFile`; `dsl_resolve::resolve_with_source(statements: &[Statement], source: &str) -> Vec<Scope>`; the `Detector` type alias (`fn(&str, TextSize, &ParsedFile, &[Scope], &Catalog) -> Option<Vec<Item>>`) already defined in `engine.rs`.
- Produces: `pub(crate) struct ExecuteStringSpan { content_start: usize, content_end: usize, dollar_quoted: bool }`, `pub(crate) fn execute_dynamic_sql_span(source: &str, offset: usize) -> Option<ExecuteStringSpan>`, `pub(crate) fn unescape_single_quoted(raw: &str, cursor_in_raw: usize) -> (String, usize)`, and the registered detector `fn detect_execute_dynamic_sql(...) -> Option<Vec<Item>>` — none of these are consumed by any later task (this is the only task in this plan), but keep the names exact since the spec references them by these names.

- [ ] **Step 1: Write the failing tests for the core in-scope cases**

Add to `dsl-completion/tests/engine.rs`, near the other `plpgsql_body_*` tests (these exercise the same "cursor inside a dollar-quoted PL/pgSQL body" buffer shape):

```rust
#[test]
fn execute_dynamic_sql_select_from_offers_tables() {
  let cat = catalog_with_users_and_orders();
  let src = "CREATE FUNCTION f() RETURNS trigger AS $$ BEGIN EXECUTE 'SELECT * FROM ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"users"), "expected `users`; got {} items, sample {:?}", labels.len(), &labels[..labels.len().min(10)]);
  assert!(labels.contains(&"orders"), "expected `orders`; got {labels:?}");
}

#[test]
fn execute_dynamic_sql_where_offers_columns() {
  let cat = catalog_with_users_and_orders();
  let src = "CREATE FUNCTION f() RETURNS trigger AS $$ BEGIN EXECUTE 'SELECT * FROM users WHERE ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"id"), "expected `id`; got {} items", labels.len());
  assert!(labels.contains(&"email"), "expected `email`; got {labels:?}");
}

#[test]
fn execute_dynamic_sql_dollar_quoted_offers_tables() {
  let cat = catalog_with_users_and_orders();
  let src = "CREATE FUNCTION f() RETURNS trigger AS $$ BEGIN EXECUTE $sql$ SELECT * FROM ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"users"), "expected `users`; got {} items, sample {:?}", labels.len(), &labels[..labels.len().min(10)]);
}

#[test]
fn execute_dynamic_sql_escaped_quote_offers_columns() {
  let cat = catalog_with_users_and_orders();
  let src = "CREATE FUNCTION f() RETURNS trigger AS $$ BEGIN EXECUTE 'SELECT * FROM users WHERE name = ''foo'' AND ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"id"), "expected `id` (proves the `''`-unescape + offset-mapping is correct, not just that some completion fired); got {} items", labels.len());
}

#[test]
fn execute_dynamic_sql_does_not_affect_unrelated_string_literal() {
  let cat = catalog_with_users_and_orders();
  let src = "INSERT INTO users (email) VALUES ('";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.is_empty(), "a plain (non-EXECUTE) string literal must stay inert; got {} items: {labels:?}", labels.len());
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p dsl-completion --release --test engine execute_dynamic_sql_`

Expected: `execute_dynamic_sql_select_from_offers_tables`, `execute_dynamic_sql_where_offers_columns`, `execute_dynamic_sql_dollar_quoted_offers_tables`, and `execute_dynamic_sql_escaped_quote_offers_columns` all FAIL (0 items, matching today's confirmed behavior). `execute_dynamic_sql_does_not_affect_unrelated_string_literal` PASSES already (nothing to break yet) — that's fine, it's a regression guard for later, not a red/green signal right now.

- [ ] **Step 3: Hand-probe boundary conditions before finalizing the implementation**

The two trickiest parts of this feature are the span-finder's offset comparisons (inclusive vs. exclusive at the start/end of the string content, and at an unclosed dollar-quote tag) and the unescaper's cursor-mapping across a `''` escape pair. Both are exactly the kind of text-scanning edge case that produced a real bug earlier this session (a 2nd RETURNING column being misread as still inside the SET clause). Before writing Step 4's implementation as final:

1. Implement Step 4 below as a first draft.
2. Write a throwaway test file (e.g. `dsl-completion/tests/_scratch_probe_execute.rs`, calling `dsl_completion::complete` directly the same way earlier probes in this project did) exercising: cursor exactly at the closing quote position (`EXECUTE 'SELECT * FROM users WHERE id = 1<cursor>'`), cursor exactly at the start of a dollar-quoted body right after the opening tag (`EXECUTE $sql$<cursor>`), an unclosed dollar-quote (already covered by the permanent `execute_dynamic_sql_dollar_quoted_offers_tables` test above, but worth re-confirming manually), and a cursor positioned between the two characters of a `''` escape pair (pathological, just confirm it doesn't panic).
3. If any of these disagree with what you expected, fix the *implementation* (these are genuine boundary bugs, not test-threshold miscalibrations like the RLS menu-size case from an earlier batch) and re-probe.
4. Delete the scratch probe file before committing (Step 8) — it must not be part of the final diff.

- [ ] **Step 4: Implement `ExecuteStringSpan`, `execute_dynamic_sql_span`, and `unescape_single_quoted`**

In `dsl-completion/src/detectors.rs`, insert immediately after `cursor_in_inert_span`'s closing `}`:

```rust
pub(crate) struct ExecuteStringSpan {
  /// Byte range of the string's content, excluding delimiters.
  pub content_start: usize,
  pub content_end: usize,
  pub dollar_quoted: bool,
}

/// Word-boundary check: does `source[..pos]`, skipping trailing
/// whitespace, end with `word` (case-insensitive)?
fn preceded_by_word(source: &str, pos: usize, word: &str) -> bool {
  let bytes = source.as_bytes();
  let mut i = pos.min(bytes.len());
  while i > 0 && bytes[i - 1].is_ascii_whitespace() {
    i -= 1;
  }
  let word_end = i;
  let mut word_start = i;
  while word_start > 0 && (bytes[word_start - 1].is_ascii_alphanumeric() || bytes[word_start - 1] == b'_') {
    word_start -= 1;
  }
  if word_start == word_end {
    return false;
  }
  source[word_start..word_end].eq_ignore_ascii_case(word)
}

/// Does `source[pos..]`, skipping leading whitespace, start with `||`?
fn followed_by_concat(source: &str, pos: usize) -> bool {
  let bytes = source.as_bytes();
  let mut i = pos;
  while i < bytes.len() && bytes[i].is_ascii_whitespace() {
    i += 1;
  }
  bytes.get(i) == Some(&b'|') && bytes.get(i + 1) == Some(&b'|')
}

/// Byte span of the string literal (single- or dollar-quoted) that
/// `offset` sits inside, when that string is the un-concatenated SQL
/// text argument of a PL/pgSQL `EXECUTE` statement -- e.g. `EXECUTE
/// 'SELECT * FROM <cursor>'` or `EXECUTE $sql$ SELECT * FROM <cursor>
/// $sql$`. Mirrors `cursor_in_inert_span`'s state-machine walk (same
/// string / comment / dollar-quote tracking, same recursion into a
/// dollar-quoted body to reach a string nested inside it -- necessary
/// because `EXECUTE` only appears inside such bodies) but returns the
/// matched string's own span instead of a bool, and only when it's
/// EXECUTE's target: the nearest word before its opening delimiter
/// must be `EXECUTE` (word-boundary checked, so `format('...'` and
/// other non-EXECUTE strings never match), and the nearest token
/// after its closing delimiter must not be `||` (ruling out a
/// concatenated segment).
pub(crate) fn execute_dynamic_sql_span(source: &str, offset: usize) -> Option<ExecuteStringSpan> {
  let bytes = source.as_bytes();
  let n = bytes.len();
  let limit = offset.min(n);
  let mut i = 0usize;
  // 0 = code, 1 = single-quoted string, 2 = line comment, 3 = block comment.
  let mut state: u8 = 0;
  let mut string_start = 0usize;
  while i < limit {
    match state {
      0 => {
        if bytes[i] == b'$' {
          let mut t = i + 1;
          while t < n && (bytes[t].is_ascii_alphanumeric() || bytes[t] == b'_') {
            t += 1;
          }
          if t < n && bytes[t] == b'$' {
            let tag_end = t + 1;
            let tag = &bytes[i..tag_end];
            let mut k = tag_end;
            let mut close = None;
            while k + tag.len() <= n {
              if &bytes[k..k + tag.len()] == tag {
                close = Some(k);
                break;
              }
              k += 1;
            }
            let body_end = close.unwrap_or(n);
            let body_close_end = close.map(|p| p + tag.len()).unwrap_or(n);
            // Is this dollar-quoted string itself EXECUTE's target?
            if offset >= tag_end
              && offset <= body_end
              && preceded_by_word(source, i, "EXECUTE")
              && !followed_by_concat(source, body_close_end)
            {
              return Some(ExecuteStringSpan { content_start: tag_end, content_end: body_end, dollar_quoted: true });
            }
            // Cursor inside the body but not (directly) EXECUTE's own
            // target -- recurse to look for a match nested deeper
            // (the common case: this is the outer PL/pgSQL function
            // body, and the real EXECUTE string is somewhere inside).
            if offset > tag_end && offset <= body_end {
              let body_off = offset - tag_end;
              let body_src = &source[tag_end..body_end];
              return execute_dynamic_sql_span(body_src, body_off).map(|s| ExecuteStringSpan {
                content_start: s.content_start + tag_end,
                content_end: s.content_end + tag_end,
                dollar_quoted: s.dollar_quoted,
              });
            }
            i = body_close_end;
            continue;
          }
        }
        match bytes[i] {
          b'\'' => {
            state = 1;
            string_start = i;
            i += 1;
            continue;
          },
          b'-' if i + 1 < n && bytes[i + 1] == b'-' => {
            state = 2;
            i += 2;
            continue;
          },
          b'/' if i + 1 < n && bytes[i + 1] == b'*' => {
            state = 3;
            i += 2;
            continue;
          },
          _ => {
            i += 1;
          },
        }
      },
      1 => {
        if bytes[i] == b'\'' {
          if i + 1 < n && bytes[i + 1] == b'\'' {
            i += 2;
            continue;
          }
          state = 0;
          i += 1;
          continue;
        }
        i += 1;
      },
      2 => {
        if bytes[i] == b'\n' {
          state = 0;
        }
        i += 1;
      },
      3 => {
        if bytes[i] == b'*' && i + 1 < n && bytes[i + 1] == b'/' {
          state = 0;
          i += 2;
          continue;
        }
        i += 1;
      },
      _ => break,
    }
  }
  if state != 1 {
    return None;
  }
  // Cursor is inside a single-quoted string that started at
  // `string_start`. Find its closing quote (handling `''` escapes) to
  // get the full span, then check it's EXECUTE's un-concatenated
  // target.
  let content_start = string_start + 1;
  let mut k = limit;
  loop {
    match bytes.get(k) {
      None => return None, // Unterminated string -- no valid span.
      Some(b'\'') => {
        if bytes.get(k + 1) == Some(&b'\'') {
          k += 2;
          continue;
        }
        break;
      },
      Some(_) => k += 1,
    }
  }
  let content_end = k;
  let close_end = k + 1;
  if !preceded_by_word(source, string_start, "EXECUTE") || followed_by_concat(source, close_end) {
    return None;
  }
  Some(ExecuteStringSpan { content_start, content_end, dollar_quoted: false })
}

/// Un-escape a single-quoted EXECUTE string's raw content (`''` ->
/// `'`) and map `cursor_in_raw` (a byte offset into `raw`, as returned
/// by `execute_dynamic_sql_span`) to the corresponding offset in the
/// unescaped output. Dollar-quoted content needs no unescaping and
/// must not be passed through this -- callers check
/// `ExecuteStringSpan::dollar_quoted` first.
pub(crate) fn unescape_single_quoted(raw: &str, cursor_in_raw: usize) -> (String, usize) {
  let bytes = raw.as_bytes();
  let n = bytes.len();
  let cursor_in_raw = cursor_in_raw.min(n);
  let mut out = String::with_capacity(raw.len());
  let mut mapped_cursor = cursor_in_raw;
  let mut i = 0usize;
  while i < n {
    if bytes[i] == b'\'' && i + 1 < n && bytes[i + 1] == b'\'' {
      // Collapsing `''` -> `'` removes one byte. Every collapse that
      // finishes strictly before the raw cursor shifts the mapped
      // cursor left by one.
      if i + 1 < cursor_in_raw {
        mapped_cursor -= 1;
      }
      out.push('\'');
      i += 2;
      continue;
    }
    let ch_len = raw[i..].chars().next().map(|c| c.len_utf8()).unwrap_or(1);
    out.push_str(&raw[i..i + ch_len]);
    i += ch_len;
  }
  (out, mapped_cursor)
}
```

- [ ] **Step 5: Implement `detect_execute_dynamic_sql` and register it**

Still in `dsl-completion/src/detectors.rs`, right after `unescape_single_quoted`:

```rust
/// `PRE_PHASE_DETECTORS` entry: cursor inside a PL/pgSQL `EXECUTE`
/// statement's dynamic-SQL string. Extracts and (for single-quoted
/// strings) unescapes the content, then recursively calls
/// `crate::engine::complete` on it as an independent tiny buffer --
/// giving full phase parity with the top-level engine "for free"
/// rather than re-implementing a subset of it. `file`/`scopes` (the
/// *outer* buffer's parse) are unused: this builds its own parse for
/// the extracted substring, same as any other pure-text-check
/// `Detector`. `Dialect::Postgres` is hardcoded -- `EXECUTE` and
/// dollar-quoting are themselves Postgres-specific syntax.
fn detect_execute_dynamic_sql(
  source: &str,
  offset: TextSize,
  _file: &ParsedFile,
  _scopes: &[Scope],
  cat: &Catalog,
) -> Option<Vec<Item>> {
  let pos: usize = u32::from(offset) as usize;
  let span = execute_dynamic_sql_span(source, pos)?;
  let raw = &source[span.content_start..span.content_end];
  let raw_cursor = pos - span.content_start;
  let (content, inner_offset) = if span.dollar_quoted {
    (raw.to_string(), raw_cursor)
  } else {
    unescape_single_quoted(raw, raw_cursor)
  };
  let inner_file = dsl_parse::parse(&content, dsl_parse::Dialect::Postgres);
  let inner_scopes = dsl_resolve::resolve_with_source(&inner_file.statements, &content);
  Some(crate::engine::complete(&content, &inner_file, &inner_scopes, cat, TextSize::from(inner_offset as u32)))
}
```

Check the top of `detectors.rs` for an existing `use dsl_parse;` / `use dsl_resolve;` — if those crates aren't already imported by name (only `dsl_parse::ParsedFile` and `dsl_resolve::Scope` are imported per the current top-of-file `use` block), add:

```rust
use dsl_parse;
use dsl_resolve;
```

(Or just call fully-qualified `dsl_parse::parse(...)` / `dsl_resolve::resolve_with_source(...)` as written above without adding a bare `use` — either compiles; prefer whichever matches what `cargo build -p dsl-completion` accepts without a warning.)

In `dsl-completion/src/engine.rs`, add the new detector to `PRE_PHASE_DETECTORS`, positioned before `detect_inert_span` so it wins the race for this specific case while every other string literal keeps today's inert-suppression behavior:

```rust
const PRE_PHASE_DETECTORS: &[Detector] = &[
  detect_fresh_name_slot,
  detect_json_path_key,
  detect_execute_dynamic_sql,
  detect_inert_span,
  detect_dot_context,
  detect_grouping_sets_inner_paren,
  detect_contexts,
];
```

- [ ] **Step 6: Run tests to verify Step 1's tests pass**

Run: `cargo test -p dsl-completion --release --test engine execute_dynamic_sql_`

Expected: all 5 tests PASS. If any fail, this is where the hand-probing from Step 3 should already have caught the issue — if not, debug now before moving on (don't adjust an assertion to match wrong behavior; these 5 tests encode real, specific expectations, not a size threshold that might legitimately need calibration).

- [ ] **Step 7: Add the scope-guard tests (concatenation, `format()`, `USING`)**

These need the cursor positioned *before* the end of `src` (unlike Step 1's tests, which all use `src.len()`) — `complete_at(src, cursor, cat)` takes `cursor` as an independent parameter, so `src` can contain text after the cursor. Add to `dsl-completion/tests/engine.rs`:

```rust
#[test]
fn execute_dynamic_sql_concatenation_stays_inert() {
  let cat = catalog_with_users_and_orders();
  let src = "CREATE FUNCTION f() RETURNS trigger AS $$ BEGIN EXECUTE 'SELECT * FROM ' || tbl_name; END; $$ LANGUAGE plpgsql;";
  let cursor = src.find("FROM ").unwrap() + "FROM ".len();
  let items = complete_at(src, cursor, &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.is_empty(), "concatenated EXECUTE should stay inert (0 items), matching today's behavior; got {} items: {labels:?}", labels.len());
}

#[test]
fn execute_dynamic_sql_format_wrapped_stays_inert() {
  let cat = catalog_with_users_and_orders();
  let src = "CREATE FUNCTION f() RETURNS trigger AS $$ BEGIN EXECUTE format('SELECT * FROM %I', tbl); END; $$ LANGUAGE plpgsql;";
  let cursor = src.find("SELECT").unwrap();
  let items = complete_at(src, cursor, &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.is_empty(), "format()-wrapped EXECUTE should stay inert (0 items); got {} items: {labels:?}", labels.len());
}

#[test]
fn execute_dynamic_sql_using_clause_still_offers_completion() {
  let cat = catalog_with_users_and_orders();
  let src = "CREATE FUNCTION f() RETURNS trigger AS $$ BEGIN EXECUTE 'SELECT * FROM users WHERE id = $1 AND ' USING 5; END; $$ LANGUAGE plpgsql;";
  let cursor = src.find("' USING").unwrap();
  let items = complete_at(src, cursor, &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"id"), "a trailing USING clause should not disqualify the string; expected `id`, got {} items", labels.len());
}
```

Run: `cargo test -p dsl-completion --release --test engine execute_dynamic_sql_`

Expected: all 8 `execute_dynamic_sql_*` tests PASS. If `execute_dynamic_sql_concatenation_stays_inert` or `execute_dynamic_sql_format_wrapped_stays_inert` fail (i.e., they unexpectedly return completion items), that means the `preceded_by_word`/`followed_by_concat` guards have a real bug -- fix the implementation, don't loosen the test.

- [ ] **Step 8: Delete the scratch probe, run full verification, and commit**

```bash
rm -f dsl-completion/tests/_scratch_probe_execute.rs
cargo test --workspace --release
cargo clippy --workspace --all-features --release -- -D warnings
```

Expected: both commands clean (0 failures, 0 warnings under `-D warnings`). Fix anything red before committing.

```bash
git add dsl-completion/src/detectors.rs dsl-completion/src/engine.rs dsl-completion/tests/engine.rs
git commit -m "feat(completion): recursive completion inside EXECUTE dynamic SQL"
```

Write a commit message body (not just the subject line above) that explains: the recursive-virtual-sub-buffer mechanism and why it was chosen over re-anchoring the existing tokenizer (single-quoted content needs `''`-unescaping, which changes byte length and breaks in-place re-anchoring); what's explicitly out of scope and why (concatenation, `format()`-wrapped dynamic SQL, hover); and the test/clippy result line, matching the style of every prior commit message on this branch.

---

## Self-Review

**Spec coverage:**
- Scope's "in scope" bullets (single-quoted, dollar-quoted, full phase parity via recursion, `USING` doesn't disqualify) -> Steps 1, 5, 7.
- Scope's "out of scope" bullets (concatenation, `format()`, hover, bind-params, diagnostics) -> concatenation and `format()` get explicit regression tests (Step 7); hover/bind-params/diagnostics require no code changes since nothing in this plan touches them -- covered by omission, called out explicitly in the commit message per Step 8.
- Design section 1 (detection + 3 conditions) -> Step 4's `execute_dynamic_sql_span` + `preceded_by_word` + `followed_by_concat`.
- Design section 2 (unescaping + offset mapping, no-remapping-needed rationale) -> Step 4's `unescape_single_quoted`; the no-remapping rationale is architectural (nothing to implement) and is restated in Step 8's commit message.
- Design section 3 (recursive completion) -> Step 5's `detect_execute_dynamic_sql`.
- Design section 4 (wiring) -> Step 5's `PRE_PHASE_DETECTORS` edit.
- Testing section's 8 listed cases -> Steps 1 and 7 (5 + 3 = 8 tests, one-to-one).
- Success criteria -> covered by the sum of all tests above; no separate task needed.

**Placeholder scan:** No TBD/TODO. Every step has complete, real code or an exact command. Step 3 (hand-probe) intentionally has no fixed assertions to check against -- that's its purpose (empirical verification before the boundary-condition code is treated as final), matching the identical pattern used for hand-probe steps in every prior batch this session's plans.

**Type consistency:** `ExecuteStringSpan { content_start: usize, content_end: usize, dollar_quoted: bool }` is defined once (Step 4) and consumed with the same field names in Step 5. `execute_dynamic_sql_span(source: &str, offset: usize) -> Option<ExecuteStringSpan>` and `unescape_single_quoted(raw: &str, cursor_in_raw: usize) -> (String, usize)` signatures match between their Step 4 definitions and Step 5's call sites. `detect_execute_dynamic_sql`'s signature matches the `Detector` type alias exactly (`fn(&str, TextSize, &ParsedFile, &[Scope], &Catalog) -> Option<Vec<Item>>`), consistent with every other `PRE_PHASE_DETECTORS` entry.
