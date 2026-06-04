//! DataGrip-style CREATE TABLE formatter.
//!
//! Reformats every `CREATE TABLE [IF NOT EXISTS] <name> (...);` block in
//! the source so columns align in three padded columns: name, type +
//! length, then any tail (`NOT NULL`, `DEFAULT ...`, inline references).
//! Constraints (CONSTRAINT, PRIMARY KEY, FOREIGN KEY, UNIQUE, CHECK,
//! LIKE) are emitted after the columns, separated by a blank line.
//!
//! Non-CREATE-TABLE text is passed through unchanged so this layer can
//! cooperate with `sql-formatter` (which handles the rest).

use crate::style::CreateTableStyle;

pub fn rewrite(source: &str, style: &CreateTableStyle) -> String {
  let stage1 = if style.align_columns { rewrite_tables(source, style) } else { source.to_string() };
  let stage2 = break_function_headers(&stage1);
  let stage3 = break_trigger_headers(&stage2);
  let stage4 = break_index_headers(&stage3);
  // Keep FK clauses inline (REFERENCES / ON DELETE / ON UPDATE / MATCH /
  // DEFERRABLE) -- align step already produced single-line constraints
  // and breaking them again pushes the closing `)` onto its own line.
  let stage5 = stage4;
  let stage6 = if style.group_indexes { collapse_index_runs(&stage5) } else { stage5 };
  align_plpgsql_bodies(&stage6)
}

/// Re-indent statements inside `$$ ... $$` bodies. BEGIN / IF / LOOP /
/// CASE bump the indent; the matching END / END IF / END LOOP /
/// END CASE pop it. Statements are split on top-level `;` and emitted
/// one per line.
fn align_plpgsql_bodies(source: &str) -> String {
  let bytes = source.as_bytes();
  let n = bytes.len();
  let mut out = String::with_capacity(n);
  let mut i = 0;
  while i < n {
    // Look for `$$` opener that follows `AS ` (function body).
    if i + 2 <= n && bytes[i] == b'$' && bytes[i + 1] == b'$' {
      out.push_str("$$");
      i += 2;
      // Find matching `$$`.
      let body_start = i;
      let mut close = body_start;
      while close + 2 <= n && !(bytes[close] == b'$' && bytes[close + 1] == b'$') {
        close += 1;
      }
      if close + 2 > n {
        out.push_str(&source[body_start..]);
        return out;
      }
      let body = &source[body_start..close];
      let aligned = align_body_text(body);
      out.push_str(&aligned);
      out.push_str("$$");
      i = close + 2;
      continue;
    }
    i = crate::push_one_char(&mut out, source, i);
  }
  out
}

