//! sql503: `WHERE non_jsonb_col ? 'key'` / `?|` / `?&` -- the
//! key-exists family of operators (`?`, `?|`, `?&`) is only defined
//! for `jsonb`, not for `json` or `text`. Using them on the wrong
//! type raises a runtime error: `operator does not exist: <type> ?
//! text`.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::{Statement, StatementKind};
use dsl_resolve::Scope;
use text_size::TextRange;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql503"
  }
  fn default_severity(&self) -> Severity {
    Severity::Error
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let Some(target) = (match &stmt.kind {
      StatementKind::Select(s) => {
        if s.from.len() != 1 {
          return;
        }
        s.from.first()
      },
      StatementKind::Update(u) => Some(&u.table),
      StatementKind::Delete(d) => Some(&d.table),
      _ => return,
    }) else {
      return;
    };
    let Some(t) = catalog.find_table(target.schema.as_deref(), &target.name) else { return };

    let (start, raw) = crate::stmt_body(stmt, source);
    let bytes = raw.as_bytes();
    let n = bytes.len();
    let mut i = 0usize;
    let mut emitted: std::collections::HashSet<usize> = std::collections::HashSet::new();
    while i < n {
      if bytes[i] == b'\'' {
        // Skip string literal.
        i += 1;
        while i < n && bytes[i] != b'\'' {
          i += 1;
        }
        i = (i + 1).min(n);
        continue;
      }
      if bytes[i] != b'?' {
        i += 1;
        continue;
      }
      // Skip parameter placeholders like `$1` -- those don't precede
      // a `?`. But `?` standalone is the jsonb key-exists operator.
      let op_start = i;
      // Determine op variant: ?, ?|, ?&
      let op_kind = if i + 1 < n && bytes[i + 1] == b'|' {
        "?|"
      } else if i + 1 < n && bytes[i + 1] == b'&' {
        "?&"
      } else {
        "?"
      };
      let op_end = op_start + op_kind.len();
      // Walk back to find LHS column.
      let mut p = op_start;
      while p > 0 && bytes[p - 1].is_ascii_whitespace() {
        p -= 1;
      }
      let id_end = p;
      while p > 0 {
        let b = bytes[p - 1];
        if b.is_ascii_alphanumeric() || b == b'_' || b == b'.' {
          p -= 1;
        } else {
          break;
        }
      }
      if p == id_end {
        i = op_end;
        continue;
      }
      let lhs = &raw[p..id_end];
      let lhs_bare = lhs.rsplit('.').next().unwrap_or(lhs);
      let Some(col) = t.columns.iter().find(|c| c.name.eq_ignore_ascii_case(lhs_bare)) else {
        i = op_end;
        continue;
      };
      let dtype = col.data_type.to_ascii_lowercase();
      // Operators are jsonb-only. json / text / anything else -> error.
      if dtype == "jsonb" {
        i = op_end;
        continue;
      }
      if emitted.insert(op_start) {
        let abs_s = start + p;
        let abs_e = start + op_end;
        out.push(Diagnostic {
          code: "sql503",
          severity: Severity::Error,
          message: format!(
            "`{lhs} {op_kind} ...` -- the `?`/`?|`/`?&` operators are defined only for `jsonb`, but `{}` is `{}`. PG raises `operator does not exist: {} {op_kind} ...` at runtime. Cast with `{lhs}::jsonb` if the column actually holds jsonb-shaped text.",
            col.name, col.data_type, col.data_type
          ),
          range: TextRange::new((abs_s as u32).into(), (abs_e as u32).into()),
        });
      }
      i = op_end;
    }
  }
}
