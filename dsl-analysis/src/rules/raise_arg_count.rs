//! sql036: `RAISE EXCEPTION` (or NOTICE/WARNING/etc.) format string `%`
//! placeholder count doesn't match the supplied argument count.
//!
//! Postgres errors with `too few parameters specified for RAISE` /
//! `too many parameters specified for RAISE` at runtime. Catch it at
//! edit time.

use crate::{Diagnostic, LintRule, Severity};
use crate::textutil::is_word;
use dsl_catalog::Catalog;
use dsl_parse::{Statement, StatementKind};
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql036"
  }
  fn default_severity(&self) -> Severity {
    Severity::Warning
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    if !matches!(stmt.kind, StatementKind::Unknown { .. }) {
      return;
    }
    let (start, body, upper) = crate::stmt_body_upper(stmt, source);
    if !upper.contains("CREATE") || !upper.contains("FUNCTION") {
      return;
    }
    let Some(body_text) = dollar_body(body) else { return };

    // Walk each RAISE statement: locate keyword, then the next
    // single-quoted format string, then count `,` arguments up to
    // `;` or `USING`.
    let upper_body = body_text.to_ascii_uppercase();
    let bytes = upper_body.as_bytes();
    let raw = body_text.as_bytes();
    let n = bytes.len();
    let mut i = 0;
    while i + 5 <= n {
      if &upper_body[i..i + 5] == "RAISE" {
        let prev_ok = i == 0 || !is_word(bytes[i - 1] as char);
        let next_ok = i + 5 == n || !is_word(bytes[i + 5] as char);
        if prev_ok
          && next_ok
          && let Some((placeholders, args, fmt_start, fmt_end)) = parse_raise(raw, i)
          && placeholders != args
        {
          // Map to absolute offset in source.
          let base = source.find(body_text).unwrap_or(start);
          let abs_start = base + fmt_start;
          let abs_end = base + fmt_end;
          out.push(Diagnostic {
            code: "sql036",
            severity: Severity::Warning,
            message: format!("RAISE format has {placeholders} `%` placeholder(s) but {args} argument(s)"),
            range: crate::range_at(abs_start, abs_end),
          });
        }
      }
      i += 1;
    }
  }
}

/// Starting at the RAISE keyword, find the format-string + args.
/// Returns (placeholder_count, arg_count, fmt_start, fmt_end) on success.
fn parse_raise(bytes: &[u8], raise_at: usize) -> Option<(usize, usize, usize, usize)> {
  let n = bytes.len();
  let mut i = raise_at + 5;
  // Skip optional level keyword: EXCEPTION / NOTICE / WARNING / INFO / LOG / DEBUG
  while i < n && bytes[i].is_ascii_whitespace() {
    i += 1;
  }
  for level in ["EXCEPTION", "NOTICE", "WARNING", "INFO", "LOG", "DEBUG"] {
    let lvl = level.as_bytes();
    if i + lvl.len() <= n
      && bytes[i..i + lvl.len()].eq_ignore_ascii_case(lvl)
      && (i + lvl.len() == n || !is_word(bytes[i + lvl.len()] as char))
    {
      i += lvl.len();
      while i < n && bytes[i].is_ascii_whitespace() {
        i += 1;
      }
      break;
    }
  }
  // Expect a single-quoted string.
  if i >= n || bytes[i] != b'\'' {
    return None;
  }
  let fmt_start = i;
  i += 1;
  let body_start = i;
  while i < n && bytes[i] != b'\'' {
    i += 1;
  }
  if i >= n {
    return None;
  }
  let fmt = &bytes[body_start..i];
  let fmt_end = i + 1;
  // Count `%` placeholders, ignoring `%%` (literal percent).
  let mut placeholders = 0usize;
  let mut k = 0;
  while k < fmt.len() {
    if fmt[k] == b'%' {
      if k + 1 < fmt.len() && fmt[k + 1] == b'%' {
        k += 2;
        continue;
      }
      placeholders += 1;
    }
    k += 1;
  }
  // Count comma-separated args after the closing quote, up to `;` or
  // `USING`.
  i = fmt_end;
  let mut args = 0usize;
  let mut started = false;
  let mut depth = 0i32;
  while i < n {
    let c = bytes[i];
    match c {
      b';' => break,
      b'(' => depth += 1,
      b')' => depth -= 1,
      b'\'' => {
        i += 1;
        while i < n && bytes[i] != b'\'' {
          i += 1;
        }
      },
      _ => {},
    }
    if depth == 0 && c.is_ascii_alphabetic() {
      // Watch for `USING` to terminate the arg list.
      if i + 5 <= n && bytes[i..i + 5].eq_ignore_ascii_case(b"USING") && (i + 5 == n || !is_word(bytes[i + 5] as char))
      {
        break;
      }
    }
    if c == b',' && depth == 0 {
      args += 1;
      started = true;
    } else if !c.is_ascii_whitespace() && c != b',' && !started && c != b';' {
      started = true;
    }
    i += 1;
  }
  // After fmt:
  //   no commas, no content -> 0 args
  //   no commas, content    -> 1 arg
  //   N commas              -> N args (N commas separate N args from fmt)
  if started && args == 0 {
    args = 1;
  }
  Some((placeholders, args, fmt_start, fmt_end))
}

fn dollar_body(text: &str) -> Option<&str> {
  let start = text.find("$$")?;
  let after = start + 2;
  let end_rel = text[after..].find("$$")?;
  Some(&text[after..after + end_rel])
}

