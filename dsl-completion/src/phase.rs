//! Phase-aware completion context.
//!
//! Walks the tokens of the current statement (text from the previous `;`
//! or buffer start, up to the cursor) through a small state machine.
//! The resulting [`Phase`] is what completion expects *next* at the
//! cursor. This is what lets us avoid suggesting `SELECT` in the middle
//! of a statement, or suggesting columns where only a table makes sense.
//!
//! The detector is token-based; it ignores comments and strings.

use text_size::TextSize;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Phase {
  /// Empty buffer or right after a `;`. Top-level statement keywords.
  Start,

  /// After `SELECT`. Expect projection items (columns / `*` / funcs /
  /// `DISTINCT` / `ALL`).
  SelectProjection,
  /// Mid-projection identifier (after a column name). Expect `AS`,
  /// comma to add another column, or `FROM`.
  InProjection,
  /// Right after a comma in the projection list. Expect another column.
  NextProjection,
  /// Right after `*` (whole row). Expect `FROM` or a comma to mix in
  /// more projections like `SELECT *, foo FROM ...`.
  AfterStar,
  /// Right after `AS` inside the projection list. Expect alias name.
  ProjectionAlias,

  /// After `FROM` / a comma inside FROM list / `JOIN`. Expect a table.
  ExpectTable,
  /// After a table name (possibly aliased). Expect comma / JOIN family
  /// / WHERE / GROUP BY / HAVING / ORDER BY / LIMIT / OFFSET / `;`.
  AfterTable,
  /// After `INNER` / `LEFT` / `RIGHT` / `FULL` / `CROSS`. Expect JOIN
  /// or OUTER.
  JoinModifier,
  /// After a join completed but before ON / USING.
  JoinComplete,
  /// After `ON`. Expect a predicate.
  OnClause,
  /// After `USING`. Expect `(`.
  UsingClause,

  /// After `WHERE`. Expect a predicate.
  WhereClause,
  /// Mid-predicate. Expect AND / OR / ORDER BY / GROUP BY / ...
  InPredicate,

  /// After `GROUP` (expect `BY`).
  AfterGroup,
  /// After `GROUP BY`. Expect columns / numbers / GROUPING SETS / ROLLUP.
  GroupByList,
  /// After `ORDER` (expect `BY`).
  AfterOrder,
  /// After `ORDER BY`. Expect column [ASC|DESC] [NULLS FIRST|LAST].
  OrderByList,
  /// After `HAVING`. Expect a predicate over aggregates.
  HavingClause,
  /// After `LIMIT`. Expect a number.
  LimitClause,
  /// After `OFFSET`. Expect a number.
  OffsetClause,

  // ----- DML -----
  AfterInsert,
  AfterInsertTable,
  InsertColumnList,
  InsertExpectValues,
  InsertValuesList,
  AfterUpdate,
  AfterUpdateTable,
  UpdateAssignment,
  AfterDelete,
  /// After `RETURNING` in INSERT / UPDATE / DELETE. Expect columns of
  /// the target table (the one named in INTO / UPDATE / DELETE FROM),
  /// plus `*`. Acts like a SELECT projection scoped to that one table.
  ReturningClause,

  // ----- DDL -----
  AfterCreate,
  AfterAlter,
  AfterDrop,

  /// After `ALTER TABLE [IF EXISTS] [ONLY]` -- expect a table name.
  /// Engine surfaces tables here.
  AfterAlterTableExpectName,
  /// After `ALTER TABLE <name>` -- expect a sub-action keyword
  /// (ADD COLUMN, DROP COLUMN, RENAME, ALTER COLUMN, ...). Engine
  /// emits the curated snippet list.
  AfterAlterTableName,

  /// After `GRANT` / `REVOKE` -- expect a privilege keyword
  /// (SELECT / INSERT / UPDATE / DELETE / TRUNCATE / REFERENCES /
  /// TRIGGER / USAGE / EXECUTE / CREATE / CONNECT / TEMPORARY) or
  /// `ALL [PRIVILEGES]`. Engine emits the curated keyword set.
  AfterGrantOrRevoke,
  /// After `GRANT/REVOKE ... ON [TABLE|SEQUENCE|FUNCTION|SCHEMA|...]`
  /// -- expect a target name. Engine emits tables/sequences/funcs.
  AfterGrantOn,
  /// After `GRANT ... TO` or `REVOKE ... FROM` -- expect a role
  /// from the catalog, plus the `PUBLIC` pseudo-role.
  AfterGrantTo,

  /// Cursor sits inside a `$$ ... $$` body (DO block or function body).
  /// PL/pgSQL keywords + functions + columns make sense here.
  PlpgsqlBody,
  /// Right-hand side of a PL/pgSQL assignment (`v_x := ...` or
  /// `NEW.col := ...`). Statement-level keywords like SELECT / CREATE
  /// are invalid here -- only expressions are.
  PlpgsqlAssignRhs,

  // ----- CREATE TABLE sub-phases -----
  /// After `CREATE TABLE [IF NOT EXISTS]` -- expect a fresh table name.
  /// No completion offered (user invents the name).
  CtlExpectTableName,
  /// Start of an entry inside CREATE TABLE body (after `(` or `,`).
  /// User could either type a column name or a constraint-line keyword
  /// (CONSTRAINT / PRIMARY KEY / FOREIGN KEY / UNIQUE / CHECK).
  CtlBodyStart,
  /// After a column name -- expect a type.
  CtlExpectType,
  /// After a column type -- expect column constraint keywords
  /// (NOT NULL, DEFAULT, PRIMARY KEY, REFERENCES, UNIQUE, CHECK, ...).
  CtlExpectColumnConstraint,
  /// After `CONSTRAINT` -- expect a fresh constraint name. No completion.
  CtlExpectConstraintName,
  /// After `CONSTRAINT <name>` -- expect a kind keyword (PRIMARY KEY,
  /// FOREIGN KEY, UNIQUE, CHECK).
  CtlExpectConstraintKind,
  /// After `REFERENCES` -- expect a target table.
  CtlExpectFkTable {/* nothing for now */},
  /// After `REFERENCES tbl (` -- expect a column of that table.
  CtlExpectFkColumn {
    table: String,
  },
  /// Inside a `CHECK ( ... )` expression. Expect this table's
  /// columns plus the full PG function library and expression
  /// keywords -- CHECK bodies are arbitrary boolean expressions.
  CtlCheckExpr {
    table: Option<String>,
  },

  /// Right after the PG `::` cast operator. Expect a type (DATE,
  /// NUMERIC, TEXT, JSONB, custom enum/domain, ...).
  CastType,

  /// Anything we couldn't classify -- the engine falls back to a broad
  /// emitter so the menu is never empty.
  Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Tok<'a> {
  Word(&'a str), // identifier or keyword; uppercase via .to_ascii_uppercase()
  Punct(char),   // single-char punctuation: , ; ( ) * . = < > ! +
  StringLit,
  NumberLit,
}

/// Tokenise from `start` (inclusive) to `end_exclusive` (exclusive),
/// skipping whitespace, line comments (`-- ...`), block comments
/// (`/* ... */`), single-quoted strings (with `\` escapes), double-quoted
/// identifiers, and dollar-quoted blocks.
fn tokenise(src: &str, start: usize, end: usize) -> Vec<Tok<'_>> {
  let bytes = src.as_bytes();
  let mut out = Vec::new();
  let mut i = start;
  while i < end {
    // Skip past any non-ASCII bytes -- the state machine only
    // recognises ASCII keywords + punctuation, and slicing into a
    // multi-byte char would panic.
    if !bytes[i].is_ascii() {
      i += 1;
      continue;
    }
    let c = bytes[i] as char;
    if c.is_whitespace() {
      i += 1;
      continue;
    }
    if c == '-' && i + 1 < end && bytes[i + 1] == b'-' {
      // Line comment
      while i < end && bytes[i] != b'\n' {
        i += 1;
      }
      continue;
    }
    if c == '/' && i + 1 < end && bytes[i + 1] == b'*' {
      i += 2;
      while i + 1 < end && !(bytes[i] == b'*' && bytes[i + 1] == b'/') {
        i += 1;
      }
      i = (i + 2).min(end);
      continue;
    }
    if c == '\'' {
      i += 1;
      while i < end {
        let b = bytes[i];
        if b == b'\'' && (i == 0 || bytes[i - 1] != b'\\') {
          i += 1;
          break;
        }
        i += 1;
      }
      out.push(Tok::StringLit);
      continue;
    }
    if c == '"' {
      // Quoted identifier; treat as a Word.
      let id_start = i + 1;
      i += 1;
      while i < end && bytes[i] != b'"' {
        i += 1;
      }
      let id_end = i;
      i = (i + 1).min(end);
      if id_end > id_start {
        out.push(Tok::Word(&src[id_start..id_end]));
      }
      continue;
    }
    if c == '$' {
      // Dollar-quoted block (tag may be empty).
      if let Some(tag_end_rel) = src[i + 1..end].find('$') {
        let tag = &src[i + 1..i + 1 + tag_end_rel];
        if tag.chars().all(|ch| ch.is_alphanumeric() || ch == '_') {
          let closer = format!("${tag}$");
          let body_start = i + 1 + tag_end_rel + 1;
          if let Some(rel) = src[body_start..end].find(&closer) {
            i = body_start + rel + closer.len();
          } else {
            i = end;
          }
          out.push(Tok::StringLit);
          continue;
        }
      }
    }
    if c.is_ascii_digit() {
      let s = i;
      while i < end && (bytes[i].is_ascii_digit() || bytes[i] == b'.') {
        i += 1;
      }
      let _ = s;
      out.push(Tok::NumberLit);
      continue;
    }
    if is_word_start(c) {
      let s = i;
      while i < end {
        // Halt on non-ASCII so the resulting slice never
        // straddles a UTF-8 char boundary.
        if !bytes[i].is_ascii() {
          break;
        }
        if !is_word_cont(bytes[i] as char) {
          break;
        }
        i += 1;
      }
      out.push(Tok::Word(&src[s..i]));
      continue;
    }
    out.push(Tok::Punct(c));
    i += 1;
  }
  out
}