/// Split a PL/pgSQL body on top-level `;` and re-emit one statement
/// per line at the current depth. BEGIN/IF/LOOP/CASE increment depth.
fn align_body_text(body: &str) -> String {
  let trimmed = body.trim_matches(|c: char| c == '\n' || c == '\r');
  if trimmed.is_empty() {
    return body.to_string();
  }
  let bytes = trimmed.as_bytes();
  let n = bytes.len();
  let mut stmts: Vec<String> = Vec::new();
  let mut cur = String::new();
  let mut i = 0;
  // PL/pgSQL block markers (BEGIN, DECLARE, EXCEPTION) appear on
  // their own line WITHOUT a trailing semicolon. Treat them as
  // statement boundaries too so the depth machine can react to them
  // before the following statement gets emitted.
  let flush = |cur: &mut String, stmts: &mut Vec<String>| {
    let t = cur.trim().to_string();
    if !t.is_empty() {
      stmts.push(t);
    }
    cur.clear();
  };
  while i < n {
    match bytes[i] {
      b'\'' => {
        cur.push('\'');
        i += 1;
        while i < n && bytes[i] != b'\'' {
          i = crate::push_one_char(&mut cur, trimmed, i);
        }
        if i < n {
          cur.push('\'');
          i += 1;
        }
      },
      b'-' if i + 1 < n && bytes[i + 1] == b'-' => {
        while i < n && bytes[i] != b'\n' {
          i = crate::push_one_char(&mut cur, trimmed, i);
        }
      },
      b';' => {
        cur.push(';');
        flush(&mut cur, &mut stmts);
        i += 1;
      },
      _ => {
        // Recognise bare block markers at the start of a fresh
        // statement: BEGIN / DECLARE / EXCEPTION followed by
        // whitespace (not `;`, not an identifier char).
        if cur.trim().is_empty() {
          let mut matched = false;
          for marker in ["BEGIN", "DECLARE", "EXCEPTION"] {
            let w = marker.len();
            if i + w <= n {
              let head = &trimmed[i..i + w];
              if head.eq_ignore_ascii_case(marker)
                && (i + w == n || !(bytes[i + w].is_ascii_alphanumeric() || bytes[i + w] == b'_'))
              {
                // Skip post-marker whitespace so we can
                // peek at the next byte cheaply.
                let mut k = i + w;
                while k < n && bytes[k].is_ascii_whitespace() {
                  k += 1;
                }
                // If the user wrote `BEGIN;` explicitly,
                // fall through and let the `;` branch
                // emit the statement normally.
                if k < n && bytes[k] == b';' {
                  break;
                }
                cur.push_str(marker);
                flush(&mut cur, &mut stmts);
                i += w;
                matched = true;
                break;
              }
            }
          }
          if matched {
            continue;
          }
        }
        i = crate::push_one_char(&mut cur, trimmed, i);
      },
    }
  }
  let tail = cur.trim().to_string();
  if !tail.is_empty() {
    stmts.push(tail);
  }

  let mut out = String::from("\n");
  let mut depth: usize = 0;
  for raw in &stmts {
    // Collapse multi-line internal whitespace so a wrapped IF / WHILE
    // condition lands on a single line. String literals + line comments
    // are preserved verbatim by collapse_one_line. Then re-split the
    // collapsed text on ` THEN ` so IF body content moves onto its own
    // indented line.
    let collapsed_full = collapse_one_line(raw.trim());
    let up_full = collapsed_full.to_ascii_uppercase();
    let (collapsed, then_tail): (String, Option<String>) =
      if (up_full.starts_with("IF ") || up_full.starts_with("ELSIF ") || up_full.starts_with("WHEN "))
        && let Some(then_at) = find_then(&up_full)
      {
        let head_end = then_at + "THEN".len();
        let head = collapsed_full[..head_end].trim_end().to_string();
        let tail = collapsed_full[head_end..].trim().to_string();
        if tail.is_empty() { (head, None) } else { (head, Some(tail)) }
      } else {
        (collapsed_full, None)
      };
    let s = collapsed.as_str();
    let up = s.to_ascii_uppercase();
    // DECLARE / BEGIN / EXCEPTION / END are peer-level section
    // markers of the *same* PL/pgSQL block: each one closes the
    // prior section and opens its own. Treat them as dedent-first
    // when we're inside the prior section's depth.
    let is_section = up == "DECLARE"
      || up == "BEGIN"
      || up == "BEGIN;"
      || up == "EXCEPTION"
      || up == "EXCEPTION;"
      || up.starts_with("EXCEPTION ");
    let is_end = up.starts_with("END");
    let dedent_first =
      is_section || is_end || up.starts_with("ELSE") || up.starts_with("ELSIF") || up.starts_with("WHEN ");
    let print_depth = if dedent_first { depth.saturating_sub(1) } else { depth };
    for _ in 0..print_depth {
      out.push_str("  ");
    }
    out.push_str(s);
    out.push('\n');
    // Emit the post-THEN tail (IF body content) at depth+1.
    if let Some(tail) = then_tail {
      for _ in 0..(print_depth + 1) {
        out.push_str("  ");
      }
      out.push_str(&tail);
      out.push('\n');
    }
    // Adjust depth for the NEXT statement.
    // Section markers (DECLARE/BEGIN/EXCEPTION) and control-flow
    // openers (IF/LOOP/FOR/WHILE/CASE/ELSE/ELSIF) reset to the
    // peer level then bump for their body.
    if is_section {
      // Close prior section + open new one => stay at print_depth + 1.
      depth = print_depth + 1;
    } else if up.starts_with("IF ")
      || up.starts_with("LOOP")
      || up.starts_with("FOR ")
      || up.starts_with("WHILE ")
      || up.starts_with("CASE ")
      || up == "ELSE;"
      || up.starts_with("ELSIF ")
    {
      depth += 1;
    } else if is_end {
      // Close the block this END terminates.
      depth = print_depth;
    }
  }
  out
}

/// sql-formatter collapses `CREATE [OR REPLACE] FUNCTION ... RETURNS X
/// STABLE AS $$` onto one line. Inject line breaks at standard clause
/// boundaries so the result reads like hand-written DDL. Clauses get
/// 4-space indent except `AS` (body opener) which sits at column 0.
fn break_function_headers(source: &str) -> String {
  let mut out = source.to_string();
  let ctx = &["CREATE FUNCTION", "CREATE OR REPLACE FUNCTION", "CREATE PROCEDURE", "CREATE OR REPLACE PROCEDURE"];
  // (needle, indent) -- 4 = nested clause, 0 = body opener.
  for (kw, indent) in [
    (" RETURNS ", 4),
    (" STABLE ", 4),
    (" IMMUTABLE ", 4),
    (" VOLATILE ", 4),
    (" STRICT ", 4),
    (" PARALLEL ", 4),
    (" SECURITY DEFINER ", 4),
    (" SECURITY INVOKER ", 4),
    (" LANGUAGE ", 4),
    (" AS $$", 0),
    (" AS $", 0),
  ] {
    out = inject_break_in(&out, kw, ctx, indent);
  }
  out
}

