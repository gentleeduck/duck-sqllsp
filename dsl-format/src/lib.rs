//! SQL formatting business logic for duck-sqllsp.
//!
//! Two layers cooperate:
//!
//!   1. [`external::run_sql_formatter`] -- shells out to the `sql-formatter`
//!      npm CLI (when present) for the bulk reflow / keyword casing /
//!      expression-width wrapping work that a dedicated grammar-aware tool
//!      already solves well.
//!   2. [`align::rewrite`] -- DataGrip-style CREATE TABLE / FUNCTION /
//!      TRIGGER / INDEX post-pass that aligns columns into padded sub-
//!      columns and breaks long clause-chained headers onto their own
//!      lines.
//!
//! The composite [`format`] entry runs both in order. Server handlers stay
//! thin shims that read the document, call this, build a TextEdit.

pub mod align;
pub mod external;
pub mod style;

pub use align::rewrite;
pub use external::run_sql_formatter;
pub use style::{CreateTableStyle, FormatterStyle};

/// One-shot format pipeline: external sql-formatter first (when available),
/// DataGrip-style alignment second. Falls back to passing the input through
/// unchanged when the external binary is missing -- callers can then
/// decide whether to skip emitting a no-op TextEdit.
pub fn format(input: &str, fmt_style: &FormatterStyle, ct_style: &CreateTableStyle) -> String {
  // Strip a leading UTF-8 BOM (`U+FEFF`). PG ignores it but tools that
  // preserve it leave invisible bytes in the file, which version control
  // and other formatters then fight over. BOMs inside string literals
  // or comments are preserved -- only the leading one is meaningful
  // metadata, and the convention is to drop it on format.
  let input = input.strip_prefix('\u{feff}').unwrap_or(input);
  // Comment guard: extract every whole-line `-- ...` comment into a
  // sentinel placeholder before sql-formatter runs. sql-formatter has
  // an unfortunate habit of treating consecutive `--` lines as section
  // separators and reflowing the surrounding statement structure. After
  // the pipeline finishes we re-inject the originals at the same line
  // positions, byte-for-byte.
  let (guarded, saved_comments) = stash_line_comments(input);
  // Stash CREATE TABLE ... PARTITION OF statements as opaque sentinels
  // before sql-formatter runs. sql-formatter v15 treats `FOR`, `VALUES`,
  // `FROM`, `TO`, `IN`, `WITH` as clause boundaries and shreds the
  // `FOR VALUES FROM (..) TO (..)` clause across multiple lines and
  // even injects a stray `;`. Pass these through verbatim by swapping
  // for `-- DUCK_PART_N` markers + restoring after.
  let (guarded, saved_partitions) = stash_partition_of_stmts(&guarded);
  let after_external = external::run_sql_formatter(&guarded, fmt_style).unwrap_or_else(|| guarded.clone());
  let after_external = restore_partition_of_stmts(&after_external, &saved_partitions);
  let after_align = align::rewrite(&after_external, ct_style);
  let after_tighten = tighten_call_parens(&after_align);
  let after_fn_set = collapse_function_set_clause(&after_tighten);
  let after_body_fmt = format_function_bodies(&after_fn_set, fmt_style);
  let after_normalised = normalize_blank_lines(&after_body_fmt);
  let final_out = if fmt_style.single_line {
    collapse_dml_lines(&after_normalised)
  } else if fmt_style.compact_clauses {
    normalize_blank_lines(&compact_clause_lines(&after_normalised))
  } else {
    after_normalised
  };
  restore_line_comments(&final_out, &saved_comments)
}

/// Replace every line whose first non-whitespace token is `--` with a
/// sentinel `-- DUCK_CMT_N` placeholder. Returns the stashed text + the
/// vec of original comment line contents indexed by N.
/// Walk `input`, find every `CREATE TABLE ... PARTITION OF ... ;`
/// statement, and replace it with `-- DUCK_PART_N` sentinel. The
/// originals are returned indexed by N so [`restore_partition_of_stmts`]
/// can swap them back AFTER sql-formatter has formatted everything
/// else.
fn stash_partition_of_stmts(input: &str) -> (String, Vec<String>) {
  let upper = input.to_ascii_uppercase();
  let mut out = String::with_capacity(input.len());
  let mut saved: Vec<String> = Vec::new();
  let bytes = input.as_bytes();
  let n = bytes.len();
  let mut from = 0usize;
  let mut last_emit = 0usize;
  while let Some(rel) = upper[from..].find("CREATE TABLE") {
    let at = from + rel;
    // Word-boundary guard.
    let prev_ok = at == 0 || {
      let p = bytes[at - 1];
      !(p.is_ascii_alphanumeric() || p == b'_')
    };
    if !prev_ok {
      from = at + 1;
      continue;
    }
    // Find end-of-statement (top-level `;`), tracking parens + strings.
    let end = find_top_level_semicolon(input, at).unwrap_or(n);
    let stmt = &input[at..=end.min(n - 1)];
    let stmt_upper = stmt.to_ascii_uppercase();
    if !stmt_upper.contains("PARTITION OF") {
      from = at + "CREATE TABLE".len();
      continue;
    }
    // Emit prefix verbatim.
    out.push_str(&input[last_emit..at]);
    let idx = saved.len();
    saved.push(stmt.to_string());
    out.push_str(&format!("-- DUCK_PART_{idx}"));
    last_emit = end.saturating_add(1).min(n);
    from = last_emit;
  }
  out.push_str(&input[last_emit..]);
  (out, saved)
}

