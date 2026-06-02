//! Diagnostic engine for duck-sqllsp.
//!
//! Each rule is a [`LintRule`] impl in [`rules`]; [`run`] fans out every
//! statement across every registered rule and returns the flat diagnostic
//! list. Rules are tagged with stable codes (sql000..sql099) so users can
//! disable individual rules through configuration.

pub mod clause_scan;
pub mod ct_model;
pub mod diagnostic;
pub mod rules;
pub mod textutil;
pub mod typing;

pub use diagnostic::{range_at, Diagnostic, Severity};

/// Return the `(start, end_clamped)` byte offsets of a statement
/// within `source`. `end` is clamped to `source.len()` because the
/// parser sometimes reports a range that extends past EOF on
/// recovered statements.
#[inline]
pub fn stmt_bounds(stmt: &Statement, source: &str) -> (usize, usize) {
  let start = u32::from(stmt.range.start()) as usize;
  let end = (u32::from(stmt.range.end()) as usize).min(source.len());
  (start, end)
}

/// Return `(start, body_slice)` for a statement. Convenience wrapper
/// around [`stmt_bounds`] for rules that only need the body slice and
/// the start offset (the end offset is implied by `body.len()`).
#[inline]
pub fn stmt_body<'a>(stmt: &Statement, source: &'a str) -> (usize, &'a str) {
  let (start, end) = stmt_bounds(stmt, source);
  (start, &source[start..end])
}

/// Return `(start, body_slice, body_uppercase)` for a statement. Most
/// rules want all three -- factoring this saves a separate
/// `to_ascii_uppercase` line per rule.
///
/// Uses a thread-local cache keyed by `(start, end)` -- many rules call
/// this on the same statement during a single `run()`, and the cache
/// turns a per-rule O(body_len) uppercase into a single allocation per
/// statement. The cache is reset between statements by `run()`.
#[inline]
pub fn stmt_body_upper<'a>(stmt: &Statement, source: &'a str) -> (usize, &'a str, String) {
  let (start, body) = stmt_body(stmt, source);
  let end = start + body.len();
  let upper = STMT_UPPER_CACHE.with(|c| {
    let mut slot = c.borrow_mut();
    if let Some((s, e, ref up)) = *slot
      && s == start
      && e == end
    {
      return up.clone();
    }
    let up = body.to_ascii_uppercase();
    *slot = Some((start, end, up.clone()));
    up
  });
  (start, body, upper)
}

thread_local! {
  static STMT_UPPER_CACHE: std::cell::RefCell<Option<(usize, usize, String)>> =
    const { std::cell::RefCell::new(None) };
}

fn reset_stmt_upper_cache() {
  STMT_UPPER_CACHE.with(|c| *c.borrow_mut() = None);
}

use dsl_catalog::Catalog;
use dsl_parse::{Dialect, ParseError, ParsedFile, Statement};
use dsl_resolve::Scope;

/// Diagnostic codes that detect MySQL syntax inside a PG buffer. When
/// the buffer's `Dialect` is MySql these are irrelevant -- the syntax
/// they flag is correct for the actual target -- so we skip them.
const MYSQL_PORT_CODES: &[&str] = &[
  "sql276", // INTERVAL literal needs quotes
  "sql313", // inline COMMENT in CREATE TABLE
  "sql314", // AUTO_INCREMENT
  "sql315", // ENGINE=
  "sql316", // TINYINT / MEDIUMINT / LONGTEXT / DATETIME / BLOB
];

/// MSSQL/T-SQL port-detection codes. Skipped on MSSQL buffers (we
/// route those through Generic since we have no MSSQL dialect yet).
const MSSQL_PORT_CODES: &[&str] = &["sql317", "sql318", "sql321", "sql322"];

/// Oracle port-detection codes.
const ORACLE_PORT_CODES: &[&str] = &["sql323", "sql324", "sql325", "sql326"];

/// Cross-dialect codes (ISNULL/NVL/IFNULL, GETDATE/SYSDATE) -- skip on
/// any non-PG buffer since the rewrite suggestion is dialect-specific.
const CROSS_DIALECT_CODES: &[&str] = &["sql319", "sql320"];