/// sql-formatter also collapses CREATE TRIGGER clauses onto one line:
///   `CREATE TRIGGER name BEFORE UPDATE ON tbl FOR EACH ROW EXECUTE FUNCTION fn()`
/// Break those at standard clause boundaries so the result reads like the
/// hand-written DataGrip-style trigger DDL.
fn break_trigger_headers(source: &str) -> String {
  let mut out = source.to_string();
  let ctx = &[
    "CREATE TRIGGER",
    "CREATE OR REPLACE TRIGGER",
    "CREATE CONSTRAINT TRIGGER",
    "CREATE OR REPLACE CONSTRAINT TRIGGER",
  ];
  for (kw, indent) in [
    (" BEFORE ", 4),
    (" AFTER ", 4),
    (" INSTEAD OF ", 4),
    (" ON ", 4),
    (" FOR EACH ROW", 4),
    (" FOR EACH STATEMENT", 4),
    (" WHEN ", 4),
    (" REFERENCING ", 4),
    (" EXECUTE FUNCTION ", 0),
    (" EXECUTE PROCEDURE ", 0),
  ] {
    out = inject_break_in(&out, kw, ctx, indent);
  }
  out
}

/// CREATE INDEX clauses (ON / USING / WHERE / INCLUDE) -- break onto
/// their own indented lines so multi-clause indexes read top-to-bottom.
fn break_index_headers(source: &str) -> String {
  let mut out = source.to_string();
  let ctx = &["CREATE INDEX", "CREATE UNIQUE INDEX"];
  for (kw, indent) in
    [(" ON ", 4), (" USING ", 4), (" INCLUDE ", 4), (" WHERE ", 4), (" WITH ", 4), (" TABLESPACE ", 4)]
  {
    out = inject_break_in(&out, kw, ctx, indent);
  }
  out
}

/// Replace every occurrence of ` <kw>` (space prefix, intentional) with
/// `\n<indent><kw>` when the current statement (scanned back to the
/// previous `;`) contains any of the supplied context markers.
/// Case-insensitive via uppercased lookup copy. `indent` is the number
/// of leading spaces to put on the new line before the keyword.
fn inject_break_in(text: &str, needle_with_space: &str, contexts: &[&str], indent: usize) -> String {
  let upper = text.to_ascii_uppercase();
  let needle_upper = needle_with_space.to_ascii_uppercase();
  let pad: String = std::iter::repeat_n(' ', indent).collect();
  let mut out = String::with_capacity(text.len() + 16);
  let mut from = 0usize;
  while let Some(rel) = upper[from..].find(&needle_upper) {
    let i = from + rel;
    let stmt_start = text[..i].rfind(';').map(|p| p + 1).unwrap_or(0);
    let head_upper = &upper[stmt_start..i];
    let in_ctx = contexts.iter().any(|c| head_upper.contains(*c));
    let already_broken = {
      let mut j = i;
      while j > 0 && matches!(text.as_bytes()[j - 1], b' ' | b'\t') {
        j -= 1;
      }
      j == 0 || text.as_bytes()[j - 1] == b'\n'
    };
    out.push_str(&text[from..i]);
    if in_ctx && !already_broken {
      out.push('\n');
      out.push_str(&pad);
      out.push_str(needle_with_space.trim_start());
    } else {
      out.push_str(&text[i..i + needle_with_space.len()]);
    }
    from = i + needle_with_space.len();
  }
  out.push_str(&text[from..]);
  out
}