/// Replace each `-- DUCK_PART_N` placeholder line with the saved
/// statement. Markers may appear on their own line after
/// sql-formatter; preserve any indentation prefix.
fn restore_partition_of_stmts(input: &str, saved: &[String]) -> String {
  let mut out = String::with_capacity(input.len());
  for line in input.lines() {
    let trimmed = line.trim_start();
    if let Some(rest) = trimmed.strip_prefix("-- DUCK_PART_") {
      let n: usize = rest.trim_end().parse().unwrap_or(usize::MAX);
      if let Some(orig) = saved.get(n) {
        out.push_str(orig);
        out.push('\n');
        continue;
      }
    }
    out.push_str(line);
    out.push('\n');
  }
  // Preserve trailing-newline shape.
  if !input.ends_with('\n') && out.ends_with('\n') {
    out.pop();
  }
  out
}

/// Find the index of the first top-level `;` at or after `start`.
/// Respects single-quoted strings, double-quoted identifiers, `$$`
/// dollar-quoted bodies, and line/block comments.
fn find_top_level_semicolon(input: &str, start: usize) -> Option<usize> {
  let bytes = input.as_bytes();
  let n = bytes.len();
  let mut i = start;
  let mut in_single = false;
  let mut in_double = false;
  let mut dollar: Option<String> = None;
  let mut block_depth = 0i32;
  while i < n {
    let c = bytes[i];
    if let Some(tag) = &dollar {
      if i + tag.len() <= n && &input[i..i + tag.len()] == tag.as_str() {
        i += tag.len();
        dollar = None;
        continue;
      }
      i += 1;
      continue;
    }
    if block_depth > 0 {
      if i + 1 < n && bytes[i] == b'*' && bytes[i + 1] == b'/' {
        block_depth -= 1;
        i += 2;
        continue;
      }
      i += 1;
      continue;
    }
    if !in_single && !in_double {
      if i + 1 < n && bytes[i] == b'/' && bytes[i + 1] == b'*' {
        block_depth = 1;
        i += 2;
        continue;
      }
      if i + 1 < n && bytes[i] == b'-' && bytes[i + 1] == b'-' {
        while i < n && bytes[i] != b'\n' {
          i += 1;
        }
        continue;
      }
      if c == b'$' {
        let rest = &input[i + 1..];
        if let Some(end) = rest.find('$') {
          let tag = &rest[..end];
          if tag.chars().all(|ch| ch.is_alphanumeric() || ch == '_') {
            dollar = Some(format!("${tag}$"));
            i += 1 + end + 1;
            continue;
          }
        }
      }
    }
    if c == b'\'' && !in_double {
      in_single = !in_single;
    } else if c == b'"' && !in_single {
      in_double = !in_double;
    } else if c == b';' && !in_single && !in_double {
      return Some(i);
    }
    i += 1;
  }
  None
}

fn stash_line_comments(input: &str) -> (String, Vec<String>) {
  let mut out = String::with_capacity(input.len());
  let mut saved: Vec<String> = Vec::new();
  for (i, line) in input.lines().enumerate() {
    let trimmed = line.trim_start();
    if trimmed.starts_with("--") {
      saved.push(line.to_string());
      out.push_str(&format!("-- DUCK_CMT_{}", saved.len() - 1));
    } else {
      out.push_str(line);
    }
    // Preserve trailing newline shape: include `\n` unless this was the
    // last line and the original didn't end with one.
    let is_last = i + 1 == input.lines().count();
    if !is_last || input.ends_with('\n') {
      out.push('\n');
    }
  }
  (out, saved)
}

/// Reverse of [`stash_line_comments`]: replace `-- DUCK_CMT_N` markers
/// with the saved originals. sql-formatter / align preserve the comment
/// lines as-is, so the markers survive verbatim.
fn restore_line_comments(input: &str, saved: &[String]) -> String {
  let mut out = String::with_capacity(input.len());
  for line in input.lines() {
    let trimmed = line.trim_start();
    if let Some(rest) = trimmed.strip_prefix("-- DUCK_CMT_")
      && let Some(idx_end) = rest.find(|c: char| !c.is_ascii_digit()).or(Some(rest.len()))
    {
      let digits = &rest[..idx_end];
      if let Ok(idx) = digits.parse::<usize>()
        && idx < saved.len()
      {
        out.push_str(&saved[idx]);
        out.push('\n');
        continue;
      }
    }
    out.push_str(line);
    out.push('\n');
  }
  if !input.ends_with('\n') && out.ends_with('\n') {
    out.pop();
  }
  out
}

