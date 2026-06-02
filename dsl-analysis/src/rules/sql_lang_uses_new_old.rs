//! sql041: `LANGUAGE sql` function body references `NEW` or `OLD`.
//!
//! NEW / OLD are PL/pgSQL trigger row aliases. A pure-SQL function has
//! no notion of them and Postgres rejects the call at runtime. Flag at
//! edit time so the user sees it before the deploy.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::{Statement, StatementKind};
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql041"
  }
  fn default_severity(&self) -> Severity {
    Severity::Warning
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    if !matches!(stmt.kind, StatementKind::Unknown { .. }) {
      return;
    }
    let (_start, body) = crate::stmt_body(stmt, source);
    let upper = body.to_ascii_uppercase();
    if !upper.contains("CREATE") || !upper.contains("FUNCTION") {
      return;
    }
    if !upper.contains("LANGUAGE SQL") {
      return;
    }
    let Some(body_text) = dollar_body(body) else { return };
    let body_upper = body_text.to_ascii_uppercase();
    let stripped = strip_quoted_and_comments(&body_upper);

    for tok in tokens(&stripped) {
      if tok == "NEW" || tok == "OLD" {
        out.push(Diagnostic {
          code: "sql041",
          severity: Severity::Warning,
          message: format!("`{tok}` only works in PL/pgSQL trigger bodies; this function is LANGUAGE sql"),
          range: stmt.range,
        });
        return;
      }
    }
  }
}

fn dollar_body(text: &str) -> Option<&str> {
  let start = text.find("$$")?;
  let after = start + 2;
  let end_rel = text[after..].find("$$")?;
  Some(&text[after..after + end_rel])
}

fn strip_quoted_and_comments(s: &str) -> String {
  let mut out = String::with_capacity(s.len());
  let bytes = s.as_bytes();
  let n = bytes.len();
  let mut i = 0;
  while i < n {
    if i + 1 < n && bytes[i] == b'-' && bytes[i + 1] == b'-' {
      while i < n && bytes[i] != b'\n' {
        i += 1;
      }
    } else if i + 1 < n && bytes[i] == b'/' && bytes[i + 1] == b'*' {
      i += 2;
      while i + 1 < n && !(bytes[i] == b'*' && bytes[i + 1] == b'/') {
        i += 1;
      }
      i = (i + 2).min(n);
    } else if bytes[i] == b'\'' {
      i += 1;
      while i < n && bytes[i] != b'\'' {
        i += 1;
      }
      if i < n {
        i += 1;
      }
    } else {
      out.push(bytes[i] as char);
      i += 1;
    }
  }
  out
}

fn tokens(s: &str) -> Vec<String> {
  let mut out = Vec::new();
  let bytes = s.as_bytes();
  let n = bytes.len();
  let mut i = 0;
  while i < n {
    if bytes[i].is_ascii_alphabetic() || bytes[i] == b'_' {
      let start = i;
      while i < n && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_') {
        i += 1;
      }
      out.push(s[start..i].to_string());
    } else {
      i += 1;
    }
  }
  out
}