/// Apply the column-alignment pass to every CREATE TABLE in `source`.
fn rewrite_tables(source: &str, style: &CreateTableStyle) -> String {
  let mut out = String::with_capacity(source.len());
  let mut i = 0usize;
  let bytes = source.as_bytes();
  let n = bytes.len();
  let upper = source.to_ascii_uppercase();

  while i < n {
    // Find next CREATE TABLE start at top level.
    let needle = "CREATE TABLE";
    let rel = upper[i..].find(needle);
    let Some(rel) = rel else {
      out.push_str(&source[i..]);
      break;
    };
    let start = i + rel;
    // Boundary check -- preceding char must not be an identifier char.
    if start > 0 {
      let prev = bytes[start - 1] as char;
      if prev.is_alphanumeric() || prev == '_' {
        out.push_str(&source[i..start + needle.len()]);
        i = start + needle.len();
        continue;
      }
    }
    out.push_str(&source[i..start]);

    // Skip CREATE TABLE ... PARTITION OF -- the body shape is
    // `FOR VALUES FROM (..) TO (..)` not `(col_defs)`. Letting
    // rewrite_tables process it shreds the FOR VALUES clause.
    let upper_tail = source[start..].to_ascii_uppercase();
    let stmt_term = upper_tail.find(';').map(|p| start + p).unwrap_or(n);
    if upper_tail[..stmt_term - start].contains("PARTITION OF") {
      // Pass through verbatim up to and including the `;`.
      let end_inclusive = (stmt_term + 1).min(n);
      out.push_str(&source[start..end_inclusive]);
      i = end_inclusive;
      continue;
    }
    // Find the body parens.
    let (paren_start, paren_end) = match find_table_body(bytes, start) {
      Some(p) => p,
      None => {
        out.push_str(&source[start..]);
        break;
      },
    };
    // Find the terminator `;` (best-effort -- if missing, stop at end).
    let mut stmt_end = paren_end + 1;
    while stmt_end < n && (bytes[stmt_end] as char).is_whitespace() {
      stmt_end += 1;
    }
    if stmt_end < n && bytes[stmt_end] == b';' {
      stmt_end += 1;
    }

    let header = source[start..paren_start].trim_end_matches(|c: char| c == '(' || c.is_whitespace());
    let body = &source[paren_start + 1..paren_end];
    out.push_str(&format_block(header.trim(), body, style));
    out.push_str(";\n");

    i = stmt_end;
    // Collapse whitespace before the next statement to a single
    // blank line at most (one extra `\n` beyond the one already
    // written by `";\n"` above).
    let mut newlines = 0usize;
    while i < n && bytes[i] == b'\n' {
      newlines += 1;
      i += 1;
    }
    if newlines > 0 {
      out.push('\n');
    }
  }
  out
}

/// Collapse blank lines between consecutive CREATE INDEX statements so
/// a wall of index DDL doesn't get sprayed across the file.
fn collapse_index_runs(source: &str) -> String {
  let lines: Vec<&str> = source.lines().collect();
  let mut out: Vec<String> = Vec::with_capacity(lines.len());
  let mut i = 0usize;
  while i < lines.len() {
    let line = lines[i];
    out.push(line.to_string());
    if is_create_index_line(line.trim()) {
      // Skip blank lines that sit between this index and another
      // CREATE INDEX on the next non-blank line.
      let mut j = i + 1;
      while j < lines.len() && lines[j].trim().is_empty() {
        j += 1;
      }
      if j > i + 1 && j < lines.len() && is_create_index_line(lines[j].trim()) {
        i = j;
        continue;
      }
    }
    i += 1;
  }
  out.join("\n") + if source.ends_with('\n') { "\n" } else { "" }
}

fn is_create_index_line(line: &str) -> bool {
  let up = line.to_ascii_uppercase();
  up.starts_with("CREATE INDEX")
    || up.starts_with("CREATE UNIQUE INDEX")
    || up.starts_with("CREATE INDEX IF NOT EXISTS")
    || up.starts_with("CREATE UNIQUE INDEX IF NOT EXISTS")
}

/// Return (open_paren_pos, close_paren_pos) for the table body that
/// follows `CREATE TABLE [IF NOT EXISTS] <name>`. Skips quoted strings
/// and balances nested parens (e.g. `NUMERIC(10,2)`).
fn find_table_body(bytes: &[u8], start: usize) -> Option<(usize, usize)> {
  let n = bytes.len();
  let mut i = start;
  while i < n && bytes[i] != b'(' {
    i += 1;
  }
  if i >= n {
    return None;
  }
  let open = i;
  let mut depth = 0i32;
  while i < n {
    match bytes[i] {
      b'\'' => {
        i += 1;
        while i < n {
          if bytes[i] == b'\'' {
            if i + 1 < n && bytes[i + 1] == b'\'' {
              i += 2;
              continue;
            }
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
        if depth == 0 {
          return Some((open, i));
        }
        i += 1;
      },
      _ => i += 1,
    }
  }
  None
}

/// Split the body on top-level commas (depth=0). Single-quoted strings
/// and nested parens are respected so `NUMERIC(10,2)` stays one entry.
fn split_entries(body: &str) -> Vec<String> {
  let bytes = body.as_bytes();
  let n = bytes.len();
  let mut out = Vec::new();
  let mut last = 0usize;
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
      b',' if depth == 0 => {
        let s = body[last..i].trim();
        if !s.is_empty() {
          out.push(s.to_string());
        }
        last = i + 1;
        i += 1;
      },
      _ => i += 1,
    }
  }
  let tail = body[last..].trim();
  if !tail.is_empty() {
    out.push(tail.to_string());
  }
  out
}

/// Classify a body entry as either a column declaration or a table-level
/// constraint clause. The first token decides.
fn is_constraint(entry: &str) -> bool {
  let upper = entry.trim_start().to_ascii_uppercase();
  matches!(
    upper.split_ascii_whitespace().next().unwrap_or(""),
    "CONSTRAINT" | "PRIMARY" | "FOREIGN" | "UNIQUE" | "CHECK" | "EXCLUDE" | "LIKE"
  )
}