/// "Compact clauses" middle ground: for every DML statement, keep each
/// top-level clause keyword (SELECT / FROM / WHERE / JOIN / GROUP BY /
/// HAVING / ORDER BY / LIMIT / RETURNING / VALUES / UPDATE / INSERT /
/// DELETE / SET / ON CONFLICT) on its own line, but inline the clause
/// body. DDL is left alone.
/// sql-formatter spreads `SET search_path = pg_catalog, pg_temp` inside
/// a CREATE FUNCTION attribute list across 3+ lines (one for SET, one
/// per comma-separated value). For a function attribute that's three
/// short identifiers max, that's noisy. Collapse the SET clause back
/// onto a single line, indented to match the surrounding attribute
/// block (RETURNS / LANGUAGE / VOLATILITY / etc).
fn collapse_function_set_clause(input: &str) -> String {
  let lines: Vec<&str> = input.lines().collect();
  let mut out = String::with_capacity(input.len());
  let mut i = 0;
  let mut in_create_fn = false;
  let mut prev_attr_indent: Option<String> = None;
  while i < lines.len() {
    let line = lines[i];
    let upper = line.to_ascii_uppercase();
    let trimmed_upper = upper.trim_start();
    // Track entering / leaving a CREATE [OR REPLACE] FUNCTION block.
    if trimmed_upper.starts_with("CREATE FUNCTION")
      || trimmed_upper.starts_with("CREATE OR REPLACE FUNCTION")
      || trimmed_upper.starts_with("CREATE PROCEDURE")
      || trimmed_upper.starts_with("CREATE OR REPLACE PROCEDURE")
    {
      in_create_fn = true;
      prev_attr_indent = None;
    }
    // Attribute lines that anchor the indent we want SET to match.
    if in_create_fn {
      let trimmed = line.trim_start();
      let first = trimmed
        .split(|c: char| !c.is_ascii_alphabetic())
        .next()
        .unwrap_or("")
        .to_ascii_uppercase();
      if matches!(
        first.as_str(),
        "RETURNS" | "LANGUAGE" | "STABLE" | "IMMUTABLE" | "VOLATILE" | "STRICT" | "PARALLEL" | "LEAKPROOF" | "SECURITY" | "COST" | "ROWS" | "WINDOW"
      ) {
        let indent: String = line.chars().take_while(|c| c.is_whitespace()).collect();
        prev_attr_indent = Some(indent);
      }
    }
    // Bare `SET` (or with leading whitespace then `SET`) followed by a
    // multi-line assignment list inside a function attribute block.
    if in_create_fn && trimmed_upper.trim_end() == "SET" {
      // Collect continuation lines until the assignment list closes.
      let mut combined = String::new();
      let mut j = i + 1;
      while j < lines.len() {
        let cont = lines[j].trim();
        if cont.is_empty() {
          break;
        }
        let cont_upper = cont.to_ascii_uppercase();
        // Stop when we hit AS, the body start, or another attribute.
        if cont_upper == "AS"
          || cont_upper.starts_with("AS ")
          || cont_upper.starts_with("AS$$")
          || cont_upper.starts_with("RETURNS")
          || cont_upper.starts_with("LANGUAGE")
          || cont_upper.starts_with("VOLATILITY")
          || cont_upper.starts_with("STABLE")
          || cont_upper.starts_with("IMMUTABLE")
          || cont_upper.starts_with("VOLATILE")
          || cont_upper.starts_with("STRICT")
          || cont_upper.starts_with("PARALLEL")
          || cont_upper.starts_with("LEAKPROOF")
          || cont_upper.starts_with("SECURITY")
          || cont_upper.starts_with("WINDOW")
          || cont_upper.starts_with("COST")
          || cont_upper.starts_with("ROWS")
          || cont_upper.starts_with("SET ")
          || cont_upper.starts_with("RESET ")
          || cont.starts_with('$')
        {
          break;
        }
        if !combined.is_empty() {
          combined.push(' ');
        }
        combined.push_str(cont);
        j += 1;
        // A line that doesn't end with `,` is the terminator of the
        // assignment value list.
        if !cont.ends_with(',') {
          break;
        }
      }
      if !combined.is_empty() {
        let indent = prev_attr_indent.clone().unwrap_or_else(|| "    ".to_string());
        // Normalise spacing around `=` and after `,`.
        let mut collapsed = combined.replace(" ,", ",");
        while collapsed.contains(",  ") {
          collapsed = collapsed.replace(",  ", ", ");
        }
        // Trim trailing comma if any (shouldn't, but defensive).
        let collapsed = collapsed.trim_end_matches(',').trim().to_string();
        out.push_str(&format!("{indent}SET {collapsed}\n"));
        i = j;
        continue;
      }
    }
    // Reset state on dollar-quote body start (`$$ ... $$` body) since
    // we don't want to touch SETs that appear inside PL/pgSQL bodies.
    if in_create_fn && (trimmed_upper.contains("$$") || trimmed_upper == "AS") {
      // Stay in_create_fn until the closing `;` or matching $$.
    }
    if trimmed_upper.starts_with(';') || (in_create_fn && trimmed_upper == "$$;") {
      in_create_fn = false;
      prev_attr_indent = None;
    }
    out.push_str(line);
    out.push('\n');
    i += 1;
  }
  // Preserve trailing-newline shape: drop one trailing `\n` if the
  // original didn't end with one.
  if !input.ends_with('\n') && out.ends_with('\n') {
    out.pop();
  }
  out
}

