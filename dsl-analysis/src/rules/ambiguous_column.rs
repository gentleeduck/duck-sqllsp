//! sql003: unqualified column reference exists in more than one in-scope
//! table; the user must qualify it.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::{Expr, Projection, SelectStmt, Statement, StatementKind};
use dsl_resolve::Scope;
use text_size::TextRange;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql003"
  }
  fn default_severity(&self) -> Severity {
    Severity::Error
  }

  fn check(&self, source: &str, stmt: &Statement, scope: &Scope, catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    if scope.is_empty() || catalog.tables().next().is_none() {
      return;
    }
    let StatementKind::Select(s) = &stmt.kind else {
      return;
    };
    // dsl-parse does not yet model NATURAL / USING joins. Inspect the
    // statement source text so we don't false-positive on bare column
    // references that those joins legally merge.
    //   - NATURAL JOIN merges every same-named column -- treat the
    //     whole statement as conservatively unambiguous.
    //   - USING (col1, col2, ...) merges only the listed names.
    let stmt_src = stmt_source(source, stmt);
    if contains_natural_join(stmt_src) {
      return;
    }
    let using_merged: std::collections::HashSet<String> = using_columns(stmt_src);
    let mut refs: Vec<(Option<String>, String, TextRange)> = Vec::new();
    collect(s, &mut refs);
    // Restrict ambiguity check to tables actually in this SELECT's
    // FROM + JOIN -- CTE definitions that are declared in WITH but
    // not referenced by the outer SELECT shouldn't shadow real
    // columns.
    let mut from_names: std::collections::HashSet<String> = std::collections::HashSet::new();
    for t in &s.from {
      from_names.insert(t.name.to_ascii_lowercase());
      if let Some(a) = &t.alias {
        from_names.insert(a.to_ascii_lowercase());
      }
    }
    for j in &s.joins {
      from_names.insert(j.table.name.to_ascii_lowercase());
      if let Some(a) = &j.table.alias {
        from_names.insert(a.to_ascii_lowercase());
      }
    }

    for (qualifier, name, col_range) in refs {
      if qualifier.is_some() {
        continue;
      } // Already qualified.
      if using_merged.iter().any(|m| m.eq_ignore_ascii_case(&name)) {
        continue;
      }
      // Dedup by underlying table: dsl-resolve binds each table
      // twice (once by alias, once by bare name) so we'd otherwise
      // report `[u, u, o, o]` instead of `[u, o]`.
      let mut hits: Vec<String> = Vec::new();
      let mut seen_tables: Vec<(Option<String>, String)> = Vec::new();
      for b in scope.tables() {
        // Only consider bindings that the outer SELECT actually
        // references in FROM/JOIN -- otherwise CTEs declared in
        // WITH but unused at this level get counted as "in scope".
        if !from_names.is_empty()
          && !from_names.contains(&b.alias.to_ascii_lowercase())
          && !from_names.contains(&b.table.name.to_ascii_lowercase())
        {
          continue;
        }
        let key = (b.table.schema.clone(), b.table.name.clone());
        if seen_tables.contains(&key) {
          continue;
        }
        seen_tables.push(key);
        if let Some(t) = catalog.find_table(b.table.schema.as_deref(), &b.table.name)
          && t.columns.iter().any(|c| c.name.eq_ignore_ascii_case(&name))
        {
          hits.push(b.alias.clone());
        }
        // Same name might also be a CTE column. Check there too
        // so `SELECT id FROM users JOIN t ON ...` flags when `id`
        // exists in both `users.columns` and CTE `t`'s projection.
        if let Some(cte_cols) = scope.cte_columns_of(&b.alias)
          && !cte_cols.is_empty()
          && cte_cols.iter().any(|c| c == &name)
          && !hits.contains(&b.alias)
        {
          hits.push(b.alias.clone());
        }
      }
      if hits.len() > 1 {
        let range = if col_range.len() > text_size::TextSize::from(0) { col_range } else { stmt.range };
        out.push(Diagnostic {
          code: "sql003",
          severity: Severity::Error,
          message: format!("ambiguous column `{name}`: appears in [{}]; qualify with the table alias", hits.join(", ")),
          range,
        });
      }
    }
  }
}

/// Slice of `source` covered by `stmt.range`, clamped to source length.
fn stmt_source<'a>(source: &'a str, stmt: &Statement) -> &'a str {
  let (start, end) = crate::stmt_bounds(stmt, source);
  if start >= end || start >= source.len() {
    return "";
  }
  &source[start..end]
}

