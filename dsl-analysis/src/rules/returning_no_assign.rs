//! sql143: `INSERT/UPDATE/DELETE ... RETURNING ...` inside a PL/pgSQL
//! block without `INTO <vars>` or `STRICT` -- the returned row is
//! silently discarded. Almost always a bug.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql143"
  }
  fn default_severity(&self) -> Severity {
    Severity::Hint
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let start: usize = u32::from(stmt.range.start()) as usize;
    let end: usize = (u32::from(stmt.range.end()) as usize).min(source.len());
    let raw = &source[start..end];
    // Strip comments/strings so a `-- RETURNING ...` header comment
    // or `'RETURNING'` literal doesn't trigger this rule.
    let body_owned = strip_noise(raw);
    let body = body_owned.as_str();
    let upper = body.to_ascii_uppercase();
    // Only fire when the statement is wrapped in a PL/pgSQL function
    // body or a DO block -- top-level RETURNING is fine.
    if !upper.contains("LANGUAGE PLPGSQL") && !upper.contains("DO $$") {
      return;
    }
    let bytes = upper.as_bytes();
    let n = bytes.len();
    let mut i = 0;
    while i + 9 <= n {
      if &upper[i..i + 9] == "RETURNING"
        && (i == 0 || !is_word(bytes[i - 1] as char))
        && (i + 9 == n || !is_word(bytes[i + 9] as char))
      {
        // Look forward to `;` or `INTO`. If `INTO` appears
        // before `;`, this RETURNING captures into a var -- OK.
        let mut j = i + 9;
        let mut into_first = false;
        while j < n && bytes[j] != b';' {
          if j + 6 <= n && &upper[j..j + 6] == " INTO " {
            into_first = true;
            break;
          }
          if bytes[j] == b'\'' {
            j += 1;
            while j < n && bytes[j] != b'\'' {
              j += 1;
            }
            if j < n {
              j += 1;
            }
            continue;
          }
          j += 1;
        }
        if !into_first {
          let abs_start = start + i;
          let abs_end = start + i + 9;
          out.push(Diagnostic {
            code: "sql143",
            severity: Severity::Hint,
            message:
              "RETURNING inside PL/pgSQL discarded -- add `INTO <var>` or use PERFORM if the result is intentional"
                .into(),
            range: text_size::TextRange::new((abs_start as u32).into(), (abs_end as u32).into()),
          });
          return;
        }
        i = j + 1;
        continue;
      }
      i += 1;
    }
  }
}

fn is_word(c: char) -> bool {
  c.is_alphanumeric() || c == '_'
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