/// Find every `CREATE [OR REPLACE] FUNCTION ... AS $tag$ ... $tag$`
/// (or PROCEDURE) and format the body slice as SQL. Without this pass
/// the body stays whatever the user wrote -- often a single long line
/// or carelessly indented draft -- because dollar-quoted regions are
/// passed through untouched by the rest of the pipeline.
///
/// Only SQL-language function bodies are touched. PL/pgSQL / plpython /
/// plperl bodies are left alone (sql-formatter can't parse them).
fn format_function_bodies(input: &str, fmt_style: &FormatterStyle) -> String {
  let bytes = input.as_bytes();
  let n = bytes.len();
  let mut out = String::with_capacity(input.len());
  let mut i = 0usize;
  while i < n {
    // Find `$$` or `$tag$` opener.
    let Some(open_rel) = input[i..].find('$') else {
      out.push_str(&input[i..]);
      break;
    };
    let open_start = i + open_rel;
    // Read dollar tag (between the two `$`).
    let mut tag_end = open_start + 1;
    while tag_end < n
      && (bytes[tag_end].is_ascii_alphanumeric() || bytes[tag_end] == b'_')
    {
      tag_end += 1;
    }
    if tag_end >= n || bytes[tag_end] != b'$' {
      // Not a dollar-quote opener (lone `$`).
      out.push_str(&input[i..=open_start]);
      i = open_start + 1;
      continue;
    }
    let tag = &input[open_start..=tag_end]; // `$tag$` (or `$$`)
    let body_start = tag_end + 1;
    let Some(close_rel) = input[body_start..].find(tag) else {
      // Unclosed dollar quote -- emit as-is, bail.
      out.push_str(&input[i..]);
      break;
    };
    let body_end = body_start + close_rel;
    let close_end = body_end + tag.len();
    // Pre-body chunk + opener.
    out.push_str(&input[i..body_start]);
    // Inspect the chunk we just emitted to decide whether this is an
    // SQL function body (vs PL/pgSQL / plpython / etc).
    let pre_upper = out.to_ascii_uppercase();
    let is_sql_fn_body =
      // Cheap upstream check: look back at the most recent LANGUAGE
      // clause, default to SQL when none seen yet (CREATE FUNCTION
      // defaults to SQL pre-PG14, otherwise the user has to declare).
      pre_upper.contains("CREATE FUNCTION") || pre_upper.contains("CREATE OR REPLACE FUNCTION")
        || pre_upper.contains("CREATE PROCEDURE") || pre_upper.contains("CREATE OR REPLACE PROCEDURE");
    let body = &input[body_start..body_end];
    // LANGUAGE clause may appear BEFORE or AFTER the body. Scan both
    // sides plus inspect the body itself for PL/pgSQL markers.
    let post_upper = input[close_end..].to_ascii_uppercase();
    // Cut post-search at the next `;` so we don't pull a LANGUAGE from
    // the NEXT statement.
    let post_clamp = post_upper.find(';').unwrap_or(post_upper.len());
    let post = &post_upper[..post_clamp];
    let body_upper = body.to_ascii_uppercase();
    let body_starts_with_plpgsql = {
      let trimmed = body_upper.trim_start();
      trimmed.starts_with("DECLARE")
        || trimmed.starts_with("BEGIN")
        || trimmed.starts_with("<<")
    };
    let mentions_non_sql_lang = |hay: &str| {
      hay.contains("LANGUAGE PLPGSQL")
        || hay.contains("LANGUAGE 'PLPGSQL'")
        || hay.contains("LANGUAGE PLPYTHON")
        || hay.contains("LANGUAGE PLPERL")
        || hay.contains("LANGUAGE PLTCL")
        || hay.contains("LANGUAGE C")
    };
    let language_is_sql = !mentions_non_sql_lang(&pre_upper)
      && !mentions_non_sql_lang(post)
      && !body_starts_with_plpgsql;
    if is_sql_fn_body && language_is_sql && !body.trim().is_empty() {
      let formatted_body = external::run_sql_formatter(body.trim(), fmt_style)
        .unwrap_or_else(|| body.trim().to_string());
      // PG convention: body content stays at column 0 between `$$`
      // markers. Leading + trailing newline so the closer sits on its
      // own line.
      out.push('\n');
      out.push_str(formatted_body.trim_end());
      if !out.ends_with('\n') {
        out.push('\n');
      }
    } else {
      // PL/pgSQL (or other non-SQL language) body: do NOT pass through
      // sql-formatter (it mangles control structures), but DO reflow
      // overly long expression lines. Specifically: if any line is
      // long (>100 chars) and contains a `(` group at top level whose
      // contents include top-level OR/AND, break that group across
      // multiple lines.
      out.push_str(&reflow_plpgsql_body(body));
    }
    // Closer.
    out.push_str(tag);
    i = close_end;
  }
  out
}