fn skip_for_dialect(dialect: Dialect, code: &str) -> bool {
  match dialect {
    Dialect::Postgres => false,
    Dialect::MySql => MYSQL_PORT_CODES.contains(&code) || CROSS_DIALECT_CODES.contains(&code),
    Dialect::MsSql => MSSQL_PORT_CODES.contains(&code) || CROSS_DIALECT_CODES.contains(&code),
    Dialect::SQLite | Dialect::Generic => {
      MYSQL_PORT_CODES.contains(&code)
        || MSSQL_PORT_CODES.contains(&code)
        || ORACLE_PORT_CODES.contains(&code)
        || CROSS_DIALECT_CODES.contains(&code)
    },
  }
}

/// Per-statement parser errors -> sql000 diagnostics. Always run.
fn parser_diags(source: &str, errors: &[ParseError]) -> Vec<Diagnostic> {
  errors
    .iter()
    .filter(|e| !error_is_psql_meta(source, e))
    .map(|e| {
      // pg_query messages embed the rejected token as `at or near "<tok>"`.
      // The default range covers the entire failing statement, which is
      // a poor pointer for editor squiggles. When we can recover the
      // offending token, narrow the range to the LAST occurrence of it
      // inside the chunk (last because the same token often appears in
      // legal context earlier -- e.g. the `(` for the column list, and
      // the `(` that actually broke the parse is the last one before
      // pg_query gave up).
      let s: usize = u32::from(e.range.start()) as usize;
      let chunk_end: usize = (u32::from(e.range.end()) as usize).min(source.len());
      let mut narrowed = e.range;
      if let Some(tok) = extract_at_or_near(&e.message) {
        let chunk = source.get(s..chunk_end).unwrap_or("");
        if let Some(rel) = chunk.rfind(tok.as_str()) {
          let abs_start = s + rel;
          let abs_end = abs_start + tok.len();
          narrowed = text_size::TextRange::new(
            (abs_start as u32).into(),
            (abs_end as u32).into(),
          );
        }
      }
      // Strip our own `pg_query: Invalid statement:` prefix that the
      // backend wraps around libpg_query's terse message -- the
      // important part is the trailing "syntax error at or near \"<tok>\"".
      let cleaned = e
        .message
        .strip_prefix("pg_query: Invalid statement: ")
        .unwrap_or(&e.message)
        .to_string();
      // Drizzle pattern: `jsonb DEFAULT {} ...` -- the bare `{}` is
      // invalid PG syntax (needs `'{}'::jsonb`), but drizzle emits it
      // for empty-object defaults. Downgrade error to hint with a
      // pointer to the fix so the migration doesn't show as red while
      // the user decides whether to patch drizzle's emit.
      let (severity, final_message) = if drizzle_jsonb_default_braces(source, s, chunk_end) {
        (
          Severity::Hint,
          "drizzle emit: `DEFAULT {}` is not valid PG syntax -- replace with `DEFAULT '{}'::jsonb` (PG parses bare `{}` as array literal terminator and rejects it)".to_string(),
        )
      } else {
        (Severity::Error, cleaned)
      };
      Diagnostic {
        code: "sql000",
        severity,
        message: final_message,
        range: narrowed,
      }
    })
    .collect()
}

/// True when the failing chunk contains `jsonb DEFAULT {` -- the exact
/// drizzle migration emit for an empty-object jsonb default. Case
/// insensitive on the keywords; tolerates arbitrary whitespace.
fn drizzle_jsonb_default_braces(source: &str, chunk_start: usize, chunk_end: usize) -> bool {
  let chunk = source.get(chunk_start..chunk_end).unwrap_or("");
  let upper = chunk.to_ascii_uppercase();
  let mut from = 0usize;
  while let Some(at) = upper[from..].find("JSONB") {
    let pos = from + at;
    let after = upper[pos + "JSONB".len()..].trim_start();
    if let Some(rest) = after.strip_prefix("DEFAULT") {
      let after_def = rest.trim_start();
      if after_def.starts_with('{') {
        return true;
      }
    }
    from = pos + "JSONB".len();
  }
  false
}