fn is_word_start(c: char) -> bool {
  c.is_alphabetic() || c == '_'
}
fn is_word_cont(c: char) -> bool {
  c.is_alphanumeric() || c == '_'
}

/// True when the cursor sits immediately after `::` (possibly with a
/// partial type-name word in between). Skips whitespace between the
/// colons and the word so `now() ::` and `now()::DAT` both qualify.
/// True when the cursor sits in the role-name slot of an
/// `ALTER ... OWNER TO ` clause. Walks back from `pos` over any
/// in-progress ASCII identifier characters and whitespace, then
/// verifies the preceding tokens are `OWNER TO`. Bails out on any
/// non-ASCII byte (we don't expect role names to be non-ASCII; the
/// preceding `OWNER TO` keywords are pure ASCII).
fn after_owner_to(src: &str, pos: usize) -> bool {
  let bytes = src.as_bytes();
  let mut i = pos.min(bytes.len());
  while i > 0 && bytes[i - 1].is_ascii() && is_word_cont(bytes[i - 1] as char) {
    i -= 1;
  }
  while i > 0 && bytes[i - 1].is_ascii() && bytes[i - 1].is_ascii_whitespace() {
    i -= 1;
  }
  if i < 2 || !src.is_char_boundary(i) {
    return false;
  }
  if !src[..i].to_ascii_uppercase().ends_with("TO") {
    return false;
  }
  let pre_end = i - 2;
  if !src.is_char_boundary(pre_end) {
    return false;
  }
  let pre = src[..pre_end].trim_end();
  pre.to_ascii_uppercase().ends_with("OWNER")
}

