//! sql445: `array_position(arr, NULL)` -- always returns NULL because
//! PG's equality is NULL-rejecting (`NULL = anything` is NULL, not
//! TRUE), and array_position uses equality to find the needle. To
//! find NULL inside an array, the user wants
//! `(SELECT i FROM generate_subscripts(arr, 1) WHERE arr[i] IS NULL)`
//! or `arr @> ARRAY[NULL]::<elem-type>[]` style checks.
//!
//! Same goes for `array_positions(arr, NULL)`.

use crate::clause_scan::is_word;
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;
use text_size::TextRange;

pub struct Rule;

const FNS: &[&[u8]] = &[b"ARRAY_POSITION", b"ARRAY_POSITIONS"];

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql445"
  }
  fn default_severity(&self) -> Severity {
    Severity::Warning
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let start: usize = u32::from(stmt.range.start()) as usize;
    let end: usize = (u32::from(stmt.range.end()) as usize).min(source.len());
    let raw = &source[start..end];
    let cleaned = crate::textutil::strip_noise_full(raw);
    let upper = cleaned.to_ascii_uppercase();
    let ub = upper.as_bytes();
    let bytes = cleaned.as_bytes();
    let n = ub.len();
    let mut i = 0usize;
    while i < n {
      let mut matched: Option<usize> = None;
      for fname in FNS {
        let m = fname.len();
        if i + m <= n
          && &ub[i..i + m] == *fname
          && (i == 0 || !is_word(ub[i - 1] as char))
          && (i + m == n || !is_word(ub[i + m] as char))
        {
          matched = Some(m);
          break;
        }
      }
      let Some(m) = matched else {
        i += 1;
        continue;
      };
      let mut k = i + m;
      while k < n && bytes[k].is_ascii_whitespace() {
        k += 1;
      }
      if k >= n || bytes[k] != b'(' {
        i += m;
        continue;
      }
      let Some(close) = match_paren(bytes, k, n) else {
        i += m;
        continue;
      };
      let inner = &cleaned[k + 1..close];
      let args = split_top_commas(inner);
      if args.len() >= 2 {
        let needle = args[1].0.trim().to_ascii_uppercase();
        if needle == "NULL" {
          let fname_str = std::str::from_utf8(&ub[i..i + m]).unwrap_or("array_position").to_ascii_lowercase();
          let abs_s = start + i;
          let abs_e = start + close + 1;
          out.push(Diagnostic {
            code: "sql445",
            severity: Severity::Warning,
            message: format!(
              "`{fname_str}(<arr>, NULL)` always returns NULL -- PG uses equality (NULL-rejecting) to find the needle; to locate NULLs inside an array, use `generate_subscripts(arr, 1) WHERE arr[i] IS NULL`"
            ),
            range: TextRange::new((abs_s as u32).into(), (abs_e as u32).into()),
          });
        }
      }
      i = close + 1;
    }
  }
}

fn split_top_commas(s: &str) -> Vec<(&str, usize)> {
  // Track both `()` and `[]` so ARRAY[1,2,3] doesn't split the
  // argument list on its inner commas.
  let bytes = s.as_bytes();
  let n = bytes.len();
  let mut out: Vec<(&str, usize)> = Vec::new();
  let mut last = 0usize;
  let mut depth: i32 = 0;
  let mut i = 0;
  while i < n {
    let c = bytes[i];
    if c == b'\'' {
      i += 1;
      while i < n && bytes[i] != b'\'' {
        i += 1;
      }
      i = (i + 1).min(n);
      continue;
    }
    if c == b'(' || c == b'[' {
      depth += 1;
    } else if c == b')' || c == b']' {
      depth -= 1;
    } else if c == b',' && depth == 0 {
      out.push((&s[last..i], last));
      last = i + 1;
    }
    i += 1;
  }
  out.push((&s[last..], last));
  out
}

fn match_paren(bytes: &[u8], open: usize, end: usize) -> Option<usize> {
  let mut depth: i32 = 0;
  let mut i = open;
  while i < end {
    let c = bytes[i];
    if c == b'\'' {
      i += 1;
      while i < end && bytes[i] != b'\'' {
        i += 1;
      }
      i = (i + 1).min(end);
      continue;
    }
    if c == b'(' {
      depth += 1;
    } else if c == b')' {
      depth -= 1;
      if depth == 0 {
        return Some(i);
      }
    }
    i += 1;
  }
  None
}
