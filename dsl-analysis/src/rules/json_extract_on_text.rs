//! sql505: `<text_col> -> 'key'` / `->>` / `#>` / `#>>` -- the JSON
//! extraction operators are defined only for `json` and `jsonb`. On
//! a `text` (or other non-JSON) column PG raises a runtime error:
//! `operator does not exist: text -> unknown`. Add a `::jsonb` cast
//! if the column actually holds JSON-shaped text, or use the
//! correct column.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::{Statement, StatementKind};
use dsl_resolve::Scope;
use text_size::TextRange;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql505"
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
        i += 1;
        while i < n && bytes[i] != b'\'' {
          i += 1;
        }
        i = (i + 1).min(n);
        continue;
      }
      // Detect one of: ->>, ->, #>>, #>
      let op_kind: Option<&str> = if i + 3 <= n && &bytes[i..i + 3] == b"->>" {
        Some("->>")
      } else if i + 2 <= n && &bytes[i..i + 2] == b"->" {
        Some("->")
      } else if i + 3 <= n && &bytes[i..i + 3] == b"#>>" {
        Some("#>>")
      } else if i + 2 <= n && &bytes[i..i + 2] == b"#>" {
        Some("#>")
      } else {
        None
      };
      let Some(op_kind) = op_kind else {
        i += 1;
        continue;
      };
      let op_start = i;
      let op_end = op_start + op_kind.len();
      // LHS: walk back over ws, then read ident.
      let mut p = op_start;
      while p > 0 && bytes[p - 1].is_ascii_whitespace() {
        p -= 1;
      }
      // Skip if the previous char is `)` -- chained extraction like
      // `(data -> 'a') -> 'b'`; need deeper analysis to know type.
      if p > 0 && bytes[p - 1] == b')' {
        i = op_end;
        continue;
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
      if dtype == "json" || dtype == "jsonb" {
        i = op_end;
        continue;
      }
      if emitted.insert(op_start) {
        let abs_s = start + p;
        let abs_e = start + op_end;
        out.push(Diagnostic {
          code: "sql505",
          severity: Severity::Error,
          message: format!(
            "`{lhs} {op_kind} ...` -- the JSON extraction operators (`->`, `->>`, `#>`, `#>>`) are defined only for `json` and `jsonb`, but `{}` is `{}`. PG raises `operator does not exist: {} {op_kind} ...` at runtime. Cast with `{lhs}::jsonb` if the column actually holds JSON-shaped text.",
            col.name, col.data_type, col.data_type
          ),
          range: TextRange::new((abs_s as u32).into(), (abs_e as u32).into()),
        });
      }
      i = op_end;
    }
  }
}