/// True when the cursor sits in the role-name slot of `SET ROLE`.
fn after_set_role(src: &str, pos: usize) -> bool {
  let bytes = src.as_bytes();
  let mut i = pos.min(bytes.len());
  while i > 0 && bytes[i - 1].is_ascii() && is_word_cont(bytes[i - 1] as char) {
    i -= 1;
  }
  while i > 0 && bytes[i - 1].is_ascii() && bytes[i - 1].is_ascii_whitespace() {
    i -= 1;
  }
  if i < 4 || !src.is_char_boundary(i) {
    return false;
  }
  if !src[..i].to_ascii_uppercase().ends_with("ROLE") {
    return false;
  }
  let pre_end = i - 4;
  if !src.is_char_boundary(pre_end) {
    return false;
  }
  let pre = src[..pre_end].trim_end();
  pre.to_ascii_uppercase().ends_with("SET")
}

fn after_double_colon(src: &str, pos: usize) -> bool {
  let bytes = src.as_bytes();
  let mut i = pos;
  // Skip the partial type identifier the user is typing.
  while i > 0 && is_word_cont(bytes[i - 1] as char) {
    i -= 1;
  }
  // Skip any whitespace between `::` and the partial word.
  while i > 0 && bytes[i - 1].is_ascii_whitespace() {
    i -= 1;
  }
  i >= 2 && bytes[i - 1] == b':' && bytes[i - 2] == b':'
}

/// Find the byte offset of the last unquoted `;` before `pos`. Returns
/// 0 when none. This is where the current statement starts.
/// When the cursor at `pos` sits inside an unclosed parenthesized
/// subquery body (the body starts with SELECT / INSERT / UPDATE /
/// DELETE), return the byte offset immediately after the opening
/// `(`. The innermost unclosed subquery wins so nested cases like
/// `SELECT * FROM (SELECT * FROM (SELECT |))` route to the deepest
/// SELECT. Strings and double-quoted identifiers are skipped.
fn subquery_body_start(src: &str, start: usize, pos: usize) -> Option<usize> {
  let bytes = src.as_bytes();
  let upper = src.to_ascii_uppercase();
  let upper_bytes = upper.as_bytes();
  let end = pos.min(bytes.len());
  let mut in_single = false;
  let mut in_double = false;
  let mut stack: Vec<usize> = Vec::new(); // bodies (after-paren positions) of SELECT-like subqueries
  let mut depth: i32 = 0; // running paren depth
  let mut subq_depths: Vec<i32> = Vec::new(); // matching depth for each stack entry
  let mut i = start;
  while i < end {
    let c = bytes[i] as char;
    if !in_double && c == '\'' && (i == 0 || bytes[i - 1] != b'\\') {
      in_single = !in_single;
      i += 1;
      continue;
    }
    if !in_single && c == '"' {
      in_double = !in_double;
      i += 1;
      continue;
    }
    if in_single || in_double {
      i += 1;
      continue;
    }
    if c == '(' {
      depth += 1;
      // Peek for SELECT / INSERT / UPDATE / DELETE after optional ws.
      let mut k = i + 1;
      while k < end && bytes[k].is_ascii_whitespace() {
        k += 1;
      }
      let starts_subquery = ["SELECT", "INSERT", "UPDATE", "DELETE", "WITH", "VALUES"]
        .iter()
        .any(|kw| k + kw.len() <= upper_bytes.len() && &upper_bytes[k..k + kw.len()] == kw.as_bytes() && (k + kw.len() == upper_bytes.len() || !is_word_cont(upper_bytes[k + kw.len()] as char)));
      if starts_subquery {
        stack.push(i + 1);
        subq_depths.push(depth);
      }
    } else if c == ')' {
      if let Some(&d) = subq_depths.last()
        && d == depth
      {
        stack.pop();
        subq_depths.pop();
      }
      depth -= 1;
    }
    i += 1;
  }
  stack.last().copied()
}

