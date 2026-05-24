//! CREATE TABLE sub-phase analyzer.
//!
//! When the cursor sits inside the parenthesised body of a CREATE TABLE
//! statement we want much narrower completion than the general SQL
//! state machine offers: types when writing a column type, column
//! constraint keywords (NOT NULL / DEFAULT / ...) after the type,
//! tables after REFERENCES, and so on. The user explicitly does NOT
//! want all-table-column completion when they are typing a column
//! identifier.
//!
//! Strategy:
//!   1. Locate every `CREATE TABLE [IF NOT EXISTS] <name> (` whose body
//!      contains the cursor.
//!   2. From the body-open paren up to the cursor, tokenize and walk an
//!      entry-level state machine that resets on each top-level `,`.
//!   3. Return a [`Phase`] reflecting what the user is about to type.

use crate::phase::Phase;
use text_size::TextSize;

/// Try to classify the cursor's position inside an enclosing CREATE
/// TABLE body. Returns None when the cursor is not inside one.
pub fn detect(source: &str, offset: TextSize) -> Option<Phase> {
  let pos: usize = offset.into();
  let pos = pos.min(source.len());

  // CREATE TABLE name?
  if let Some(p) = detect_expect_name(source, pos) {
    return Some(p);
  }

  let body = find_enclosing_body(source, pos)?;
  let entry = current_entry(source, body.open + 1, pos);
  let enclosing = enclosing_table_name(source, body.open);
  Some(classify_entry(source, &entry, enclosing.as_deref()))
}

/// Walk back from the body-open paren to capture the CREATE TABLE name.
/// Lets us suggest columns of THIS table inside `PRIMARY KEY (...)` etc.
fn enclosing_table_name(source: &str, open: usize) -> Option<String> {
  let before = &source[..open];
  let upper = before.to_ascii_uppercase();
  let kw_pos = upper.rfind("CREATE TABLE")?;
  let after = &before[kw_pos + "CREATE TABLE".len()..];
  let after_trim = after.trim_start();
  let after_trim = after_trim
    .strip_prefix("IF NOT EXISTS")
    .or_else(|| after_trim.strip_prefix("if not exists"))
    .map(|s| s.trim_start())
    .unwrap_or(after_trim);
  let name: String = after_trim.chars().take_while(|c| c.is_alphanumeric() || *c == '_' || *c == '.').collect();
  if name.is_empty() {
    return None;
  }
  // Strip schema qualifier `public.users` -> `users`.
  Some(name.rsplit('.').next().unwrap_or(&name).to_string())
}

/// Cursor is right after `CREATE TABLE [IF NOT EXISTS] ` -- expect a name.
fn detect_expect_name(source: &str, pos: usize) -> Option<Phase> {
  let before = &source[..pos];
  let trimmed_upper = strip_trailing_word(before).to_uppercase();
  if trimmed_upper.ends_with("CREATE TABLE") || trimmed_upper.ends_with("CREATE TABLE IF NOT EXISTS") {
    return Some(Phase::CtlExpectTableName);
  }
  None
}

struct Body {
  open: usize,
  /// Inclusive (exclusive paren index doesn't matter for our purposes;
  /// we only need open).
  #[allow(dead_code)]
  close: usize,
}

