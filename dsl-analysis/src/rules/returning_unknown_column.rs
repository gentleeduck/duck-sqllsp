//! sql350: `INSERT/UPDATE/DELETE ... RETURNING <list>` lists a column
//! not on the target table. Mirrors sql349 + sql002 coverage gaps.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::{Statement, StatementKind, TableRef};
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql350"
  }
  fn default_severity(&self) -> Severity {
    Severity::Error
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let table_ref: &TableRef = match &stmt.kind {
      StatementKind::Insert(ins) => &ins.table,
      StatementKind::Update(u) => &u.table,
      StatementKind::Delete(d) => &d.table,
      _ => return,
    };
    let Some(t) = catalog.find_table(table_ref.schema.as_deref(), &table_ref.name) else { return };
    let start: usize = u32::from(stmt.range.start()) as usize;
    let end: usize = (u32::from(stmt.range.end()) as usize).min(source.len());
    let raw_body = &source[start..end];
    // Strip line comments so `-- RETURNING bogus col` doesn't match.
    let body_owned = strip_line_comments(raw_body);
    let body = body_owned.as_str();
    let _upper = body.to_ascii_uppercase();
    // Find the OUTERMOST RETURNING (depth=0). A CTE body like
    // `WITH foo AS (UPDATE ... RETURNING id, data) INSERT INTO ...`
    // has its RETURNING inside parens; that RETURNING isn't the
    // outer statement's RETURNING -- and its column list shouldn't
    // be validated against the outer target table.
    let Some(ret_at) = find_top_returning(body) else { return };
    let after = ret_at + "RETURNING".len();
    let rest = &body[after..];
    let stop = rest.find(';').unwrap_or(rest.len());
    let list = &rest[..stop];
    // Walk top-level (paren-depth 0) commas.
    for raw in split_top_level_commas(list) {
      let token_full = raw.trim();
      // Strip a trailing alias: `col AS alias`.
      let token = token_full.split_whitespace().next().unwrap_or(token_full).trim_matches('"');
      // Skip *, expression forms, string literals, function calls.
      if token == "*" || token.is_empty() {
        continue;
      }
      if token.starts_with('\'') || token.starts_with('"') {
        continue;
      }
      if token.contains('(') || token.contains('.') || token.contains('-') || token.contains('+') {
        continue;
      }
      // First char must be a letter/underscore to be a bare column.
      let first = token.chars().next();
      if !first.is_some_and(|c| c.is_ascii_alphabetic() || c == '_') {
        continue;
      }
      if t.columns.iter().any(|c| c.name.eq_ignore_ascii_case(token)) {
        continue;
      }
      let local = list.find(token).unwrap_or(0);
      let abs_s = start + after + local;
      let abs_e = abs_s + token.len();
      out.push(Diagnostic {
        code: "sql350",
        severity: Severity::Error,
        message: format!("RETURNING references unknown column `{token}` on `{}`", table_ref.name),
        range: text_size::TextRange::new((abs_s as u32).into(), (abs_e as u32).into()),
      });
    }
  }
}

/// First depth-0 occurrence of the word `RETURNING` -- skips RETURNINGs
/// that sit inside a CTE body's parens.
fn find_top_returning(s: &str) -> Option<usize> {
  let bytes = s.as_bytes();
  let n = bytes.len();
  let kw = "RETURNING";
  let klen = kw.len();
  let mut depth = 0i32;
  let mut i = 0usize;
  while i + klen <= n {
    match bytes[i] {
      b'(' => {
        depth += 1;
        i += 1;
        continue;
      },
      b')' => {
        depth -= 1;
        i += 1;
        continue;
      },
      b'\'' => {
        i += 1;
        while i < n && bytes[i] != b'\'' {
          i += 1
        }
        if i < n {
          i += 1
        }
        continue;
      },
      _ => {},
    }
    if depth == 0 && s[i..i + klen].eq_ignore_ascii_case(kw) {
      let prev_ok = i == 0 || !(bytes[i - 1].is_ascii_alphanumeric() || bytes[i - 1] == b'_');
      let next_ok = i + klen == n || !(bytes[i + klen].is_ascii_alphanumeric() || bytes[i + klen] == b'_');
      if prev_ok && next_ok {
        return Some(i);
      }
    }
    i += 1;
  }
  None
}

fn split_top_level_commas(s: &str) -> Vec<&str> {
  let bytes = s.as_bytes();
  let n = bytes.len();
  let mut out = Vec::new();
  let mut start = 0usize;
  let mut depth = 0i32;
  let mut i = 0usize;
  while i < n {
    match bytes[i] {
      b'(' | b'[' => depth += 1,
      b')' | b']' => depth -= 1,
      b'\'' => {
        i += 1;
        while i < n && bytes[i] != b'\'' {
          i += 1
        }
      },
      b',' if depth == 0 => {
        out.push(&s[start..i]);
        start = i + 1;
      },
      _ => {},
    }
    i += 1;
  }
  if start < n {
    out.push(&s[start..]);
  }
  out
}

/// Replace `-- comment` runs with spaces so offsets stay 1:1.
fn strip_line_comments(s: &str) -> String {
  let mut out = String::with_capacity(s.len());
  let bytes = s.as_bytes();
  let n = bytes.len();
  let mut i = 0usize;
  while i < n {
    if i + 1 < n && bytes[i] == b'-' && bytes[i + 1] == b'-' {
      while i < n && bytes[i] != b'\n' {
        out.push(' ');
        i += 1;
      }
    } else if bytes[i].is_ascii() {
      out.push(bytes[i] as char);
      i += 1;
    } else {
      out.push(' ');
      i += 1;
    }
  }
  out
}