/// When the cursor sits inside a `CREATE [OR REPLACE] [TEMP|TEMPORARY]
/// [RECURSIVE] [MATERIALIZED] VIEW <name> AS <body>` statement,
/// return the offset just after the ` AS ` so the walker treats the
/// body as a fresh SELECT/WITH. Returns None when the cursor is
/// before the AS or the prefix doesn't match.
fn create_view_body_start(src: &str, start: usize, pos: usize) -> Option<usize> {
  let end = pos.min(src.len());
  if start >= end {
    return None;
  }
  let upper = src[start..end].to_ascii_uppercase();
  // Quick reject -- avoid the expensive scan when there's no CREATE.
  if !upper.contains("CREATE") || !upper.contains("VIEW") {
    return None;
  }
  let bytes = upper.as_bytes();
  let n = bytes.len();
  // Skip leading whitespace.
  let mut i = 0usize;
  while i < n && bytes[i].is_ascii_whitespace() {
    i += 1;
  }
  // Must begin with CREATE (word-bounded).
  if !match_word(bytes, i, b"CREATE") {
    return None;
  }
  i += 6;
  // Consume optional `OR REPLACE`, `TEMP|TEMPORARY`, `RECURSIVE`,
  // `MATERIALIZED` modifiers in any plausible order before VIEW.
  loop {
    while i < n && bytes[i].is_ascii_whitespace() {
      i += 1;
    }
    if match_word(bytes, i, b"OR") {
      i += 2;
      while i < n && bytes[i].is_ascii_whitespace() {
        i += 1;
      }
      if !match_word(bytes, i, b"REPLACE") {
        return None;
      }
      i += 7;
      continue;
    }
    if match_word(bytes, i, b"TEMP") {
      i += 4;
      continue;
    }
    if match_word(bytes, i, b"TEMPORARY") {
      i += 9;
      continue;
    }
    if match_word(bytes, i, b"RECURSIVE") {
      i += 9;
      continue;
    }
    if match_word(bytes, i, b"MATERIALIZED") {
      i += 12;
      continue;
    }
    break;
  }
  while i < n && bytes[i].is_ascii_whitespace() {
    i += 1;
  }
  // Must reach VIEW.
  if !match_word(bytes, i, b"VIEW") {
    return None;
  }
  i += 4;
  while i < n && bytes[i].is_ascii_whitespace() {
    i += 1;
  }
  // Optional IF NOT EXISTS.
  if match_word(bytes, i, b"IF") {
    i += 2;
    while i < n && bytes[i].is_ascii_whitespace() {
      i += 1;
    }
    if !match_word(bytes, i, b"NOT") {
      return None;
    }
    i += 3;
    while i < n && bytes[i].is_ascii_whitespace() {
      i += 1;
    }
    if !match_word(bytes, i, b"EXISTS") {
      return None;
    }
    i += 6;
    while i < n && bytes[i].is_ascii_whitespace() {
      i += 1;
    }
  }
  // View name -- read until whitespace / `(` / end.
  let name_start = i;
  while i < n && !bytes[i].is_ascii_whitespace() && bytes[i] != b'(' {
    i += 1;
  }
  if i == name_start {
    return None;
  }
  while i < n && bytes[i].is_ascii_whitespace() {
    i += 1;
  }
  // Optional `(col1, col2, ...)` column-alias list.
  if i < n && bytes[i] == b'(' {
    let mut depth = 1i32;
    i += 1;
    while i < n && depth > 0 {
      match bytes[i] {
        b'(' => depth += 1,
        b')' => depth -= 1,
        _ => {},
      }
      i += 1;
    }
    while i < n && bytes[i].is_ascii_whitespace() {
      i += 1;
    }
  }
  // Optional WITH (...) options.
  if match_word(bytes, i, b"WITH") {
    let after = i + 4;
    let mut k = after;
    while k < n && bytes[k].is_ascii_whitespace() {
      k += 1;
    }
    if k < n && bytes[k] == b'(' {
      let mut depth = 1i32;
      i = k + 1;
      while i < n && depth > 0 {
        match bytes[i] {
          b'(' => depth += 1,
          b')' => depth -= 1,
          _ => {},
        }
        i += 1;
      }
      while i < n && bytes[i].is_ascii_whitespace() {
        i += 1;
      }
    }
  }
  // Now must see AS.
  if !match_word(bytes, i, b"AS") {
    return None;
  }
  let after_as = i + 2;
  if after_as > pos.saturating_sub(start) {
    return None;
  }
  Some(start + after_as)
}