/// Scan source for `CREATE TABLE ... (` whose open-paren / close-paren
/// span contains `pos`. Returns the open paren index. Quotes and dollar
/// quotes are honoured so a `(` inside a literal does not confuse us.
fn find_enclosing_body(source: &str, pos: usize) -> Option<Body> {
  let bytes = source.as_bytes();
  let mut i = 0usize;
  let upper_src = source.to_ascii_uppercase();

  while i < bytes.len() {
    // Skip strings + comments quickly.
    let c = bytes[i] as char;
    if c == '-' && i + 1 < bytes.len() && bytes[i + 1] == b'-' {
      while i < bytes.len() && bytes[i] != b'\n' {
        i += 1;
      }
      continue;
    }
    if c == '/' && i + 1 < bytes.len() && bytes[i + 1] == b'*' {
      i += 2;
      while i + 1 < bytes.len() && !(bytes[i] == b'*' && bytes[i + 1] == b'/') {
        i += 1;
      }
      i = (i + 2).min(bytes.len());
      continue;
    }
    if c == '\'' {
      i += 1;
      while i < bytes.len() && !(bytes[i] == b'\'' && bytes[i - 1] != b'\\') {
        i += 1;
      }
      i = (i + 1).min(bytes.len());
      continue;
    }
    if c == '$' {
      if let Some(end) = source[i + 1..].find('$') {
        let tag = &source[i + 1..i + 1 + end];
        if tag.chars().all(|ch| ch.is_alphanumeric() || ch == '_') {
          let closer = format!("${tag}$");
          let body_start = i + 1 + end + 1;
          if let Some(off) = source[body_start..].find(&closer) {
            i = body_start + off + closer.len();
            continue;
          }
          return None;
        }
      }
    }
    // Look for "CREATE TABLE" as a whole word at i. Byte compare
    // so a non-ASCII byte at `i` (mid multi-byte char) can't
    // panic-slice the uppercased string.
    const CT: &[u8] = b"CREATE TABLE";
    if i + CT.len() <= bytes.len()
      && upper_src.as_bytes()[i..i + CT.len()].eq_ignore_ascii_case(CT)
      && (i == 0 || !is_word(bytes[i - 1] as char))
      && bytes.get(i + CT.len()).map_or(true, |b| !is_word(*b as char))
    {
      // Walk forward to the next '(' that is not inside a quote.
      let mut j = i + "CREATE TABLE".len();
      while j < bytes.len() && bytes[j] as char != '(' {
        if bytes[j] == b';' {
          break;
        }
        j += 1;
      }
      if j < bytes.len() && bytes[j] as char == '(' {
        let open = j;
        // Find matching close paren.
        let mut depth: i32 = 1;
        let mut k = open + 1;
        while k < bytes.len() && depth > 0 {
          match bytes[k] as char {
            '(' => depth += 1,
            ')' => depth -= 1,
            '\'' => {
              k += 1;
              while k < bytes.len() && !(bytes[k] == b'\'' && bytes[k - 1] != b'\\') {
                k += 1;
              }
            },
            _ => {},
          }
          k += 1;
        }
        // When the closing paren is missing (user still typing
        // the body), treat the end-of-source as the close so
        // any cursor inside still resolves to this body.
        let close = if depth > 0 { bytes.len() } else { k.saturating_sub(1) };
        if pos > open && pos <= close {
          return Some(Body { open, close });
        }
        i = close + 1;
        continue;
      }
    }
    i += 1;
  }
  None
}

/// Slice of body text since the last top-level comma (or the open paren).
struct Entry<'a> {
  text: &'a str,
}

fn current_entry<'a>(source: &'a str, body_start: usize, cursor: usize) -> Entry<'a> {
  let bytes = source.as_bytes();
  let end = cursor.min(bytes.len());
  let mut depth: i32 = 0;
  let mut last_split = body_start;
  let mut i = body_start;
  while i < end {
    let c = bytes[i] as char;
    if c == '(' {
      depth += 1;
    } else if c == ')' {
      depth -= 1;
    } else if c == ',' && depth == 0 {
      last_split = i + 1;
    }
    i += 1;
  }
  Entry { text: &source[last_split..end] }
}

