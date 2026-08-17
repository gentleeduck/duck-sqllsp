# EXECUTE dynamic-SQL completion

## Motivation

The body-context-completion-hover project (merged into
`feat/completion-engine-redesign`) made completion work inside every
embedded SQL "body" construct except one, explicitly deferred at the
time: a PL/pgSQL `EXECUTE '...'` statement's dynamic SQL string.
Confirmed by probing: the cursor inside `EXECUTE 'SELECT * FROM
<cursor>'` currently gets 0 completion items. This isn't a missing
slot in an existing detector -- `cursor_in_inert_span` (the function
that decides whether a cursor position is "inert", i.e. inside a
string literal / comment) correctly identifies the cursor as being
inside a single-quoted string and suppresses completion entirely, the
same as it would for any other string literal (`INSERT INTO t (name)
VALUES ('<cursor>')`, for example) where suppression is exactly the
right behavior.

The difference is that an `EXECUTE` string's content *is* SQL text,
not a data value -- so unlike a generic string literal, completion
inside it is exactly as useful as completion at the top level. This
project makes that work, reusing the full completion engine rather
than adding another narrow, slot-by-slot detector, and deliberately
scopes out the one sub-case that has no reliable static answer:
runtime string concatenation.

## Scope

Explicitly **in scope**:
- `EXECUTE '<sql text>'` -- single-quoted, unconcatenated string
  immediately following the `EXECUTE` keyword.
- `EXECUTE $tag$<sql text>$tag$` -- dollar-quoted (any tag, including
  empty `$$`), same position.
- Every phase/slot the top-level engine already supports (SELECT
  projection, FROM/JOIN, WHERE, INSERT/UPDATE/DELETE, RETURNING,
  subqueries, ...) -- achieved by reusing the engine recursively, not
  by re-implementing a subset of it.
- Standalone `EXECUTE` statements, and `EXECUTE '...' USING ...`
  (a trailing `USING` clause does not disqualify the string).

Explicitly **out of scope** (all confirmed via the brainstorming
session; revisit only as a separate, explicitly-requested follow-up):
- **String concatenation**: `EXECUTE 'SELECT * FROM ' || tbl_name`.
  Only the literal segments are statically knowable; the cursor's
  segment can be identified, but what it concatenates with at runtime
  cannot, so any completion offered there would be guessing dressed up
  as a suggestion. These keep today's behavior (0 items) unchanged.
- **Function-wrapped dynamic SQL**: `EXECUTE format('SELECT * FROM
  %I', tbl)`. Same reasoning as concatenation -- the text isn't a
  plain literal. Falls out of scope naturally (see Design section 1:
  the detector's "preceded by EXECUTE" check does not match a string
  preceded by `format(`), not via special-case code.
- **Hover** inside EXECUTE strings. Hover's architecture (token-at-
  cursor resolution) is unrelated to completion's (phase/next-clause
  prediction) -- explicitly deferred as a separate future decision,
  not bundled here.
- **Bind-parameter-aware completion** (`$1`, `$2` placeholders
  matching a trailing `USING` clause's argument types). The ask is "SQL
  completion works the same as everywhere else" -- parameter-slot
  intelligence is a refinement on top of that, not required to meet it.
- **Diagnostics / lint / hover-adjacent features** running against the
  extracted SQL text. Not requested; would need its own scoping pass.

## Design

### 1. Detection: `execute_dynamic_sql_span`

A new function in `dsl-completion/src/detectors.rs`, structurally a
sibling of the existing `cursor_in_inert_span` (same state-machine
shape: code / single-quoted-string / line-comment / block-comment
states, same recursion into a dollar-quoted PL/pgSQL body to reach a
string nested inside it -- necessary because `EXECUTE` only appears
inside such bodies). Where `cursor_in_inert_span` returns a bool, this
returns the matched string's span plus its quoting style, and only
when three conditions all hold:

1. The cursor sits inside a single-quoted or dollar-quoted string.
2. The nearest word before the string's opening delimiter (skipping
   whitespace, word-boundary checked so `MYEXECUTE` doesn't match) is
   `EXECUTE`, case-insensitive.
3. The nearest token after the string's closing delimiter (skipping
   whitespace) is not `||`.

Condition 2 is what makes `format('...')`-wrapped dynamic SQL fall
outside the match with no special-case code: the character
immediately before that string's opening quote is `(`, not whitespace
leading back to a word, so no word is found there at all. Condition 3
is the concatenation guard for the trailing side (a string on the
*leading* side of a `||` chain); a leading `||` before the opening
quote is already excluded by condition 2 (the preceding word, if any,
would not be `EXECUTE`).

```rust
pub(crate) struct ExecuteStringSpan {
  /// Byte range of the string's content, excluding delimiters.
  pub content_start: usize,
  pub content_end: usize,
  pub dollar_quoted: bool,
}

pub(crate) fn execute_dynamic_sql_span(source: &str, offset: usize) -> Option<ExecuteStringSpan>
```

### 2. Extraction: unescaping and offset mapping

Dollar-quoted content needs no transformation -- dollar-quoting has no
internal escape sequences by design, so the substring between the tags
is used directly, and the cursor's offset within it maps 1:1.

Single-quoted content uses `''` as an escaped literal `'`
(`EXECUTE 'SELECT * FROM t WHERE name = ''foo'''`). A new helper
unescapes the raw content into valid SQL text and maps the cursor's
raw-content offset forward through the same pass (fewer bytes exist
after unescaping, so the mapped offset is `<=` the raw one):