fn match_word(bytes: &[u8], i: usize, word: &[u8]) -> bool {
  let n = bytes.len();
  if i + word.len() > n {
    return false;
  }
  if &bytes[i..i + word.len()] != word {
    return false;
  }
  if i + word.len() < n && is_word_cont(bytes[i + word.len()] as char) {
    return false;
  }
  if i > 0 && is_word_cont(bytes[i - 1] as char) {
    return false;
  }
  true
}

fn statement_start(src: &str, pos: usize) -> usize {
  let bytes = src.as_bytes();
  let mut in_single = false;
  let mut in_double = false;
  let mut last_semi = 0;
  let mut i = 0;
  while i < pos.min(bytes.len()) {
    let c = bytes[i] as char;
    if !in_double && c == '\'' && (i == 0 || bytes[i - 1] != b'\\') {
      in_single = !in_single;
    } else if !in_single && c == '"' {
      in_double = !in_double;
    } else if !in_single && !in_double && c == ';' {
      last_semi = i + 1;
    }
    i += 1;
  }
  last_semi
}

/// Walk the tokens to derive the current phase.
pub fn detect(src: &str, offset: TextSize) -> Phase {
  let pos: usize = offset.into();
  let pos = pos.min(src.len());

  // PG cast operator `::` -- when the immediately-preceding non-word
  // chars are `::`, the cursor is in type position. Works even when
  // the user has started typing a partial type name after the colons.
  if after_double_colon(src, pos) {
    return Phase::CastType;
  }

  // `OWNER TO` slot: ALTER TABLE/SCHEMA/SEQUENCE/FUNCTION/DATABASE/
  // TYPE/INDEX/VIEW/MATERIALIZED VIEW/DOMAIN/COLLATION ... OWNER TO
  // <cursor>. Routes to the role-list slot used by GRANT TO.
  if after_owner_to(src, pos) {
    return Phase::AfterGrantTo;
  }

  // `SET ROLE <cursor>` and `RESET ROLE` -- session role switch.
  // SET ROLE expects a role; RESET ROLE has no completion target
  // (handled by Phase::Unknown fallback).
  if after_set_role(src, pos) {
    return Phase::AfterGrantTo;
  }

  // Inside an unclosed dollar-quoted block: cursor is in a PL/pgSQL
  // or SQL function body. Route to body-style completion.
  if inside_dollar_quoted(src, pos) {
    // Narrow to expression-only when the cursor sits after a `:=`
    // (or `=` inside an UPDATE SET clause) on the same statement.
    if after_assignment(src, pos) {
      return Phase::PlpgsqlAssignRhs;
    }
    return Phase::PlpgsqlBody;
  }

  let raw_start = statement_start(src, pos);
  // If the cursor sits inside an unclosed `(SELECT ...` /
  // `(INSERT ...)` / `(UPDATE ...)` / `(DELETE ...)` -- typical CTE
  // body, subquery in FROM, or scalar subquery -- pretend the
  // statement starts at the subquery body. Otherwise the outer
  // `WITH t AS (` / `SELECT * FROM (` prefix collapses the walker to
  // Phase::Start and the user sees the whole DDL menu.
  //
  // CREATE VIEW / MATERIALIZED VIEW share the problem at top level
  // (no parens): the body after `AS` is a SELECT but the walker
  // doesn't recognize the CREATE VIEW prefix. Anchor at the body
  // start so it sees a fresh SELECT statement.
  let start = subquery_body_start(src, raw_start, pos)
    .or_else(|| create_view_body_start(src, raw_start, pos))
    .unwrap_or(raw_start);
  let toks = tokenise(src, start, pos);

  // Strip the trailing partial identifier the user is typing right now
  // -- it's not a "completed" token from the state-machine's point of
  // view. We keep it though if the previous char wasn't a word char
  // (e.g. after `,` or whitespace before nothing).
  // Easier rule: if the cursor sits right after a word char, drop the
  // last Tok::Word; it's the in-progress token.
  let mut effective = toks;
  if pos > 0 && is_word_cont(src.as_bytes()[pos - 1] as char) && matches!(effective.last(), Some(Tok::Word(_))) {
    effective.pop();
  }

  // Skip past a leading WITH ... CTE-list so the walker sees the inner
  // statement (SELECT/INSERT/UPDATE/DELETE) as if it started fresh.
  // Tracks paren depth and resumes at the first top-level statement
  // keyword after the last `)` of the CTE definitions. Without this,
  // the CTE name (an unknown Word at Start) collapses the walker into
  // Phase::Unknown and the menu degrades to a 600-item dump.
  let trimmed = strip_with_prefix(&effective);
  walk(trimmed)
}