fn classify_entry(_source: &str, entry: &Entry<'_>, enclosing: Option<&str>) -> Phase {
  let raw = entry.text.trim_start();
  if raw.is_empty() {
    return Phase::CtlBodyStart;
  }
  // Was the user mid-typing a word right before the cursor? When the
  // last char is a word char and not separated by whitespace from what
  // came before, that token is "in progress" -- it belongs to the
  // next slot we expect the user to fill. Strip it so the phase
  // reflects what they are typing, not what they have committed.
  let bytes = raw.as_bytes();
  let trailing_partial = match bytes.last() {
    Some(&b) if (b as char).is_alphanumeric() || b == b'_' => true,
    _ => false,
  };
  let committed: &str = if trailing_partial {
    let mut end = raw.len();
    while end > 0 {
      let c = bytes[end - 1] as char;
      if c.is_alphanumeric() || c == '_' {
        end -= 1;
      } else {
        break;
      }
    }
    &raw[..end]
  } else {
    raw
  };
  let committed = committed.trim_end();
  let upper = committed.to_uppercase();

  // Constraint-line entries.
  if upper.starts_with("CONSTRAINT") {
    let rest = strip_kw(committed, "CONSTRAINT");
    // Strip the constraint name (single identifier) before classifying
    // the rest -- so `CONSTRAINT pk_users PRIMARY KEY (` lands in the
    // column-list arm below.
    let after_name = {
      let trimmed = rest.trim_start();
      let ident_len = trimmed.chars().take_while(|c| c.is_alphanumeric() || *c == '_').count();
      &trimmed[ident_len..]
    };
    let after_upper = after_name.trim_start().to_uppercase();
    if after_upper.starts_with("PRIMARY KEY") || after_upper.starts_with("UNIQUE") {
      if inside_paren(committed) {
        if let Some(t) = enclosing.map(str::to_string) {
          return Phase::CtlExpectFkColumn { table: t };
        }
      }
      return Phase::Unknown;
    }
    if after_upper.starts_with("CHECK") {
      // Named CHECK constraint: CONSTRAINT <name> CHECK ( <expr> )
      // -- inside the paren the user types an arbitrary expression,
      // so surface columns + the full PG function library +
      // expression keywords (sql, char_length, length, now, ...).
      if inside_paren(committed) {
        return Phase::CtlCheckExpr { table: enclosing.map(str::to_string) };
      }
      return Phase::Unknown;
    }
    if after_upper.starts_with("FOREIGN KEY") {
      if !after_upper.contains("REFERENCES") && inside_paren(committed) {
        if let Some(t) = enclosing.map(str::to_string) {
          return Phase::CtlExpectFkColumn { table: t };
        }
      }
      return fk_phase(&after_upper);
    }
    if let Some(pos) = after_upper.rfind("REFERENCES") {
      let after = after_upper[pos + "REFERENCES".len()..].trim_start();
      return fk_phase_after_references(after);
    }
    return if has_at_most_one_identifier(rest) {
      Phase::CtlExpectConstraintName
    } else {
      Phase::CtlExpectConstraintKind
    };
  }
  // Bare table-level PRIMARY KEY / UNIQUE clause -- once the cursor is
  // inside the parens, suggest columns of the table being created.
  if upper.starts_with("PRIMARY KEY") || upper.starts_with("UNIQUE") {
    if inside_paren(committed) {
      if let Some(t) = enclosing.map(str::to_string) {
        return Phase::CtlExpectFkColumn { table: t };
      }
    }
    return Phase::Unknown;
  }
  if upper.starts_with("CHECK") {
    if inside_paren(committed) {
      return Phase::CtlCheckExpr { table: enclosing.map(str::to_string) };
    }
    return Phase::Unknown;
  }
  if upper.starts_with("FOREIGN KEY") {
    // Inside the LEFT paren (the local-column list) but before REFERENCES
    // -- suggest columns of this table.
    if !upper.contains("REFERENCES") && inside_paren(committed) {
      if let Some(t) = enclosing.map(str::to_string) {
        return Phase::CtlExpectFkColumn { table: t };
      }
    }
    return fk_phase(&upper);
  }
  if let Some(pos) = upper.rfind("REFERENCES") {
    let after = upper[pos + "REFERENCES".len()..].trim_start();
    return fk_phase_after_references(after);
  }

  // Column declaration: <ident> [TYPE] [constraints ...]
  let committed_tokens = simple_tokens(committed);
  match committed_tokens.len() {
    // Nothing committed yet -- user is typing the column name. We
    // do NOT want catalog columns / functions to appear here, just
    // the alternative starter keywords (CONSTRAINT, PRIMARY KEY, ...).
    0 => Phase::CtlBodyStart,
    // One identifier committed -- now expecting the type.
    1 => Phase::CtlExpectType,
    // Identifier + type-word committed (or more). Past the type
    // stage: expecting column constraints next.
    _ => {
      let second = committed_tokens[1].as_str();
      if is_complete_type_token(second) || committed_tokens.len() > 2 {
        Phase::CtlExpectColumnConstraint
      } else {
        Phase::CtlExpectType
      }
    },
  }
}