/// Reflow PL/pgSQL function body for readability. The plpgsql grammar
/// is not parsed; this is a textual best-effort pass that:
///
///   - preserves every control-flow line (BEGIN/END/IF/THEN/...);
///   - locates long lines (>100 chars) inside the body;
///   - finds the outermost `(...)` group in each long line and breaks
///     its contents at top-level OR / AND, putting each operand on
///     its own indented line.
///
/// Lines that fit the width budget are passed through unchanged. This
/// keeps the rest of the body shape intact so the existing PL/pgSQL
/// structure remains readable.
fn reflow_plpgsql_body(body: &str) -> String {
  const MAX_WIDTH: usize = 100;
  let mut out = String::with_capacity(body.len());
  for line in body.lines() {
    if line.len() <= MAX_WIDTH {
      out.push_str(line);
      out.push('\n');
      continue;
    }
    let Some(reflowed) = reflow_long_line(line) else {
      out.push_str(line);
      out.push('\n');
      continue;
    };
    out.push_str(&reflowed);
    if !reflowed.ends_with('\n') {
      out.push('\n');
    }
  }
  if !body.ends_with('\n') {
    let _ = out.pop();
  }
  out
}

/// Attempt to break a long line at top-level OR / AND inside its
/// outermost `(...)` group. Returns None when no group is found or
/// the contents don't have top-level OR/AND.
fn reflow_long_line(line: &str) -> Option<String> {
  let bytes = line.as_bytes();
  // Find the FIRST `(`.
  let open = bytes.iter().position(|&b| b == b'(')?;
  // Find its matching `)`.
  let mut depth = 1i32;
  let mut close = open + 1;
  while close < bytes.len() {
    match bytes[close] {
      b'(' => depth += 1,
      b')' => {
        depth -= 1;
        if depth == 0 {
          break;
        }
      },
      _ => {},
    }
    close += 1;
  }
  if close >= bytes.len() {
    return None;
  }
  let body = &line[open + 1..close];
  // Split body on top-level OR / AND.
  let parts = split_on_top_level_bool_op(body);
  if parts.len() < 2 {
    return None;
  }
  // Determine indent from the start of the line.
  let line_indent: String = line.chars().take_while(|c| c.is_whitespace()).collect();
  let inner_indent = format!("{line_indent}    ");
  let prefix = &line[..=open];
  let suffix = &line[close..];
  let mut out = String::new();
  out.push_str(prefix);
  out.push('\n');
  for (i, (op, content)) in parts.iter().enumerate() {
    out.push_str(&inner_indent);
    if i > 0
      && let Some(op_text) = op
    {
      out.push_str(op_text);
      out.push(' ');
    }
    out.push_str(content.trim());
    out.push('\n');
  }
  out.push_str(&line_indent);
  out.push_str(suffix);
  Some(out)
}

