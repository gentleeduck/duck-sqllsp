//! sql458: `SUM(bool_col)` / `AVG(bool_col)` -- PG raises 42883
//! "function sum(boolean) does not exist". Users almost always want
//! either `count(*) FILTER (WHERE bool_col)` (PG-idiomatic count of
//! trues) or `sum(bool_col::int)` (boolean->int cast).
//!
//! Implementation: our parser collapses projection expressions to a
//! flat `Expr::List` of column refs, so a structural AST walk can't
//! see the surrounding sum/avg call. Use a text scan.

use crate::clause_scan::is_word;
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;
use text_size::TextRange;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql458"
  }
  fn default_severity(&self) -> Severity {
    Severity::Error
  }

  fn check(&self, source: &str, stmt: &Statement, scope: &Scope, catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    if scope.is_empty() {
      return;
    }
    let (start, raw) = crate::stmt_body(stmt, source);
    let cleaned = crate::textutil::strip_noise_full(raw);
    let upper = cleaned.to_ascii_uppercase();
    let ub = upper.as_bytes();
    let bytes = cleaned.as_bytes();
    let n = ub.len();
    for (fname_u, fname_l) in [(&b"SUM"[..], "sum"), (&b"AVG"[..], "avg")] {
      let m = fname_u.len();
      let mut i = 0usize;
      while i + m <= n {
        if !(&ub[i..i + m] == fname_u && (i == 0 || !is_word(ub[i - 1] as char)) && (i + m == n || !is_word(ub[i + m] as char))) {
          i += 1;
          continue;
        }
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
        let inner = cleaned[k + 1..close].trim();
        // Skip if inner has any operator / call / non-identifier
        // structure -- we only flag pure column references.
        if let Some((qualifier, name)) = parse_bare_ident(inner)
          && let Some(ty) = resolve_column_type(scope, catalog, qualifier, name)
        {
          let lt = ty.to_ascii_lowercase();
          if lt == "boolean" || lt == "bool" {
            let abs_s = start + i;
            let abs_e = start + close + 1;
            let _ = TextRange::new((abs_s as u32).into(), (abs_e as u32).into());
            out.push(Diagnostic {
              code: "sql458",
              severity: Severity::Error,
              message: format!(
                "{fname_l}(boolean) -- PG raises 42883 \"function {fname_l}(boolean) does not exist\". Use `count(*) FILTER (WHERE {name})` to count trues, or `sum({name}::int)` to coerce booleans to 0/1"
              ),
              range: TextRange::new((abs_s as u32).into(), (abs_e as u32).into()),
            });
          }
        }
        i = close + 1;
      }
    }
  }
}

fn parse_bare_ident(s: &str) -> Option<(Option<&str>, &str)> {
  let t = s.trim();
  if t.is_empty() {
    return None;
  }
  // Disallow anything that's not an identifier (or qualified).
  for c in t.chars() {
    if !(c.is_alphanumeric() || c == '_' || c == '.') {
      return None;
    }
  }
  if t.starts_with('.') || t.ends_with('.') || t.contains("..") {
    return None;
  }
  // Pure numeric? Not a column.
  if t.chars().all(|c| c.is_ascii_digit() || c == '.') {
    return None;
  }
  if let Some(dot) = t.rfind('.') {
    Some((Some(&t[..dot]), &t[dot + 1..]))
  } else {
    Some((None, t))
  }
}

fn resolve_column_type(scope: &Scope, catalog: &Catalog, qualifier: Option<&str>, name: &str) -> Option<String> {
  let lname = name.to_ascii_lowercase();
  for binding in scope.tables() {
    if let Some(q) = qualifier {
      let key_matches = binding.alias.eq_ignore_ascii_case(q) || binding.table.name.eq_ignore_ascii_case(q);
      if !key_matches {
        continue;
      }
    }
    if let Some(t) = catalog.find_table(binding.table.schema.as_deref(), &binding.table.name) {
      for col in &t.columns {
        if col.name.eq_ignore_ascii_case(&lname) {
          return Some(col.data_type.clone());
        }
      }
    }
  }
  None
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