/// One parsed CREATE TABLE column declaration with its tail clauses
/// already separated, so the aligner can pad each sub-column to a
/// shared width across the whole table.
#[derive(Default, Debug)]
struct ColParts {
  name: String,
  ty: String,
  nullability: String, // "NOT NULL", "NULL", or empty
  default: String,     // "DEFAULT ..." or empty
  extra: String,       // REFERENCES / CHECK / GENERATED / COLLATE / PRIMARY KEY / UNIQUE etc.
  /// Inline `/* ... */` or `-- ...` comment the user attached to this
  /// column on the same line. Emitted on its own indented line above
  /// the column row so the column name aligns with siblings.
  leading_comment: String,
  /// Trailing inline comment that originally followed the column on
  /// the same line. Appended AFTER the row's comma so the comma
  /// doesn't accidentally land inside a `-- ...` line comment.
  trailing_comment: String,
}

/// Tear a column declaration apart into (name, type, tail) where the
/// tail is everything after the type (NOT NULL / DEFAULT ... etc).
/// Preserves type arguments like `NUMERIC(10, 2)` and `VARCHAR(255)`.
fn split_column(entry: &str) -> (String, String, String) {
  let bytes = entry.as_bytes();
  let n = bytes.len();
  // Read the name (identifier).
  let mut i = 0usize;
  while i < n && (bytes[i] as char).is_whitespace() {
    i += 1;
  }
  let name_start = i;
  if i < n && bytes[i] == b'"' {
    i += 1;
    while i < n && bytes[i] != b'"' {
      i += 1;
    }
    if i < n {
      i += 1;
    }
  } else {
    while i < n && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_') {
      i += 1;
    }
  }
  let name = entry[name_start..i].to_string();
  while i < n && (bytes[i] as char).is_whitespace() {
    i += 1;
  }
  // Read the type up to first whitespace at depth 0 OR a keyword that
  // breaks the type (NOT, NULL, DEFAULT, REFERENCES, CHECK, GENERATED,
  // PRIMARY, UNIQUE, COLLATE).
  let type_start = i;
  let mut depth = 0i32;
  while i < n {
    let c = bytes[i] as char;
    if depth == 0 && c.is_whitespace() {
      // Peek the next word -- stop if it's a tail keyword.
      let mut j = i;
      while j < n && (bytes[j] as char).is_whitespace() {
        j += 1;
      }
      let mut k = j;
      while k < n && (bytes[k].is_ascii_alphabetic() || bytes[k] == b'_') {
        k += 1;
      }
      let word_upper = entry[j..k].to_ascii_uppercase();
      let is_tail = matches!(
        word_upper.as_str(),
        "NOT" | "NULL" | "DEFAULT" | "REFERENCES" | "CHECK" | "GENERATED" | "PRIMARY" | "UNIQUE" | "COLLATE"
      );
      // Special case: `WITH TIME ZONE`, `WITHOUT TIME ZONE`,
      // `DOUBLE PRECISION`, `CHARACTER VARYING`, `BIT VARYING`.
      let is_type_continuation =
        matches!(word_upper.as_str(), "WITH" | "WITHOUT" | "PRECISION" | "VARYING" | "TIME" | "ZONE" | "CHARACTER");
      if is_tail && !is_type_continuation {
        break;
      }
      // Otherwise the type continues -- pad with one space.
      i = j;
      continue;
    }
    match c {
      '(' => depth += 1,
      ')' => depth -= 1,
      _ => {},
    }
    i += 1;
  }
  let ty = entry[type_start..i].trim().to_string();
  let tail = entry[i..].trim().to_string();
  (name, ty, tail)
}

/// Like `split_column`, but the tail is further decomposed into
/// nullability / DEFAULT / extra so the aligner can pad each sub-column.
/// Pull a leading `/* ... */` or `-- ...` (to first newline) comment
/// off the entry. Returns `(comment, remainder)` where `comment`
/// is empty when none is present. Whitespace between the comment
/// and the column body is dropped.
fn strip_leading_comment(entry: &str) -> (String, String) {
  let s = entry.trim_start();
  if let Some(rest) = s.strip_prefix("/*")
    && let Some(close) = rest.find("*/")
  {
    let comment = format!("/*{}*/", &rest[..close]);
    let after = rest[close + 2..].trim_start();
    return (comment, after.to_string());
  }
  if let Some(rest) = s.strip_prefix("--") {
    if let Some(nl) = rest.find('\n') {
      let comment = format!("--{}", &rest[..nl]);
      let after = rest[nl + 1..].trim_start();
      return (comment, after.to_string());
    }
    // Comment runs to end -- whole entry was just a comment.
    return (format!("--{}", rest), String::new());
  }
  (String::new(), entry.to_string())
}