/// Split `s` at top-level boolean operators (OR / AND). Returns a list
/// of `(operator_before_chunk, chunk_text)`. The first chunk has no
/// operator. Operators are matched case-insensitively as whole words.
fn split_on_top_level_bool_op(s: &str) -> Vec<(Option<&'static str>, &str)> {
  let bytes = s.as_bytes();
  let n = bytes.len();
  let upper = s.to_ascii_uppercase();
  let upper_bytes = upper.as_bytes();
  let mut depth = 0i32;
  let mut in_single = false;
  let mut parts: Vec<(Option<&'static str>, &str)> = Vec::new();
  let mut chunk_start = 0usize;
  let mut prev_op: Option<&'static str> = None;
  let mut i = 0usize;
  while i < n {
    let b = bytes[i];
    if !in_single {
      if b == b'(' {
        depth += 1;
        i += 1;
        continue;
      }
      if b == b')' {
        depth -= 1;
        i += 1;
        continue;
      }
    }
    if b == b'\'' {
      in_single = !in_single;
      i += 1;
      continue;
    }
    if !in_single && depth == 0 {
      // Check for `OR ` or `AND ` as whole-word boundaries.
      let at_word_boundary = i == 0 || !is_word_byte(bytes[i - 1]);
      if at_word_boundary {
        if i + 2 < n && &upper_bytes[i..i + 2] == b"OR" && bytes[i + 2].is_ascii_whitespace() {
          parts.push((prev_op, &s[chunk_start..i]));
          prev_op = Some("OR");
          chunk_start = i + 3;
          i += 3;
          continue;
        }
        if i + 3 < n && &upper_bytes[i..i + 3] == b"AND" && bytes[i + 3].is_ascii_whitespace() {
          parts.push((prev_op, &s[chunk_start..i]));
          prev_op = Some("AND");
          chunk_start = i + 4;
          i += 4;
          continue;
        }
      }
    }
    i += 1;
  }
  parts.push((prev_op, &s[chunk_start..]));
  parts
}

fn is_word_byte(b: u8) -> bool {
  b.is_ascii_alphanumeric() || b == b'_'
}

fn compact_clause_lines(input: &str) -> String {
  rewrite_dml_statements(input, compact_clauses)
}

/// Walk a single statement; when a top-level clause keyword is detected
/// at start-of-line (after sql-formatter), join the clause body lines
/// (collapsing internal whitespace) until the next top-level clause
/// keyword. String literals / line comments / block comments / dollar-
/// quoted bodies are preserved verbatim.
fn compact_clauses(s: &str) -> String {
  // sql-formatter v15 puts every primary keyword at start-of-line. Detect
  // primary keywords + glue lines until next primary keyword.
  let primary: &[&str] = &[
    "SELECT", "FROM", "WHERE", "GROUP", "HAVING", "ORDER", "LIMIT", "OFFSET", "FETCH", "RETURNING", "VALUES", "INSERT",
    "UPDATE", "DELETE", "SET", "JOIN", "LEFT", "RIGHT", "INNER", "OUTER", "CROSS", "FULL", "NATURAL", "UNION", "EXCEPT",
    "INTERSECT", "WITH", "ON",
  ];
  let mut out = String::with_capacity(s.len());
  let mut current_clause: Option<String> = None;
  for line in s.lines() {
    let trimmed = line.trim_start();
    let first = trimmed.split(|c: char| !c.is_ascii_alphabetic()).next().unwrap_or("").to_ascii_uppercase();
    let starts_clause = !first.is_empty() && primary.contains(&first.as_str());
    if starts_clause {
      if let Some(buf) = current_clause.take() {
        out.push_str(buf.trim_end());
        out.push('\n');
      }
      current_clause = Some(line.to_string());
    } else if let Some(buf) = current_clause.as_mut() {
      let glue = trimmed.trim_end();
      if !glue.is_empty() {
        if !buf.ends_with(' ') {
          buf.push(' ');
        }
        buf.push_str(glue);
      }
    } else {
      out.push_str(line);
      out.push('\n');
    }
  }
  if let Some(buf) = current_clause {
    out.push_str(buf.trim_end());
    out.push('\n');
  }
  out
}

/// Collapse each DML statement (SELECT / INSERT / UPDATE / DELETE / WITH)
/// onto a single line. Walks top-level statements, joining internal
/// whitespace runs into single spaces. Leaves CREATE TABLE / FUNCTION /
/// VIEW / TRIGGER / etc untouched so table layouts stay readable.
fn collapse_dml_lines(input: &str) -> String {
  rewrite_dml_statements(input, collapse_whitespace)
}

/// True when `stmt` opens with a DML keyword whose body the post-passes
/// are allowed to reflow. CREATE / ALTER / DROP / DO / etc are excluded
/// so DDL bodies stay byte-for-byte as sql-formatter produced them.
/// Leading `-- ...` / `/* ... */` comment lines are skipped so a stmt
/// preceded by a comment block still classifies by its real keyword.
fn is_dml_statement(stmt: &str) -> bool {
  let mut rest = stmt;
  loop {
    rest = rest.trim_start_matches([' ', '\t', '\n', '\r']);
    if let Some(after) = rest.strip_prefix("--") {
      let end = after.find('\n').map(|i| i + 1).unwrap_or(after.len());
      rest = &after[end..];
      continue;
    }
    if rest.starts_with("/*")
      && let Some(end) = rest.find("*/")
    {
      rest = &rest[end + 2..];
      continue;
    }
    break;
  }
  let upper = rest.to_ascii_uppercase();
  upper.starts_with("SELECT")
    || upper.starts_with("INSERT")
    || upper.starts_with("UPDATE")
    || upper.starts_with("DELETE")
    || upper.starts_with("WITH")
    || upper.starts_with("VALUES")
}

/// Per-statement rewriter: split top-level statements, apply `f` to each
/// DML one, leave the rest verbatim, ensure each statement ends with `\n`.
fn rewrite_dml_statements(input: &str, f: impl Fn(&str) -> String) -> String {
  let stmts = split_top_level_statements(input);
  let mut out = String::with_capacity(input.len());
  for stmt in stmts {
    if is_dml_statement(&stmt) {
      out.push_str(&f(&stmt));
    } else {
      out.push_str(&stmt);
    }
    if !out.ends_with('\n') {
      out.push('\n');
    }
  }
  out
}

/// Split `src` at every top-level `;`, returning each statement (with
/// its trailing `;`). Honours single-quoted strings, line comments,
/// block comments, and `$$ ... $$` bodies so semicolons inside any of
/// those don't split.
fn split_top_level_statements(src: &str) -> Vec<String> {
  let bytes = src.as_bytes();
  let n = bytes.len();
  let mut out = Vec::new();
  let mut start = 0usize;
  let mut i = 0usize;
  while i < n {
    match bytes[i] {
      b'\'' => {
        i += 1;
        while i < n && bytes[i] != b'\'' {
          i += 1;
        }
        if i < n {
          i += 1;
        }
      },
      b'-' if i + 1 < n && bytes[i + 1] == b'-' => {
        while i < n && bytes[i] != b'\n' {
          i += 1;
        }
      },
      b'/' if i + 1 < n && bytes[i + 1] == b'*' => {
        i += 2;
        while i + 1 < n && !(bytes[i] == b'*' && bytes[i + 1] == b'/') {
          i += 1;
        }
        if i + 1 < n {
          i += 2;
        }
      },
      b'$' if i + 1 < n && bytes[i + 1] == b'$' => {
        i += 2;
        while i + 1 < n && !(bytes[i] == b'$' && bytes[i + 1] == b'$') {
          i += 1;
        }
        if i + 1 < n {
          i += 2;
        }
      },
      b';' => {
        i += 1;
        out.push(src[start..i].to_string());
        start = i;
      },
      _ => {
        i += 1;
      },
    }
  }
  if start < n {
    out.push(src[start..].to_string());
  }
  out
}

/// Replace every whitespace run inside `s` with a single space, but
/// preserve single-quoted strings + dollar-quoted bodies + line comments
/// (those would change semantics if collapsed). Also keep a single
/// leading newline so adjacent statements visually separate.
fn collapse_whitespace(s: &str) -> String {
  let bytes = s.as_bytes();
  let n = bytes.len();
  let leading_nl = bytes.iter().take_while(|b| **b == b'\n' || **b == b'\r').count();
  let leading = "\n".repeat(leading_nl.min(1));
  let mut out = String::with_capacity(s.len());
  out.push_str(&leading);
  let mut i = leading_nl;
  let mut prev_space = true; // suppress leading run
  while i < n {
    match bytes[i] {
      b'\'' => {
        out.push('\'');
        i += 1;
        while i < n && bytes[i] != b'\'' {
          out.push(bytes[i] as char);
          i += 1;
        }
        if i < n {
          out.push('\'');
          i += 1;
        }
        prev_space = false;
      },
      b'-' if i + 1 < n && bytes[i + 1] == b'-' => {
        // Line comment up to end of line. To keep the rest on one line,
        // convert to a block comment.
        let mut end = i + 2;
        while end < n && bytes[end] != b'\n' {
          end += 1;
        }
        let comment = &s[i + 2..end];
        out.push_str("/* ");
        out.push_str(comment.trim());
        out.push_str(" */");
        i = end;
        prev_space = false;
      },
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
      b'$' if i + 1 < n && bytes[i + 1] == b'$' => {
        let start = i;
        i += 2;
        while i + 1 < n && !(bytes[i] == b'$' && bytes[i + 1] == b'$') {
          i += 1;
        }
        if i + 1 < n {
          i += 2;
        }
        out.push_str(&s[start..i]);
        prev_space = false;
      },
      c if (c as char).is_whitespace() => {
        if !prev_space {
          out.push(' ');
          prev_space = true;
        }
        i += 1;
      },
      c => {
        out.push(c as char);
        i += 1;
        prev_space = false;
      },
    }
  }
  // Strip a trailing space before `;` to keep `SELECT 1;` not `SELECT 1 ;`.
  if out.ends_with(' ') {
    out.pop();
  }
  out
}

/// Drop the space between a function name and its opening `(`. PG's
/// canonical style is `length(x)`, not `length (x)`. Applies to both
/// declarations (`CREATE FUNCTION foo()`) and calls (`EXECUTE FUNCTION
/// set_updated_at()`, `SELECT length(x)`).
///
/// Conservative: only collapse when the identifier sits in a function
/// position. SQL keywords that introduce a grouping paren (`IN (...)`,
/// `EXISTS (...)`, `VALUES (...)`, `SELECT ...(...)`, etc.) are left
/// alone because their paren is not a call.
fn tighten_call_parens(input: &str) -> String {
  let bytes = input.as_bytes();
  let mut out = String::with_capacity(input.len());
  let mut i = 0usize;
  while i < bytes.len() {
    // Skip string literals + line comments + dollar-quoted bodies untouched.
    if bytes[i] == b'\'' {
      out.push('\'');
      i += 1;
      while i < bytes.len() && bytes[i] != b'\'' {
        i = push_one_char(&mut out, input, i);
      }
      if i < bytes.len() {
        out.push('\'');
        i += 1;
      }
      continue;
    }
    if i + 1 < bytes.len() && bytes[i] == b'-' && bytes[i + 1] == b'-' {
      while i < bytes.len() && bytes[i] != b'\n' {
        i = push_one_char(&mut out, input, i);
      }
      continue;
    }
    // Identifier?
    if bytes[i].is_ascii_alphabetic() || bytes[i] == b'_' {
      let id_start = i;
      while i < bytes.len() && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_') {
        i += 1;
      }
      let id = &input[id_start..i];
      // Look at whitespace + `(`.
      let mut j = i;
      let ws_start = j;
      while j < bytes.len() && (bytes[j] == b' ' || bytes[j] == b'\t') {
        j += 1;
      }
      if j > ws_start && j < bytes.len() && bytes[j] == b'(' {
        let upper = id.to_ascii_uppercase();
        if !KEEP_SPACE_BEFORE_PAREN.contains(&upper.as_str()) {
          out.push_str(id);
          // Skip the whitespace -- write nothing then continue from `(`.
          i = j;
          continue;
        }
      }
      out.push_str(id);
      continue;
    }
    i = push_one_char(&mut out, input, i);
  }
  out
}