/// Pull `<tok>` out of pg_query's `... at or near "<tok>" ...` error
/// shape. Returns None when the message is in a different form.
fn extract_at_or_near(msg: &str) -> Option<String> {
  let at = msg.find("at or near \"")?;
  let after = &msg[at + "at or near \"".len()..];
  let end = after.find('"')?;
  if end == 0 {
    return None;
  }
  Some(after[..end].to_string())
}

/// True when ANY line in the chunk spanned by the error starts with
/// `\<letter>` -- a psql meta-command. sql310 already reports those;
/// suppress the redundant sql000 from pg_query.
fn error_is_psql_meta(source: &str, e: &ParseError) -> bool {
  let s: usize = u32::from(e.range.start()) as usize;
  let e_end: usize = (u32::from(e.range.end()) as usize).min(source.len());
  let chunk = source.get(s..e_end).unwrap_or("");
  chunk.lines().any(|line| {
    let t = line.trim_start();
    let b = t.as_bytes();
    b.len() >= 2 && b[0] == b'\\' && b[1].is_ascii_alphabetic()
  })
}

pub trait LintRule: Send + Sync {
  fn code(&self) -> &'static str;
  fn default_severity(&self) -> Severity;
  fn check(&self, source: &str, stmt: &Statement, scope: &Scope, catalog: &Catalog, out: &mut Vec<Diagnostic>);
}

pub fn run(source: &str, file: &ParsedFile, scopes: &[Scope], catalog: &Catalog) -> Vec<Diagnostic> {
  run_with_dialect(source, file, scopes, catalog, Dialect::Postgres)
}

/// Like [`run`] but skips port-detection rules that don't apply when
/// the buffer's dialect already matches the would-be foreign syntax.
/// e.g. on a MySQL buffer, the AUTO_INCREMENT-is-MySQL hint is wrong.
pub fn run_with_dialect(
  source: &str,
  file: &ParsedFile,
  scopes: &[Scope],
  catalog: &Catalog,
  dialect: Dialect,
) -> Vec<Diagnostic> {
  // Silence panic messages from caught rule panics; catch_unwind still
  // unwinds the stack normally, but the default hook prints to stderr.
  let prev_hook = std::panic::take_hook();
  std::panic::set_hook(Box::new(|_| {}));
  let mut out = parser_diags(source, &file.errors);
  let registered = rules::all();
  // Pre-filter dialect-skipped rules once instead of per-statement.
  let active: Vec<&Box<dyn LintRule>> =
    registered.iter().filter(|r| !skip_for_dialect(dialect, r.code())).collect();
  for (stmt, scope) in file.statements.iter().zip(scopes.iter()) {
    reset_stmt_upper_cache();
    let trimmed = trim_stmt_range(stmt, source);
    // Defensive: a rule that panics (e.g. byte-indexing a multi-byte
    // codepoint) shouldn't kill diagnostics for the whole buffer. To
    // keep panic-isolation cheap, wrap the whole rule loop for each
    // statement in a single catch_unwind. Trade-off: a panic in one
    // rule loses diagnostics from rules that hadn't run yet for the
    // SAME statement -- but every subsequent statement still runs all
    // rules cleanly. Per-rule wrapping was ~600x more catch_unwind
    // frames per statement, dominating runtime on large buffers.
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
      let mut local: Vec<Diagnostic> = Vec::new();
      for rule in &active {
        rule.check(source, &trimmed, scope, catalog, &mut local);
      }
      local
    }));
    if let Ok(mut local) = result {
      out.append(&mut local);
    } else {
      // One rule panicked. Fall back to per-rule isolation for THIS
      // statement so the remaining rules still contribute.
      for rule in &active {
        let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
          let mut local: Vec<Diagnostic> = Vec::new();
          rule.check(source, &trimmed, scope, catalog, &mut local);
          local
        }));
        if let Ok(mut l) = r {
          out.append(&mut l);
        }
      }
    }
  }
  // Restore the previous panic hook so non-rule panics surface normally.
  std::panic::set_hook(prev_hook);
  // Belt-and-suspenders: some rules emit a different diagnostic code
  // than their `rule.code()` value (e.g. composite rules). Drop any
  // produced diagnostic whose emitted code is dialect-skipped.
  out.retain(|d| !skip_for_dialect(dialect, d.code));
  apply_suppressions(source, &mut out);
  // Drop exact duplicates (same code + same range + same message).
  // Rules occasionally emit identical hits when, e.g., a multi-stmt
  // chunk has overlapping scans -- pruning here keeps the UI clean.
  let mut seen: std::collections::HashSet<(String, u32, u32, String)> = std::collections::HashSet::new();
  out.retain(|d| {
    seen.insert((d.code.to_string(), u32::from(d.range.start()), u32::from(d.range.end()), d.message.clone()))
  });
  out
}

