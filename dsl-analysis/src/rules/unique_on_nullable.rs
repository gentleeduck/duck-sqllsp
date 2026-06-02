//! sql139: `UNIQUE` on a nullable column with `NULLS DISTINCT` (the
//! PG default) -- multiple NULL rows are allowed. Usually surprising.
//! Suggest `UNIQUE NULLS NOT DISTINCT` (PG 15+) or making the column
//! `NOT NULL`.

use crate::{Diagnostic, LintRule, Severity};
use crate::textutil::is_word;
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql139"
  }
  fn default_severity(&self) -> Severity {
    Severity::Hint
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, body, upper) = crate::stmt_body_upper(stmt, source);
    let trimmed = upper.trim_start();
    if !trimmed.starts_with("CREATE TABLE") {
      return;
    }
    // Build the effective-column model: PK columns are implicitly
    // NOT NULL, SERIAL columns are NOT NULL + defaulted, etc. This
    // catches the case where a column is `id uuid PRIMARY KEY` and
    // a UNIQUE includes it -- without the model, the rule would
    // see `id uuid` as nullable.
    let cols: Vec<(String, bool)> =
      crate::ct_model::effective_columns(body).into_iter().map(|c| (c.name, c.nullable)).collect();
    let bytes = upper.as_bytes();
    let n = bytes.len();
    let mut i = 0;
    while i + 6 <= n {
      if &upper[i..i + 6] == "UNIQUE"
        && (i == 0 || !is_word(bytes[i - 1] as char))
        && (i + 6 == n || !is_word(bytes[i + 6] as char))
      {
        // Opted into NULLS NOT DISTINCT? Skip.
        let after_kw = &upper[i + 6..];
        if after_kw.trim_start().starts_with("NULLS NOT DISTINCT") {
          i += 6;
          continue;
        }
        // Case 1: inline column-level UNIQUE -- `<col> <type> ... UNIQUE`.
        // The column we walked back to *is* the unique column.
        let mut k = i;
        while k > 0 && bytes[k - 1] != b',' && bytes[k - 1] != b'(' {
          k -= 1;
        }
        let col_text = &upper[k..i];
        let is_inline_form = !col_text.trim().is_empty()
          && !col_text.trim_start().starts_with("CONSTRAINT")
          && !col_text.trim_start().starts_with("UNIQUE");
        if is_inline_form {
          if col_text.contains("NOT NULL") {
            i += 6;
            continue;
          }
          // Inline column with no NOT NULL = nullable + UNIQUE. Fire.
          let abs_start = start + i;
          let abs_end = start + i + 6;
          out.push(Diagnostic {
                        code: "sql139",
                        severity: Severity::Hint,
                        message: "UNIQUE on a nullable column -- multiple NULLs are allowed (default NULLS DISTINCT); add NOT NULL or `NULLS NOT DISTINCT` (PG 15+)".into(),
                        range: crate::range_at(abs_start, abs_end),
                    });
          return;
        }
        // Case 2: table-level UNIQUE `(col1, col2, ...)`. Only
        // fire when at least one referenced column is nullable.
        let mut j = i + 6;
        while j < n && bytes[j].is_ascii_whitespace() {
          j += 1;
        }
        if j < n && bytes[j] == b'(' {
          let list_start = j + 1;
          let mut depth = 1i32;
          let mut m = list_start;
          while m < n && depth > 0 {
            match bytes[m] {
              b'(' => depth += 1,
              b')' => depth -= 1,
              _ => {},
            }
            if depth == 0 {
              break;
            }
            m += 1;
          }
          let list_text = &body[list_start..m];
          let any_nullable = list_text.split(',').any(|raw| {
            let col = raw.trim().trim_matches('"').to_ascii_lowercase();
            cols.iter().any(|(name, nullable)| name.eq_ignore_ascii_case(&col) && *nullable)
          });
          if any_nullable {
            let abs_start = start + i;
            let abs_end = start + i + 6;
            out.push(Diagnostic {
                            code: "sql139",
                            severity: Severity::Hint,
                            message: format!("UNIQUE ({}) includes a nullable column -- multiple NULL rows allowed (default NULLS DISTINCT); add NOT NULL or `NULLS NOT DISTINCT` (PG 15+)", list_text.trim()),
                            range: crate::range_at(abs_start, abs_end),
                        });
            return;
          }
          i = m + 1;
          continue;
        }
        i += 6;
        continue;
      }
      i += 1;
    }
  }
}

/// Lightweight parser of `CREATE TABLE (...)` column definitions.
/// Returns `(name, nullable)` per column. A column is nullable unless
/// its definition contains the literal `NOT NULL` (case-insensitive)
/// or `PRIMARY KEY` (implies NOT NULL).
#[allow(dead_code)]
fn parse_columns(body: &str) -> Vec<(String, bool)> {
  let upper = body.to_ascii_uppercase();
  let Some(open) = body.find('(') else { return Vec::new() };
  let bytes = body.as_bytes();
  let n = bytes.len();
  let mut depth = 1i32;
  let mut end = open + 1;
  while end < n && depth > 0 {
    match bytes[end] {
      b'(' => depth += 1,
      b')' => depth -= 1,
      b'\'' => {
        end += 1;
        while end < n && bytes[end] != b'\'' {
          end += 1;
        }
      },
      _ => {},
    }
    if depth == 0 {
      break;
    }
    end += 1;
  }
  let list = &body[open + 1..end];
  let list_up = &upper[open + 1..end];
  let mut out = Vec::new();
  // Split on top-level commas.
  let lb = list.as_bytes();
  let ln = list.len();
  let mut d = 0i32;
  let mut start = 0usize;
  let mut idx = 0usize;
  while idx <= ln {
    let at_end = idx == ln;
    let c = if at_end { b',' } else { lb[idx] };
    match c {
      b'(' => {
        d += 1;
        idx += 1;
        continue;
      },
      b')' => {
        d -= 1;
        idx += 1;
        continue;
      },
      b'\'' if !at_end => {
        idx += 1;
        while idx < ln && lb[idx] != b'\'' {
          idx += 1;
        }
        if idx < ln {
          idx += 1;
        }
        continue;
      },
      _ => {},
    }
    if c == b',' && d == 0 {
      let chunk = &list[start..idx];
      let chunk_up = &list_up[start..idx];
      let trimmed = chunk.trim();
      // Skip table-level constraints.
      if !trimmed.is_empty()
        && !chunk_up.trim_start().starts_with("CONSTRAINT")
        && !chunk_up.trim_start().starts_with("UNIQUE")
        && !chunk_up.trim_start().starts_with("PRIMARY")
        && !chunk_up.trim_start().starts_with("FOREIGN")
        && !chunk_up.trim_start().starts_with("CHECK")
        && !chunk_up.trim_start().starts_with("EXCLUDE")
        && !chunk_up.trim_start().starts_with("LIKE")
      {
        let name: String = trimmed.chars().take_while(|c| c.is_alphanumeric() || *c == '_' || *c == '"').collect();
        let nullable = !chunk_up.contains("NOT NULL") && !chunk_up.contains("PRIMARY KEY");
        out.push((name.trim_matches('"').to_string(), nullable));
      }
      start = idx + 1;
    }
    if at_end {
      break;
    }
    idx += 1;
  }
  out
}