/// Push the UTF-8 char starting at byte index `i` of `src` onto `out`
/// and return the new index past that char. Multi-byte aware -- using
/// `bytes[i] as char` here would reinterpret each UTF-8 continuation
/// byte as a Latin-1 codepoint, mangling every non-ASCII character.
pub(crate) fn push_one_char(out: &mut String, src: &str, i: usize) -> usize {
  let c = src[i..].chars().next().expect("caller guarantees i < src.len()");
  out.push(c);
  i + c.len_utf8()
}

/// SQL keywords whose following `(` is a grouping / sub-query / IN-list
/// paren rather than a function call. Keep the space for readability.
const KEEP_SPACE_BEFORE_PAREN: &[&str] = &[
  "SELECT",
  "IN",
  "NOT",
  "EXISTS",
  "VALUES",
  "RETURNING",
  "WHERE",
  "HAVING",
  "ON",
  "USING",
  "FROM",
  "INTO",
  "AS",
  "WITH",
  "CASE",
  "WHEN",
  "THEN",
  "ELSE",
  "ANY",
  "ALL",
  "SOME",
  "AND",
  "OR",
  "BY",
  "IS",
  "BETWEEN",
  "LIKE",
  "ILIKE",
  "SIMILAR",
  "OVERLAPS",
  "FILTER",
  "OVER",
  "PARTITION",
  "WITHIN",
  "PRECEDING",
  "FOLLOWING",
  "UNBOUNDED",
  "FOR",
  "ROW",
  "ROWS",
  "GROUPS",
  "RANGE",
  "DEFAULT",
  "REFERENCES",
  "CHECK",
  "UNIQUE",
  "PRIMARY",
  "FOREIGN",
  "KEY",
  "CONSTRAINT",
  "DISTINCT",
  "GROUP",
  "ORDER",
  "LIMIT",
  "OFFSET",
  "FETCH",
  "INTERSECT",
  "UNION",
  "EXCEPT",
  "DO",
  "LANGUAGE",
  "MATCH",
  "TO",
  "OF",
  "RESTRICT",
  "CASCADE",
];