/// True when the statement source contains `NATURAL JOIN` as a keyword
/// pair (word-bounded, case-insensitive, ASCII-aware).
fn contains_natural_join(src: &str) -> bool {
  let bytes = src.as_bytes();
  let n = bytes.len();
  let mut i = 0usize;
  while i + 12 <= n {
    if (bytes[i] == b'N' || bytes[i] == b'n')
      && src[i..].len() >= 12
      && src[i..i + 7].eq_ignore_ascii_case("NATURAL")
      && bytes[i + 7].is_ascii_whitespace()
    {
      let left_ok = i == 0 || !bytes[i - 1].is_ascii_alphanumeric() && bytes[i - 1] != b'_';
      // Skip whitespace after NATURAL.
      let mut j = i + 7;
      while j < n && bytes[j].is_ascii_whitespace() {
        j += 1;
      }
      // Optional LEFT/RIGHT/FULL/INNER between NATURAL and JOIN.
      for mod_kw in ["LEFT", "RIGHT", "FULL", "INNER"] {
        if j + mod_kw.len() < n && src[j..j + mod_kw.len()].eq_ignore_ascii_case(mod_kw) {
          j += mod_kw.len();
          while j < n && bytes[j].is_ascii_whitespace() {
            j += 1;
          }
          break;
        }
      }
      if j + 4 <= n && src[j..j + 4].eq_ignore_ascii_case("JOIN") {
        let after = j + 4;
        let right_ok = after >= n || !bytes[after].is_ascii_alphanumeric() && bytes[after] != b'_';
        if left_ok && right_ok {
          return true;
        }
      }
    }
    i += 1;
  }
  false
}

/// Collect every column name listed in a `USING (col1, col2, ...)` clause
/// of the statement source. Case-preserved as written.
fn using_columns(src: &str) -> std::collections::HashSet<String> {
  let mut out: std::collections::HashSet<String> = std::collections::HashSet::new();
  let bytes = src.as_bytes();
  let n = bytes.len();
  let mut i = 0usize;
  while i + 5 <= n {
    if (bytes[i] == b'U' || bytes[i] == b'u')
      && src[i..i + 5].eq_ignore_ascii_case("USING")
    {
      let left_ok = i == 0 || !bytes[i - 1].is_ascii_alphanumeric() && bytes[i - 1] != b'_';
      let after = i + 5;
      let right_ok = after >= n || !bytes[after].is_ascii_alphanumeric() && bytes[after] != b'_';
      if left_ok && right_ok {
        // Skip whitespace, then expect `(`.
        let mut j = after;
        while j < n && bytes[j].is_ascii_whitespace() {
          j += 1;
        }
        if j < n && bytes[j] == b'(' {
          let list_start = j + 1;
          let mut depth = 1i32;
          let mut k = list_start;
          while k < n && depth > 0 {
            match bytes[k] {
              b'(' => depth += 1,
              b')' => depth -= 1,
              _ => {},
            }
            if depth == 0 {
              break;
            }
            k += 1;
          }
          if k < n {
            for part in src[list_start..k].split(',') {
              let name = part.trim().trim_matches('"');
              if !name.is_empty() {
                out.insert(name.to_string());
              }
            }
            i = k + 1;
            continue;
          }
        }
      }
    }
    i += 1;
  }
  out
}

fn collect(s: &SelectStmt, out: &mut Vec<(Option<String>, String, TextRange)>) {
  for p in &s.projections {
    if let Projection::Expr { expr, .. } = p {
      walk(expr, out);
    }
  }
  if let Some(w) = &s.where_clause {
    walk(w, out);
  }
  for j in &s.joins {
    if let Some(on) = &j.on {
      walk(on, out);
    }
  }
}

fn walk(e: &Expr, out: &mut Vec<(Option<String>, String, TextRange)>) {
  match e {
    Expr::Column { qualifier, name, range } => out.push((qualifier.clone(), name.clone(), *range)),
    Expr::BinaryOp { left, right, .. } => {
      walk(left, out);
      walk(right, out);
    },
    Expr::Call { args, .. } => {
      for a in args {
        walk(a, out);
      }
    },
    Expr::List(items) => {
      for it in items {
        walk(it, out);
      }
    },
    _ => {},
  }
}