/// Pull a trailing `-- ...` (rest of line) or final `/* ... */`
/// comment off the entry. Returns `(comment, remainder)` where the
/// remainder has the comment AND any trailing whitespace removed.
/// Only considers comments that aren't followed by more SQL content
/// -- so `a int /* note */ NOT NULL` leaves the comment in place.
fn strip_trailing_comment(entry: &str) -> (String, String) {
  let trimmed = entry.trim_end();
  // Walk the entry tracking `'...'` strings and balanced `/* */`
  // blocks so the dash-dash test only fires when the cursor is on
  // an actual line-comment marker outside a string.
  let bytes = trimmed.as_bytes();
  let n = bytes.len();
  let mut i = 0usize;
  let mut last_line_comment: Option<usize> = None;
  let mut last_block_comment: Option<usize> = None;
  while i < n {
    let b = bytes[i];
    if b == b'\'' {
      i += 1;
      while i < n && bytes[i] != b'\'' {
        i += 1;
      }
      i = (i + 1).min(n);
      last_line_comment = None;
      last_block_comment = None;
      continue;
    }
    if i + 1 < n && b == b'-' && bytes[i + 1] == b'-' {
      // Line comment runs to next newline -- if no newline (typical for
      // a single-line entry), it consumes the rest.
      let after = trimmed[i + 2..].find('\n').map(|p| i + 2 + p);
      if after.is_none() {
        last_line_comment = Some(i);
      }
      // Otherwise: comment ends mid-entry; skip past it.
      i = after.unwrap_or(n);
      continue;
    }
    if i + 1 < n && b == b'/' && bytes[i + 1] == b'*' {
      // Block comment.
      let mut j = i + 2;
      let mut depth = 1i32;
      while j + 1 < n && depth > 0 {
        if bytes[j] == b'/' && bytes[j + 1] == b'*' {
          depth += 1;
          j += 2;
        } else if bytes[j] == b'*' && bytes[j + 1] == b'/' {
          depth -= 1;
          j += 2;
        } else {
          j += 1;
        }
      }
      if depth == 0 && j == n {
        last_block_comment = Some(i);
      } else {
        last_block_comment = None;
      }
      i = j.min(n);
      continue;
    }
    if !b.is_ascii_whitespace() {
      last_line_comment = None;
      last_block_comment = None;
    }
    i += 1;
  }
  let cut = last_line_comment.or(last_block_comment);
  if let Some(start) = cut {
    let comment = trimmed[start..].trim().to_string();
    let head = trimmed[..start].trim_end().to_string();
    return (comment, head);
  }
  (String::new(), entry.to_string())
}

fn split_parts(entry: &str) -> ColParts {
  let (lead, mid) = strip_leading_comment(entry);
  let (trail, rest) = strip_trailing_comment(&mid);
  let (name, ty, tail) = split_column(&rest);
  let mut parts = ColParts { name, ty, leading_comment: lead, trailing_comment: trail, ..ColParts::default() };

  let mut remaining = tail.as_str().trim();
  // NOT NULL / NULL must appear before DEFAULT in legal Postgres DDL,
  // but accept either order defensively.
  let upper_tail = remaining.to_ascii_uppercase();
  if upper_tail.starts_with("NOT NULL") {
    parts.nullability = "NOT NULL".into();
    remaining = remaining[8..].trim_start();
  } else if upper_tail.starts_with("NULL") && !upper_tail.starts_with("NULLS") {
    // Bare NULL is legal in column DDL ("explicit NULL"). Postgres
    // discards it but DataGrip-style output keeps it.
    parts.nullability = "NULL".into();
    remaining = remaining[4..].trim_start();
  }

  // DEFAULT <expr> spans up to the next top-level keyword
  // (NOT NULL we already handled, REFERENCES / CHECK / GENERATED /
  // COLLATE / PRIMARY KEY / UNIQUE).
  if remaining.to_ascii_uppercase().starts_with("DEFAULT") {
    let after_kw = remaining[7..].trim_start();
    let expr_end = scan_default_expr(after_kw);
    let expr = after_kw[..expr_end].trim_end();
    parts.default = format!("DEFAULT {expr}");
    remaining = after_kw[expr_end..].trim_start();
  }

  // Try again for NOT NULL after DEFAULT, just in case the user wrote
  // them in the reverse order.
  if parts.nullability.is_empty() {
    let up = remaining.to_ascii_uppercase();
    if up.starts_with("NOT NULL") {
      parts.nullability = "NOT NULL".into();
      remaining = remaining[8..].trim_start();
    }
  }

  parts.extra = remaining.to_string();
  parts
}