#[cfg(test)]
mod tighten_tests {
  use super::*;

  #[test]
  fn collapses_function_call_space() {
    assert_eq!(tighten_call_parens("SELECT length (x);"), "SELECT length(x);");
  }

  #[test]
  fn collapses_execute_function() {
    let input = "CREATE TRIGGER t BEFORE UPDATE ON users EXECUTE FUNCTION set_updated_at ();";
    let output = tighten_call_parens(input);
    assert!(output.contains("set_updated_at()"), "got: {output}");
  }

  #[test]
  fn collapses_create_function_decl() {
    let input = "CREATE FUNCTION foo () RETURNS int AS $$ SELECT 1 $$ LANGUAGE sql;";
    let output = tighten_call_parens(input);
    assert!(output.contains("foo()"), "got: {output}");
  }

  #[test]
  fn keeps_space_after_select() {
    assert_eq!(tighten_call_parens("SELECT (1 + 2);"), "SELECT (1 + 2);");
  }

  #[test]
  fn keeps_space_after_in() {
    assert_eq!(tighten_call_parens("WHERE id IN (1, 2);"), "WHERE id IN (1, 2);");
  }

  #[test]
  fn leaves_string_literal_alone() {
    assert_eq!(tighten_call_parens("SELECT 'foo (bar)';"), "SELECT 'foo (bar)';");
  }
}

/// Collapse runs of >=2 blank lines down to a single blank line and
/// strip trailing blank lines entirely, ensuring the output ends with
/// exactly one `\n`. Common editor hygiene that every formatter is
/// expected to do.
fn normalize_blank_lines(input: &str) -> String {
  let mut out: Vec<&str> = Vec::with_capacity(input.lines().count());
  let mut prev_blank = false;
  for line in input.lines() {
    let blank = line.chars().all(|c| c.is_whitespace());
    if blank && prev_blank {
      continue;
    }
    out.push(line);
    prev_blank = blank;
  }
  while out.last().is_some_and(|l| l.chars().all(|c| c.is_whitespace())) {
    out.pop();
  }
  let mut s = out.join("\n");
  if !s.is_empty() {
    s.push('\n');
  }
  s
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn collapses_consecutive_blank_lines() {
    let input = "SELECT 1;\n\n\n\nSELECT 2;\n";
    assert_eq!(normalize_blank_lines(input), "SELECT 1;\n\nSELECT 2;\n");
  }

  #[test]
  fn strips_trailing_blank_lines() {
    let input = "SELECT 1;\n\n\n\n";
    assert_eq!(normalize_blank_lines(input), "SELECT 1;\n");
  }

  #[test]
  fn preserves_single_blank_separator() {
    let input = "SELECT 1;\n\nSELECT 2;\n";
    assert_eq!(normalize_blank_lines(input), "SELECT 1;\n\nSELECT 2;\n");
  }

  #[test]
  fn ensures_trailing_newline() {
    let input = "SELECT 1;";
    assert_eq!(normalize_blank_lines(input), "SELECT 1;\n");
  }

  #[test]
  fn empty_input_stays_empty() {
    assert_eq!(normalize_blank_lines(""), "");
    assert_eq!(normalize_blank_lines("\n\n\n"), "");
  }
}
