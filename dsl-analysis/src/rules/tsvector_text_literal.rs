//! sql499: `WHERE tsvector_col @@ 'plain text'` -- PG coerces the
//! string literal to `tsquery`, which has its own syntax (operators
//! `& | ! :`). A literal like `'foo bar'` (with a space) raises a
//! runtime syntax error; a single-word literal like `'foo'` works
//! but is rarely the author's intent. Wrap user-input-style text
//! with `plainto_tsquery(...)` (whitespace -> AND) or
//! `websearch_to_tsquery(...)` (google-style); use `to_tsquery(...)`
//! only when the literal really is tsquery syntax.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::{Statement, StatementKind};
use dsl_resolve::Scope;
use text_size::TextRange;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql499"
  }
  fn default_severity(&self) -> Severity {
    Severity::Warning
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    // Resolve the primary table (only handle single-FROM-table
    // SELECTs to keep column resolution simple).
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

    let start: usize = u32::from(stmt.range.start()) as usize;
    let end: usize = (u32::from(stmt.range.end()) as usize).min(source.len());
    let raw = &source[start..end];
    let bytes = raw.as_bytes();
    let n = bytes.len();
    let mut i = 0usize;
    let mut emitted: std::collections::HashSet<usize> = std::collections::HashSet::new();
    while i + 2 <= n {
      if !(bytes[i] == b'@' && bytes[i + 1] == b'@') {
        i += 1;
        continue;
      }
      let op_at = i;
      // LHS: walk back over whitespace, then read an identifier
      // (possibly qualified) as the column.
      let mut l = op_at;
      while l > 0 && bytes[l - 1].is_ascii_whitespace() {
        l -= 1;
      }
      let id_end = l;
      while l > 0 {
        let b = bytes[l - 1];
        if b.is_ascii_alphanumeric() || b == b'_' || b == b'.' {
          l -= 1;
        } else {
          break;
        }
      }
      if l == id_end {
        i += 2;
        continue;
      }
      let lhs = &raw[l..id_end];
      let lhs_bare = lhs.rsplit('.').next().unwrap_or(lhs);
      // Look up the column in the target table; must be tsvector.
      let Some(col) = t.columns.iter().find(|c| c.name.eq_ignore_ascii_case(lhs_bare)) else {
        i += 2;
        continue;
      };
      if !col.data_type.eq_ignore_ascii_case("tsvector") {
        i += 2;
        continue;
      }
      // RHS: skip ws, expect `'<literal>'` exactly (not a function call).
      let mut r = op_at + 2;
      while r < n && bytes[r].is_ascii_whitespace() {
        r += 1;
      }
      if r >= n || bytes[r] != b'\'' {
        i += 2;
        continue;
      }
      // Read string literal (with `''` escape).
      let lit_start = r;
      let mut k = r + 1;
      let mut lit_text = String::new();
      while k < n {
        if bytes[k] == b'\'' {
          if k + 1 < n && bytes[k + 1] == b'\'' {
            lit_text.push('\'');
            k += 2;
            continue;
          }
          break;
        }
        lit_text.push(bytes[k] as char);
        k += 1;
      }
      if k >= n {
        i += 2;
        continue;
      }
      let lit_end = k + 1;
      // Warn unconditionally for tsvector @@ string-literal. If the
      // literal contains a space without a tsquery operator, the
      // diagnostic adds a "runtime error" note.
      if emitted.insert(op_at) {
        let has_unprotected_space = lit_text.contains(' ') && !lit_text.chars().any(|c| matches!(c, '&' | '|' | '!' | ':'));
        let abs_s = start + l;
        let abs_e = start + lit_end;
        let msg = if has_unprotected_space {
          format!(
            "`{} @@ '{}'` -- PG coerces the literal to `tsquery` and parsing fails on the embedded space (runtime syntax error). Wrap with `plainto_tsquery('{}')` or `websearch_to_tsquery('{}')`.",
            lhs, lit_text, lit_text, lit_text
          )
        } else {
          format!(
            "`{} @@ '{}'` -- the literal is implicitly coerced to `tsquery` which has its own syntax (`&`/`|`/`!`/`:`). Use `plainto_tsquery(...)` for user-input-style text or `to_tsquery(...)` if the literal really is tsquery syntax.",
            lhs, lit_text
          )
        };
        out.push(Diagnostic {
          code: "sql499",
          severity: Severity::Warning,
          message: msg,
          range: TextRange::new((abs_s as u32).into(), (abs_e as u32).into()),
        });
      }
      let _ = lit_start;
      i = lit_end;
    }
  }
}
