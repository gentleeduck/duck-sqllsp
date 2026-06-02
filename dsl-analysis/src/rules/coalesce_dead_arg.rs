//! sql417: `COALESCE(a, a, ...)` or `COALESCE(a, NULL, ...)` -- the
//! duplicate / NULL argument is dead. COALESCE short-circuits on the
//! first non-NULL arg; a later identical arg (when the first returned
//! NULL the second will too, assuming determinism) or a NULL literal
//! never contributes. Almost always a typo.

use crate::{Diagnostic, LintRule, Severity};
use crate::textutil::is_word;
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;
use std::collections::HashSet;
use text_size::TextRange;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql417"
  }
  fn default_severity(&self) -> Severity {
    Severity::Hint
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, raw) = crate::stmt_body(stmt, source);
    let cleaned = crate::textutil::strip_noise_full(raw);
    let upper = cleaned.to_ascii_uppercase();
    let ub = upper.as_bytes();
    let n = ub.len();
    let needle = b"COALESCE";
    let mut i = 0usize;
    while i + needle.len() < n {
      if &ub[i..i + needle.len()] != needle
        || (i > 0 && is_word(ub[i - 1] as char))
      {
        i += 1;
        continue;
      }
      // Skip whitespace, expect `(`.
      let mut j = i + needle.len();
      while j < n && ub[j].is_ascii_whitespace() {
        j += 1;
      }
      if j >= n || ub[j] != b'(' {
        i = j.max(i + needle.len());
        continue;
      }
      // Walk paren body collecting depth-0 comma-separated items.
      let mut depth: i32 = 1;
      let mut item_start = j + 1;
      let mut items: Vec<(usize, usize)> = Vec::new();
      let mut k = j + 1;
      let bytes = cleaned.as_bytes();
      while k < n && depth > 0 {
        match bytes[k] {
          b'\'' => {
            k += 1;
            while k < n && bytes[k] != b'\'' {
              k += 1;
            }
            k = (k + 1).min(n);
            continue;
          },
          b'(' => depth += 1,
          b')' => {
            depth -= 1;
            if depth == 0 {
              items.push((item_start, k));
              k += 1;
              break;
            }
          },
          b',' if depth == 1 => {
            items.push((item_start, k));
            item_start = k + 1;
          },
          _ => {},
        }
        k += 1;
      }
      // Look for dups + NULL literal.
      let mut seen: HashSet<String> = HashSet::new();
      let mut emitted = false;
      for (s, e) in &items {
        let raw_item = source[start + s..start + e].trim();
        let key = raw_item.to_ascii_lowercase().split_whitespace().collect::<Vec<_>>().join(" ");
        if key.is_empty() {
          continue;
        }
        // Bare NULL: always dead in COALESCE.
        if key == "null" && !emitted {
          out.push(Diagnostic {
            code: "sql417",
            severity: Severity::Hint,
            message: "`COALESCE(... NULL ...)` -- NULL never contributes; drop it".into(),
            range: TextRange::new(((start + i) as u32).into(), ((start + k) as u32).into()),
          });
          emitted = true;
          continue;
        }
        // Skip args containing a function call (`(`) -- could be
        // non-deterministic (random(), now(), etc).
        if key.contains('(') {
          seen.insert(key);
          continue;
        }
        if !seen.insert(key.clone()) && !emitted {
          out.push(Diagnostic {
            code: "sql417",
            severity: Severity::Hint,
            message: format!("`COALESCE(... {raw_item} ...)` repeats an earlier argument -- the duplicate is dead"),
            range: TextRange::new(((start + i) as u32).into(), ((start + k) as u32).into()),
          });
          emitted = true;
        }
      }
      i = k.max(i + needle.len());
    }
  }
}