/// Read a DEFAULT expression up to but not including the next top-level
/// constraint keyword. Respects parens and single-quoted strings.
fn scan_default_expr(s: &str) -> usize {
  let bytes = s.as_bytes();
  let n = bytes.len();
  let mut i = 0usize;
  let mut depth = 0i32;
  while i < n {
    let c = bytes[i] as char;
    match c {
      '(' => {
        depth += 1;
        i += 1;
        continue;
      },
      ')' => {
        depth -= 1;
        i += 1;
        continue;
      },
      '\'' => {
        i += 1;
        while i < n {
          if bytes[i] == b'\'' {
            i += 1;
            break;
          }
          i += 1;
        }
        continue;
      },
      _ if depth == 0 && c.is_whitespace() => {
        // Peek the next word.
        let mut j = i;
        while j < n && (bytes[j] as char).is_whitespace() {
          j += 1;
        }
        let start = j;
        while j < n && (bytes[j].is_ascii_alphabetic() || bytes[j] == b'_') {
          j += 1;
        }
        let word_upper = s[start..j].to_ascii_uppercase();
        if matches!(
          word_upper.as_str(),
          "NOT" | "REFERENCES" | "CHECK" | "GENERATED" | "COLLATE" | "PRIMARY" | "UNIQUE"
        ) {
          return i;
        }
        i = j;
        continue;
      },
      _ => {
        i += 1;
      },
    }
  }
  n
}

/// Build the reformatted block (header + body + closing paren). Does not
/// include the trailing `;`. Aligns four sub-columns across all rows so
/// `NOT NULL` / `NULL` / `DEFAULT ...` all line up vertically.
/// Find the top-level `THEN` keyword (whole-word, case-insensitive)
/// outside of string literals and parenthesised expressions. Returns
/// its byte offset in `upper` (already uppercased text) or None.
fn find_then(upper: &str) -> Option<usize> {
  let bytes = upper.as_bytes();
  let n = bytes.len();
  let mut depth = 0i32;
  let mut i = 0usize;
  while i < n {
    match bytes[i] {
      b'(' => {
        depth += 1;
        i += 1;
      },
      b')' => {
        depth -= 1;
        i += 1;
      },
      b'\'' => {
        i += 1;
        while i < n && bytes[i] != b'\'' {
          i += 1;
        }
        if i < n {
          i += 1;
        }
      },
      _ => {
        if depth == 0
          && i + 4 <= n
          && &upper[i..i + 4] == "THEN"
          && (i == 0 || !(bytes[i - 1].is_ascii_alphanumeric() || bytes[i - 1] == b'_'))
          && (i + 4 == n || !(bytes[i + 4].is_ascii_alphanumeric() || bytes[i + 4] == b'_'))
        {
          return Some(i);
        }
        i += 1;
      },
    }
  }
  None
}

/// Collapse a multi-line constraint body (e.g. CHECK with wrapped
/// predicate, FK with REFERENCES on the next line) onto a single line.
/// Honours string literals so quoted multi-line content stays intact.
fn collapse_one_line(s: &str) -> String {
  let bytes = s.as_bytes();
  let n = bytes.len();
  let mut out = String::with_capacity(n);
  let mut prev_space = false;
  let mut i = 0usize;
  while i < n {
    match bytes[i] {
      b'\'' => {
        let start = i;
        i += 1;
        while i < n && bytes[i] != b'\'' {
          i += 1;
        }
        if i < n {
          i += 1;
        }
        out.push_str(&s[start..i]);
        prev_space = false;
      },
      // Line comment: preserve the comment + the terminating newline so
      // we don't accidentally absorb the rest of the constraint into a
      // `-- ...` tail.
      b'-' if i + 1 < n && bytes[i + 1] == b'-' => {
        let start = i;
        while i < n && bytes[i] != b'\n' {
          i += 1;
        }
        out.push_str(&s[start..i]);
        if i < n {
          out.push('\n');
          i += 1;
        }
        prev_space = false;
      },
      // Block comment: keep verbatim so `/* ... */` survives the
      // single-line collapse with its boundaries intact.
      b'/' if i + 1 < n && bytes[i + 1] == b'*' => {
        let start = i;
        i += 2;
        while i + 1 < n && !(bytes[i] == b'*' && bytes[i + 1] == b'/') {
          i += 1;
        }
        if i + 1 < n {
          i += 2;
        }
        out.push_str(&s[start..i]);
        prev_space = false;
      },
      c if c.is_ascii_whitespace() => {
        if !prev_space && !out.is_empty() {
          out.push(' ');
        }
        prev_space = true;
        i += 1;
      },
      _ => {
        // Multi-byte char aware: push the whole UTF-8 sequence at once.
        i = crate::push_one_char(&mut out, s, i);
        prev_space = false;
      },
    }
  }
  out.trim().to_string()
}

