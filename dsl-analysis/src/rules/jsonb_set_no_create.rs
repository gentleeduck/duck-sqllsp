//! sql115: `jsonb_set(col, path, val)` -- 4th arg defaults to `true`
//! (create-if-missing). But explicit `false` silently drops updates
//! when the key isn't already present. Flag explicit `false`.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql115"
  }
  fn default_severity(&self) -> Severity {
    Severity::Hint
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let start: usize = u32::from(stmt.range.start()) as usize;
    let end: usize = (u32::from(stmt.range.end()) as usize).min(source.len());
    let body = &source[start..end];
    let upper = body.to_ascii_uppercase();
    let bytes = upper.as_bytes();
    let n = bytes.len();
    let mut i = 0;
    while i + 10 <= n {
      if &upper[i..i + 10] == "JSONB_SET(" && (i == 0 || !is_word(bytes[i - 1] as char)) {
        // Find matching close paren and walk through args.
        let mut depth = 1i32;
        let mut k = i + 10;
        let mut commas: Vec<usize> = Vec::new();
        while k < n && depth > 0 {
          match bytes[k] {
            b'(' => depth += 1,
            b')' => depth -= 1,
            b',' if depth == 1 => commas.push(k),
            b'\'' => {
              k += 1;
              while k < n && bytes[k] != b'\'' {
                k += 1;
              }
            },
            _ => {},
          }
          if depth == 0 {
            break;
          }
          k += 1;
        }
        if depth == 0 && commas.len() == 3 {
          // 4th arg is between commas[2]+1 and k.
          let arg4 = upper[commas[2] + 1..k].trim();
          if arg4 == "FALSE" {
            let abs_start = start + i;
            let abs_end = start + k + 1;
            out.push(Diagnostic {
                            code: "sql115",
                            severity: Severity::Hint,
                            message: "jsonb_set(..., false) silently drops updates when the key is absent -- usually you want the default (true)".into(),
                            range: text_size::TextRange::new(
                                (abs_start as u32).into(),
                                (abs_end as u32).into(),
                            ),
                        });
            return;
          }
        }
        i = k + 1;
        continue;
      }
      i += 1;
    }
  }
}

fn is_word(c: char) -> bool {
  c.is_alphanumeric() || c == '_'
}
