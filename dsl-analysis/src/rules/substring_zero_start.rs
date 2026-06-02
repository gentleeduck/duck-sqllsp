//! sql479: `substring(s, 0, n)` / `substring(s FROM 0 FOR n)` /
//! `substr(s, 0, n)` -- PostgreSQL's `substring` is 1-indexed, but
//! a 0 (or negative) FROM argument silently truncates the FOR count
//! by `1 - start`. So `substring('abc', 0, 2)` returns `'a'`, not
//! `'ab'` -- a classic off-by-one. Almost always the author meant
//! `1` as the start.

use crate::clause_scan::is_word;
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;
use text_size::TextRange;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql479"
  }
  fn default_severity(&self) -> Severity {
    Severity::Warning
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, raw) = crate::stmt_body(stmt, source);
    let cleaned = crate::textutil::strip_noise_full(raw);
    let lower = cleaned.to_ascii_lowercase();
    let lb = lower.as_bytes();
    let n = lb.len();
    let mut i = 0usize;
    let mut emitted: std::collections::HashSet<usize> = std::collections::HashSet::new();
    while i < n {
      let (matched, nlen) = if word_eq(lb, i, b"substring") {
        (true, 9usize)
      } else if word_eq(lb, i, b"substr") {
        (true, 6usize)
      } else {
        (false, 0)
      };
      if !matched {
        i += 1;
        continue;
      }
      // Skip whitespace, expect `(`
      let mut j = i + nlen;
      while j < n && lb[j].is_ascii_whitespace() {
        j += 1;
      }
      if j >= n || lb[j] != b'(' {
        i += nlen;
        continue;
      }
      let open_paren = j;
      // Find matching close paren
      let mut depth = 1i32;
      let mut k = j + 1;
      while k < n && depth > 0 {
        match lb[k] {
          b'(' => depth += 1,
          b')' => depth -= 1,
          _ => {},
        }
        if depth == 0 {
          break;
        }
        k += 1;
      }
      if depth != 0 {
        i += nlen;
        continue;
      }
      let close_paren = k;
      let inner = &cleaned[open_paren + 1..close_paren];
      if has_zero_start(inner) && emitted.insert(i) {
        let abs_s = start + i;
        let abs_e = start + close_paren + 1;
        out.push(Diagnostic {
          code: "sql479",
          severity: Severity::Warning,
          message: "`substring`/`substr` with a 0 start: Postgres `substring` is 1-indexed, and a 0 FROM silently truncates the FOR length by 1 (e.g. `substring('abc', 0, 2)` returns `'a'`, not `'ab'`). Use 1 as the start.".into(),
          range: TextRange::new((abs_s as u32).into(), (abs_e as u32).into()),
        });
      }
      i = close_paren + 1;
    }
  }
}

fn word_eq(lb: &[u8], i: usize, w: &[u8]) -> bool {
  let m = w.len();
  if i + m > lb.len() {
    return false;
  }
  if &lb[i..i + m] != w {
    return false;
  }
  let prev_ok = i == 0 || !is_word(lb[i - 1] as char);
  let next_ok = i + m == lb.len() || !is_word(lb[i + m] as char);
  prev_ok && next_ok
}

/// True iff the second positional arg (or the FROM expression) is
/// the literal `0` (with optional `::<type>` cast).
fn has_zero_start(inner: &str) -> bool {
  // Try FROM form first: look for " from " word-bounded at depth 0.
  let lower = inner.to_ascii_lowercase();
  let lb = lower.as_bytes();
  let n = lb.len();
  let mut depth = 0i32;
  let mut i = 0usize;
  while i < n {
    let c = lb[i];
    if c == b'(' {
      depth += 1;
    } else if c == b')' {
      depth -= 1;
    } else if depth == 0 && i + 4 <= n {
      // ' from ' check (lowercased), need word boundary on both sides
      if &lb[i..i + 4] == b"from"
        && (i == 0 || !is_word(lb[i - 1] as char))
        && (i + 4 == n || !is_word(lb[i + 4] as char))
      {
        // Extract expression after FROM until FOR (or end)
        let after = i + 4;
        let mut j = after;
        let mut d = 0i32;
        while j < n {
          let cc = lb[j];
          if cc == b'(' {
            d += 1;
          } else if cc == b')' {
            d -= 1;
          } else if d == 0 && j + 3 <= n && &lb[j..j + 3] == b"for" && (j == 0 || !is_word(lb[j - 1] as char)) && (j + 3 == n || !is_word(lb[j + 3] as char)) {
            break;
          }
          j += 1;
        }
        let expr = inner[after..j].trim();
        return is_literal_zero(expr);
      }
    }
    i += 1;
  }
  // Comma form: split top-level commas, return true iff second arg is `0`.
  let mut depth = 0i32;
  let mut start_arg = 0usize;
  let bytes = inner.as_bytes();
  let mut args: Vec<&str> = Vec::new();
  for (idx, &b) in bytes.iter().enumerate() {
    if b == b'(' {
      depth += 1;
    } else if b == b')' {
      depth -= 1;
    } else if depth == 0 && b == b',' {
      args.push(inner[start_arg..idx].trim());
      start_arg = idx + 1;
    }
  }
  args.push(inner[start_arg..].trim());
  if args.len() >= 2 { is_literal_zero(args[1]) } else { false }
}

fn is_literal_zero(s: &str) -> bool {
  let s = s.trim();
  if s == "0" {
    return true;
  }
  // Allow `0::<type>` (any type word)
  if let Some(rest) = s.strip_prefix("0::") {
    return rest.chars().all(is_word);
  }
  false
}