```rust
pub(crate) fn unescape_single_quoted(raw: &str, cursor_in_raw: usize) -> (String, usize)
```

Only the forward direction (raw cursor -> unescaped cursor) is needed.
Completion items carry no source range or `TextEdit` (`Item`'s fields
are label/kind/detail/insert_text/sort_priority only -- confirmed by
reading `dsl-completion/src/item.rs` and `dsl-server/src/handlers/
completion.rs`'s `to_lsp_item`, which builds a plain `CompletionItem`
with `insert_text` and no `text_edit`); the client positions the
insertion using its own cursor-relative logic. This means results from
completing against the extracted, unescaped substring are returned
as-is -- no reverse mapping back into the outer buffer's coordinates
is needed anywhere.

### 3. Recursive completion: `detect_execute_dynamic_sql`

The new `PRE_PHASE_DETECTORS` entry ties the above together:

```rust
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
  Some(complete(&content, &inner_file, &inner_scopes, cat, TextSize::from(inner_offset as u32)))
}
```

`file`/`scopes` (the *outer* buffer's parse) are intentionally unused
-- this detector builds its own parse for the extracted substring,
exactly like every other `Detector` in the registry that's a pure text
check rather than an AST consumer (see the `Detector` type's own doc
comment in `engine.rs`). `cat` is passed through unchanged: the
dynamic SQL still targets the same database, so the same
already-merged catalog the outer call received is correct.

The inner parse is hardcoded to `Dialect::Postgres` rather than
threaded through from the outer document's configured dialect --
`EXECUTE` and dollar-quoting are themselves Postgres/PL-pgSQL-specific
syntax, so this detector can only ever match inside a buffer that's
already being parsed as Postgres. This matches existing precedent in
this codebase: `dsl-hover`'s `alias_lookup` does the same thing for
its own dollar-quoted-body fallback parse.

`complete()` is dsl-completion's own public top-level entry point --
the same one `dsl-server` calls. Calling it recursively on the
extracted substring means the substring is treated as if it were an
independent tiny document: `complete()` computes its own
(near-certainly-empty, since a dynamic-SQL fragment rarely declares
tables) derived catalog from the substring and merges it with `cat`,
exactly like it does for any real document. This is what gives full
phase parity "for free" -- SELECT projections, FROM/JOIN, WHERE,
INSERT/UPDATE/RETURNING, subqueries, and everything else the top-level
engine already understands all work inside the string, because it
*is* the top-level engine, just invoked on a smaller buffer.

Nested `EXECUTE` (dynamic SQL that itself contains another `EXECUTE
'...'`) is not special-cased -- the recursive `complete()` call would
naturally re-enter this same detector on the smaller inner substring
and terminate, since each recursion operates on a strictly smaller
buffer. Rare enough not to warrant a dedicated test; expected to work
by construction.

### 4. Wiring

`detect_execute_dynamic_sql` is added to `PRE_PHASE_DETECTORS`,
positioned before `detect_inert_span` so it wins the race for this
specific case while every other string literal in the buffer keeps
today's inert-suppression behavior unchanged:

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

## Testing

Empirical-probe-before-permanent-tests discipline, same as every prior
batch this session: hand-probe each case via a throwaway test file
(deleted before committing) to confirm actual behavior before writing
the permanent assertion, since text-scanning edge cases (escaping,
boundary detection) have repeatedly turned out to differ from
first-pass reasoning in this project.

Permanent tests (`dsl-completion/tests/engine.rs`):
- `EXECUTE 'SELECT * FROM <cursor>'` offers tables.
- `EXECUTE 'SELECT * FROM users WHERE <cursor>'` offers columns.
- `EXECUTE $sql$ SELECT * FROM <cursor> $sql$` (dollar-quoted) offers
  tables.
- Escaped-quote correctness: `EXECUTE 'SELECT * FROM users WHERE name
  = ''foo'' AND <cursor>'` offers columns (proves the unescape +
  offset-mapping is correct, not just "some completion fires").
- Concatenation guard: `EXECUTE 'SELECT * FROM ' || tbl_name` -- cursor
  inside the literal segment still yields today's behavior (0 items),
  not a crash or a wrong-but-plausible-looking menu.
- `format()`-wrapped guard: `EXECUTE format('SELECT * FROM <cursor>',
  tbl)` -- same, 0 items, proving condition 2 (word-boundary check on
  the preceding token) excludes it without dedicated exclusion code.
- `USING` clause does not disqualify: `EXECUTE 'SELECT * FROM users
  WHERE id = $1' USING x;` with cursor inside the string still offers
  completion (proves the closing-delimiter guard only rejects `||`,
  not any trailing clause).
- A plain (non-EXECUTE) string literal elsewhere in the same buffer is
  unaffected -- still 0 items, proving `detect_inert_span` still wins
  for every other string.

`cargo test --workspace --release` and `cargo clippy --workspace
--all-features --release -- -D warnings` both clean, matching every
prior batch.

## Success criteria

- Cursor inside an un-concatenated `EXECUTE '...'` or `EXECUTE
  $tag$...$tag$` string offers the same completion menu the same SQL
  text would get at the top level, for every phase/slot the engine
  already supports there -- not a narrow slot-by-slot subset.
- Concatenated and `format()`-wrapped dynamic SQL are unaffected
  (still today's 0-items behavior) -- no wrong-but-plausible menu from
  guessing across a runtime-computed boundary.
- Every other string literal in the buffer (data values, not dynamic
  SQL) is unaffected.
- Zero regressions: full workspace test suite and clippy clean.