/// Return the suffix of `toks` after a leading `WITH [RECURSIVE] <cte
/// list>`. If the first token isn't WITH, returns `toks` unchanged.
/// If the inner statement keyword hasn't been typed yet (cursor still
/// inside a CTE body), returns `toks` so the walker keeps the existing
/// behaviour for that position.
fn strip_with_prefix<'a, 'b>(toks: &'a [Tok<'b>]) -> &'a [Tok<'b>] {
  let Some(Tok::Word(w)) = toks.first() else {
    return toks;
  };
  if !w.eq_ignore_ascii_case("WITH") {
    return toks;
  }
  let mut depth = 0i32;
  for (i, t) in toks.iter().enumerate().skip(1) {
    match t {
      Tok::Punct('(') => depth += 1,
      Tok::Punct(')') => depth -= 1,
      Tok::Word(w) if depth == 0 => {
        let up = w.to_ascii_uppercase();
        if matches!(up.as_str(), "SELECT" | "INSERT" | "UPDATE" | "DELETE") {
          return &toks[i..];
        }
      },
      _ => {},
    }
  }
  toks
}

fn upper(s: &str) -> String {
  s.to_ascii_uppercase()
}

fn walk(toks: &[Tok<'_>]) -> Phase {
  use Phase::*;
  let mut phase = Start;
  let mut i = 0;
  while i < toks.len() {
    let t = &toks[i];
    phase = match (&phase, t) {
      // Top-level statement starters
      (Start, Tok::Word(w)) => match upper(w).as_str() {
        "SELECT" => SelectProjection,
        "INSERT" => AfterInsert,
        "UPDATE" => AfterUpdate,
        "DELETE" => AfterDelete,
        "CREATE" => AfterCreate,
        "ALTER" => AfterAlter,
        "DROP" => AfterDrop,
        "GRANT" => AfterGrantOrRevoke,
        "REVOKE" => AfterGrantOrRevoke,
        "WITH" => Start, // CTE; fall through to inner SELECT
        _ => Unknown,
      },

      // ----- SELECT branch -----
      (SelectProjection, Tok::Word(w)) => match upper(w).as_str() {
        "FROM" => ExpectTable,
        "DISTINCT" | "ALL" => SelectProjection,
        _ => InProjection,
      },
      (SelectProjection, Tok::Punct('*')) => AfterStar,
      (SelectProjection, Tok::Punct(',')) => NextProjection,
      (SelectProjection, Tok::Punct('(')) => InProjection,
      (SelectProjection, _) => InProjection,

      (NextProjection, Tok::Word(w)) => match upper(w).as_str() {
        "FROM" => ExpectTable,
        _ => InProjection,
      },
      (NextProjection, Tok::Punct('*')) => AfterStar,
      (NextProjection, _) => InProjection,

      (InProjection, Tok::Word(w)) => match upper(w).as_str() {
        "FROM" => ExpectTable,
        "AS" => ProjectionAlias,
        _ => InProjection,
      },
      (InProjection, Tok::Punct(',')) => NextProjection,
      (InProjection, Tok::Punct('.')) => InProjection,
      (InProjection, Tok::Punct('(')) => InProjection,
      (InProjection, _) => InProjection,

      (ProjectionAlias, Tok::Word(_)) => InProjection,
      (ProjectionAlias, _) => ProjectionAlias,

      (AfterStar, Tok::Word(w)) => match upper(w).as_str() {
        "FROM" => ExpectTable,
        _ => InProjection,
      },
      (AfterStar, Tok::Punct(',')) => NextProjection,
      (AfterStar, _) => AfterStar,

      // ----- FROM / table list / joins -----
      (ExpectTable, Tok::Word(_)) => AfterTable,
      (ExpectTable, _) => ExpectTable,

      (AfterTable, Tok::Word(w)) => match upper(w).as_str() {
        "AS" => AfterTable,
        "JOIN" => ExpectTable,
        "INNER" | "LEFT" | "RIGHT" | "FULL" | "CROSS" => JoinModifier,
        "WHERE" => WhereClause,
        "GROUP" => AfterGroup,
        "ORDER" => AfterOrder,
        "HAVING" => HavingClause,
        "LIMIT" => LimitClause,
        "OFFSET" => OffsetClause,
        "ON" => OnClause,
        "USING" => UsingClause,
        _ => AfterTable, // alias name
      },
      (AfterTable, Tok::Punct(',')) => ExpectTable,
      (AfterTable, Tok::Punct(';')) => Start,
      (AfterTable, _) => AfterTable,

      (JoinModifier, Tok::Word(w)) => match upper(w).as_str() {
        "JOIN" => ExpectTable,
        "OUTER" => JoinModifier,
        _ => JoinModifier,
      },
      (JoinModifier, _) => JoinModifier,

      (OnClause, Tok::Word(w)) => match upper(w).as_str() {
        "AND" | "OR" => OnClause,
        "JOIN" => ExpectTable,
        "WHERE" => WhereClause,
        "GROUP" => AfterGroup,
        "ORDER" => AfterOrder,
        "INNER" | "LEFT" | "RIGHT" | "FULL" | "CROSS" => JoinModifier,
        _ => InPredicate,
      },
      (OnClause, Tok::Punct(';')) => Start,
      (OnClause, _) => OnClause,

      // ----- WHERE / predicate -----
      (WhereClause, Tok::Word(w)) => match upper(w).as_str() {
        "GROUP" => AfterGroup,
        "ORDER" => AfterOrder,
        "HAVING" => HavingClause,
        "LIMIT" => LimitClause,
        "RETURNING" => ReturningClause,
        _ => InPredicate,
      },
      (WhereClause, Tok::Punct(';')) => Start,
      (WhereClause, _) => WhereClause,

      (InPredicate, Tok::Word(w)) => match upper(w).as_str() {
        "AND" | "OR" | "NOT" => WhereClause,
        "GROUP" => AfterGroup,
        "ORDER" => AfterOrder,
        "HAVING" => HavingClause,
        "LIMIT" => LimitClause,
        "RETURNING" => ReturningClause,
        _ => InPredicate,
      },
      (InPredicate, Tok::Punct(';')) => Start,
      (InPredicate, _) => InPredicate,

      // ----- GROUP BY / ORDER BY -----
      (AfterGroup, Tok::Word(w)) if upper(w) == "BY" => GroupByList,
      (AfterGroup, _) => AfterGroup,
      (GroupByList, Tok::Word(w)) => match upper(w).as_str() {
        "ORDER" => AfterOrder,
        "HAVING" => HavingClause,
        "LIMIT" => LimitClause,
        _ => GroupByList,
      },
      (GroupByList, Tok::Punct(';')) => Start,
      (GroupByList, _) => GroupByList,

      (AfterOrder, Tok::Word(w)) if upper(w) == "BY" => OrderByList,
      (AfterOrder, _) => AfterOrder,
      (OrderByList, Tok::Word(w)) => match upper(w).as_str() {
        "LIMIT" => LimitClause,
        _ => OrderByList,
      },
      (OrderByList, Tok::Punct(';')) => Start,
      (OrderByList, _) => OrderByList,

      (HavingClause, Tok::Word(w)) => match upper(w).as_str() {
        "ORDER" => AfterOrder,
        "LIMIT" => LimitClause,
        _ => HavingClause,
      },
      (HavingClause, Tok::Punct(';')) => Start,
      (HavingClause, _) => HavingClause,

      (LimitClause, Tok::Word(w)) if upper(w) == "OFFSET" => OffsetClause,
      (LimitClause, Tok::Punct(';')) => Start,
      (LimitClause, _) => LimitClause,
      (OffsetClause, Tok::Punct(';')) => Start,
      (OffsetClause, _) => OffsetClause,

      // ----- INSERT / UPDATE / DELETE skeletons -----
      (AfterInsert, Tok::Word(w)) if upper(w) == "INTO" => AfterInsertTable,
      (AfterInsert, _) => AfterInsert,
      (AfterInsertTable, Tok::Word(_)) => InsertColumnList,
      (AfterInsertTable, _) => AfterInsertTable,
      (InsertColumnList, Tok::Word(w)) if upper(w) == "VALUES" => InsertValuesList,
      (InsertColumnList, Tok::Word(w)) if upper(w) == "SELECT" => SelectProjection,
      (InsertColumnList, Tok::Punct(';')) => Start,
      (InsertColumnList, _) => InsertColumnList,
      (InsertExpectValues, _) => InsertValuesList,
      (InsertValuesList, Tok::Word(w)) if upper(w) == "RETURNING" => ReturningClause,
      (InsertValuesList, Tok::Punct(';')) => Start,
      (InsertValuesList, _) => InsertValuesList,

      (AfterUpdate, Tok::Word(_)) => AfterUpdateTable,
      (AfterUpdate, _) => AfterUpdate,
      (AfterUpdateTable, Tok::Word(w)) if upper(w) == "SET" => UpdateAssignment,
      (AfterUpdateTable, _) => AfterUpdateTable,
      (UpdateAssignment, Tok::Word(w)) => match upper(w).as_str() {
        "WHERE" => WhereClause,
        "RETURNING" => ReturningClause,
        _ => UpdateAssignment,
      },
      (UpdateAssignment, Tok::Punct(';')) => Start,
      (UpdateAssignment, _) => UpdateAssignment,

      (AfterDelete, Tok::Word(w)) if upper(w) == "FROM" => ExpectTable,
      (AfterDelete, _) => AfterDelete,

      // RETURNING accepts a comma-separated column list followed by `;`.
      (ReturningClause, Tok::Punct(';')) => Start,
      (ReturningClause, _) => ReturningClause,

      // ----- DDL -----
      (AfterAlter, Tok::Word(w)) if upper(w) == "TABLE" => AfterAlterTableExpectName,
      (AfterAlter, _) => Unknown,
      (AfterCreate, _) | (AfterDrop, _) => Unknown,

      // ALTER TABLE [IF EXISTS] [ONLY] <name>
      (AfterAlterTableExpectName, Tok::Word(w)) => match upper(w).as_str() {
        "IF" | "EXISTS" | "ONLY" => AfterAlterTableExpectName,
        _ => AfterAlterTableName,
      },
      (AfterAlterTableExpectName, Tok::Punct('.')) => AfterAlterTableExpectName, // schema.name
      (AfterAlterTableExpectName, _) => AfterAlterTableExpectName,

      (AfterAlterTableName, Tok::Punct(';')) => Start,
      (AfterAlterTableName, _) => AfterAlterTableName,

      // GRANT / REVOKE: <privilege> [, <privilege>]* ON <target> TO/FROM <role>
      (AfterGrantOrRevoke, Tok::Word(w)) => match upper(w).as_str() {
        "ON" => AfterGrantOn,
        "TO" | "FROM" => AfterGrantTo,
        _ => AfterGrantOrRevoke, // privilege keyword (SELECT / ALL / ...)
      },
      (AfterGrantOrRevoke, Tok::Punct(',')) => AfterGrantOrRevoke,
      (AfterGrantOrRevoke, Tok::Punct(';')) => Start,
      (AfterGrantOrRevoke, _) => AfterGrantOrRevoke,

      (AfterGrantOn, Tok::Word(w)) => match upper(w).as_str() {
        "TO" | "FROM" => AfterGrantTo,
        _ => AfterGrantOn, // target name / TABLE / SEQUENCE / ...
      },
      (AfterGrantOn, Tok::Punct(';')) => Start,
      (AfterGrantOn, _) => AfterGrantOn,

      (AfterGrantTo, Tok::Punct(';')) => Start,
      (AfterGrantTo, _) => AfterGrantTo,

      // Reset on semicolon from anywhere.
      (_, Tok::Punct(';')) => Start,
      (_, _) => phase.clone(),
    };
    i += 1;
  }
  phase
}

/// Returns true when `pos` sits inside an unclosed `$tag$ ... $tag$`
/// block. Scans from buffer start counting opens vs closes; an odd
/// count puts the cursor inside the body.
fn inside_dollar_quoted(src: &str, pos: usize) -> bool {
  let bytes = src.as_bytes();
  let mut i = 0usize;
  let mut current_tag: Option<String> = None;
  while i < pos.min(bytes.len()) {
    if let Some(tag) = &current_tag {
      let closer = format!("${tag}$");
      if src[i..].starts_with(&closer) {
        i += closer.len();
        current_tag = None;
        continue;
      }
      i += 1;
      continue;
    }
    if bytes[i] == b'$'
      && let Some(end) = src[i + 1..].find('$')
    {
      let tag = &src[i + 1..i + 1 + end];
      if tag.chars().all(|c| c.is_alphanumeric() || c == '_') {
        current_tag = Some(tag.to_string());
        i += 1 + end + 1;
        continue;
      }
    }
    i += 1;
  }
  current_tag.is_some()
}

/// True when the most recent `:=` (or `=` inside an UPDATE SET clause)
/// before `pos` lies on the same PL/pgSQL statement -- i.e. between the
/// last `;` and the cursor. Strings and parens are respected so a `:=`
/// inside a literal doesn't fake-trigger.
fn after_assignment(src: &str, pos: usize) -> bool {
  let bytes = src.as_bytes();
  let upper_pos = pos.min(bytes.len());
  // Find the previous statement terminator.
  let start = src[..upper_pos].rfind(';').map(|i| i + 1).unwrap_or(0);
  let slice = &src[start..upper_pos];
  let bs = slice.as_bytes();
  let n = bs.len();
  let mut depth = 0i32;
  let mut i = 0usize;
  let mut found = false;
  while i < n {
    match bs[i] {
      b'\'' => {
        i += 1;
        while i < n {
          if bs[i] == b'\'' {
            i += 1;
            break;
          }
          i += 1;
        }
      },
      b'(' => {
        depth += 1;
        i += 1;
      },
      b')' => {
        depth -= 1;
        i += 1;
      },
      b':' if depth == 0 && i + 1 < n && bs[i + 1] == b'=' => {
        found = true;
        i += 2;
      },
      _ => i += 1,
    }
  }
  found
}
