//! sql463: `IF TG_OP = 'inserted' THEN ...` -- PG's TG_OP returns
//! one of exactly four uppercase strings: `INSERT`, `UPDATE`,
//! `DELETE`, `TRUNCATE`. Any other literal makes the comparison
//! always FALSE, so the branch is silently dead. Most common typo
//! is lowercase (`'insert'`) or past-tense (`'inserted'`).
//!
//! Also handles `TG_OP IN ('INSERT', 'updated', ...)`.

use crate::clause_scan::is_word;
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;
use text_size::TextRange;

pub struct Rule;

/// (TG-variable name, valid literal set). Each TG_* variable has a
/// fixed set of uppercase string values; comparing to anything else
/// (including the lowercase form) is always FALSE.
const TG_VARS: &[(&[u8], &[&str])] = &[
  (b"TG_OP", &["INSERT", "UPDATE", "DELETE", "TRUNCATE"]),
  (b"TG_LEVEL", &["ROW", "STATEMENT"]),
  (b"TG_WHEN", &["BEFORE", "AFTER", "INSTEAD OF"]),
];

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql463"
  }
  fn default_severity(&self) -> Severity {
    Severity::Warning
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, raw) = crate::stmt_body(stmt, source);
    // Use raw bytes -- we need the literal text inside quotes.
    // For TG_OP detection use uppercase since `tg_op` is case-insensitive
    // in PG / pl/pgsql.
    let upper = raw.to_ascii_uppercase();
    let ub = upper.as_bytes();
    let raw_bytes = raw.as_bytes();
    let n = ub.len();
    let mut i = 0usize;
    while i < n {
      // Match any of the TG_* variables (longest first not needed --
      // each is a distinct prefix).
      let mut matched: Option<(usize, &[&str], String)> = None;
      for (kw, valid) in TG_VARS {
        let m = kw.len();
        if i + m <= n
          && &ub[i..i + m] == *kw
          && (i == 0 || !is_word(ub[i - 1] as char))
          && (i + m == n || !is_word(ub[i + m] as char))
        {
          matched = Some((m, *valid, std::str::from_utf8(kw).unwrap().to_string()));
          break;
        }
      }
      let Some((m, valid, var_name)) = matched else {
        i += 1;
        continue;
      };
      // Skip whitespace.
      let mut k = i + m;
      while k < n && raw_bytes[k].is_ascii_whitespace() {
        k += 1;
      }
      // Check for comparison operator.
      let op_len = peek_eq_or_ne(raw_bytes, k);
      if op_len > 0 {
        k += op_len;
        while k < n && raw_bytes[k].is_ascii_whitespace() {
          k += 1;
        }
        if let Some((lit, lit_end)) = read_string_literal(raw_bytes, k) {
          check_literal(&var_name, valid, &lit, start + k, start + lit_end, out);
        }
        i = k.max(i + m);
        continue;
      }
      // Check for `IN (`.
      if k + 2 <= n && &ub[k..k + 2] == b"IN" && (k + 2 == n || !is_word(ub[k + 2] as char)) {
        let mut p = k + 2;
        while p < n && raw_bytes[p].is_ascii_whitespace() {
          p += 1;
        }
        if p < n && raw_bytes[p] == b'(' {
          // Walk the list.
          let mut q = p + 1;
          while q < n && raw_bytes[q] != b')' {
            while q < n && (raw_bytes[q].is_ascii_whitespace() || raw_bytes[q] == b',') {
              q += 1;
            }
            if q >= n || raw_bytes[q] != b'\'' {
              break;
            }
            if let Some((lit, lit_end)) = read_string_literal(raw_bytes, q) {
              check_literal(&var_name, valid, &lit, start + q, start + lit_end, out);
              q = lit_end;
            } else {
              break;
            }
          }
        }
      }
      i = k.max(i + m);
    }
    // Second pass: commuted form `'lit' <op> TG_*`.
    let mut j = 0usize;
    while j < n {
      if raw_bytes[j] != b'\'' {
        j += 1;
        continue;
      }
      let Some((lit, lit_end)) = read_string_literal(raw_bytes, j) else {
        j += 1;
        continue;
      };
      // Skip ws, then an op, then ws, then look for TG_*.
      let mut k = lit_end;
      while k < n && raw_bytes[k].is_ascii_whitespace() {
        k += 1;
      }
      let op_len = peek_eq_or_ne(raw_bytes, k);
      if op_len == 0 {
        j = lit_end;
        continue;
      }
      k += op_len;
      while k < n && raw_bytes[k].is_ascii_whitespace() {
        k += 1;
      }
      // Try to match a TG_* var here.
      let mut matched_var: Option<(usize, &[&str], String)> = None;
      for (kw, valid) in TG_VARS {
        let m = kw.len();
        if k + m <= n
          && &ub[k..k + m] == *kw
          && (k == 0 || !is_word(ub[k - 1] as char))
          && (k + m == n || !is_word(ub[k + m] as char))
        {
          matched_var = Some((m, *valid, std::str::from_utf8(kw).unwrap().to_string()));
          break;
        }
      }
      if let Some((_m, valid, var_name)) = matched_var {
        check_literal(&var_name, valid, &lit, start + j, start + lit_end, out);
      }
      j = lit_end;
    }
  }
}

fn check_literal(var_name: &str, valid: &[&str], lit: &str, abs_s: usize, abs_e: usize, out: &mut Vec<Diagnostic>) {
  if valid.contains(&lit) {
    return;
  }
  // Suggest a close match (case-insensitive).
  let upper_lit = lit.to_ascii_uppercase();
  let suggestion = valid.iter().find(|v| **v == upper_lit).copied();
  let valid_list = valid.to_vec().join("/");
  let msg = match suggestion {
    Some(s) => format!(
      "{var_name} compared to `'{lit}'` -- PG returns uppercase only; the comparison is always FALSE. Did you mean `'{s}'`?"
    ),
    None => format!(
      "{var_name} compared to `'{lit}'` -- PG returns exactly {valid_list} (uppercase). The comparison is always FALSE"
    ),
  };
  out.push(Diagnostic {
    code: "sql463",
    severity: Severity::Warning,
    message: msg,
    range: TextRange::new((abs_s as u32).into(), (abs_e as u32).into()),
  });
}

fn peek_eq_or_ne(bytes: &[u8], i: usize) -> usize {
  if i >= bytes.len() {
    return 0;
  }
  let n = bytes.len();
  if i + 2 <= n {
    let two = &bytes[i..i + 2];
    if two == b"!=" || two == b"<>" {
      return 2;
    }
  }
  if bytes[i] == b'=' {
    return 1;
  }
  0
}

fn read_string_literal(bytes: &[u8], i: usize) -> Option<(String, usize)> {
  if i >= bytes.len() || bytes[i] != b'\'' {
    return None;
  }
  let mut k = i + 1;
  while k < bytes.len() {
    if bytes[k] == b'\'' {
      if k + 1 < bytes.len() && bytes[k + 1] == b'\'' {
        k += 2;
        continue;
      }
      let lit = std::str::from_utf8(&bytes[i + 1..k]).ok()?.to_string();
      return Some((lit, k + 1));
    }
    k += 1;
  }
  None
}
