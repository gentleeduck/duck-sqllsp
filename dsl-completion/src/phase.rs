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

  let start = statement_start(src, pos);
  let toks = tokenise(src, start, pos);

  // Strip the trailing partial identifier the user is typing right now
  // -- it's not a "completed" token from the state-machine's point of
  // view. We keep it though if the previous char wasn't a word char
  // (e.g. after `,` or whitespace before nothing).
  // Easier rule: if the cursor sits right after a word char, drop the
  // last Tok::Word; it's the in-progress token.
  let mut effective = toks;
  if pos > 0 && is_word_cont(src.as_bytes()[pos - 1] as char) {
    if matches!(effective.last(), Some(Tok::Word(_))) {
      effective.pop();
    }
  }

  walk(&effective)
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
      (InsertValuesList, Tok::Punct(';')) => Start,
      (InsertValuesList, _) => InsertValuesList,

      (AfterUpdate, Tok::Word(_)) => AfterUpdateTable,
      (AfterUpdate, _) => AfterUpdate,
      (AfterUpdateTable, Tok::Word(w)) if upper(w) == "SET" => UpdateAssignment,
      (AfterUpdateTable, _) => AfterUpdateTable,
      (UpdateAssignment, Tok::Word(w)) => match upper(w).as_str() {
        "WHERE" => WhereClause,
        _ => UpdateAssignment,
      },
      (UpdateAssignment, Tok::Punct(';')) => Start,
      (UpdateAssignment, _) => UpdateAssignment,

      (AfterDelete, Tok::Word(w)) if upper(w) == "FROM" => ExpectTable,
      (AfterDelete, _) => AfterDelete,

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
    if bytes[i] == b'$' {
      if let Some(end) = src[i + 1..].find('$') {
        let tag = &src[i + 1..i + 1 + end];
        if tag.chars().all(|c| c.is_alphanumeric() || c == '_') {
          current_tag = Some(tag.to_string());
          i += 1 + end + 1;
          continue;
        }
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