fn format_block(header: &str, body: &str, style: &CreateTableStyle) -> String {
  // Pull out whole-line comments first so they don't pollute width
  // calculations or get parsed as fake columns. Returns interleaved
  // items in declaration order so they re-emit in the same slot.
  enum Item {
    Comment(String),
    Column(ColParts),
    Constraint(String),
  }
  let entries = split_entries(body);
  let mut items: Vec<Item> = Vec::new();
  for e in entries {
    // Pull leading whole-line comments off the entry. `-- foo\n` lines
    // before the column body become their own Comment items so the
    // align widths only see real declarations.
    let mut rest = e.as_str();
    loop {
      let trimmed = rest.trim_start_matches(['\n', '\r', ' ', '\t']);
      if trimmed.starts_with("--") {
        let end = trimmed.find('\n').unwrap_or(trimmed.len());
        items.push(Item::Comment(trimmed[..end].trim_end().to_string()));
        rest = &trimmed[end..];
        continue;
      }
      if trimmed.starts_with("/*")
        && let Some(end) = trimmed.find("*/")
      {
        items.push(Item::Comment(trimmed[..end + 2].to_string()));
        rest = &trimmed[end + 2..];
        continue;
      }
      break;
    }
    let rest = rest.trim();
    if rest.is_empty() {
      continue;
    }
    if is_constraint(rest) {
      items.push(Item::Constraint(rest.to_string()));
    } else {
      items.push(Item::Column(split_parts(rest)));
    }
  }
  let mut columns: Vec<&ColParts> = Vec::new();
  let mut constraints: Vec<&String> = Vec::new();
  for it in &items {
    match it {
      Item::Column(c) => columns.push(c),
      Item::Constraint(c) => constraints.push(c),
      Item::Comment(_) => {},
    }
  }

  let name_w = columns.iter().map(|p| p.name.len()).max().unwrap_or(0);
  let type_w = columns.iter().map(|p| p.ty.len()).max().unwrap_or(0);
  let null_w = columns.iter().map(|p| p.nullability.len()).max().unwrap_or(0);
  let def_w = columns.iter().map(|p| p.default.len()).max().unwrap_or(0);
  let gap = " ".repeat(style.column_gap.min(2)); // tighter gap for sub-columns
  let inter_gap = " ".to_string(); // single space between sub-columns

  let mut s = String::new();
  s.push_str(header);
  if style.open_paren_on_new_line {
    s.push('\n');
    s.push('(');
    s.push('\n');
  } else {
    s.push(' ');
    s.push('(');
    s.push('\n');
  }

  let order: Vec<(String, String)> = {
    let mut rows: Vec<(String, String)> = Vec::with_capacity(items.len() + 1);
    let mut emitted_columns = false;
    for it in &items {
      match it {
        Item::Comment(c) => {
          rows.push((format!("    {}", collapse_one_line(c)), String::new()));
        },
        Item::Column(p) => {
          if !p.leading_comment.is_empty() {
            rows.push((format!("    {}", p.leading_comment), String::new()));
          }
          let mut row = format!("    {:<nw$}{}{:<tw$}", p.name, gap, p.ty, nw = name_w, tw = type_w);
          if null_w > 0 {
            row.push_str(&inter_gap);
            row.push_str(&format!("{:>w$}", p.nullability, w = null_w));
          }
          if def_w > 0 {
            row.push_str(&inter_gap);
            row.push_str(&format!("{:<w$}", p.default, w = def_w));
          }
          if !p.extra.is_empty() {
            row.push_str(&inter_gap);
            row.push_str(&p.extra);
          }
          rows.push((row.trim_end().to_string(), p.trailing_comment.clone()));
          emitted_columns = true;
        },
        Item::Constraint(_) if style.constraints_at_end => {},
        Item::Constraint(c) => {
          rows.push((format!("    {}", collapse_one_line(c)), String::new()));
        },
      }
    }
    if style.constraints_at_end && !constraints.is_empty() && emitted_columns {
      rows.push((String::new(), String::new()));
      for c in &constraints {
        rows.push((format!("    {}", collapse_one_line(c)), String::new()));
      }
    } else if style.constraints_at_end && !constraints.is_empty() {
      for c in &constraints {
        rows.push((format!("    {}", collapse_one_line(c)), String::new()));
      }
    }
    rows
  };

  let last = order.len().saturating_sub(1);
  for (i, (mut line, trail)) in order.into_iter().enumerate() {
    let trimmed = line.trim_start();
    let is_blank = trimmed.is_empty();
    let is_comment = trimmed.starts_with("--") || trimmed.starts_with("/*");
    if !is_blank && !is_comment && i < last {
      line.push(',');
    }
    if !trail.is_empty() {
      line.push(' ');
      line.push_str(&trail);
    }
    s.push_str(&line);
    s.push('\n');
  }
  s.push(')');
  s
}