/// True when the most recently opened `(` in `text` is still unclosed --
/// i.e. the cursor is inside a paren group. Respects single-quoted
/// strings so an apostrophe doesn't fake-open the group.
fn inside_paren(text: &str) -> bool {
  let bytes = text.as_bytes();
  let n = bytes.len();
  let mut depth = 0i32;
  let mut i = 0usize;
  while i < n {
    match bytes[i] {
      b'\'' => {
        i += 1;
        while i < n {
          if bytes[i] == b'\'' {
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
      _ => i += 1,
    }
  }
  depth > 0
}

fn fk_phase(upper: &str) -> Phase {
  if let Some(p) = upper.find("REFERENCES") {
    let after = upper[p + "REFERENCES".len()..].trim_start();
    return fk_phase_after_references(after);
  }
  Phase::Unknown
}

fn fk_phase_after_references(after_upper: &str) -> Phase {
  if after_upper.is_empty() {
    return Phase::CtlExpectFkTable {};
  }
  let table_tok = after_upper.split(|c: char| c.is_whitespace() || c == '(').next().unwrap_or("");
  if table_tok.is_empty() {
    return Phase::CtlExpectFkTable {};
  }
  if after_upper[table_tok.len()..].trim_start().starts_with('(') {
    return Phase::CtlExpectFkColumn { table: table_tok.to_ascii_lowercase() };
  }
  Phase::CtlExpectFkTable {}
}

// ---------------------------------------------------------------------------
// helpers
// ---------------------------------------------------------------------------

fn is_word(c: char) -> bool {
  c.is_alphanumeric() || c == '_'
}

fn strip_trailing_word(s: &str) -> &str {
  let bytes = s.as_bytes();
  let mut end = bytes.len();
  while end > 0 && bytes[end - 1].is_ascii() && is_word(bytes[end - 1] as char) {
    end -= 1;
  }
  while end > 0 && bytes[end - 1].is_ascii_whitespace() {
    end -= 1;
  }
  // Ensure we land on a UTF-8 char boundary -- non-ASCII tails can
  // leave `end` inside a multi-byte sequence.
  while end > 0 && !s.is_char_boundary(end) {
    end -= 1;
  }
  &s[..end]
}

fn strip_kw<'a>(text: &'a str, kw: &str) -> &'a str {
  let kw_len = kw.len();
  if text.len() >= kw_len && text[..kw_len].eq_ignore_ascii_case(kw) {
    return &text[kw_len..];
  }
  text
}

fn has_at_most_one_identifier(s: &str) -> bool {
  simple_tokens(s).len() <= 1
}

/// Split into rough whitespace-bounded tokens, but respect parenthesised
/// blocks so `NUMERIC(10,2)` stays one token.
fn simple_tokens(s: &str) -> Vec<String> {
  let mut out: Vec<String> = Vec::new();
  let mut cur = String::new();
  let mut depth: i32 = 0;
  for c in s.chars() {
    match c {
      '(' => {
        depth += 1;
        cur.push(c);
      },
      ')' => {
        depth -= 1;
        cur.push(c);
      },
      _ if c.is_whitespace() && depth == 0 => {
        if !cur.is_empty() {
          out.push(std::mem::take(&mut cur));
        }
      },
      _ => cur.push(c),
    }
  }
  if !cur.is_empty() {
    out.push(cur);
  }
  out
}

fn is_complete_type_token(tok: &str) -> bool {
  let upper = tok.to_ascii_uppercase();
  if tok.ends_with(')') {
    return true;
  }
  matches!(
    upper.as_str(),
    "UUID"
      | "TEXT"
      | "DATE"
      | "TIME"
      | "TIMESTAMP"
      | "TIMESTAMPTZ"
      | "BOOLEAN"
      | "BOOL"
      | "INT"
      | "INTEGER"
      | "BIGINT"
      | "SMALLINT"
      | "SERIAL"
      | "BIGSERIAL"
      | "SMALLSERIAL"
      | "REAL"
      | "JSON"
      | "JSONB"
      | "BYTEA"
      | "INET"
      | "CIDR"
      | "MACADDR"
      | "INTERVAL"
      | "MONEY"
  )
}
