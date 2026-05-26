//! sql081: `ORDER BY random()` -- slow, no index can help, runs a sort
//! over the entire result set. Hint: TABLESAMPLE BERNOULLI for sampling.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::{Statement, StatementKind};
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql081"
  }
  fn default_severity(&self) -> Severity {
    Severity::Hint
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    if !matches!(stmt.kind, StatementKind::Select(_)) {
      return;
    }
    let start: usize = u32::from(stmt.range.start()) as usize;
    let end: usize = (u32::from(stmt.range.end()) as usize).min(source.len());
    let raw = &source[start..end];
    let body_owned = strip_noise(raw);
    let body = body_owned.as_str();
    let upper = body.to_ascii_uppercase();
    let Some(order_idx) = upper.find("ORDER BY") else { return };
    let after = order_idx + 8;
    let Some(rand_rel) = upper[after..].find("RANDOM(") else { return };
    let rand_pos = after + rand_rel;
    // Span through the matching close paren.
    let bytes = body.as_bytes();
    let mut depth = 1i32;
    let mut close = rand_pos + 7;
    while close < bytes.len() && depth > 0 {
      match bytes[close] {
        b'(' => depth += 1,
        b')' => depth -= 1,
        _ => {},
      }
      close += 1;
    }
    let abs_start = start + rand_pos;
    let abs_end = start + close;
    out.push(Diagnostic {
      code: "sql081",
      severity: Severity::Hint,
      message: "ORDER BY random() sorts every row -- consider TABLESAMPLE for sampling, or pick a small subset first"
        .into(),
      range: text_size::TextRange::new((abs_start as u32).into(), (abs_end as u32).into()),
    });
  }
}

fn strip_noise(s: &str) -> String {
  let mut out: Vec<u8> = s.as_bytes().to_vec();
  let n = out.len();
  let mut i = 0usize;
  while i < n {
    if i + 1 < n && out[i] == b'-' && out[i + 1] == b'-' {
      while i < n && out[i] != b'\n' { out[i] = b' '; i += 1 }
      continue;
    }
    if i + 1 < n && out[i] == b'/' && out[i + 1] == b'*' {
      let mut depth = 1u32;
      out[i] = b' '; out[i + 1] = b' '; i += 2;
      while i + 1 < n && depth > 0 {
        if out[i] == b'/' && out[i + 1] == b'*' { depth += 1; out[i] = b' '; out[i + 1] = b' '; i += 2; }
        else if out[i] == b'*' && out[i + 1] == b'/' { depth -= 1; out[i] = b' '; out[i + 1] = b' '; i += 2; }
        else { out[i] = b' '; i += 1; }
      }
      continue;
    }
    if out[i] == b'\'' {
      out[i] = b' '; i += 1;
      while i < n && out[i] != b'\'' { out[i] = b' '; i += 1 }
      if i < n { out[i] = b' '; i += 1 }
      continue;
    }
    i += 1;
  }
  String::from_utf8(out).unwrap_or_else(|_| s.to_string())
}
