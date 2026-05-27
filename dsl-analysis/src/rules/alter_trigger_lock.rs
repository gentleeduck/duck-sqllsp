//! sql347: `ALTER TABLE t ENABLE|DISABLE TRIGGER ...`. Takes an
//! ACCESS EXCLUSIVE lock on the target table, which blocks every read
//! AND every write until the catalog mutation commits. Hint about
//! running during low traffic or wrapping in `lock_timeout`.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql347"
  }
  fn default_severity(&self) -> Severity {
    Severity::Hint
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let start: usize = u32::from(stmt.range.start()) as usize;
    let end: usize = (u32::from(stmt.range.end()) as usize).min(source.len());
    let raw = &source[start..end];
    let body_owned = strip_noise(raw);
    let body = body_owned.as_str();
    let upper = body.to_ascii_uppercase();
    if !upper.trim_start().starts_with("ALTER TABLE") {
      return;
    }
    let needle = if let Some(at) = upper.find("DISABLE TRIGGER") {
      (at, "DISABLE TRIGGER")
    } else if let Some(at) = upper.find("ENABLE TRIGGER") {
      (at, "ENABLE TRIGGER")
    } else if let Some(at) = upper.find("ENABLE ALWAYS TRIGGER") {
      (at, "ENABLE ALWAYS TRIGGER")
    } else if let Some(at) = upper.find("ENABLE REPLICA TRIGGER") {
      (at, "ENABLE REPLICA TRIGGER")
    } else {
      return;
    };
    let abs_s = start + needle.0;
    let abs_e = abs_s + needle.1.len();
    out.push(Diagnostic {
      code: "sql347",
      severity: Severity::Hint,
      message: format!(
        "{} takes ACCESS EXCLUSIVE on the table -- blocks readers + writers; run during low traffic or set lock_timeout",
        needle.1
      ),
      range: text_size::TextRange::new((abs_s as u32).into(), (abs_e as u32).into()),
    });
  }
}

fn strip_noise(s: &str) -> String {
  let mut out: Vec<u8> = s.as_bytes().to_vec();
  let n = out.len();
  let mut i = 0usize;
  while i < n {
    if i + 1 < n && out[i] == b'-' && out[i + 1] == b'-' {
      while i < n && out[i] != b'\n' {
        out[i] = b' ';
        i += 1
      }
      continue;
    }
    if i + 1 < n && out[i] == b'/' && out[i + 1] == b'*' {
      let mut depth = 1u32;
      out[i] = b' ';
      out[i + 1] = b' ';
      i += 2;
      while i + 1 < n && depth > 0 {
        if out[i] == b'/' && out[i + 1] == b'*' {
          depth += 1;
          out[i] = b' ';
          out[i + 1] = b' ';
          i += 2;
        } else if out[i] == b'*' && out[i + 1] == b'/' {
          depth -= 1;
          out[i] = b' ';
          out[i + 1] = b' ';
          i += 2;
        } else {
          out[i] = b' ';
          i += 1;
        }
      }
      continue;
    }
    if out[i] == b'\'' {
      out[i] = b' ';
      i += 1;
      while i < n && out[i] != b'\'' {
        out[i] = b' ';
        i += 1
      }
      if i < n {
        out[i] = b' ';
        i += 1
      }
      continue;
    }
    i += 1;
  }
  String::from_utf8(out).unwrap_or_else(|_| s.to_string())
}
