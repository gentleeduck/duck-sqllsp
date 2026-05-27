//! sql493: `COALESCE(<not-null-col>, ...)` -- when the first
//! argument is a NOT NULL column, COALESCE always returns it; the
//! remaining defaults are dead code. Drop the wrapper or move the
//! NOT NULL guarantee somewhere visible.

use crate::clause_scan::is_word;
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::{Statement, StatementKind};
use dsl_resolve::Scope;
use text_size::TextRange;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql493"
  }
  fn default_severity(&self) -> Severity {
    Severity::Hint
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    // Resolve the primary target table -- COALESCE inside a query
    // that has many tables would need full column resolution; we
    // keep it conservative and only check single-table queries.
    let target = match &stmt.kind {
      StatementKind::Select(s) => {
        if s.from.len() != 1 {
          return;
        }
        s.from.first()
      },
      StatementKind::Update(u) => Some(&u.table),
      StatementKind::Delete(d) => Some(&d.table),
      _ => return,
    };
    let Some(target) = target else { return };
    let Some(t) = catalog.find_table(target.schema.as_deref(), &target.name) else { return };
    let alias_lc = target.alias.as_deref().map(|s| s.to_ascii_lowercase());
    let table_name_lc = target.name.to_ascii_lowercase();

    let start: usize = u32::from(stmt.range.start()) as usize;
    let end: usize = (u32::from(stmt.range.end()) as usize).min(source.len());
    let raw = &source[start..end];
    let cleaned = crate::textutil::strip_noise_full(raw);
    let lower = cleaned.to_ascii_lowercase();
    let lb = lower.as_bytes();
    let bytes = cleaned.as_bytes();
    let n = lb.len();
    let mut emitted: std::collections::HashSet<usize> = std::collections::HashSet::new();
    let mut i = 0usize;
    while i + 8 <= n {
      if &lb[i..i + 8] != b"coalesce" || is_word(lb[i + 8] as char) || i != 0 && is_word(lb[i - 1] as char) {
        i += 1;
        continue;
      }
      let mut k = i + 8;
      while k < n && bytes[k].is_ascii_whitespace() {
        k += 1;
      }
      if k >= n || bytes[k] != b'(' {
        i += 8;
        continue;
      }
      let Some(close) = match_paren(bytes, k, n) else {
        i += 8;
        continue;
      };
      let inner_start = k + 1;
      let inner_end = close;
      let commas = find_top_commas(bytes, inner_start, inner_end);
      // Need at least 2 args (first + at least one default) for the
      // hint to be meaningful.
      if commas.is_empty() {
        i = close + 1;
        continue;
      }
      let arg1_end = commas[0];
      let arg1 = cleaned[inner_start..arg1_end].trim();
      // Only flag if arg1 is a bare identifier (or qualifier.ident).
      let Some((qualifier, name)) = parse_simple_ident(arg1) else {
        i = close + 1;
        continue;
      };
      // If qualified, the qualifier must match this table's alias or
      // name. Bare identifiers go straight to the column lookup.
      if let Some(q) = &qualifier {
        let q_lc = q.to_ascii_lowercase();
        let matches = alias_lc.as_deref() == Some(q_lc.as_str()) || q_lc == table_name_lc;
        if !matches {
          i = close + 1;
          continue;
        }
      }
      let Some(col) = t.columns.iter().find(|c| c.name.eq_ignore_ascii_case(&name)) else {
        i = close + 1;
        continue;
      };
      if col.nullable {
        i = close + 1;
        continue;
      }
      if emitted.insert(i) {
        let abs_s = start + i;
        let abs_e = start + close + 1;
        out.push(Diagnostic {
          code: "sql493",
          severity: Severity::Hint,
          message: format!(
            "`COALESCE({}, ...)` -- `{}` is NOT NULL, so COALESCE always returns it and the remaining default(s) are dead code. Drop the wrapper.",
            arg1, col.name
          ),
          range: TextRange::new((abs_s as u32).into(), (abs_e as u32).into()),
        });
      }
      i = close + 1;
    }
  }
}

/// Returns (qualifier, name) if `s` is a bare ident or `<ident>.<ident>`.
fn parse_simple_ident(s: &str) -> Option<(Option<String>, String)> {
  let t = s.trim();
  if t.is_empty() {
    return None;
  }
  // Split on the LAST dot at the top level.
  if let Some((q, n)) = t.rsplit_once('.') {
    if is_simple_word(q) && is_simple_word(n) {
      return Some((Some(q.to_string()), n.to_string()));
    }
    return None;
  }
  if is_simple_word(t) { Some((None, t.to_string())) } else { None }
}

fn is_simple_word(s: &str) -> bool {
  let s = s.trim_matches('"');
  !s.is_empty() && s.chars().all(|c| c.is_alphanumeric() || c == '_')
}

fn find_top_commas(bytes: &[u8], from: usize, to: usize) -> Vec<usize> {
  let mut depth: i32 = 0;
  let mut out = Vec::new();
  let mut i = from;
  while i < to {
    let c = bytes[i];
    if c == b'\'' {
      i += 1;
      while i < to && bytes[i] != b'\'' {
        i += 1;
      }
      i = (i + 1).min(to);
      continue;
    }
    if c == b'(' || c == b'[' {
      depth += 1;
    } else if c == b')' || c == b']' {
      depth -= 1;
    } else if c == b',' && depth == 0 {
      out.push(i);
    }
    i += 1;
  }
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