fn apply_suppressions(source: &str, diags: &mut Vec<Diagnostic>) {
  let suppressions = collect_suppressions(source);
  diags.retain(|d| {
    let line = line_of(source, u32::from(d.range.start()) as usize);
    !suppressions.iter().any(|(suppr_line, codes)| *suppr_line == line && (codes.is_empty() || codes.contains(&d.code)))
  });
}

/// Walk every line; for each `-- duck-sqllsp: ignore[-next-line] [<code>...]`
/// emit a `(target_line, codes)` tuple. Empty `codes` means "every
/// diagnostic on the target line".
fn collect_suppressions(source: &str) -> Vec<(usize, Vec<&'static str>)> {
  let mut out: Vec<(usize, Vec<&'static str>)> = Vec::new();
  for (idx, line) in source.lines().enumerate() {
    let Some(at) = line.to_ascii_lowercase().find("-- duck-sqllsp:") else { continue };
    let payload = line[at + "-- duck-sqllsp:".len()..].trim().to_ascii_lowercase();
    let next_line = payload.starts_with("ignore-next-line");
    let same_line = payload.starts_with("ignore");
    if !next_line && !same_line {
      continue;
    }
    let after_kw = if next_line { "ignore-next-line".len() } else { "ignore".len() };
    let codes_raw = payload[after_kw..].trim();
    let codes: Vec<&'static str> = codes_raw
      .split(|c: char| c.is_whitespace() || c == ',')
      .filter(|s| !s.is_empty())
      .filter_map(|s| static_code_name(s))
      .collect();
    let target = if next_line { idx + 1 } else { idx };
    out.push((target, codes));
  }
  out
}

/// Diagnostic.code is &'static str; only allow well-formed sqlNNN /
/// custom-named codes that fit into a leaked static slot. Returning
/// None for anything else (typo / garbage) is safer than allocating
/// per-comment.
fn static_code_name(raw: &str) -> Option<&'static str> {
  let trimmed = raw.trim();
  if !trimmed.starts_with("sql") {
    return None;
  }
  // Allocate a 'static slot for each unique code seen. Cheap: there
  // are only ~150 distinct codes in the rule registry.
  use std::collections::HashMap;
  use std::sync::Mutex;
  use std::sync::OnceLock;
  static CACHE: OnceLock<Mutex<HashMap<String, &'static str>>> = OnceLock::new();
  let cache = CACHE.get_or_init(|| Mutex::new(HashMap::new()));
  let mut guard = cache.lock().ok()?;
  if let Some(s) = guard.get(trimmed) {
    return Some(*s);
  }
  let leaked: &'static str = Box::leak(trimmed.to_string().into_boxed_str());
  guard.insert(trimmed.to_string(), leaked);
  Some(leaked)
}

fn line_of(source: &str, byte: usize) -> usize {
  source[..byte.min(source.len())].bytes().filter(|b| *b == b'\n').count()
}

/// Build a `Statement` clone whose range starts at the first non-
/// whitespace byte. Per-rule arithmetic on stmt.range then maps to
/// the actual statement body instead of leading whitespace.
fn trim_stmt_range(stmt: &dsl_parse::Statement, source: &str) -> dsl_parse::Statement {
  let s: u32 = stmt.range.start().into();
  let e: u32 = stmt.range.end().into();
  let mut start = s as usize;
  let end = (e as usize).min(source.len());
  let bytes = source.as_bytes();
  while start < end && bytes[start].is_ascii_whitespace() {
    start += 1;
  }
  let mut out = stmt.clone();
  out.range = text_size::TextRange::new((start as u32).into(), (end as u32).into());
  out
}
